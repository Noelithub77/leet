//! Public Striver curriculum metadata and the current Take U Forward account client.
use std::{collections::HashSet, sync::LazyLock, time::Duration};
use anyhow::{Result, anyhow, ensure};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use crate::creds::Creds;

const API: &str = "https://backend-go.takeuforward.org/api";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Sheet { #[default] A2z, Sde }

impl Sheet {
    pub const ALL: [Self; 2] = [Self::A2z, Self::Sde];
    pub fn slug(self) -> &'static str { match self { Self::A2z => "strivers-a2z-dsa-sheet", Self::Sde => "strivers-sde-sheet" } }
    pub fn label(self) -> &'static str { match self { Self::A2z => "Striver A2Z", Self::Sde => "Striver SDE" } }
    pub fn url(self) -> String { format!("https://takeuforward.org/prep-hub/{}", self.slug()) }
    pub fn data(self) -> &'static Curriculum { &CURRICULA[match self { Self::A2z => 0, Self::Sde => 1 }] }
}

#[derive(Serialize, Deserialize)]
#[non_exhaustive]
pub struct Curriculum {
    pub title: String,
    pub source_url: String,
    pub sections: Vec<Section>,
}

#[derive(Serialize, Deserialize)]
#[non_exhaustive]
pub struct Section {
    pub id: u64,
    pub path: Vec<String>,
    pub items: Vec<Item>,
}

#[derive(Serialize, Deserialize)]
#[non_exhaustive]
pub struct Item {
    pub id: u64,
    pub title: String,
    pub url: String,
    pub leetcode_slug: Option<String>,
    pub learning: bool,
}

static CURRICULA: LazyLock<[Curriculum; 2]> = LazyLock::new(|| serde_json::from_str(include_str!("../data/striver.json")).expect("bundled Striver metadata"));

/// Read only the public curriculum fields from Next's streamed page payload.
pub fn parse_page(html: &str, sheet: Sheet) -> Result<Curriculum> {
    let document = scraper::Html::parse_document(html);
    let selector = scraper::Selector::parse("script").map_err(|_| anyhow!("Invalid script selector"))?;
    let mut stream = String::new();
    for script in document.select(&selector) {
        let text = script.inner_html();
        let Some(payload) = text.strip_prefix("self.__next_f.push(").and_then(|s| s.strip_suffix(')')) else { continue; };
        let Ok(Value::Array(parts)) = serde_json::from_str::<Value>(payload) else { continue; };
        if parts.first().and_then(Value::as_u64) == Some(1) {
            if let Some(text) = parts.get(1).and_then(Value::as_str) { stream.push_str(text); }
        }
    }
    for line in stream.lines() {
        let Some((_, raw)) = line.split_once(':') else { continue; };
        let Ok(value) = serde_json::from_str::<Value>(raw) else { continue; };
        if value["sheetSlug"].as_str() == Some(sheet.slug()) && value["sheet"]["sheet_syllabus"].is_object() {
            return parse_curriculum(&value["sheet"], sheet);
        }
    }
    Err(anyhow!("Take U Forward curriculum payload changed or is unavailable"))
}

fn parse_curriculum(value: &Value, sheet: Sheet) -> Result<Curriculum> {
    let packed = &value["sheet_syllabus"];
    let rows = packed["rows"].as_array().ok_or_else(|| anyhow!("Missing curriculum rows"))?;
    let fields = packed["fields"].as_array().ok_or_else(|| anyhow!("Missing curriculum fields"))?;
    let roots = packed["roots"].as_array().ok_or_else(|| anyhow!("Missing curriculum roots"))?;
    let mut sections = Vec::new();
    let mut visited = HashSet::new();
    for root in roots { walk(root, sheet, rows, fields, &[], &mut visited, &mut sections)?; }
    ensure!(!sections.is_empty(), "Empty curriculum");
    Ok(Curriculum { title: value["info"]["name"].as_str().unwrap_or(sheet.label()).to_owned(), source_url: sheet.url(), sections })
}

fn walk(index: &Value, sheet: Sheet, rows: &[Value], fields: &[Value], path: &[String], visited: &mut HashSet<usize>, sections: &mut Vec<Section>) -> Result<()> {
    let index = index.as_u64().and_then(|n| usize::try_from(n).ok()).ok_or_else(|| anyhow!("Invalid curriculum index"))?;
    ensure!(path.len() < 16 && visited.insert(index), "Cyclic or repeated curriculum category");
    let row = rows.get(index).and_then(Value::as_array).ok_or_else(|| anyhow!("Missing curriculum row"))?;
    let shape = row.first().and_then(Value::as_u64).and_then(|n| usize::try_from(n).ok()).and_then(|n| fields.get(n)).and_then(Value::as_array)
        .ok_or_else(|| anyhow!("Invalid curriculum shape"))?;
    let field = |name| shape.iter().position(|v| v.as_str() == Some(name)).and_then(|i| row.get(i + 1)).unwrap_or(&Value::Null);
    ensure!(field("type").as_str() == Some("category"), "Expected curriculum category");
    let mut path = path.to_vec();
    path.push(field("label").as_str().ok_or_else(|| anyhow!("Missing section title"))?.to_owned());
    let id = field("id").as_u64().ok_or_else(|| anyhow!("Missing section ID"))?;
    let children = field("children").as_array().ok_or_else(|| anyhow!("Missing section children"))?;
    let mut section = Section { id, path: path.clone(), items: Vec::new() };
    for child in children {
        let n = child.as_u64().and_then(|n| usize::try_from(n).ok()).ok_or_else(|| anyhow!("Invalid child index"))?;
        let row = rows.get(n).and_then(Value::as_array).ok_or_else(|| anyhow!("Missing child row"))?;
        let shape = row.first().and_then(Value::as_u64).and_then(|n| usize::try_from(n).ok()).and_then(|n| fields.get(n)).and_then(Value::as_array).ok_or_else(|| anyhow!("Invalid child shape"))?;
        let field = |name| shape.iter().position(|v| v.as_str() == Some(name)).and_then(|i| row.get(i + 1)).unwrap_or(&Value::Null);
        if field("type").as_str() == Some("category") { walk(child, sheet, rows, fields, &path, visited, sections)?; continue; }
        ensure!(visited.insert(n), "Repeated curriculum item");
        let slug = field("slug").as_str().ok_or_else(|| anyhow!("Missing item slug"))?;
        let learning = field("layoutType").as_str() == Some("learning");
        ensure!(!slug.is_empty(), "Invalid item slug");
        let mut url = url::Url::parse("https://takeuforward.org/")?;
        url.path_segments_mut().map_err(|_| anyhow!("Invalid provider URL"))?
            .extend([if learning { "learning" } else { "practice" }, "dsa", slug]);
        url.query_pairs_mut().append_pair("source", sheet.slug());
        let leetcode_slug = field("leetcode_link").as_str().and_then(leetcode_slug);
        section.items.push(Item {
            id: field("id").as_u64().ok_or_else(|| anyhow!("Missing item ID"))?,
            title: field("label").as_str().ok_or_else(|| anyhow!("Missing item title"))?.trim().to_owned(),
            url: url.to_string(),
            leetcode_slug, learning,
        });
    }
    if !section.items.is_empty() { sections.push(section); }
    Ok(())
}

fn leetcode_slug(raw: &str) -> Option<String> {
    let url = url::Url::parse(raw).ok()?;
    if url.scheme() != "https" || url.host_str() != Some("leetcode.com") { return None; }
    let mut parts = url.path_segments()?;
    if parts.next()? != "problems" { return None; }
    let slug = parts.next()?;
    (!slug.is_empty() && slug.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-')).then(|| slug.to_owned())
}

pub fn validate(session: &Creds) -> Result<Creds> {
    let value = request("/v1/auth/me", session)?;
    let mut session = session.clone();
    session.user_id = identity(&value)?.to_owned();
    session.username = value["data"]["username"].as_str().filter(|s| !s.is_empty()).unwrap_or(&session.user_id).to_owned();
    Ok(session)
}

fn identity(value: &Value) -> Result<&str> {
    ensure!(value["success"] == true && value["data"]["logged_in"] == true && value["data"]["is_banned"] != true, "Take U Forward session rejected; sign in again");
    value["data"]["email"].as_str().filter(|s| !s.is_empty()).ok_or_else(|| anyhow!("Missing Take U Forward account identity"))
}

pub fn progress(sheet: Sheet, session: &Creds) -> Result<HashSet<u64>> {
    let value = request(&format!("/v2/syllabus/track?type=source&key={}", sheet.slug()), session)?;
    let payload = value.get("data").unwrap_or(&value);
    let ids = payload["solvedProblemIds"].as_array().ok_or_else(|| anyhow!("Take U Forward progress response changed"))?;
    ids.iter().map(|id| id.as_u64().ok_or_else(|| anyhow!("Invalid Take U Forward progress item"))).collect()
}

fn request(path: &str, session: &Creds) -> Result<Value> {
    ensure!(!session.cookie.is_empty() && !session.cookie.contains(['\r', '\n']) && !session.user_agent.contains(['\r', '\n']), "Invalid Take U Forward session");
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(25)))
        .http_status_as_error(false).max_redirects(0).build().into();
    let mut response = agent.get(format!("{API}{path}")).header("Cookie", &session.cookie)
        .header("User-Agent", &session.user_agent).header("Origin", "https://takeuforward.org")
        .header("Referer", "https://takeuforward.org/").call().map_err(|_| anyhow!("Take U Forward unavailable; retry"))?;
    ensure!(response.status().is_success(), "Take U Forward request failed ({}); retry or sign in again", response.status().as_u16());
    response.body_mut().with_config().limit(8 << 20).read_json().map_err(|_| anyhow!("Invalid Take U Forward response"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn streamed_curricula_preserve_sections_and_encode_provider_slugs() {
        let sheet = serde_json::json!({"sheetSlug":Sheet::Sde.slug(),"sheet":{"info":{"name":"Fixture sheet"},"sheet_syllabus":{
            "fields":[["id","slug","type","label","children"],["id","slug","type","layoutType","label","leetcode_link"]],
            "rows":[[0,10,"root","category","Arrays",[1]],[1,20,"kadane's-algorithm","item","practice","Maximum subarray","https://leetcode.com/problems/maximum-subarray/"]],"roots":[0]
        }}});
        let stream = format!("ab:{sheet}\n");
        let midpoint = stream.len() / 2;
        let html = format!("<script>self.__next_f.push({})</script><script>self.__next_f.push({})</script>", serde_json::json!([1,&stream[..midpoint]]), serde_json::json!([1,&stream[midpoint..]]));
        let data = parse_page(&html, Sheet::Sde).unwrap();
        assert_eq!(data.sections[0].path, ["Arrays"]);
        assert_eq!(data.sections[0].items[0].leetcode_slug.as_deref(), Some("maximum-subarray"));
        let url = url::Url::parse(&data.sections[0].items[0].url).unwrap();
        assert_eq!(url.host_str(), Some("takeuforward.org"));
        assert_eq!(url.query_pairs().next().unwrap().1, Sheet::Sde.slug());
        let mut cyclic = sheet["sheet"].clone();
        cyclic["sheet_syllabus"]["rows"][0][5] = serde_json::json!([0]);
        assert!(parse_curriculum(&cyclic, Sheet::Sde).is_err());
    }

    #[test]
    fn linked_problems_require_the_real_leetcode_host() {
        assert_eq!(leetcode_slug("https://leetcode.com/problems/two-sum/description/"), Some("two-sum".into()));
        assert!(leetcode_slug("https://leetcode.com.evil.test/problems/two-sum/").is_none());
        assert!(leetcode_slug("https://leetcode.com/discuss/two-sum/").is_none());
    }
    #[test]
    fn signed_out_and_banned_accounts_are_rejected() {
        assert!(identity(&serde_json::json!({"success":true,"data":{"logged_in":false,"email":"fixture@example.test"}})).is_err());
        assert!(identity(&serde_json::json!({"success":true,"data":{"logged_in":true,"is_banned":true,"email":"fixture@example.test"}})).is_err());
        assert_eq!(identity(&serde_json::json!({"success":true,"data":{"logged_in":true,"email":"fixture@example.test"}})).unwrap(), "fixture@example.test");
    }
    #[test]
    fn bundled_sheets_keep_full_curricula_and_public_metadata_only() {
        for sheet in Sheet::ALL {
            let data = sheet.data();
            let items: Vec<_> = data.sections.iter().flat_map(|s| &s.items).collect();
            assert!(items.len() > 100);
            assert!(items.iter().any(|i| i.leetcode_slug.is_some()));
            for section in &data.sections {
                let ids: HashSet<_> = section.items.iter().map(|item| item.id).collect();
                assert_eq!(ids.len(), section.items.len());
            }
            assert!(items.iter().all(|i| i.url.starts_with("https://takeuforward.org/")));
        }
    }
}
