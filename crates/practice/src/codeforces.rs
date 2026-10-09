//! Public Codeforces API. Calls share a limiter, including account validation.
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde::de::DeserializeOwned;
use url::Url;

static LAST_CALL: LazyLock<Mutex<Option<Instant>>> = LazyLock::new(|| Mutex::new(None));

#[derive(Clone, Debug, Deserialize)]
pub struct User { pub handle: String, pub rating: Option<u32> }

#[derive(Deserialize)]
struct Envelope<T> { status: String, comment: Option<String>, result: Option<T> }

pub fn validate_handle(handle: &str) -> Result<&str> {
    let handle = handle.trim();
    if handle.is_empty() || handle.len() > 24 || !handle.chars().all(|ch| ch.is_ascii_alphanumeric() || "._-".contains(ch)) {
        bail!("Enter a Codeforces handle (letters, numbers, dot, underscore, or hyphen)");
    }
    Ok(handle)
}

pub fn request<T: DeserializeOwned>(method: &str, params: &[(&str, &str)]) -> Result<T> {
    let mut url = Url::parse(&format!("https://codeforces.com/api/{method}"))?;
    url.query_pairs_mut().extend_pairs(params.iter().copied());
    {
        let mut last = LAST_CALL.lock().unwrap_or_else(|error| error.into_inner());
        if let Some(previous) = *last { std::thread::sleep(Duration::from_secs(2).saturating_sub(previous.elapsed())); }
        *last = Some(Instant::now());
    }
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(30))).build().into();
    let mut response = agent.get(url.as_str()).header("User-Agent", "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 Chrome/140.0 Safari/537.36").call().context("Codeforces API request")?;
    let response: Envelope<T> = response.body_mut().with_config().limit(32 << 20).read_json().context("Read Codeforces API response")?;
    if response.status != "OK" { bail!("Codeforces: {}", response.comment.unwrap_or_else(|| "request failed".into())); }
    response.result.context("Codeforces response has no result")
}

pub fn user(handle: &str) -> Result<User> {
    let handle = validate_handle(handle)?;
    let mut users: Vec<User> = request("user.info", &[("handles", handle)])?;
    users.pop().context("Codeforces account was not found")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn statement_keeps_sample_lines_entities_and_language_starters() {
        let html = r#"<div class="problem-statement"><div class="header"><div class="title">A. Sum</div></div><p>Compute a &lt; b.</p><div class="sample-test"><div class="input"><pre><div>2</div><div>3 4</div></pre></div><div class="output"><pre>7<br>8</pre></div></div></div>"#;
        let question = parse_statement("cf:1:A", html).unwrap();
        assert_eq!(question.title, "Sum");
        assert_eq!(question.examples, ["2\n3 4"]);
        assert_eq!(question.outputs, ["7\n8"]);
        assert!(question.content.contains("&lt;"));
        for language in crate::language::Language::ALL { assert!(question.starter(language).is_some()); }
        assert!(parse_statement("cf:1:A", "<html>blocked</html>").is_err());
        assert!(problem_id("cf:1:../../submit").is_err());
        assert_eq!(problem_url("cf:1:A").unwrap(), "https://codeforces.com/problemset/problem/1/A");
    }
    #[test]
    fn sample_only_import_is_upgraded_without_replacing_its_cases() {
        let dir = tempfile::tempdir().unwrap(); let db = crate::db::Db::open_unseeded(&dir.path().join("cache.db")).unwrap();
        let import: crate::companion::Import = serde_json::from_value(serde_json::json!({"name":"Copper Squanderer","group":"CodeChef","url":"https://codeforces.com/problemset/problem/2275/G","tests":[{"input":"browser input","output":"browser output"}]})).unwrap();
        let slug = import.cache(&db).unwrap();
        let full = cached_question_with(&db, &slug, |slug| parse_statement(slug, "<div class=\"problem-statement\"><div class=\"title\">G. Copper Squander</div><p>Full network rebuilding description.</p></div>")).unwrap();
        assert!(full.content.contains("Full network rebuilding description."));
        assert_eq!(full.meta["source"], "codeforces");
        assert_ne!(full.meta["statementSource"], "competitive-companion");
        assert_eq!(db.test_cases(&slug).unwrap().unwrap()[0].input, "browser input");
        let cached = cached_question_with(&db, &slug, |_| panic!("Complete cached statements must remain offline")).unwrap();
        assert_eq!(cached.content, full.content);
    }
    #[test]
    #[ignore]
    fn live_public_catalog_and_statement() {
        assert!(!catalog().unwrap().is_empty());
        assert_eq!(user("tourist").unwrap().handle, "tourist");
        let question = question("cf:1:A").unwrap();
        assert!(!question.examples.is_empty());
    }
    #[test]
    fn validates_handles_without_accepting_query_injection() {
        assert_eq!(validate_handle(" tourist ").unwrap(), "tourist");
        assert!(validate_handle("").is_err());
        assert!(validate_handle("a;another-user").is_err());
        assert!(validate_handle("a&handles=b").is_err());
    }
    #[test]
    fn curl_fallback_accepts_only_validated_problem_urls_and_bounds_requests() {
        assert!(curl_command("cf:1:../../submit").is_err());
        assert!(curl_command("cc:START01").is_err());
        let command = curl_command("cf:2275:G").unwrap();
        let args: Vec<_> = command.get_args().map(|arg| arg.to_str().unwrap()).collect();
        assert_eq!(args[0], "--disable");
        for pair in [["--proto", "=https"], ["--proto-redir", "=https"], ["--max-time", "30"], ["--max-filesize", "8388608"]] {
            assert!(args.windows(2).any(|args| args == pair));
        }
        assert_eq!(args.last().unwrap(), &"https://codeforces.com/problemset/problem/2275/G");
    }
    #[test]
    #[ignore = "Requires installed curl and live Codeforces access"]
    fn live_curl_upgrades_copper_squander_import() {
        let dir = tempfile::tempdir().unwrap(); let db = crate::db::Db::open_unseeded(&dir.path().join("cache.db")).unwrap();
        let import: crate::companion::Import = serde_json::from_value(serde_json::json!({"name":"Copper Squanderer","group":"CodeChef","url":"https://codeforces.com/problemset/problem/2275/G","tests":[{"input":"browser input","output":"browser output"}]})).unwrap();
        let slug = import.cache(&db).unwrap();
        let full = cached_question_with(&db, &slug, curl_question).unwrap();
        assert_eq!(full.title, "Copper Squander");
        assert!(full.content.contains("input-specification"));
        assert!(full.content.contains("output-specification"));
        assert!(full.content.len() > 5000);
        assert_eq!(full.meta["source"], "codeforces");
        assert_eq!(full.meta["statementSource"], "codeforces-curl");
        assert!(!full.examples.is_empty());
        assert_eq!(db.test_cases(&slug).unwrap().unwrap()[0].input, "browser input");
    }
}


