//! Preserve provider math while converting HTML, and normalize prose delimiters.
use std::sync::LazyLock;
use regex::Regex;

static HTML_CODE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?is)<pre\b[^>]*>.*?</pre>|<code\b[^>]*>.*?</code>").unwrap());

struct Formula<'a> { start: usize, end: usize, latex: &'a str, display: bool }

fn formulas(text: &str) -> Vec<Formula<'_>> {
    let mut result = Vec::new();
    let mut index = 0;
    while index < text.len() {
        let rest = &text[index..];
        if rest.starts_with('`') || rest.starts_with("~~~") {
            let character = rest.as_bytes()[0];
            let count = rest.bytes().take_while(|byte| *byte == character).count();
            let marker = &rest[..count];
            if let Some(end) = rest[count..].find(marker) { index += count + end + count; continue; }
            // An unfinished streamed code fence must not reinterpret its contents.
            if count >= 3 { break; }
            index += count; continue;
        }
        let delimiter = if rest.starts_with("$$$") { Some(("$$$", "$$$", false)) }
            else if rest.starts_with("$$") { Some(("$$", "$$", true)) }
            else if rest.starts_with(r"\(") { Some((r"\(", r"\)", false)) }
            else if rest.starts_with(r"\[") { Some((r"\[", r"\]", true)) }
            else if rest.starts_with('$') { Some(("$", "$", false)) }
            else { None };
        if let Some((open, close, display)) = delimiter {
            let start = index + open.len();
            let mut end = start;
            while end < text.len() {
                let tail = &text[end..];
                if tail.starts_with(close) {
                    if end > start {
                        result.push(Formula { start: index, end: end + close.len(), latex: &text[start..end], display });
                        index = end + close.len();
                    } else { index = end + close.len(); }
                    break;
                }
                if !display && tail.starts_with('\n') { break; }
                if tail.starts_with('\\') {
                    end += 1;
                    if end < text.len() { end += text[end..].chars().next().unwrap().len_utf8(); }
                } else { end += tail.chars().next().unwrap().len_utf8(); }
            }
            if index >= start { continue; }
        }
        let character = rest.chars().next().unwrap();
        index += character.len_utf8();
        if character == '\\' && index < text.len() { index += text[index..].chars().next().unwrap().len_utf8(); }
    }
    result
}

fn markdown_formula(latex: &str, display: bool) -> String {
    if display { format!("\n\n$$\n{}\n$$\n\n", latex.trim()) } else { format!("${}$", latex.trim()) }
}

pub fn normalize_math(markdown: &str) -> String {
    let mut output = String::with_capacity(markdown.len());
    let mut previous = 0;
    for formula in formulas(markdown) {
        output.push_str(&markdown[previous..formula.start]);
        if formula.display && markdown[formula.start..].starts_with("$$") {
            output.push_str(&markdown[formula.start..formula.end]);
        } else { output.push_str(&markdown_formula(formula.latex, formula.display)); }
        previous = formula.end;
    }
    output.push_str(&markdown[previous..]);
    output
}

/// Replace formulas before HTML-to-Markdown escaping, then restore their raw TeX.
pub(crate) fn protect_html_math(html: &str) -> (String, Vec<(String, String)>) {
    let mut prefix = "LEETMATHPLACEHOLDER".to_owned();
    while html.contains(&prefix) { prefix.push('Z'); }
    let mut protected = String::with_capacity(html.len());
    let mut replacements = Vec::new();
    let mut protect = |text: &str, output: &mut String| {
        let mut previous = 0;
        for formula in formulas(text) {
            output.push_str(&text[previous..formula.start]);
            let key = format!("{prefix}{}END", replacements.len());
            output.push_str(&key);
            let latex = html_escape::decode_html_entities(formula.latex);
            replacements.push((key, markdown_formula(&latex, formula.display)));
            previous = formula.end;
        }
        output.push_str(&text[previous..]);
    };
    let mut previous = 0;
    for code in HTML_CODE.find_iter(html) {
        protect(&html[previous..code.start()], &mut protected);
        protected.push_str(code.as_str());
        previous = code.end();
    }
    protect(&html[previous..], &mut protected);
    (protected, replacements)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn normalizes_provider_and_chat_math_without_changing_code() {
        let source = r"Reading $$$a_i$$$ and \(\frac{1}{2}\), then \[\sum_{i=1}^n i\]. `$$$code$$$` and \$literal\$.";
        let normalized = normalize_math(source);
        assert!(normalized.contains(r"$a_i$"));
        assert!(normalized.contains(r"$\frac{1}{2}$"));
        assert!(normalized.contains("$$\n\\sum_{i=1}^n i\n$$"));
        assert!(normalized.contains(r"`$$$code$$$` and \$literal\$"));
        assert_eq!(normalize_math(&normalized), normalized);
    }
    #[test]
    fn streamed_fences_unclosed_math_and_unicode_remain_readable() {
        assert_eq!(normalize_math("```python\nx = '$$$a$$$'"), "```python\nx = '$$$a$$$'");
        assert_eq!(normalize_math("α and \\(\\sqrt{x}"), "α and \\(\\sqrt{x}");
        assert_eq!(normalize_math("α $$$β_i$$$"), "α $β_i$");
        for source in ["$x", "$$x", "$$$x", "~~~python\nx = '$$$a$$$'"] {
            assert_eq!(normalize_math(source), source);
        }
    }
    #[test]
    fn html_conversion_keeps_real_codeforces_latex_and_other_content() {
        let html = r"<p>In the $$$i$$$-th laboratory, $$$c_i \mathrel{+}= \operatorname{sgn}(a_i - b_i)$$$ when $$$x &gt; 0$$$.</p><pre><code>$$$literal_code$$$</code></pre><table><tr><th>Name</th><th>Value</th></tr><tr><td>A</td><td>$$$a_i$$$</td></tr></table><p><img src='https://example.com/figure.png' alt='Figure'></p>";
        let markdown = crate::prompts::statement_markdown(html);
        assert!(markdown.contains(r"$c_i \mathrel{+}= \operatorname{sgn}(a_i - b_i)$"), "{markdown}");
        assert!(markdown.contains("$x > 0$"));
        assert!(markdown.contains("$$$literal_code$$$"));
        assert!(markdown.contains("Name") && markdown.contains("Value") && markdown.contains('|'), "{markdown}");
        assert!(markdown.contains("![Figure](https://example.com/figure.png)"));
        let (protected, replacements) = protect_html_math("<pre><code>sample</code>$$$literal$$$</pre> $$$x$$$");
        assert!(protected.contains("$$$literal$$$"));
        assert_eq!(replacements.len(), 1);
    }
}
