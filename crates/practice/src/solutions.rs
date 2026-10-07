//! Public NeetCode reference solutions, cached separately from user solution files.
use std::{collections::HashMap, sync::{Arc, LazyLock, Mutex, Weak}, time::Duration};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use crate::{db::Db, language::Language};
const ROOT: &str = "https://raw.githubusercontent.com/neetcode-gh/leetcode/main";
pub const LICENSE: &str = include_str!("../assets/neetcode-license.txt");
static LOADERS: LazyLock<Mutex<HashMap<String, Weak<Mutex<()>>>>> = LazyLock::new(|| Mutex::new(HashMap::new()));
fn loader(key: &str) -> Arc<Mutex<()>> {
    let mut loaders = LOADERS.lock().unwrap_or_else(|error| error.into_inner());
    loaders.retain(|_, loader| loader.strong_count() > 0);
    if let Some(loader) = loaders.get(key).and_then(Weak::upgrade) { return loader; }
    let loader = Arc::new(Mutex::new(())); loaders.insert(key.into(), Arc::downgrade(&loader)); loader
}
#[derive(Clone, Debug, Serialize, Deserialize)] pub struct Reference { pub code: String, pub url: String, pub language: Language }
#[derive(Deserialize)] struct Entry {
    link: String, code: String,
    #[serde(default)] python: bool, #[serde(default)] cpp: bool, #[serde(default)] go: bool, #[serde(default)] c: bool, #[serde(default)] java: bool,
}
fn fetch(url: &str) -> Result<String> {
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(30))).build().into();
    Ok(agent.get(url).call()?.body_mut().with_config().limit(4 << 20).read_to_string()?)
}
fn index(db: &Db) -> Result<Vec<Entry>> {
    let loader = loader("reference-index"); let _guard = loader.lock().unwrap_or_else(|error| error.into_inner());
    if let Some(raw) = db.get("reference-index")? { if let Ok(index) = serde_json::from_str(&raw) { return Ok(index); } }
    let raw = fetch(&format!("{ROOT}/.problemSiteData.json"))?;
    let index = serde_json::from_str(&raw)?; db.set("reference-index", &raw)?; Ok(index)
}
pub fn get(db: &Db, slug: &str, language: Language) -> Result<Reference> {
    if slug.starts_with("cf:") { bail!("NeetCode reference code is available for LeetCode problems"); }
    let key = format!("reference:neetcode:{slug}:{}", language.id());
    let loader = loader(&key); let _guard = loader.lock().unwrap_or_else(|error| error.into_inner());
    if let Some(raw) = db.get(&key)? { if let Ok(reference) = serde_json::from_str(&raw) { return Ok(reference); } }
    let entry = index(db)?.into_iter().find(|entry| entry.link.trim_matches('/') == slug).context("No public NeetCode solution for this problem")?;
    let supported = match language { Language::Python => entry.python, Language::Cpp => entry.cpp, Language::Go => entry.go, Language::C => entry.c, Language::Java => entry.java };
    if !supported { bail!("No {} reference; select another language", language.label()); }
    if entry.code.is_empty() || !entry.code.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_') { bail!("Invalid reference filename"); }
    let folder = if language == Language::Cpp { "cpp" } else { language.id() };
    let path = format!("{folder}/{}.{}", entry.code, language.extension());
    let code = fetch(&format!("{ROOT}/{path}"))?;
    let reference = Reference { code, url: format!("https://github.com/neetcode-gh/leetcode/blob/main/{path}"), language };
    db.set(&key, &serde_json::to_string(&reference)?)?; Ok(reference)
}
#[cfg(test)] mod tests {
    use super::*;
    #[test] #[ignore = "fetches public NeetCode reference code"]
    fn live_reference_and_offline_cache() {
        let dir = tempfile::tempdir().unwrap(); let db = Db::open(&dir.path().join("test.db")).unwrap();
        let reference = get(&db, "reorder-list", Language::Python).unwrap();
        assert!(reference.code.contains("def reorderList"));
        db.set("reference-index", "broken").unwrap();
        let cached = get(&db, "reorder-list", Language::Python).unwrap();
        assert_eq!(cached.code, reference.code);
        assert!(cached.url.starts_with("https://github.com/neetcode-gh/leetcode/blob/"));
        for language in [Language::Cpp, Language::Go, Language::C] {
            let reference = get(&db, "contains-duplicate", language).unwrap();
            assert!(!reference.code.is_empty()); assert_eq!(reference.language, language);
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Article { pub markdown: String, pub videos: Vec<String>, pub url: String }

pub fn parse_article(html: &str, url: &str, language: Language) -> Result<Article> {
    use scraper::{ElementRef, Html, Selector};
    fn visit(element: ElementRef<'_>, language: Language, out: &mut String, videos: &mut Vec<String>) {
        let tag = element.value().name();
        if matches!(tag, "script" | "style" | "button" | "svg" | "h1") { return; }
        if tag == "iframe" || tag == "video" {
            if let Some(url) = element.value().attr("src") {
                if url.starts_with("https://") && !videos.iter().any(|video| video == url) { videos.push(url.into()); }
            }
            return;
        }
        if tag == "app-code-tabs" {
            let selector = Selector::parse("pre").expect("static code selector");
            let id = if language == Language::Cpp { "cpp" } else { language.id() };
            for code in element.select(&selector).filter(|code| code.value().classes().any(|class| class == format!("language-{id}"))) {
                visit(code, language, out, videos);
            }
            return;
        }
        if tag == "pre" {
            let text = element.text().collect::<String>();
            let syntax = element.value().classes().find_map(|class| class.strip_prefix("language-")).unwrap_or(language.id());
            let fence = "`".repeat(text.split(|ch| ch != '`').map(str::len).max().unwrap_or(0).max(2) + 1);
            out.push_str(&format!("\n\n{fence}{syntax}\n{text}\n{fence}\n\n")); return;
        }
        if matches!(tag, "div" | "main" | "article" | "app-code" | "section" | "details") {
            for child in element.children() {
                if let Some(child) = ElementRef::wrap(child) { visit(child, language, out, videos); }
                else if let Some(text) = child.value().as_text() { if !text.trim().is_empty() { out.push_str(text); } }
            }
            return;
        }
        out.push_str(&crate::prompts::statement_markdown(&element.html())); out.push_str("\n\n");
    }
    let document = Html::parse_document(html);
    let selector = Selector::parse("main.neeter-article-content, article").expect("static article selector");
    let article = document.select(&selector).next().context("No solution article was returned; open the source page or retry")?;
    let mut markdown = String::new(); let mut videos = Vec::new(); visit(article, language, &mut markdown, &mut videos);
    if markdown.trim().is_empty() { bail!("The solution article is empty"); }
    let base = url::Url::parse(url)?;
    let links = regex::Regex::new(r"(!?\[[^\]]*\]\()([^\s)]+)(\))")?;
    let markdown = links.replace_all(&markdown, |capture: &regex::Captures| {
        base.join(&capture[2]).map_or_else(|_| capture[0].to_owned(), |url| format!("{}{}{}", &capture[1], url, &capture[3]))
    }).into_owned();
    Ok(Article { markdown, videos, url: url.into() })
}

pub fn article(db: &Db, slug: &str, language: Language) -> Result<Article> {
    if slug.starts_with("cf:") {
        let markdown = db.get(&format!("editorial:{slug}"))?.context("No cached editorial for this question")?;
        return Ok(Article { markdown, videos: vec![], url: crate::codeforces::problem_url(slug)? });
    }
    if slug.is_empty() || !slug.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '-') { bail!("No NeetCode article for this source"); }
    let key = format!("article:neetcode:v1:{slug}:{}", language.id());
    let loader = loader(&key); let _guard = loader.lock().unwrap_or_else(|error| error.into_inner());
    if let Some(raw) = db.get(&key)? { if let Ok(article) = serde_json::from_str(&raw) { return Ok(article); } }
    let url = format!("https://neetcode.io/solutions/{slug}");
    let article = parse_article(&fetch(&url)?, &url, language)?;
    db.set(&key, &serde_json::to_string(&article)?)?; Ok(article)
}

#[cfg(test)] mod article_tests {
    use super::*;
    #[test] fn keeps_approaches_complexity_images_and_media_without_duplicate_languages() {
        let article = parse_article(r#"<main class="neeter-article-content"><h1>Title</h1><h2>Recursion</h2><p>Reason <code>x</code>.</p><app-code-tabs><pre class="language-python">return 1</pre><pre class="language-cpp">return 2;</pre></app-code-tabs><h3>Complexity</h3><ul><li>O(n)</li></ul><img src="/diagram.png"><iframe src="https://www.youtube.com/embed/abc"></iframe><script>bad()</script><h2>Pitfalls</h2><p>Check empty input.</p></main>"#, "https://neetcode.io/solutions/example", Language::Python).unwrap();
        assert!(article.markdown.contains("Recursion") && article.markdown.contains("Complexity") && article.markdown.contains("Pitfalls"));
        assert!(article.markdown.contains("return 1") && !article.markdown.contains("return 2"));
        assert!(article.markdown.contains("https://neetcode.io/diagram.png"));
        assert!(!article.markdown.contains("bad()")); assert_eq!(article.videos.len(), 1);
    }
    #[test] #[ignore = "fetches a full NeetCode solution article"]
    fn live_article_and_cached_read() {
        let dir = tempfile::tempdir().unwrap(); let db = Db::open(&dir.path().join("test.db")).unwrap();
        let article = article(&db, "minimum-path-sum", Language::Python).unwrap();
        assert!(article.markdown.contains("Space Optimized") || article.markdown.contains("Space Optimized".to_lowercase().as_str()));
        assert!(article.markdown.contains("Common Pitfalls")); assert!(!article.videos.is_empty());
        assert_eq!(super::article(&db, "minimum-path-sum", Language::Python).unwrap().markdown, article.markdown);
    }
}

/// Public premium alternatives have no judge starter; keep signatures, never the answer.
pub fn python_starter(code: &str) -> String {
    let code = code.split("class Solution:").nth(1).unwrap_or(code);
    let mut starter = String::from("from typing import List, Optional\n\nclass Solution:\n");
    for line in code.lines() {
        let signature = line.trim();
        if signature.starts_with("def ") && signature.ends_with(':') && !signature.starts_with("def __") {
            starter.push_str("    "); starter.push_str(signature); starter.push_str("\n        pass\n\n");
        }
    }
    if !starter.contains("    def ") { starter.push_str("    pass\n"); }
    starter
}
