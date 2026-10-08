//! Private conversation records, shared by shortcuts and free-form chat.

use crate::agents::AgentKind;
use crate::assist::{Action, Answer};

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Thread {
    pub id: i64,
    pub problem: Option<String>,
    pub title: String,
    pub updated_at: i64,
    pub draft: String,
    pub fork_of: Option<i64>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Message {
    pub id: i64,
    pub thread_id: i64,
    pub turn: SavedTurn,
}

/// Legacy fields stay compatible with the previous newest-first JSON history.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct SavedTurn {
    pub model: String,
    pub agent: Option<AgentKind>,
    pub answer: Option<Answer>,
    #[serde(default)] pub request_prompt: String,
    #[serde(default)] pub instructions: String,
    #[serde(default)] pub action: Option<Action>,
    #[serde(default)] pub phase: SavedPhase,
    #[serde(default)] pub problem: String,
    #[serde(default)] pub problem_title: String,
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub enum SavedPhase { #[default] Done, Stopped, Failed(String) }

impl SavedTurn {
    pub fn action(&self) -> Action {
        self.action.unwrap_or_else(|| match &self.answer {
            Some(Answer::Hints(_)) => Action::Hints, Some(Answer::Tests(_)) => Action::Tests,
            Some(Answer::Bugs(_)) => Action::Bugs, Some(Answer::Analyze(_)) => Action::Analyze,
            Some(Answer::Stuck(_)) => Action::Stuck, Some(Answer::Explain(_)) => Action::Explain,
            Some(Answer::Visualize(_)) => Action::Visualize, Some(Answer::Optimize(_)) => Action::Optimize,
            Some(Answer::Pattern(_)) => Action::Pattern, Some(Answer::DryRun(_)) => Action::DryRun,
            Some(Answer::Solve(_)) => Action::Solve, Some(Answer::Review(_)) => Action::Review,
            _ => Action::Ask,
        })
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
pub(crate) enum Undo {
    RemoveThread { id: i64, previous: Option<i64> },
    RestoreThread { thread: Thread, messages: Vec<Message> },
}
