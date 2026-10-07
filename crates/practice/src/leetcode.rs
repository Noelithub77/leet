//! leetcode.com: catalog, question details, and judge run/submit.

use std::sync::LazyLock;
use std::thread::sleep;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow, bail};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use ureq::Agent;

use crate::creds::Creds;
use crate::language::Language;
use std::collections::BTreeMap;

const BASE: &str = "https://leetcode.com";
const DEFAULT_UA: &str =
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0 Safari/537.36";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CatalogItem {
    pub slug: String,
    pub frontend_id: u32,
    pub title: String,
    /// 1 easy, 2 medium, 3 hard.
    pub level: u8,
    pub paid_only: bool,
    pub ac_rate: f32,
    /// `ac`, `notac`, or none; only present for a signed-in catalog fetch.
    pub status: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Question {
    pub question_id: String,
    pub frontend_id: String,
    pub title: String,
    pub slug: String,
    pub difficulty: String,
    pub paid_only: bool,
    pub content: String,
    pub python: String,
    #[serde(default)]
    pub snippets: BTreeMap<String, String>,
    pub meta: Value,
    /// Each example's input, one argument per line.
    pub examples: Vec<String>,
    /// Expected output per example, parsed from the statement.
    pub outputs: Vec<String>,
    pub hints: Vec<String>,
    pub topics: Vec<String>,
}

