//! Local coding-agent CLIs: detection, live model catalogs, and one-shot or resumable runs.
//! Each CLI uses its best native protocol; see `docs/development.md` for the matrix.
//! Every call here blocks and belongs on a background thread.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};

mod acp;
mod antigravity;
mod claude;
mod codex;
mod detect;
mod install;
mod schema;
mod transport;
mod cursor;

pub use detect::detect;
pub use install::install;
pub use schema::strict_schema;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentKind {
    Codex,
    Claude,
    OpenCode,
    Antigravity,
    Gemini,
    Cursor,
}

impl AgentKind {
    pub const ALL: [Self; 6] = [Self::Codex, Self::Claude, Self::OpenCode, Self::Antigravity, Self::Gemini, Self::Cursor];

    pub fn label(self) -> &'static str {
        match self {
            Self::Codex => "Codex",
            Self::Claude => "Claude Code",
            Self::OpenCode => "OpenCode",
            Self::Antigravity => "Antigravity",
            Self::Gemini => "Gemini CLI",
            Self::Cursor => "Cursor Agent",
        }
    }

    pub fn binary(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::Claude => "claude",
            Self::OpenCode => "opencode",
            Self::Antigravity => "agy",
            Self::Gemini => "gemini",
            Self::Cursor => "cursor-agent",
        }
    }

    /// Agents leet can install with their official installer when nothing is detected.
    pub fn installable(self) -> bool {
        matches!(self, Self::OpenCode | Self::Antigravity)
    }
}

/// An installed agent CLI.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Detected {
    pub kind: AgentKind,
    pub path: PathBuf,
    pub version: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Effort {
    pub id: String,
    pub label: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Model {
    /// The exact id the CLI accepts.
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub description: String,
    /// Reasoning levels the model accepts, lowest first. Empty when the CLI exposes none.
    #[serde(default)]
    pub efforts: Vec<Effort>,
    #[serde(default)]
    pub default_effort: Option<String>,
    /// Supports a faster service tier (Codex `fast`, Claude fast mode).
    #[serde(default)]
    pub fast: bool,
    /// Usable without a paid plan, such as OpenCode Zen free models.
    #[serde(default)]
    pub free: bool,
}

/// What an installed agent reports right now.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Catalog {
    pub agent: AgentKind,
    pub models: Vec<Model>,
    /// leet's preferred default: newest Luna for Codex, Haiku for Claude,
    /// a free model for OpenCode, the first Flash for Antigravity and Gemini.
    pub default_model: Option<String>,
    /// Shown when the agent is installed but not signed in.
    #[serde(default)]
    pub sign_in_hint: Option<String>,
}

/// The user's choice, persisted in `Config`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Selection {
    pub agent: AgentKind,
    pub model: String,
    #[serde(default)]
    pub effort: Option<String>,
    #[serde(default)]
    pub fast: bool,
}

/// What the agent may touch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Access {
    /// No file edits or commands: answer from the prompt alone.
    ReadOnly,
    /// May edit files and run commands inside `cwd` under the agent's own sandbox.
    Edit,
}

#[derive(Clone, Debug)]
pub struct Request {
    pub agent: Detected,
    pub selection: Selection,
    pub prompt: String,
    /// JSON Schema the final answer must match. Use [`strict_schema`] output.
    pub schema: Option<serde_json::Value>,
    pub cwd: PathBuf,
    pub access: Access,
    /// Continue an earlier conversation, from [`Outcome::session`].
    pub resume: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolKind {
    Read,
    Edit,
    Command,
    Search,
    Other,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// The agent accepted the request; carries its session or thread id.
    Started { session: Option<String> },
    /// Streamed answer text.
    Text(String),
    /// Streamed reasoning summary, when the agent shares one.
    Thinking(String),
    Tool { title: String, kind: ToolKind, done: bool },
    Usage { input_tokens: u64, output_tokens: u64 },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Outcome {
    /// The full final answer text.
    pub text: String,
    /// Parsed JSON when a schema was requested and the answer matched it.
    pub structured: Option<serde_json::Value>,
    pub session: Option<String>,
}

/// Stops a run; the driver kills the child process promptly.
#[derive(Clone, Debug, Default)]
pub struct Cancel(Arc<AtomicBool>);

impl Cancel {
    pub fn cancel(&self) { self.0.store(true, Ordering::SeqCst); }
    pub fn is_cancelled(&self) -> bool { self.0.load(Ordering::SeqCst) }
}

/// Lists models and options live from the installed CLI. Never hardcoded.
pub fn catalog(agent: &Detected) -> anyhow::Result<Catalog> {
    match agent.kind {
        AgentKind::Codex => codex::catalog(agent),
        AgentKind::Claude => claude::catalog(agent),
        AgentKind::Antigravity => antigravity::catalog(agent),
        AgentKind::OpenCode | AgentKind::Gemini | AgentKind::Cursor => acp::catalog(agent),
    }
}

/// Runs one turn, streaming events. Returns when the turn ends, fails, or is cancelled.
pub fn run(request: &Request, events: &mut dyn FnMut(Event), cancel: &Cancel) -> anyhow::Result<Outcome> {
    match request.agent.kind {
        AgentKind::Codex => codex::run(request, events, cancel),
        AgentKind::Claude => claude::run(request, events, cancel),
        AgentKind::Antigravity => antigravity::run(request, events, cancel),
        AgentKind::OpenCode | AgentKind::Gemini | AgentKind::Cursor => acp::run(request, events, cancel),
    }
}
