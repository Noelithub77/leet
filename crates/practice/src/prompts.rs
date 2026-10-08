//! Web chat links with a prompt prefilled, and statement Markdown shared by prompts and display.

use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Provider {
    #[default]
    ChatGpt,
    Claude,
    Gemini,
}

impl Provider {
    pub const ALL: [Self; 3] = [Self::ChatGpt, Self::Claude, Self::Gemini];
    pub fn label(self) -> &'static str {
        match self {
            Provider::ChatGpt => "ChatGPT",
            Provider::Claude => "Claude",
            Provider::Gemini => "Gemini",
        }
    }

    pub fn toggle(self) -> Self {
        match self {
            Provider::ChatGpt => Provider::Claude,
            Provider::Claude => Provider::Gemini,
            Provider::Gemini => Provider::ChatGpt,
        }
    }
}

pub fn url(provider: Provider, prompt: &str) -> String {
    let mut url = match provider {
        Provider::ChatGpt => Url::parse("https://chatgpt.com/").unwrap(),
        Provider::Claude => Url::parse("https://claude.ai/new").unwrap(),
        Provider::Gemini => return "https://gemini.google.com/app".into(),
    };
    {
        let mut q = url.query_pairs_mut();
        q.append_pair("q", prompt);
    }
    url.into()
}

static SUP: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)<sup>(.*?)</sup>").unwrap());
static PRE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?is)<pre[^>]*>(.*?)</pre>").unwrap());
static TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<[^>]+>").unwrap());

/// LeetCode statement HTML as Markdown, for display and prompts. Exponents become `^n`.
pub fn statement_markdown(html: &str) -> String {
    let (html, formulas) = crate::rich_text::protect_html_math(html);
    let html = SUP.replace_all(&html, "^$1");
    // Example blocks mix bold labels into preformatted text; keep them as plain code blocks.
    let html = PRE.replace_all(&html, |c: &regex::Captures| {
        let text = TAG.replace_all(&c[1], "");
        format!("<pre><code>{}</code></pre>", text.trim_matches('\n'))
    });
    let mut markdown = htmd::convert(&html).unwrap_or_else(|_| html.to_string());
    for (key, formula) in formulas { markdown = markdown.replace(&key, &formula); }
    markdown.replace('\u{a0}', " ").trim().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn urls_encode_prompts_without_forcing_models() {
        let u = url(Provider::ChatGpt, "a b&c");
        assert_eq!(u, "https://chatgpt.com/?q=a+b%26c");
        assert_eq!(url(Provider::Claude, "hi"), "https://claude.ai/new?q=hi");
        assert_eq!(url(Provider::Gemini, "hi"), "https://gemini.google.com/app");
    }

    #[test]
    fn example_blocks_become_code_blocks() {
        let md = statement_markdown("<pre>\n<strong>Input:</strong> nums = [1,2]\n<strong>Output:</strong> [1]\n</pre>");
        assert!(md.contains("```") && md.contains("Input: nums = [1,2]\nOutput: [1]"), "{md:?}");
    }

    #[test]
    fn statement_markdown_keeps_code_lists_and_exponents() {
        let md = statement_markdown("<p>Given <code>nums</code>, 10<sup>4</sup> max.</p><ul><li>x</li></ul>");
        assert!(md.contains("`nums`") && md.contains("10^4") && md.contains("x"), "{md}");
    }
}
