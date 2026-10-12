//! Ordered semantic statement blocks; unknown content remains rendered Markdown.
use scraper::{ElementRef, Html, Selector};
use crate::prompts::statement_markdown;
pub mod media;
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)] pub enum Part { Markdown(String), Field { label: String, value: String } }
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)] pub enum Block { Markdown(String), Example { title: String, parts: Vec<Part> }, Constraints(String) }
fn plain(element: ElementRef<'_>) -> String {
    fn collect(element: ElementRef<'_>, text: &mut String) {
        for child in element.children() {
            if let Some(value) = child.value().as_text() { text.push_str(value); }
            else if let Some(element) = ElementRef::wrap(child) {
                if element.value().name() == "br" { text.push('\n'); }
                else { collect(element, text); if element.value().name() == "div" { text.push('\n'); } }
            }
        }
    }
    let mut text = String::new(); collect(element, &mut text); text.trim().into()
}
fn fields(text: &str) -> Option<Vec<Part>> {
    let mut fields: Vec<Part> = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim_start();
        if let Some((label, value)) = ["Input:", "Output:", "Explanation:"].into_iter().find_map(|label| trimmed.strip_prefix(label).map(|value| (label.trim_end_matches(':'), value.trim()))) {
            fields.push(Part::Field { label: label.into(), value: value.into() });
        } else if let Some(Part::Field { value, .. }) = fields.last_mut() { value.push('\n'); value.push_str(line); }
        else if !trimmed.is_empty() { return None; }
    }
    (!fields.is_empty()).then_some(fields)
}
fn push(blocks: &mut Vec<Block>, part: Part) {
    if let Some(Block::Example { parts, .. }) = blocks.last_mut() { parts.push(part); }
    else { match part { Part::Markdown(markdown) => blocks.push(Block::Markdown(markdown)), Part::Field { label, value } => blocks.push(Block::Markdown(format!("**{label}:** {value}"))) } }
}
pub fn parse(html: &str) -> Vec<Block> {
    fn visit(element: ElementRef<'_>, blocks: &mut Vec<Block>, constraints: &mut bool) {
        let tag = element.value().name();
        if matches!(tag, "script" | "style") { return; }
        if element.value().classes().any(|class| class == "sample-test") {
            if element.html().contains("<img") || element.html().contains("<svg") {
                blocks.push(Block::Example { title: "Examples".into(), parts: vec![Part::Markdown(statement_markdown(&element.html()))] });
                return;
            }
            let selector = |value| Selector::parse(value).expect("static sample selector");
            let inputs: Vec<_> = element.select(&selector(".input pre")).map(plain).collect();
            let outputs: Vec<_> = element.select(&selector(".output pre")).map(plain).collect();
            for (index, input) in inputs.into_iter().enumerate() {
                let mut parts = vec![Part::Field { label: "Input".into(), value: input }];
                if let Some(output) = outputs.get(index) { parts.push(Part::Field { label: "Output".into(), value: output.clone() }); }
                blocks.push(Block::Example { title: format!("Example {}", index + 1), parts });
            }
            for child in element.children().filter_map(ElementRef::wrap) {
                if !child.value().classes().any(|class| matches!(class, "input" | "output" | "title")) { visit(child, blocks, constraints); }
                else {
                    for extra in child.children().filter_map(ElementRef::wrap) {
                        if extra.value().name() != "pre" && !extra.value().classes().any(|class| class == "title") { visit(extra, blocks, constraints); }
                    }
                }
            }
            return;
        }
        if matches!(tag, "div" | "section" | "article" | "main") {
            if element.value().classes().any(|class| class == "title") && element.ancestors().filter_map(ElementRef::wrap).any(|parent| parent.value().classes().any(|class| class == "header")) { return; }
            for child in element.children() { if let Some(child) = ElementRef::wrap(child) { visit(child, blocks, constraints); } else if let Some(text) = child.value().as_text() { if !text.trim().is_empty() { push(blocks, Part::Markdown(text.to_string())); } } }
            return;
        }
        let text = plain(element);
        let markup = element.html();
        let media = markup.contains("<img") || markup.contains("<svg");
        if text.is_empty() && !media { return; }
        if !media && tag.starts_with('h') {
            let heading = text.trim_end_matches(':').to_lowercase();
            if heading.starts_with("sample input") {
                *constraints = false;
                let index = blocks.iter().filter(|block| matches!(block, Block::Example { .. })).count() + 1;
                blocks.push(Block::Example { title: format!("Example {index}"), parts: vec![Part::Field { label: "Input".into(), value: String::new() }] }); return;
            }
            if heading.starts_with("sample output") && matches!(blocks.last(), Some(Block::Example { .. })) {
                push(blocks, Part::Field { label: "Output".into(), value: String::new() }); return;
            }
        }
        if tag == "pre" && !media && let Some(Block::Example { parts, .. }) = blocks.last_mut()
            && let Some(Part::Field { label, value }) = parts.last_mut() && matches!(label.as_str(), "Input" | "Output") && value.is_empty() {
            *value = text; return;
        }
        if !media && (tag == "p" || tag.starts_with('h')) && text.starts_with("Example") && text.len() < 40 {
            *constraints = false; blocks.push(Block::Example { title: text.trim_end_matches(':').into(), parts: vec![] }); return;
        }
        if (tag == "p" || tag.starts_with('h')) && text.trim_end_matches(':').eq_ignore_ascii_case("Constraints") {
            *constraints = true; blocks.push(Block::Constraints(String::new())); return;
        }
        let markdown = statement_markdown(&markup);
        if tag.starts_with('h') || tag == "details" {
            *constraints = false; blocks.push(Block::Markdown(markdown)); return;
        }
        if text.starts_with("Follow-up") { *constraints = false; blocks.push(Block::Markdown(markdown)); return; }
        if *constraints {
            if let Some(Block::Constraints(content)) = blocks.last_mut() { if !content.is_empty() { content.push_str("\n\n"); } content.push_str(&markdown); return; }
        }
        if matches!(blocks.last(), Some(Block::Example { .. })) {
            if !media && !markup.contains("href=") && !markup.contains('$') && !["<sup", "<sub", "<math", "tex-span"].iter().any(|tag| markup.contains(tag)) && let Some(fields) = fields(&text) {
                if tag == "pre" { for field in fields { push(blocks, field); } return; }
                if tag == "p" {
                    for field in fields {
                        if let Part::Field { label, value } = field { push(blocks, Part::Field { label, value }); }
                    }
                    return;
                }
            }
        }
        push(blocks, Part::Markdown(markdown));
    }
    let document = Html::parse_fragment(html); let mut blocks = Vec::new(); let mut constraints = false;
    for child in document.root_element().children() {
        if let Some(element) = ElementRef::wrap(child) { visit(element, &mut blocks, &mut constraints); }
        else if let Some(text) = child.value().as_text() { if !text.trim().is_empty() { push(&mut blocks, Part::Markdown(text.to_string())); } }
    }
    if blocks.is_empty() { blocks.push(Block::Markdown(statement_markdown(html))); }
    blocks
}
/// Render the complete provider payload, including structured content absent from legacy HTML.
pub fn for_question(question: &crate::leetcode::Question) -> anyhow::Result<Vec<Block>> {
    let base = if question.meta["statementProvider"] == "neetcode" {
        crate::roadmap::entry(&question.slug).map(|entry| entry.neetcode_url()).unwrap_or_else(|| "https://neetcode.io/".into())
    } else { media::base_url(&question.slug) };
    let mut blocks = if question.meta["statementMarkdown"].as_bool() == Some(true) {
        vec![Block::Markdown(question.content.clone())]
    } else { parse(&media::normalize(&question.content, &base)) };
    if let Some(extras) = question.meta.get("statementExtras") {
        for extra in serde_json::from_value::<Vec<Block>>(extras.clone())? {
            if let Block::Example { title, parts } = &extra
                && let Some(Block::Example { parts: existing, .. }) = blocks.iter_mut().find(|block| matches!(block, Block::Example { title: existing, .. } if existing == title)) {
                existing.extend(parts.clone());
            } else { blocks.push(extra); }
        }
    }
    for block in &mut blocks {
        match block {
            Block::Markdown(markdown) | Block::Constraints(markdown) => *markdown = media::normalize_markdown(&media::normalize(markdown, &base), &base),
            Block::Example { parts, .. } => for part in parts {
                if let Part::Markdown(markdown) = part { *markdown = media::normalize_markdown(&media::normalize(markdown, &base), &base); }
            },
        }
    }
    Ok(blocks)
}
#[cfg(test)] mod tests {
    use super::*;
    #[test] fn codeforces_complete_html_preserves_nested_sections_and_sample_notes() {
        let html = "<div class='problem-statement'><div class='header'><div class='title'>A. Full</div><div class='time-limit'>2 seconds</div><div class='memory-limit'>256 MB</div></div><section><p>Description $x_i$.</p><img src='/diagram.svg'><table><tr><th>Value</th><td>7</td></tr></table></section><div class='input-specification'><div class='title'>Input</div><p>Read n.</p></div><div class='output-specification'><div class='title'>Output</div><p>Print n.</p></div><div class='sample-test'><div class='input'><div class='title'>Input</div><pre>7</pre><p>Input note.</p></div><div class='output'><pre>7</pre></div><p>Sample note.</p></div><h3>Note</h3><details><summary>Extra case</summary><p>Consider zero.</p><a href='/contest/1'>Contest link</a></details></div>";
        let question = crate::codeforces::parse_statement("cf:1:A", html).unwrap();
        let rendered = format!("{:?}", for_question(&question).unwrap());
        for token in ["2 seconds", "256 MB", "Description", "x_i", "https://codeforces.com/diagram.svg", "Value", "Read n", "Print n", "Input note", "Sample note", "Extra case", "Consider zero", "https://codeforces.com/contest/1"] { assert!(rendered.contains(token), "Dropped {token}: {rendered}"); }
    }
    #[test] fn diagrams_and_formatted_explanations_survive_examples() {
        let html = "<p>Given bars.</p><p><strong>Example 1:</strong></p><p><img src='https://example.com/water.png' alt='Water diagram'></p><pre><b>Input:</b> height = [0,2,0]\n<img src='https://example.com/trace.svg'>\n<b>Output:</b> 0</pre><p><b>Explanation:</b> see <img src='https://example.com/result.png'> and <a href='https://example.com/details'>details</a>.</p><table><tr><th>Height</th><td>2</td></tr></table><p><b>Constraints:</b></p><ul><li>n &gt; 0</li></ul>";
        let blocks = parse(html);
        let Block::Example { parts, .. } = &blocks[1] else { panic!("Missing example") };
        let rendered: String = parts.iter().map(|part| match part { Part::Markdown(text) => text.clone(), Part::Field { value, .. } => value.clone() }).collect();
        for value in ["water.png", "trace.svg", "result.png", "details", "Height", "height = [0,2,0]", "Output:"] { assert!(rendered.contains(value), "Dropped {value}: {rendered}"); }
        assert!(matches!(blocks.last(), Some(Block::Constraints(text)) if text.contains("n > 0")));
    }
    #[test] fn structured_examples_preserve_lines_constraints_and_explanations() {
        let blocks = parse("<p>Given <code>nums</code>.</p><p><strong>Example 1:</strong></p><pre><b>Input:</b> nums = [1,2]\n<b>Output:</b> 3\n<b>Explanation:</b> Add both.\nKeep this line.</pre><p><b>Constraints:</b></p><ul><li>1 &lt;= n &lt;= 10<sup>4</sup></li></ul>");
        assert!(matches!(&blocks[0], Block::Markdown(text) if text.contains("`nums`")));
        assert!(matches!(&blocks[1], Block::Example { parts, .. } if parts.len() == 3 && matches!(&parts[2], Part::Field { value, .. } if value.contains("Keep this line."))));
        assert!(matches!(&blocks[2], Block::Constraints(text) if text.contains("10^4")));
    }
    #[test] fn modern_paragraph_examples_and_unrecognized_code_survive() {
        let blocks = parse("<div><p><b>Example 1:</b></p><p><b>Input:</b> s = &quot;abc&quot;</p><p><b>Output:</b> true</p><p>Reason <code>x</code>.</p><pre>unlabelled code\nnext line</pre><p><b>Constraints:</b></p><ul><li><code>n &gt; 0</code></li></ul></div>");
        assert!(matches!(&blocks[0], Block::Example { parts, .. } if parts.len() == 4));
        assert!(matches!(&blocks[1], Block::Constraints(text) if text.contains("`n > 0`")));
    }
}