#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Problem {
    pub contest_id: Option<u32>,
    pub index: String,
    pub name: String,
    pub rating: Option<u32>,
    pub tags: Vec<String>,
}
impl Problem {
    pub fn slug(&self) -> Option<String> { self.contest_id.map(|id| format!("cf:{id}:{}", self.index)) }
}

pub fn problem_id(slug: &str) -> Result<(u32, &str)> {
    let mut parts = slug.split(':');
    if parts.next() != Some("cf") { bail!("Not a Codeforces problem"); }
    let contest = parts.next().context("Missing contest id")?.parse()?;
    let index = parts.next().context("Missing problem index")?;
    if parts.next().is_some() || index.is_empty() || !index.chars().all(|ch| ch.is_ascii_alphanumeric()) { bail!("Invalid Codeforces problem index"); }
    Ok((contest, index))
}

pub fn problem_url(slug: &str) -> Result<String> {
    let (contest, index) = problem_id(slug)?;
    Ok(format!("https://codeforces.com/problemset/problem/{contest}/{index}"))
}

pub fn catalog() -> Result<Vec<Problem>> {
    #[derive(Deserialize)]
    struct Catalog { problems: Vec<Problem> }
    let catalog: Catalog = request("problemset.problems", &[])?;
    Ok(catalog.problems.into_iter().filter(|problem| problem.contest_id.is_some()).collect())
}

pub fn solved(handle: &str) -> Result<std::collections::HashSet<String>> {
    #[derive(Deserialize)]
    struct Submission { problem: Problem, verdict: Option<String> }
    let handle = validate_handle(handle)?;
    let mut solved = std::collections::HashSet::new();
    for page in 0..100 {
        let from = (page * 1000 + 1).to_string();
        let submissions: Vec<Submission> = request("user.status", &[("handle", handle), ("from", &from), ("count", "1000")])?;
        let last = submissions.len() < 1000;
        for submission in submissions {
            if submission.verdict.as_deref() == Some("OK") {
                if let Some(slug) = submission.problem.slug() { solved.insert(slug); }
            }
        }
        if last { return Ok(solved); }
    }
    bail!("Codeforces history exceeds 100,000 submissions; cached progress was kept")
}

