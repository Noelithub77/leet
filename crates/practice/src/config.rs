use std::path::{Path, PathBuf};
use std::collections::BTreeMap;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::prompts::{Provider, Style};
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
    pub companion_enabled: bool,
    pub companion_port: u16,
    pub external_editor: String,
    pub roadmap_list: List,
    pub prompt_provider: Provider,
    pub prompt_style: Style,
    pub test_timeout_secs: u64,
    /// UI scale; `ctrl+=` / `ctrl+-` change it.
    pub zoom: f32,
    pub keybindings: BTreeMap<String, String>,
    pub prompt_instructions: BTreeMap<String, String>,
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
            companion_enabled: true,
            companion_port: 13337,
            external_editor: "zed".into(),
            roadmap_list: List::NeetCode150,
            prompt_provider: Provider::ChatGpt,
            prompt_style: Style::Guided,
            test_timeout_secs: 10,
            zoom: 1.0,
            keybindings: BTreeMap::new(),
            prompt_instructions: BTreeMap::new(),
        }
    }
}

impl Config {
    pub fn path() -> PathBuf {
        config_dir().join("config.toml")
    }

    pub fn load() -> Result<Self> {
        Self::load_from(&Self::path())
    }

    pub fn load_from(path: &Path) -> Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(text) => toml::from_str(&text).with_context(|| format!("parse {}", path.display())),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(err) => Err(err).with_context(|| format!("read {}", path.display())),
        }
    }

    pub fn save(&self) -> Result<()> {
        self.save_to(&Self::path())
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
    fn missing_file_gives_defaults_and_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let mut config = Config::load_from(&path).unwrap();
        assert_eq!(config.theme, "Vesper");
        assert_eq!(config.font_family, "Liberation Sans");
        assert!(!config.show_tags);
        config.show_tags = true;
        config.theme = "Ayu Dark".into();
        config.preferred_language = Language::Go;
        config.source = Source::Codeforces;
        config.codeforces_handle = "tourist".into();
        config.onboarding_completed = true;
        config.font_family = "DejaVu Sans".into();
        config.keybindings.insert("Search".into(), "ctrl-alt-k".into());
        config.prompt_instructions.insert("guided".into(), "Ask one question.\nWait for my answer.".into());
        config.save_to(&path).unwrap();
        let saved = Config::load_from(&path).unwrap();
        assert_eq!(saved.theme, "Ayu Dark");
        assert!(saved.show_tags);
        assert_eq!(saved.preferred_language, Language::Go);
        assert_eq!(saved.source, Source::Codeforces);
        assert_eq!(saved.codeforces_handle, "tourist");
        assert!(saved.onboarding_completed);
        assert_eq!(saved.font_family, "DejaVu Sans");
        assert_eq!(saved.keybindings["Search"], "ctrl-alt-k");
        assert_eq!(saved.prompt_instructions["guided"], "Ask one question.\nWait for my answer.");
    }

    #[test]
    fn partial_file_keeps_other_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "python = \"pypy3\"\nchatgpt_model = \"legacy-model\"\n").unwrap();
        let config = Config::load_from(&path).unwrap();
        assert_eq!(config.python, "pypy3");
        assert_eq!(config.preferred_language, Language::Python);
        assert!(!config.onboarding_completed);
        assert_eq!(config.font_family, "Liberation Sans");
        assert_eq!(config.roadmap_list, List::NeetCode150);
    }
}
