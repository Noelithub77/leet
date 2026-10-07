use gpui_kit::component::Icon;
use practice::prompts::Provider;

pub fn icon(provider: Provider) -> Icon {
    Icon::default().path(match provider {
        Provider::ChatGpt => "providers/chatgpt.svg",
        Provider::Claude => "providers/claude.svg",
        Provider::Gemini => "providers/gemini.svg",
    })
}

/// Provider silhouettes share the app's pastel accents and remain legible on dark panels.
pub fn source_icon(source: practice::language::Source) -> Icon {
    use gpui_kit::{Styled as _, rgb};
    let (path, color) = match source {
        practice::language::Source::NeetCode => ("providers/neetcode.svg", 0x99ffe4),
        practice::language::Source::LeetCode => ("providers/leetcode.svg", 0xffc799),
        practice::language::Source::Codeforces => ("providers/codeforces.svg", 0xa0c4ff),
    };
    Icon::default().path(path).text_color(rgb(color))
}