impl Question {
    pub fn starter(&self, language: Language) -> Option<&str> {
        if language == Language::Python { Some(&self.python) }
        else { self.snippets.get(language.judge_id()).map(String::as_str) }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct JudgeResult {
    pub submission: bool,
    pub status_code: i64,
    pub status: String,
    pub total_correct: Option<i64>,
    pub total_testcases: Option<i64>,
    pub runtime: String,
    pub memory: String,
    pub runtime_percentile: Option<f64>,
    pub memory_percentile: Option<f64>,
    pub answers: Vec<String>,
    pub expected: Vec<String>,
    /// Per-case `1`/`0` for runs.
    pub compare: String,
    pub stdout: Vec<String>,
    pub last_input: String,
    pub expected_output: String,
    pub actual_output: String,
    pub error: String,
}

impl JudgeResult {
    pub fn accepted(&self) -> bool {
        self.status_code == 10 && (self.submission || !self.compare.contains('0'))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Submission {
    pub id: String,
    #[serde(rename = "statusDisplay")] pub status: String,
    pub lang: String,
    pub timestamp: String,
    pub runtime: String,
    pub memory: String,
}

pub struct Client {
    agent: Agent,
    creds: Option<Creds>,
}

impl Client {
    pub fn new(creds: Option<Creds>) -> Self {
        let agent = Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(30)))
            .http_status_as_error(false)
            .build()
            .into();
        Self { agent, creds }
    }

    pub fn signed_in(&self) -> bool {
        self.creds.as_ref().is_some_and(|c| c.cookie_value("LEETCODE_SESSION").is_some())
    }

    pub fn cache_identity(&self) -> Option<String> {
        use std::hash::{Hash, Hasher};
        let cookie = self.creds.as_ref()?.cookie_value("LEETCODE_SESSION")?;
        let mut hash = std::collections::hash_map::DefaultHasher::new(); cookie.hash(&mut hash);
        Some(format!("{:016x}", hash.finish()))
    }

    pub fn submissions(&self, slug: &str) -> Result<Vec<Submission>> {
        self.require_session()?;
        let query = "query($offset:Int!,$limit:Int!,$lastKey:String,$questionSlug:String!) { submissionList(offset:$offset,limit:$limit,lastKey:$lastKey,questionSlug:$questionSlug) { lastKey hasNext submissions { id statusDisplay lang timestamp runtime memory } } }";
        let mut out = Vec::new(); let mut last = Value::Null;
        for _ in 0..100 {
            let body = self.post(&format!("{BASE}/graphql/"), BASE, &json!({"query":query,"variables":{"offset":out.len(),"limit":20,"lastKey":last,"questionSlug":slug}}))?;
            let page = &body["data"]["submissionList"];
            if page.is_null() { bail!("Submission history unavailable; check your LeetCode sign-in"); }
            let values = page["submissions"].as_array().context("Invalid submission history response")?;
            let entries: Vec<Submission> = serde_json::from_value(Value::Array(values.clone()))?;
            if entries.is_empty() && page["hasNext"] == true { bail!("Submission pagination made no progress"); }
            out.extend(entries);
            if page["hasNext"] != true { return Ok(out); } last = page["lastKey"].clone();
        }
        bail!("History exceeds 2,000 submissions; cached versions were kept")
    }

    pub fn submission_code(&self, id: &str) -> Result<String> {
        self.require_session()?; let id: i64 = id.parse()?;
        let body = self.post(&format!("{BASE}/graphql/"), BASE, &json!({"query":"query($id:Int!) { submissionDetails(submissionId:$id) { code } }","variables":{"id":id}}))?;
        body["data"]["submissionDetails"]["code"].as_str().map(str::to_owned).context("Submission code unavailable; sign in to its owning account")
    }

    fn user_agent(&self) -> &str {
        self.creds
            .as_ref()
            .map(|c| c.user_agent.as_str())
            .filter(|ua| !ua.is_empty())
            .unwrap_or(DEFAULT_UA)
    }

    fn get(&self, url: &str) -> Result<Value> {
        let mut req = self.agent.get(url).header("User-Agent", self.user_agent());
        if let Some(c) = &self.creds {
            req = req.header("Cookie", &c.cookie);
        }
        let mut resp = req.call()?;
        let status = resp.status();
        let body: Value = resp.body_mut().with_config().limit(64 << 20).read_json()?;
        if !status.is_success() {
            bail!("GET {url}: HTTP {status}");
        }
        Ok(body)
    }

    fn post(&self, url: &str, referer: &str, body: &Value) -> Result<Value> {
        let mut req = self
            .agent
            .post(url)
            .header("User-Agent", self.user_agent())
            .header("Referer", referer)
            .header("Origin", BASE);
        if let Some(c) = &self.creds {
            req = req.header("Cookie", &c.cookie);
            if let Some(csrf) = c.cookie_value("csrftoken") {
                req = req.header("x-csrftoken", csrf);
            }
        }
        let mut resp = req.send_json(body)?;
        let status = resp.status();
        let text = resp.body_mut().read_to_string()?;
        match status.as_u16() {
            200..=299 => serde_json::from_str(&text).with_context(|| format!("decode {url}")),
            401 | 403 => bail!("LeetCode rejected the session (HTTP {status}); sign in again"),
            429 => bail!("LeetCode rate limit; wait a few seconds"),
            _ => bail!("LeetCode request failed (HTTP {status}); retry"),
        }
    }

    pub fn validate_session(&self) -> Result<String> {
        self.require_session()?;
        let body = self.post(&format!("{BASE}/graphql/"), BASE,
            &json!({"query": "query { userStatus { isSignedIn username } }"}))?;
        let user = &body["data"]["userStatus"];
        if user["isSignedIn"].as_bool() != Some(true) { bail!("LeetCode session expired; sign in again"); }
        user["username"].as_str().filter(|s| !s.is_empty()).map(str::to_owned)
            .context("LeetCode returned no account identity")
    }

    /// Every problem with difficulty and paid flag in one request.
    pub fn catalog(&self) -> Result<Vec<CatalogItem>> {
        let body = self.get(&format!("{BASE}/api/problems/all/"))?;
        let pairs = body["stat_status_pairs"].as_array().context("catalog shape")?;
        Ok(pairs
            .iter()
            .filter_map(|p| {
                let stat = &p["stat"];
                if stat["question__hide"].as_bool() == Some(true) {
                    return None;
                }
                let acs = stat["total_acs"].as_f64().unwrap_or(0.0);
                let submitted = stat["total_submitted"].as_f64().unwrap_or(0.0).max(1.0);
                Some(CatalogItem {
                    slug: stat["question__title_slug"].as_str()?.to_owned(),
                    frontend_id: stat["frontend_question_id"].as_u64()? as u32,
                    title: stat["question__title"].as_str()?.to_owned(),
                    level: p["difficulty"]["level"].as_u64().unwrap_or(2) as u8,
                    paid_only: p["paid_only"].as_bool().unwrap_or(false),
                    ac_rate: (acs / submitted * 100.0) as f32,
                    status: p["status"].as_str().map(str::to_owned),
                })
            })
            .collect())
    }

    pub fn question(&self, slug: &str) -> Result<Question> {
        let query = "query q($titleSlug: String!) { question(titleSlug: $titleSlug) { \
            questionId questionFrontendId title titleSlug difficulty isPaidOnly content metaData \
            exampleTestcaseList hints topicTags { name } codeSnippets { langSlug code } } }";
        let referer = format!("{BASE}/problems/{slug}/");
        let body = self.post(
            &format!("{BASE}/graphql/"),
            &referer,
            &json!({ "query": query, "variables": { "titleSlug": slug } }),
        )?;
        let question = Self::parse_question(&body["data"]["question"], slug)?;
        if question.content.is_empty() { bail!("{slug} needs LeetCode Premium or has no public statement"); }
        Ok(question)
    }

    /// Bounded public GraphQL batches for building the offline snapshot.
    pub fn question_batch(&self, slugs: &[String]) -> Result<Vec<Result<Question>>> {
        if slugs.is_empty() || slugs.len() > 20 { bail!("Question batches require 1..20 slugs"); }
        let fields = "questionId questionFrontendId title titleSlug difficulty isPaidOnly content metaData exampleTestcaseList hints topicTags { name } codeSnippets { langSlug code }";
        let entries = slugs.iter().enumerate().map(|(index, slug)| format!("q{index}: question(titleSlug: {}) {{ {fields} }}", serde_json::to_string(slug).expect("serialize slug"))).collect::<Vec<_>>().join(" ");
        let body = self.post(&format!("{BASE}/graphql/"), BASE, &json!({"query": format!("query {{ {entries} }}")}))?;
        Ok(slugs.iter().enumerate().map(|(index, slug)| Self::parse_question(&body["data"][format!("q{index}")], slug)).collect())
    }

    fn parse_question(q: &Value, slug: &str) -> Result<Question> {
        if q.is_null() {
            bail!("LeetCode has no question {slug}");
        }
        let s = |v: &Value| v.as_str().unwrap_or_default().to_owned();
        let content = s(&q["content"]);
        let python = q["codeSnippets"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|c| c["langSlug"] == "python3")
            .map(|c| s(&c["code"]))
            .unwrap_or_default();
        let snippets = q["codeSnippets"].as_array().into_iter().flatten()
            .filter_map(|snippet| Some((snippet["langSlug"].as_str()?.to_owned(), snippet["code"].as_str()?.to_owned()))).collect();
        let strings = |v: &Value| -> Vec<String> {
            v.as_array().into_iter().flatten().filter_map(|x| x.as_str().map(str::to_owned)).collect()
        };
        Ok(Question {
            question_id: s(&q["questionId"]),
            frontend_id: s(&q["questionFrontendId"]),
            title: s(&q["title"]),
            slug: s(&q["titleSlug"]),
            difficulty: s(&q["difficulty"]),
            paid_only: q["isPaidOnly"].as_bool().unwrap_or(false),
            meta: serde_json::from_str(q["metaData"].as_str().unwrap_or("{}")).unwrap_or_default(),
            examples: strings(&q["exampleTestcaseList"]),
            outputs: example_outputs(&content),
            hints: strings(&q["hints"]),
            topics: q["topicTags"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|t| s(&t["name"]))
                .collect(),
            python,
            snippets,
            content,
        })
    }

    /// Runs `code` on the judge against `inputs` (each one argument per line).
    pub fn run(&self, q: &Question, code: &str, inputs: &[String], language: Language) -> Result<JudgeResult> {
        self.require_session()?;
        let referer = format!("{BASE}/problems/{}/", q.slug);
        let body = self.post(
            &format!("{BASE}/problems/{}/interpret_solution/", q.slug),
            &referer,
            &json!({
                "lang": language.judge_id(),
                "question_id": q.question_id,
                "typed_code": code,
                "data_input": inputs.join("\n"),
            }),
        )?;
        let id = body["interpret_id"].as_str().context("no interpret_id in response")?.to_owned();
        self.check(&id, &referer, false)
    }

    pub fn submit(&self, q: &Question, code: &str, language: Language) -> Result<JudgeResult> {
        self.require_session()?;
        let referer = format!("{BASE}/problems/{}/", q.slug);
        let body = self.post(
            &format!("{BASE}/problems/{}/submit/", q.slug),
            &referer,
            &json!({ "lang": language.judge_id(), "question_id": q.question_id, "typed_code": code }),
        )?;
        let id = body["submission_id"]
            .as_i64()
            .map(|n| n.to_string())
            .context("no submission_id in response")?;
        self.check(&id, &referer, true)
    }

    fn require_session(&self) -> Result<()> {
        if self.signed_in() {
            Ok(())
        } else {
            Err(anyhow!("LeetCode sign-in required"))
        }
    }

    fn check(&self, id: &str, referer: &str, submission: bool) -> Result<JudgeResult> {
        let deadline = Instant::now() + Duration::from_secs(90);
        loop {
            sleep(Duration::from_millis(700));
            let mut req = self
                .agent
                .get(format!("{BASE}/submissions/detail/{id}/check/"))
                .header("User-Agent", self.user_agent())
                .header("Referer", referer);
            if let Some(c) = &self.creds {
                req = req.header("Cookie", &c.cookie);
            }
            let body: Value = req.call()?.body_mut().read_json()?;
            if body["state"] == "SUCCESS" {
                return Ok(parse_result(&body, submission));
            }
            if Instant::now() > deadline {
                bail!("LeetCode judge timed out");
            }
        }
    }
}

fn parse_result(b: &Value, submission: bool) -> JudgeResult {
    let s = |v: &Value| match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    };
    let list = |v: &Value| -> Vec<String> {
        v.as_array().into_iter().flatten().map(s).collect()
    };
    JudgeResult {
        submission,
        status_code: b["status_code"].as_i64().unwrap_or(0),
        status: s(&b["status_msg"]),
        total_correct: b["total_correct"].as_i64(),
        total_testcases: b["total_testcases"].as_i64(),
        runtime: s(&b["status_runtime"]),
        memory: s(&b["status_memory"]),
        runtime_percentile: b["runtime_percentile"].as_f64(),
        memory_percentile: b["memory_percentile"].as_f64(),
        answers: list(&b["code_answer"]),
        expected: list(&b["expected_code_answer"]),
        compare: s(&b["compare_result"]),
        stdout: list(&b["std_output_list"]),
        last_input: s(&b["last_testcase"]),
        expected_output: s(&b["expected_output"]),
        actual_output: s(&b["code_output"]),
        error: [s(&b["full_compile_error"]), s(&b["full_runtime_error"])].concat(),
    }
}

static OUTPUT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?s)<strong>\s*Output:?\s*</strong>:?\s*(?:<span[^>]*>)?(.*?)(?:</span>|\n|</p>|</pre>|<strong>)")
        .unwrap()
});
static TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<[^>]+>").unwrap());

