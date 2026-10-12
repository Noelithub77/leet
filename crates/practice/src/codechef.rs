//! Public CodeChef practice catalog and statement scraper; no account/session access.
use std::time::Duration;
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use crate::{language::Language, leetcode::{CatalogItem, Question}};

pub fn problem_code(slug: &str) -> Result<&str> {
    let code = slug.strip_prefix("cc:").context("Not a CodeChef problem")?;
    if code.is_empty() || code.len() > 40 || !code.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_') {
        bail!("Invalid CodeChef problem code");
    }
    Ok(code)
}
pub fn problem_url(slug: &str) -> Result<String> { Ok(format!("https://www.codechef.com/problems/{}", problem_code(slug)?)) }
pub fn slug_from_url(url: &url::Url) -> Result<String> {
    if !matches!(url.scheme(), "http" | "https") || !matches!(url.host_str(), Some("codechef.com" | "www.codechef.com")) {
        bail!("Use a CodeChef problem page");
    }
    let parts: Vec<_> = url.path_segments().context("Problem URL has no path")?.filter(|part| !part.is_empty()).collect();
    let code = match parts.as_slice() { ["problems", code] | [_, "problems", code] | ["practice", "course", _, "problems", code] => *code, _ => bail!("Use a CodeChef problem page") };
    let slug = format!("cc:{code}"); problem_code(&slug)?; Ok(slug)
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Problem {
    pub code: String,
    pub name: String,
    #[serde(deserialize_with = "rating")]
    pub difficulty_rating: u32,
}
fn rating<'de, D: serde::Deserializer<'de>>(deserializer: D) -> std::result::Result<u32, D::Error> {
    #[derive(Deserialize)] #[serde(untagged)] enum Rating { Number(u32), Text(String) }
    match Rating::deserialize(deserializer)? { Rating::Number(rating) => Ok(rating), Rating::Text(text) => text.parse().map_err(serde::de::Error::custom) }
}
fn optional_rating<'de, D: serde::Deserializer<'de>>(deserializer: D) -> std::result::Result<Option<u32>, D::Error> {
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    value.map(|value| match value {
        serde_json::Value::Number(number) => number.as_u64().and_then(|n| u32::try_from(n).ok()).ok_or_else(|| serde::de::Error::custom("Invalid rating")),
        serde_json::Value::String(text) => text.parse().map_err(serde::de::Error::custom),
        _ => Err(serde::de::Error::custom("Invalid rating")),
    }).transpose()
}
impl Problem {
    pub fn slug(&self) -> Result<String> { let slug = format!("cc:{}", self.code); problem_code(&slug)?; Ok(slug) }
    pub fn item(&self) -> Result<CatalogItem> {
        Ok(CatalogItem { slug: self.slug()?, frontend_id: 0, title: self.name.clone(), level: if self.difficulty_rating < 1000 { 1 } else { 2 }, paid_only: false, ac_rate: 0., status: None })
    }
}
fn fetch<T: serde::de::DeserializeOwned>(url: &str) -> Result<T> {
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(30))).build().into();
    agent.get(url).header("User-Agent", "leet public practice client").call()?.body_mut().with_config().limit(8 << 20).read_json().context("Read CodeChef public response")
}
pub fn catalog() -> Result<Vec<Problem>> {
    #[derive(Deserialize)] struct Catalog { status: String, data: Vec<Problem> }
    let mut problems = vec![];
    for (start, end) in [(0, 999), (1000, 1800)] {
        let url = format!("https://www.codechef.com/api/list/problems?limit=50&offset=0&sort_by=successful_submissions&sort_order=desc&start_rating={start}&end_rating={end}");
        let response: Catalog = fetch(&url)?;
        if response.status != "success" || response.data.len() != 50 { bail!("CodeChef did not return the requested 50-problem rating band"); }
        for problem in response.data {
            problem.slug()?;
            if !(start..=end).contains(&problem.difficulty_rating) || problem.name.trim().is_empty() { bail!("Invalid CodeChef catalog entry"); }
            problems.push(problem);
        }
    }
    let unique: std::collections::HashSet<_> = problems.iter().map(|problem| &problem.code).collect();
    if unique.len() != 100 { bail!("CodeChef catalog contains duplicate problems"); }
    Ok(problems)
}