fn sample_text(element: scraper::ElementRef<'_>) -> String {
    fn append(element: scraper::ElementRef<'_>, out: &mut String) {
        for child in element.children() {
            if let Some(text) = child.value().as_text() { out.push_str(text); }
            else if let Some(element) = scraper::ElementRef::wrap(child) {
                if element.value().name() == "br" { out.push('\n'); }
                else {
                    append(element, out);
                    if element.value().name() == "div" && !out.ends_with('\n') { out.push('\n'); }
                }
            }
        }
    }
    let mut text = String::new();
    append(element, &mut text);
    text.trim_matches('\n').to_owned()
}

pub fn parse_statement(slug: &str, html: &str) -> Result<crate::leetcode::Question> {
    use scraper::{Html, Selector};
    let (contest, index) = problem_id(slug)?;
    let document = Html::parse_document(html);
    let selector = |value: &str| Selector::parse(value).expect("static Codeforces selector");
    let statement = document.select(&selector(".problem-statement")).next().context("Codeforces returned no statement; open the problem in your browser and retry")?;
    let title = statement.select(&selector(".header .title")).next().map(|title| title.text().collect::<String>()).unwrap_or_else(|| format!("{contest}{index}"));
    let inputs = statement.select(&selector(".sample-test .input pre")).map(sample_text).collect();
    let outputs = statement.select(&selector(".sample-test .output pre")).map(sample_text).collect();
    let snippets = crate::language::Language::ALL.into_iter().map(|language| (language.judge_id().to_owned(), language.stdin_template().to_owned())).collect();
    Ok(crate::leetcode::Question {
        question_id: format!("{contest}{index}"), frontend_id: format!("{contest}{index}"),
        title: title.split_once(". ").map_or(title.clone(), |(_, title)| title.to_owned()),
        slug: slug.into(), difficulty: "Codeforces".into(), paid_only: false,
        content: statement.inner_html(), python: crate::language::Language::Python.stdin_template().into(), snippets,
        meta: serde_json::json!({"source": "codeforces"}), examples: inputs, outputs,
        hints: vec![], topics: vec![],
    })
}

pub fn question(slug: &str) -> Result<crate::leetcode::Question> {
    match live_question(slug) {
        Ok(question) => Ok(question),
        Err(live) => match curl_question(slug) {
            Ok(question) => Ok(question),
            Err(curl) => crate::codeforces_snapshot::question(slug)
                .map_err(|snapshot| anyhow::anyhow!("Codeforces statement unavailable ({live}). Curl fallback: {curl:#}. Snapshot fallback: {snapshot}")),
        },
    }
}

pub fn cached_question(db: &crate::db::Db, slug: &str) -> Result<crate::leetcode::Question> {
    cached_question_with(db, slug, question)
}
fn cached_question_with(db: &crate::db::Db, slug: &str, fetch: impl FnOnce(&str) -> Result<crate::leetcode::Question>) -> Result<crate::leetcode::Question> {
    if let Some(question) = db.question(slug)? {
        if question.meta["statementSource"] != "competitive-companion" { return Ok(question); }
    }
    let question = fetch(slug)?;
    db.save_question(&question)?;
    Ok(question)
}

fn live_question(slug: &str) -> Result<crate::leetcode::Question> {
    let url = problem_url(slug)?;
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(30))).build().into();
    let mut response = agent.get(&url).header("User-Agent", "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 Chrome/140.0 Safari/537.36").call()?;
    let html = response.body_mut().with_config().limit(8 << 20).read_to_string()?;
    parse_statement(slug, &html)
}

fn curl_command(slug: &str) -> Result<std::process::Command> {
    let url = problem_url(slug)?;
    let mut command = crate::background_process::command("curl");
    // Ignore personal curl configuration; fetch only bounded public HTTPS statements.
    command.args(["--disable", "--silent", "--show-error", "--fail", "--location",
        "--max-redirs", "5", "--connect-timeout", "10", "--max-time", "30",
        "--max-filesize", "8388608", "--proto", "=https", "--proto-redir", "=https",
        "--user-agent", "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 Chrome/140.0 Safari/537.36",
        "--url", &url]);
    command.stdin(std::process::Stdio::null());
    Ok(command)
}

fn curl_question(slug: &str) -> Result<crate::leetcode::Question> {
    let output = curl_command(slug)?.output().context("Run optional installed curl client")?;
    if !output.status.success() {
        bail!("curl exited with {}: {}", output.status, String::from_utf8_lossy(&output.stderr).trim());
    }
    if output.stdout.len() > 8 << 20 { bail!("Codeforces statement exceeds 8 MiB"); }
    let html = String::from_utf8(output.stdout).context("Read curl statement as UTF-8")?;
    let mut question = parse_statement(slug, &html)?;
    question.meta["statementSource"] = serde_json::json!("codeforces-curl");
    Ok(question)
}
