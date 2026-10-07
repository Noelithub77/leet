//! Learning prompts opened in ChatGPT or Claude with the problem, Solution and failures prefilled.

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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Style {
    Hints,
    #[default]
    Guided,
    Full,
    SolutionOnly,
}

impl Style {
    pub const ALL: [Style; 4] = [Style::Hints, Style::Guided, Style::Full, Style::SolutionOnly];

    pub fn label(self) -> &'static str {
        match self {
            Style::Hints => "Hints only",
            Style::Guided => "Guided learning",
            Style::Full => "Full explanation",
            Style::SolutionOnly => "Solution only",
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Self::Hints => "hints",
            Self::Guided => "guided",
            Self::Full => "full",
            Self::SolutionOnly => "solution-only",
        }
    }

    pub fn instructions(self) -> &'static str {
        match self {
            Style::Hints => "Give me hints only, never code or the full approach. Start with the smallest nudge (what to notice in the problem). Give one hint, then stop and wait for me to ask for the next. Escalate gradually: observation, then useful data structure or pattern, then the key insight. If my attempt is below, point at the first thing to reconsider without fixing it.",
            Style::Guided => "Teach me to solve this like a patient tutor using the Socratic method. Do not give the solution or code up front. Ask me one question at a time that leads me toward the key insight, wait for my answer, and adapt to it. Build intuition from brute force to the optimal approach, and make me state the time and space complexity myself. If my attempt is below, start from where I am.",
            Style::Full => "Explain this problem completely: 1) the intuition and the pattern it belongs to, 2) brute force, then the optimal approach and why it works, 3) clean, idiomatic code in the requested language, 4) time and space complexity, 5) edge cases and common pitfalls. If my attempt and failing tests are below, explain exactly why each fails and how to fix my code with minimal changes.",
            Style::SolutionOnly => "Return only the complete, optimal solution for the active problem in the requested language. Preserve the required entry point exactly. Include necessary imports. Output one code block, without prose, pseudocode, placeholders, or a solution to a different problem. Check the examples and edge cases before answering.",
        }
    }
}

/// What the prompt knows about the current problem.
pub struct Context<'a> {
    pub title: &'a str,
    pub difficulty: &'a str,
    pub url: &'a str,
    pub statement_html: &'a str,
    pub code: &'a str,
    pub starter: &'a str,
    pub language: crate::language::Language,
    /// Human-readable failing cases (input, expected, actual), already formatted.
    pub failures: &'a [String],
}

/// Statements are trimmed so the URL stays within what chat sites accept.
const STATEMENT_LIMIT: usize = 3500;
const CODE_LIMIT: usize = 4000;

pub fn build(style: Style, ctx: &Context) -> String {
    build_custom(style, ctx, style.instructions())
}

pub fn build_custom(style: Style, ctx: &Context, instructions: &str) -> String {
    let mut statement = statement_markdown(ctx.statement_html);
    truncate(&mut statement, STATEMENT_LIMIT);
    let mut out = format!(
        "{}\n\nProblem: {} ({}) {}\n\n{}\n",
        instructions,
        ctx.title,
        ctx.difficulty,
        ctx.url,
        statement.trim()
    );
    let code = ctx.code.trim();
    out.push_str(&format!("\nRequired language: {}. Solve the problem named above; an existing attempt may belong to another question.\n", ctx.language.label()));
    if !ctx.starter.trim().is_empty() {
        out.push_str(&format!("\nRequired judge interface (preserve class, method names, parameters, and return type):\n```{}\n{}\n```\n", ctx.language.id(), ctx.starter.trim()));
    }
    if ctx.url.contains("codeforces.com") {
        out.push_str("Use stdin/stdout and the provided program entry point. Do not return a LeetCode Solution class.\n");
    }
    if style != Style::SolutionOnly && !code.is_empty() && !is_starter(code) {
        let mut code = code.to_owned();
        truncate(&mut code, CODE_LIMIT);
        out.push_str(&format!("\nMy attempt ({}):\n```{}\n{code}\n```\n", ctx.language.label(), ctx.language.id()));
        if !ctx.failures.is_empty() {
            out.push_str("\nFailing tests:\n");
            for f in ctx.failures.iter().take(3) {
                out.push_str(&format!("- {f}\n"));
            }
        }
    }
    out
}