#[derive(Deserialize)]
struct Response {
    status: String, problem_code: String, problem_name: String, body: String,
    #[serde(default)] problem_components: Option<Components>,
    #[serde(default, deserialize_with = "optional_rating")] difficulty_rating: Option<u32>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Components { #[serde(default)] sample_test_cases: Vec<Sample>, #[serde(flatten)] sections: std::collections::BTreeMap<String, serde_json::Value> }
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Sample { input: String, output: String, #[serde(default)] explanation: String, #[serde(default)] is_deleted: bool }

pub fn question(slug: &str) -> Result<Question> {
    let code = problem_code(slug)?;
    let response = fetch::<serde_json::Value>(&format!("https://www.codechef.com/api/contests/PRACTICE/problems/{code}"))?;
    parse_response(slug, response)
}
pub fn parse_response(slug: &str, mut value: serde_json::Value) -> Result<Question> {
    // The public API mixes snake_case envelope fields with a camelCase components key.
    if let Some(components) = value.get("problemComponents").cloned() { value["problem_components"] = components; }
    let response: Response = serde_json::from_value(value)?;
    if response.status != "success" || response.problem_code != problem_code(slug)? || response.problem_name.trim().is_empty() || response.body.trim().is_empty() {
        bail!("CodeChef returned no public statement for {}", problem_code(slug)?);
    }
    let body_blocks = crate::description::parse(&response.body);
    let body_text: String = scraper::Html::parse_fragment(&response.body).root_element().text().collect();
    let mut extras = Vec::new();
    let (mut examples, mut outputs) = (vec![], vec![]);
    if let Some(components) = response.problem_components {
        let document = scraper::Html::parse_fragment(&response.body);
        let heading_selector = scraper::Selector::parse("h1,h2,h3,h4,h5,h6").expect("static heading selector");
        let headings: Vec<_> = document.select(&heading_selector).map(|node| node.text().collect::<String>().to_lowercase()).collect();
        for (key, label) in [("statement", "Problem statement"), ("inputFormat", "Input"), ("outputFormat", "Output"), ("constraints", "Constraints"), ("subtasks", "Subtasks")] {
            if components.sections.get(&format!("{key}State")).and_then(serde_json::Value::as_bool) == Some(false) { continue; }
            if let Some(content) = components.sections.get(key).and_then(serde_json::Value::as_str).filter(|content| !content.trim().is_empty())
                && !headings.iter().any(|heading| heading.contains(&label.to_lowercase())) && !body_text.contains(content.trim()) {
                extras.push(if key == "constraints" { crate::description::Block::Constraints(content.into()) } else { crate::description::Block::Markdown(format!("### {label}\n\n{content}")) });
            }
        }
        for sample in components.sample_test_cases.into_iter().filter(|sample| !sample.is_deleted) {
            let title = format!("Example {}", examples.len() + 1);
            let mut parts = Vec::new();
            let in_body = body_blocks.iter().any(|block| matches!(block, crate::description::Block::Example { parts, .. } if parts.iter().any(|part| matches!(part, crate::description::Part::Field { label, value } if label == "Input" && value.trim() == sample.input.trim())) && parts.iter().any(|part| matches!(part, crate::description::Part::Field { label, value } if label == "Output" && value.trim() == sample.output.trim()))));
            if !in_body {
                parts.push(crate::description::Part::Field { label: "Input".into(), value: sample.input.clone() });
                parts.push(crate::description::Part::Field { label: "Output".into(), value: sample.output.clone() });
            }
            if !sample.explanation.trim().is_empty() && !body_text.contains(sample.explanation.trim()) {
                parts.push(crate::description::Part::Markdown(sample.explanation));
            }
            if !parts.is_empty() { extras.push(crate::description::Block::Example { title, parts }); }
            examples.push(sample.input); outputs.push(sample.output);
        }
    }
    if examples.is_empty() {
        use scraper::{Html, Selector};
        let document = Html::parse_fragment(&response.body);
        let selector = Selector::parse("h3, h4, pre").expect("static sample selector");
        let mut kind = None;
        for element in document.select(&selector) {
            let text = element.text().collect::<String>();
            if element.value().name() == "pre" {
                match kind.take() { Some(true) => examples.push(text.trim_matches(['\r','\n']).into()), Some(false) => outputs.push(text.trim_matches(['\r','\n']).into()), None => {} }
            } else {
                let heading = text.to_lowercase();
                kind = if heading.contains("sample input") || heading.contains("example input") { Some(true) } else if heading.contains("sample output") || heading.contains("example output") { Some(false) } else { None };
            }
        }
    }
    if examples.len() > 100 || outputs.len() > 100 { bail!("CodeChef statement exceeds the sample limit"); }
    let mut question = imported_question(slug, &response.problem_name, response.body, examples, outputs, response.difficulty_rating);
    question.meta["statementExtras"] = serde_json::to_value(extras)?;
    question.meta["statementVersion"] = serde_json::json!(2);
    Ok(question)
}
pub fn imported_question(slug: &str, title: &str, content: String, examples: Vec<String>, outputs: Vec<String>, rating: Option<u32>) -> Question {
    let snippets = Language::ALL.into_iter().map(|language| (language.judge_id().into(), language.stdin_template().into())).collect();
    Question { question_id: slug.trim_start_matches("cc:").into(), frontend_id: slug.trim_start_matches("cc:").into(), slug: slug.into(), title: title.into(), difficulty: if rating.is_some_and(|rating| rating >= 1000) { "Medium" } else { "Easy" }.into(), paid_only: false, content, python: Language::Python.stdin_template().into(), snippets,
        meta: serde_json::json!({"source":"codechef","provenance":"https://www.codechef.com","difficultyRating":rating}), examples, outputs, hints: vec![], topics: vec![] }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_samples_do_not_hide_a_complete_statement() {
        let question = parse_response("cc:START01", serde_json::json!({"status":"success","problem_code":"START01","problem_name":"Statement only","body":"<p>Complete description.</p><img src='/diagram.svg'><p>Constraints:</p><ul><li>n &gt; 0</li></ul>"})).unwrap();
        assert!(question.examples.is_empty());
        let rendered = format!("{:?}", crate::description::for_question(&question).unwrap());
        assert!(rendered.contains("Complete description") && rendered.contains("diagram.svg"));
    }
    #[test]
    fn complete_response_keeps_structured_samples_explanations_and_missing_sections() {
        let value = serde_json::json!({"status":"success","problem_code":"START01","problem_name":"Number Mirror","body":"<h3>Problem Statement</h3><p>Print n.</p><h3>Sample Input</h3><pre>123</pre><h3>Sample Output</h3><pre>123</pre><aside>Public note <img src='/note.png'></aside>","problemComponents":{"subtasks":"Score 50 points for n &lt; 10.","subtasksState":true,"constraints":"- $0 \u{2264} n \u{2264} 100$","sampleTestCases":[{"input":"123","output":"123","explanation":"First explanation ![trace](/trace.png)"},{"input":"15","output":"15","explanation":"Second explanation."},{"input":"99","output":"99","isDeleted":true}]}});
        let question = parse_response("cc:START01", value).unwrap();
        assert_eq!(question.examples, ["123", "15"]);
        let blocks = crate::description::for_question(&question).unwrap();
        let rendered = format!("{blocks:?}");
        for token in ["Print n", "https://www.codechef.com/note.png", "123", "15", "First explanation", "Second explanation", "https://www.codechef.com/trace.png", "Score 50 points", "100"] { assert!(rendered.contains(token), "Dropped {token}: {rendered}"); }
        assert!(!rendered.contains("99"));
        assert_eq!(blocks.iter().filter(|block| matches!(block, crate::description::Block::Example { title, .. } if title == "Example 1")).count(), 1);
    }
    #[test]
    #[ignore = "fetches public CodeChef question data"]
    fn live_complete_codechef_statement() {
        let question = question("cc:START01").unwrap();
        let blocks = crate::description::for_question(&question).unwrap();
        let rendered = format!("{blocks:?}");
        assert!(rendered.contains("Example 2") && rendered.contains("15") && rendered.contains("The input is 123"));
    }
    #[test]
    fn identities_reject_untrusted_hosts_and_paths() {
        for url in ["https://www.codechef.com/problems/START01", "https://www.codechef.com/JAN24/problems/START01", "https://www.codechef.com/practice/course/basic/problems/START01"] {
            assert_eq!(slug_from_url(&url::Url::parse(url).unwrap()).unwrap(), "cc:START01");
        }
        for url in ["https://www.codechef.com.evil.example/problems/START01", "https://www.codechef.com/problems/../users", "https://www.codechef.com/problems/%2e%2e", "file:///problems/START01"] { assert!(slug_from_url(&url::Url::parse(url).unwrap()).is_err()); }
        assert!(problem_code("cc:../../file").is_err());
    }
    #[test]
    fn structured_samples_and_legacy_html_have_stdin_starters() {
        let value = serde_json::json!({"status":"success","problem_code":"START01","problem_name":"Number Mirror","body":"<h3>Sample Input</h3><pre>123\n</pre><h3>Sample Output</h3><pre>123\n</pre>","problemComponents":{"sampleTestCases":[{"input":"15\n","output":"15\n"},{"input":"bad","output":"bad","isDeleted":true}]}});
        let q = parse_response("cc:START01", value.clone()).unwrap(); assert_eq!(q.examples, ["15\n"]); assert_eq!(q.outputs, ["15\n"]);
        for language in Language::ALL { assert!(q.starter(language).is_some()); }
        let mut legacy = value.clone(); legacy["problemComponents"] = serde_json::Value::Null;
        assert_eq!(parse_response("cc:START01", legacy).unwrap().examples, ["123"]);
        assert!(parse_response("cc:FLOW001", value).is_err());
    }
}
