//! Code snippets: VS Code–compatible bodies, a bundled competitive-programming library,
//! user files under the config directory, and importers for other editors.
use serde::{Deserialize, Serialize};

use crate::language::Language;

pub mod body;
mod builtin;
pub mod import;
pub mod store;
pub mod session;
pub mod cli;

pub use builtin::builtin;
pub mod guide;

/// Where a snippet came from. Only builtins are read-only; editing one saves a user copy.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Origin {
    Builtin,
    #[default]
    User,
    VsCode,
    Neovim,
    Sublime,
    Ai,
}

impl Origin {
    pub fn label(self) -> &'static str {
        match self {
            Self::Builtin => "Built-in",
            Self::User => "Yours",
            Self::VsCode => "VS Code",
            Self::Neovim => "Neovim",
            Self::Sublime => "Sublime Text",
            Self::Ai => "AI",
        }
    }
}

/// One snippet. `scope: None` applies to every language.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snippet {
    /// Unique within its scope; the VS Code JSON key.
    pub name: String,
    /// Non-empty trigger words without whitespace. The first is shown.
    pub prefixes: Vec<String>,
    /// VS Code / TextMate syntax with `\n` line breaks; see [`body`].
    pub body: String,
    pub description: String,
    pub scope: Option<Language>,
    /// VS Code's `isFileTemplate`: offered as a starting file.
    pub template: bool,
    pub origin: Origin,
}

impl Snippet {
    pub fn prefix(&self) -> &str { self.prefixes.first().map_or("", String::as_str) }
    pub fn applies_to(&self, language: Language) -> bool { self.scope.is_none_or(|scope| scope == language) }
    /// Stable identity used for overrides and hidden builtins: `<scope>/<name>`.
    pub fn key(&self) -> String { format!("{}/{}", self.scope.map_or("all", Language::id), self.name) }
}

/// Snippet preferences stored in `config.toml` under `[snippets]`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Include the bundled library.
    pub builtin: bool,
    /// Builtin [`Snippet::key`]s the user removed.
    pub hidden: Vec<String>,
    /// Starting file per language id (`cpp`), by snippet name; empty uses leet's default.
    pub templates: std::collections::BTreeMap<String, String>,
    /// Expand a prefix with Tab.
    pub tab_expand: bool,
}

impl Default for Settings {
    fn default() -> Self { Self { builtin: true, hidden: Vec::new(), templates: Default::default(), tab_expand: true } }
}
