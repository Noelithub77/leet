//! NeetCode question content, independent of the linked LeetCode judge payload.
use anyhow::{Context, Result, ensure};
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Serialize, Deserialize)]
pub struct Statement { pub html: String, pub title: String, pub hints: Vec<String>, pub topics: Vec<String> }

pub fn parse(html: &str, base: &str) -> Result<Statement> {
    let document = Html::parse_document(html);
    let selector = |value| Selector::parse(value).expect("static NeetCode statement selector");
    let resolved_base = document.select(&selector("base[href]")).next().and_then(|node| node.value().attr("href"))
        .and_then(|href| url::Url::parse(base).ok()?.join(href).ok()).map(|url| url.to_string());
    let base = resolved_base.as_deref().unwrap_or(base);
    let question = document.select(&selector(".question-tab")).next().context("NeetCode returned no question tab")?;
    let article = question.select(&selector("app-article main.neeter-article-content")).next().context("NeetCode returned no question description")?;
    let title = question.select(&selector(".problem-title")).next().map(|node| node.text().collect::<String>()).context("NeetCode returned no question title")?;
    ensure!(!article.text().collect::<String>().trim().is_empty(), "NeetCode statement is empty");
    let mut html = article.inner_html();
    let mut hints = Vec::new(); let mut topics = Vec::new();
    for detail in article.select(&selector("details")) {
        let Some(summary) = detail.select(&selector("summary")).next() else { continue; };
        let label = summary.text().collect::<String>();
        if label.trim().starts_with("Hint ") {
            hints.push(crate::description::media::normalize(&detail.inner_html().replacen(&summary.html(), "", 1), base));
            html = html.replacen(&detail.html(), "", 1);
        } else if label.trim() == "Topics" {
            topics.extend(detail.select(&selector("a")).map(|node| node.text().collect::<String>()));
            html = html.replacen(&detail.html(), "", 1);
        }
    }
    Ok(Statement { html: crate::description::media::normalize(&html, base), title, hints, topics })
}

pub fn get(db: &crate::db::Db, slug: &str) -> Result<Statement> {
    let entry = crate::roadmap::entry(slug).context("No NeetCode question mapping for this problem")?;
    let key = format!("statement:neetcode:v2:{slug}");
    if let Some(raw) = db.get(&key)? { if let Ok(statement) = serde_json::from_str(&raw) { return Ok(statement); } }
    let url = entry.neetcode_url();
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(20))).build().into();
    let html = agent.get(&url).call()?.body_mut().with_config().limit(8 << 20).read_to_string()?;
    let statement = parse(&html, &url)?;
    db.set(&key, &serde_json::to_string(&statement)?)?;
    Ok(statement)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn question_container_preserves_media_and_details_without_scraping_solution() {
        let html = "<base href='/'><div class='question-tab'><h1 class='problem-title'>Water</h1><app-article><main class='neeter-article-content'><p>Full description.</p><img data-src='diagram.png'><a href='reference'>Reference</a><p><b>Example 1:</b></p><pre>Input: height = [0,2,0]\nOutput: 0</pre><p>Constraints:</p><ul><li>n &gt; 0</li></ul><details><summary>Topics</summary><a href='/practice/array'>Array</a></details><details><summary>Recommended Time &amp; Space Complexity</summary><p>O(n)</p></details><details><summary>Hint 1</summary><p>Try <code>prefix</code>.</p><img src='/hint.png'></details></main></app-article></div><main class='neeter-article-content'>SECRET SOLUTION</main>";
        let statement = parse(html, "https://neetcode.io/problems/water/question").unwrap();
        for text in ["Full description", "https://neetcode.io/diagram.png", "https://neetcode.io/reference", "height", "Constraints", "Recommended Time", "O(n)"] { assert!(statement.html.contains(text), "Dropped {text}"); }
        assert!(!statement.html.contains("SECRET SOLUTION") && !statement.html.contains("Hint 1"));
        assert_eq!(statement.topics, ["Array"]); assert!(statement.hints[0].contains("hint.png"));
        assert!(parse("<main>Sign in</main>", "https://neetcode.io/").is_err());
    }
    #[test]
    #[ignore = "fetches public NeetCode question HTML"]
    fn live_neetcode_statement_and_offline_cache() {
        let directory = tempfile::tempdir().unwrap(); let db = crate::db::Db::open_unseeded(&directory.path().join("cache.sqlite")).unwrap();
        let statement = get(&db, "trapping-rain-water").unwrap();
        assert!(statement.html.contains("imagedelivery.net") && statement.html.contains("Recommended Time"));
        assert!(!statement.hints.is_empty());
        let document = Html::parse_fragment(&statement.html);
        let image = document.select(&Selector::parse("img").unwrap()).next().unwrap();
        let bytes = crate::description::media::cached_image(&directory.path().join("images"), image.value().attr("src").unwrap()).unwrap();
        assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
        assert_eq!(get(&db, "trapping-rain-water").unwrap().html, statement.html);
    }
}
