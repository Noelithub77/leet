use std::path::{Path, PathBuf};
use std::collections::BTreeMap;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::agents::{AgentKind, Selection};
use crate::prompts::Provider;
use crate::roadmap::List;
use crate::language::{Language, Source};

/// User settings persisted to `~/.config/leet/config.toml`, with legacy vg paths supported.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub workspace: PathBuf,
    pub theme: String,
    pub font_family: String,
    pub python: String,
    pub preferred_language: Language,
    pub source: Source,
    pub codeforces_handle: String,
    pub onboarding_completed: bool,
    pub show_tags: bool,
    pub external_editor: String,
    pub roadmap_list: List,
    /// Web chat for Assist when no local agent is chosen.
    #[serde(alias = "prompt_provider")]
    pub web_chat: Provider,
    /// The preferred local agent; `None` detects the first installed agent.
    pub agent: Option<AgentKind>,
    /// Explicit web choice survives detection and restarts.
    pub ai_web: bool,
    /// The last model choice per agent.
    pub agent_models: Vec<Selection>,
    pub test_timeout_secs: u64,
    /// UI scale; `ctrl+=` / `ctrl+-` change it.
    pub zoom: f32,
    pub keybindings: BTreeMap<String, String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            workspace: home().join("leet"),
            theme: "Vesper".into(),
            font_family: "Liberation Sans".into(),
            python: if cfg!(windows) { "python" } else { "python3" }.into(),
            preferred_language: Language::Python,
            source: Source::NeetCode,
            codeforces_handle: String::new(),
            onboarding_completed: false,
            show_tags: false,
            external_editor: "zed".into(),
            roadmap_list: List::NeetCode150,
            web_chat: Provider::ChatGpt,
            agent: None,
            ai_web: false,
            agent_models: Vec::new(),
            test_timeout_secs: 10,
            zoom: 1.0,
            keybindings: BTreeMap::new(),
        }
    }
}

fn migrated_workspace(path: &Path, home: &Path) -> PathBuf {
    let current = home.join("leet");
    if path == home.join("vg") && std::fs::read_link(path).is_ok_and(|target| target == current) { current }
    else { path.to_owned() }
}

impl Config {
    /// The remembered choice for `agent`, if any.
    pub fn selection(&self, agent: AgentKind) -> Option<&Selection> {
        self.agent_models.iter().find(|selection| selection.agent == agent)
    }

    pub fn remember(&mut self, selection: Selection) {
        self.agent_models.retain(|s| s.agent != selection.agent);
        self.agent_models.push(selection);
    }

    pub fn path() -> PathBuf {
        config_dir().join("config.toml")
    }

    pub fn load() -> Result<Self> {
        Self::load_from(&Self::path())
    }

    pub fn load_from(path: &Path) -> Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(text) => {
                let mut config: Self = toml::from_str(&text).with_context(|| format!("parse {}", path.display()))?;
                config.workspace = migrated_workspace(&config.workspace, &home());
                Ok(config)
            },
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(err) => Err(err).with_context(|| format!("read {}", path.display())),
        }
    }

    pub fn save(&self) -> Result<()> {
        self.save_to(&Self::path())
    }

    /// Restores app preferences without touching solutions, credentials, or the cache.
    pub fn reset_settings(&self) -> Result<Self> {
        self.reset_settings_to(&Self::path())
    }

    fn reset_settings_to(&self, path: &Path) -> Result<Self> {
        let defaults = Self { workspace: self.workspace.clone(), codeforces_handle: self.codeforces_handle.clone(), ..Self::default() };
        defaults.save_to(path)?;
        Ok(defaults)
    }

    pub fn save_to(&self, path: &Path) -> Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("toml.tmp");
        std::fs::write(&tmp, toml::to_string_pretty(self)?)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }
}

fn home() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
}

pub fn config_dir() -> PathBuf {
    let root = dirs::config_dir().unwrap_or_else(|| home().join(".config"));
    let current = root.join("leet"); let legacy = root.join("vg");
    if !current.join("config.toml").exists() && legacy.join("config.toml").exists() { legacy } else { current }
}

pub fn data_dir() -> PathBuf {
    let root = dirs::data_dir().unwrap_or_else(|| home().join(".local/share"));
    let current = root.join("leet"); let legacy = root.join("vg");
    if !current.exists() && legacy.join("vg.db").exists() { legacy } else { current }
}

