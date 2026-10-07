//! On-demand Open-R1 rows. The bundled index contains identities only.
use std::{collections::BTreeMap, sync::LazyLock, time::Duration};
use anyhow::{Context, Result, bail};
use serde::Deserialize;
use crate::leetcode::Question;

static INDEX: LazyLock<BTreeMap<String, (String, u64, String)>> = LazyLock::new(||
    serde_json::from_str(include_str!("../data/codeforces-snapshot-index.json")).expect("bundled snapshot identity index"));

#[derive(Deserialize)]
struct Slice { rows: Vec<Row> }
#[derive(Deserialize)]
struct Row { row: Snapshot, #[serde(default)] truncated_cells: Vec<String> }
#[derive(Deserialize)]
struct Sample { input: String, output: String }
#[derive(Deserialize)]
struct Snapshot {
    id: String, title: String, description: String,
    input_format: Option<String>, output_format: Option<String>,
    interaction_format: Option<String>, note: Option<String>,
    examples: Vec<Sample>, tags: Vec<String>,
    time_limit: Option<f64>, memory_limit: Option<f64>,
}

pub fn question(slug: &str) -> Result<Question> {
    let (contest, problem) = crate::codeforces::problem_id(slug)?;
    let identity = format!("{contest}/{problem}");
    let (split, offset, expected) = INDEX.get(&identity).context("This newer problem is not in the Open-R1 snapshot; import its samples with Competitive Companion")?;
    let mut url = url::Url::parse("https://datasets-server.huggingface.co/rows")?;
    url.query_pairs_mut().extend_pairs([
        ("dataset", "open-r1/codeforces"), ("config", "default"), ("split", split),
        ("offset", &offset.to_string()), ("length", "1"),
    ]);
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(20))).build().into();
    let mut response = agent.get(url.as_str()).call().context("Open-R1 snapshot download")?;
    let slice: Slice = response.body_mut().with_config().limit(8 << 20).read_json()?;
    let row = slice.rows.into_iter().next().context("Snapshot row is unavailable")?;
    if row.truncated_cells.iter().any(|field| ["description", "input_format", "output_format", "examples"].contains(&field.as_str())) {
        bail!("Snapshot viewer truncated this statement; import its samples from your browser");
    }
    let row = row.row;
    if &row.id != expected { bail!("Snapshot order has changed; import from the browser until the index is refreshed"); }
    row.into_question(slug)
}

impl Snapshot {
    fn into_question(self, slug: &str) -> Result<Question> {
        let escape = |value: &str| html_escape::encode_text(value).into_owned();
        let mut html = format!("<div class=\"problem-statement\"><div class=\"header\"><div class=\"title\">{}</div></div><p>{} s · {} MB</p>",
            escape(&self.title), self.time_limit.unwrap_or(0.), self.memory_limit.unwrap_or(0.));
        for (heading, body) in [("", Some(self.description)), ("Input", self.input_format), ("Output", self.output_format), ("Interaction", self.interaction_format), ("Note", self.note)] {
            if let Some(body) = body.filter(|value| !value.trim().is_empty()) {
                if !heading.is_empty() { html.push_str(&format!("<h3>{heading}</h3>")); }
                for paragraph in body.split("\n\n") { html.push_str(&format!("<p>{}</p>", escape(paragraph).replace('\n', "<br>"))); }
            }
        }
        html.push_str("<div class=\"sample-test\">");
        for sample in self.examples { html.push_str(&format!("<div class=\"input\"><pre>{}</pre></div><div class=\"output\"><pre>{}</pre></div>", escape(&sample.input), escape(&sample.output))); }
        html.push_str("</div><p>Historical snapshot · Open-R1 / Codeforces · CC BY 4.0</p></div>");
        let mut q = crate::codeforces::parse_statement(slug, &html)?;
        q.title = self.title;
        q.topics = self.tags;
        q.meta["statementSource"] = serde_json::json!("open-r1/codeforces");
        Ok(q)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn snapshot_keeps_statement_samples_and_source() {
        let row: Snapshot = serde_json::from_value(serde_json::json!({"id":"4/A", "title":"Watermelon", "description":"Split < 8.\n\nKeep both parts even.", "examples":[{"input":"8\n", "output":"YES\n"}], "tags":["math"]})).unwrap();
        let q = row.into_question("cf:4:A").unwrap();
        assert_eq!(q.title, "Watermelon"); assert_eq!(q.examples, ["8"]); assert_eq!(q.outputs, ["YES"]);
        assert!(q.content.contains("Split &lt; 8.")); assert!(q.starter(crate::language::Language::Cpp).is_some());
        assert_eq!(q.meta["statementSource"], "open-r1/codeforces");
        assert!(INDEX.contains_key("4/A"));
    }
    #[test]
    #[ignore = "downloads a public snapshot row"]
    fn live_watermelon_snapshot() { let q = question("cf:4:A").unwrap(); assert_eq!(q.title, "Watermelon"); assert!(!q.examples.is_empty()); }
}