/// Expected outputs of the statement's examples, in order.
pub fn example_outputs(html: &str) -> Vec<String> {
    OUTPUT
        .captures_iter(html)
        .map(|c| {
            let text = TAG.replace_all(&c[1], "");
            html_escape::decode_html_entities(text.trim()).into_owned()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_pre_and_block_example_outputs() {
        let pre = "<pre>\n<strong>Input:</strong> nums = [2,7]\n<strong>Output:</strong> [0,1]\n<strong>Explanation:</strong> x</pre>";
        assert_eq!(example_outputs(pre), vec!["[0,1]"]);
        let block = r#"<p><strong>Output:</strong> <span class="example-io">&quot;fl&quot;</span></p>"#;
        assert_eq!(example_outputs(block), vec!["\"fl\""]);
        let design = "<strong>Output</strong>\n[null, null, 1]\n\n<strong>Explanation</strong>";
        assert_eq!(example_outputs(design), vec!["[null, null, 1]"]);
    }

    #[test]
    fn judge_run_with_a_failed_case_is_not_accepted() {
        let run = JudgeResult { status_code: 10, compare: "101".into(), ..Default::default() };
        assert!(!run.accepted());
        let sub = JudgeResult { status_code: 10, submission: true, ..Default::default() };
        assert!(sub.accepted());
    }
}

#[cfg(test)]
mod live {
    use super::*;

    /// `cargo test -p practice live -- --ignored`: hits leetcode.com with the shared session.
    #[test]
    #[ignore]
    fn catalog_question_and_session() {
        let creds = crate::creds::load(crate::creds::Account::LeetCode).ok();
        let client = Client::new(creds);
        let catalog = client.catalog().unwrap();
        assert!(catalog.len() > 3000);
        eprintln!("signed in: {}, solved: {}", client.signed_in(), catalog.iter().filter(|c| c.status.as_deref() == Some("ac")).count());
        let q = client.question("two-sum").unwrap();
        assert_eq!(q.examples.len(), q.outputs.len());
        assert!(q.python.contains("class Solution"));
    }
}
