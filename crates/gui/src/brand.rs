use gpui_kit::component::Icon;
use gpui_kit::{Styled as _, rgb};
use practice::agents::AgentKind;
use practice::prompts::Provider;

pub fn icon(provider: Provider) -> Icon {
    Icon::default().path(match provider {
        Provider::ChatGpt => "providers/chatgpt.svg",
        Provider::Claude => "providers/claude.svg",
        Provider::Gemini => "providers/gemini.svg",
    })
}

/// Agent marks in the app's pastel accents.
pub fn agent_icon(agent: AgentKind) -> Icon {
    let (path, color) = match agent {
        AgentKind::Codex => ("providers/codex.svg", 0xa0c4ff),
        AgentKind::Claude => ("providers/claude-code.svg", 0xffc799),
        AgentKind::OpenCode => ("providers/opencode.svg", 0xe6e6e6),
        AgentKind::Antigravity => ("providers/antigravity.svg", 0x99ffe4),
        AgentKind::Gemini => ("providers/gemini-cli.svg", 0xc3b1ff),
        AgentKind::Cursor => ("providers/cursor.svg", 0xe6e6e6),
    };
    Icon::default().path(path).text_color(rgb(color))
}

/// Provider silhouettes share the app's pastel accents and remain legible on dark panels.
pub fn source_icon(source: practice::language::Source) -> Icon {
    let (path, color) = match source {
        practice::language::Source::NeetCode => ("providers/neetcode.svg", 0x99ffe4),
        practice::language::Source::LeetCode => ("providers/leetcode.svg", 0xffc799),
        practice::language::Source::Codeforces => ("providers/codeforces.svg", 0xa0c4ff),
        practice::language::Source::CodeChef => ("providers/codechef.svg", 0xffc799),
    };
    Icon::default().path(path).text_color(rgb(color))
}
