//! Bounded conversation context shared by local-agent and web prompts.

use crate::assist::{Action, Answer};

pub mod history;
pub mod artifacts;

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct Turn {
    pub action: Action,
    pub instructions: String,
    pub answer: Answer,
}

const HISTORY_LIMIT: usize = 32_000;
const TURN_LIMIT: usize = 8_000;
pub const INSTRUCTIONS_LIMIT: usize = 8_000;

fn bounded(text: &str, limit: usize) -> String { text.chars().take(limit).collect() }

/// Newest turns are supplied first; the prompt presents them chronologically.
/// Every turn includes its request, even if its answer is too long to include fully.
pub fn context(turns: impl IntoIterator<Item = Turn>) -> String {
    let mut retained = Vec::new();
    let mut remaining = HISTORY_LIMIT;
    for turn in turns.into_iter().take(12) {
        let request = bounded(&turn.instructions, 2_000);
        let answer = match &turn.answer {
            Answer::Chat(text) => text.clone(),
            answer => serde_json::to_string(answer).unwrap_or_default(),
        };
        let text = format!("User: {}\n{}\nAssistant: {}\n", turn.action.label(), request, bounded(&answer, TURN_LIMIT));
        let length = text.chars().count();
        if length > remaining { break; }
        remaining -= length;
        retained.push(text);
    }
    retained.reverse();
    retained.concat()
}

pub fn prompt(base: String, history: &str, instructions: &str) -> String {
    let mut out = base;
    if !history.is_empty() {
        out.push_str("\n## Prior conversation (earlier code may be outdated)\n");
        out.push_str(&bounded(history, HISTORY_LIMIT));
    }
    if !instructions.trim().is_empty() {
        out.push_str("\n## My request for this turn\n");
        out.push_str(&bounded(instructions.trim(), INSTRUCTIONS_LIMIT));
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_is_chronological_and_keeps_requests_with_answers() {
        let turns = [Turn { action: Action::Ask, instructions: "Why the lookup?".into(), answer: Answer::Chat("Check the complement".into()) },
            Turn { action: Action::Explain, instructions: "No solution code".into(), answer: Answer::Chat("Start with a map".into()) }];
        let context = context(turns);
        assert!(context.find("No solution code").unwrap() < context.find("Why the lookup?").unwrap());
        assert!(context.contains("Assistant: Check the complement"));
        let prompt = prompt("Current code and statement".into(), &context, "Explain duplicates instead");
        assert!(prompt.ends_with("Explain duplicates instead\n"));
        assert!(prompt.contains("earlier code may be outdated"));
    }

    #[test]
    fn large_unicode_history_and_requests_are_bounded_without_panicking() {
        let turns = (0..100).map(|_| Turn { action: Action::Ask, instructions: "é".repeat(10_000), answer: Answer::Chat("λ".repeat(20_000)) });
        let history = context(turns);
        assert!(history.chars().count() <= HISTORY_LIMIT);
        let prompt = prompt(String::new(), &history, &"λ".repeat(20_000));
        assert!(prompt.chars().count() < HISTORY_LIMIT + INSTRUCTIONS_LIMIT + 200);
    }
}