/// A starter snippet has only signatures; sending it adds noise.
fn is_starter(code: &str) -> bool {
    code.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .all(|l| l.starts_with("class ") || l.starts_with("def ") || l.starts_with('@') || l == "pass")
}

fn truncate(s: &mut String, limit: usize) {
    if s.len() > limit {
        let mut end = limit;
        while !s.is_char_boundary(end) {
            end -= 1;
        }
        s.truncate(end);
        s.push_str("\n…");
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
    let html = SUP.replace_all(html, "^$1");
    // Example blocks mix bold labels into preformatted text; keep them as plain code blocks.
    let html = PRE.replace_all(&html, |c: &regex::Captures| {
        let text = TAG.replace_all(&c[1], "");
        format!("<pre><code>{}</code></pre>", text.trim_matches('\n'))
    });
    let markdown = htmd::convert(&html).unwrap_or_else(|_| html.to_string());
    markdown.replace('\u{a0}', " ").trim().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn solution_only_preserves_the_active_judge_interface_in_all_languages() {
        let mut context = ctx("class Solution:\n    def minWindow(self, s, t): pass", &[]);
        context.title = "Insert Interval";
        context.starter = "class Solution:\n    def insert(self, intervals: list[list[int]], newInterval: list[int]) -> list[list[int]]:";
        let prompt = build(Style::SolutionOnly, &context);
        assert!(prompt.contains("def insert(")); assert!(!prompt.contains("def minWindow("));
        context.language = crate::language::Language::Cpp;
        context.starter = "int main() { }"; context.url = "https://codeforces.com/contest/4/problem/A";
        let prompt = build_custom(Style::SolutionOnly, &context, "Custom instruction");
        assert!(prompt.contains("Required language: C++")); assert!(prompt.contains("Use stdin/stdout"));
        assert!(!prompt.contains("Python 3 solution"));
    }

    fn ctx<'a>(code: &'a str, failures: &'a [String]) -> Context<'a> {
        Context {
            title: "Two Sum",
            difficulty: "Easy",
            url: "https://leetcode.com/problems/two-sum/",
            statement_html: "<p>Given <code>nums</code>, 10<sup>4</sup>&nbsp;max.</p><ul><li>one</li></ul>",
            code,
            language: crate::language::Language::Python,
            starter: "class Solution:\n    def twoSum(self, nums: list[int], target: int) -> list[int]:",
            failures,
        }
    }

    #[test]
    fn includes_attempt_and_failures_but_not_starter_code() {
        let failures = vec!["input [3,3], 6 → expected [0,1], got [1,0]".to_owned()];
        let p = build(Style::Full, &ctx("class Solution:\n    def f(self):\n        return 1\n", &failures));
        assert!(p.contains("My attempt") && p.contains("Failing tests") && p.contains("10^4 max."));
        let starter = build(Style::Hints, &ctx("class Solution:\n    def twoSum(self, nums):\n        \n", &failures));
        assert!(!starter.contains("My attempt"));
        let sol = build(Style::SolutionOnly, &ctx("class Solution:\n    x = 1\n", &[]));
        assert!(!sol.contains("My attempt"));
    }

    #[test]
    fn custom_instructions_retain_problem_and_failure_context() {
        let failures = vec!["expected 2, actual 3".into()];
        let prompt = build_custom(Style::Guided, &ctx("return 3", &failures), "Use diagrams.");
        assert!(prompt.starts_with("Use diagrams."));
        assert!(prompt.contains("Two Sum") && prompt.contains("My attempt") && prompt.contains("expected 2"));
        assert!(!prompt.contains("Socratic"));
    }

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
