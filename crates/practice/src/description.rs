//! Ordered semantic statement blocks; unknown content remains rendered Markdown.
use scraper::{ElementRef, Html, Selector};
use crate::prompts::statement_markdown;
#[derive(Clone, Debug, PartialEq)] pub enum Part { Markdown(String), Field { label: String, value: String } }
#[derive(Clone, Debug, PartialEq)] pub enum Block { Markdown(String), Example { title: String, parts: Vec<Part> }, Constraints(String) }
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
        if element.value().classes().any(|class| class == "sample-test") {
            let selector = |value| Selector::parse(value).expect("static sample selector");
            let inputs: Vec<_> = element.select(&selector(".input pre")).map(plain).collect();
            let outputs: Vec<_> = element.select(&selector(".output pre")).map(plain).collect();
            for (index, input) in inputs.into_iter().enumerate() {
                let mut parts = vec![Part::Field { label: "Input".into(), value: input }];
                if let Some(output) = outputs.get(index) { parts.push(Part::Field { label: "Output".into(), value: output.clone() }); }
                blocks.push(Block::Example { title: format!("Example {}", index + 1), parts });
            }
            return;
        }
        if tag == "div" {
            if element.value().classes().any(|class| class == "title") { return; }
            for child in element.children() { if let Some(child) = ElementRef::wrap(child) { visit(child, blocks, constraints); } else if let Some(text) = child.value().as_text() { if !text.trim().is_empty() { push(blocks, Part::Markdown(text.to_string())); } } }
            return;
        }
        let text = plain(element);
        if text.is_empty() && !element.html().contains("<img") { return; }
        if (tag == "p" || tag.starts_with('h')) && text.starts_with("Example") && text.len() < 40 {
            *constraints = false; blocks.push(Block::Example { title: text.trim_end_matches(':').into(), parts: vec![] }); return;
        }
        if (tag == "p" || tag.starts_with('h')) && text.trim_end_matches(':').eq_ignore_ascii_case("Constraints") {
            *constraints = true; blocks.push(Block::Constraints(String::new())); return;
        }
        let markdown = statement_markdown(&element.html());
        if text.starts_with("Follow-up") { *constraints = false; blocks.push(Block::Markdown(markdown)); return; }
        if *constraints {
            if let Some(Block::Constraints(content)) = blocks.last_mut() { if !content.is_empty() { content.push_str("\n\n"); } content.push_str(&markdown); return; }
        }
        if matches!(blocks.last(), Some(Block::Example { .. })) {
            if let Some(fields) = fields(&text) {
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
#[cfg(test)] mod tests {
    use super::*;
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