pub fn database_path() -> PathBuf {
    let dir = data_dir();
    if dir.join("vg.db").exists() { dir.join("vg.db") } else { dir.join("leet.db") }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reset_only_replaces_app_preferences_and_keeps_personal_data() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let workspace = dir.path().join("custom-solutions");
        std::fs::create_dir_all(workspace.join(".git")).unwrap();
        let protected = [workspace.join("solution.py"), workspace.join(".git/HEAD"), dir.path().join("leet.db"), dir.path().join("credentials.json")];
        for file in &protected { std::fs::write(file, "keep me").unwrap(); }
        let mut config = Config { workspace: workspace.clone(), codeforces_handle: "tourist".into(), onboarding_completed: true, theme: "Sarah Pink".into(), ..Config::default() };
        config.keybindings.insert("Search".into(), "ctrl-alt-k".into());
        config.save_to(&path).unwrap();
        let reset = config.reset_settings_to(&path).unwrap();
        assert_eq!(reset.workspace, workspace);
        assert_eq!(reset.codeforces_handle, "tourist");
        assert!(!reset.onboarding_completed);
        assert_eq!(reset.theme, "Vesper");
        assert!(reset.keybindings.is_empty());
        let saved = Config::load_from(&path).unwrap();
        assert!(!saved.onboarding_completed);
        assert_eq!(saved.workspace, workspace);
        for file in protected { assert_eq!(std::fs::read_to_string(file).unwrap(), "keep me"); }
        // A failed write must leave the existing in-memory preferences intact.
        assert!(config.reset_settings_to(&workspace).is_err());
        assert!(config.onboarding_completed);
    }

    #[cfg(unix)]
    #[test]
    fn migrated_workspace_follows_only_the_known_compatibility_link() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path(); let legacy = home.join("vg"); let current = home.join("leet");
        std::fs::create_dir(&current).unwrap();
        assert_eq!(migrated_workspace(&legacy, home), legacy);
        std::os::unix::fs::symlink(&current, &legacy).unwrap();
        assert_eq!(migrated_workspace(&legacy, home), current);
        let custom = home.join("custom"); std::os::unix::fs::symlink(&current, &custom).unwrap();
        assert_eq!(migrated_workspace(&custom, home), custom);
        std::fs::remove_file(&legacy).unwrap(); std::os::unix::fs::symlink(&custom, &legacy).unwrap();
        assert_eq!(migrated_workspace(&legacy, home), legacy);
    }

    #[test]
    fn missing_file_gives_defaults_and_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let mut config = Config::load_from(&path).unwrap();
        assert_eq!(config.theme, "Vesper");
        assert_eq!(config.workspace, home().join("leet"));
        assert_eq!(config.font_family, "Liberation Sans");
        assert!(!config.show_tags);
        config.show_tags = true;
        config.ai_web = true;
        config.theme = "Ayu Dark".into();
        config.preferred_language = Language::Go;
        config.source = Source::Codeforces;
        config.codeforces_handle = "tourist".into();
        config.onboarding_completed = true;
        config.font_family = "DejaVu Sans".into();
        config.keybindings.insert("Search".into(), "ctrl-alt-k".into());
        config.agent = Some(AgentKind::Codex);
        config.remember(Selection { agent: AgentKind::Codex, model: "gpt-6-luna".into(), effort: Some("low".into()), fast: true });
        config.save_to(&path).unwrap();
        let saved = Config::load_from(&path).unwrap();
        assert_eq!(saved.theme, "Ayu Dark");
        assert!(saved.show_tags);
        assert!(saved.ai_web);
        assert_eq!(saved.preferred_language, Language::Go);
        assert_eq!(saved.source, Source::Codeforces);
        assert_eq!(saved.codeforces_handle, "tourist");
        assert!(saved.onboarding_completed);
        assert_eq!(saved.font_family, "DejaVu Sans");
        assert_eq!(saved.keybindings["Search"], "ctrl-alt-k");
        assert_eq!(saved.agent, Some(AgentKind::Codex));
        assert_eq!(saved.selection(AgentKind::Codex).map(|s| (s.model.as_str(), s.fast)), Some(("gpt-6-luna", true)));
    }

    #[test]
    fn partial_file_keeps_other_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "python = \"pypy3\"\nchatgpt_model = \"legacy-model\"\nprompt_provider = \"claude\"\nprompt_style = \"hints\"\n").unwrap();
        let config = Config::load_from(&path).unwrap();
        assert_eq!(config.python, "pypy3");
        assert_eq!(config.workspace, home().join("leet"));
        assert_eq!(config.preferred_language, Language::Python);
        assert!(!config.onboarding_completed);
        assert_eq!(config.font_family, "Liberation Sans");
        assert_eq!(config.roadmap_list, List::NeetCode150);
        assert_eq!(config.web_chat, Provider::Claude);
        assert_eq!(config.agent, None);
    }
}
