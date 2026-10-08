//! Every command, its default key, and its palette label. One table drives both.

use gpui_kit::*;
use gpui_kit::component::input::{GoToDefinition, ToggleCodeActions};
use crate::language_server::Complete;

gpui_kit::actions!(
    workspace_commands,
    [
        ShowHome,
        CloseProblem,
        CloseAllProblems,
        ReopenProblem,
        CycleTabs,
        PreviousTab,
        ToggleLeft,
        ToggleRight,
        ToggleDescription,
        ToggleReference,
        ToggleTags,
        ToggleBottom,
        ToggleHistory,
        ToggleZen,
        ToggleRoadmap,
        FocusSidebar,
        FocusEditor,
        FocusStatement,
        FindProblem,
        Search,
        ShortcutHelp,
        GuidedTour,
        ShowContests,
        TogglePastContests,
        OpenSettings,
        PickTheme,
        CycleList,
        CycleSource,
        RunTests,
        JudgeRun,
        Submit,
        NextProblem,
        PrevProblem,
        NextCase,
        PrevCase,
        AddCustomTest,
        EditTestCase,
        ResetTestCases,
        AssistHints,
        AssistStuck,
        AssistBugs,
        AssistTests,
        AssistAnalyze,
        AssistOptimize,
        AssistVisualize,
        AssistDryRun,
        AssistPattern,
        AssistExplain,
        AssistSolve,
        ToggleDebug,
        StopAssist,
        ConfigureAi,
        CycleAgent,
        OpenExternal,
        OpenInBrowser,
        RefreshCatalog,
        MarkNeetCode,
        RevealHint,
        HideHints,
        ResetSolution,
        Restart,
        ZoomIn,
        ZoomOut,
        ZoomReset,
        Quit,
        // Sidebar and roadmap navigation, active only while they have focus.
        Up,
        Down,
        Left,
        Right,
        Confirm,
        Back,
    ]
);

pub const WORKSPACE: &str = "Workspace";
pub const NAV: &str = "VgNav";

pub struct Command {
    pub id: &'static str,
    pub label: &'static str,
    /// Default shortcuts; the first is primary and `|` separates alternatives.
    pub key: &'static str,
    pub action: fn() -> Box<dyn Action>,
}

macro_rules! cmd {
    ($label:literal, $key:literal, $action:ident) => {
        Command { id: stringify!($action), label: $label, key: $key, action: || Box::new($action) }
    };
}

/// Global commands. Keys follow the user's VS Code bindings (modifiers only, no bare letters).
pub const COMMANDS: &[Command] = &[
    cmd!("Show Home", "ctrl-h|ctrl-.", ShowHome),
    cmd!("Close problem tab", "ctrl-w", CloseProblem),
    cmd!("Close all problem tabs", "ctrl-shift-w", CloseAllProblems),
    cmd!("Reopen closed problem tab", "ctrl-shift-t", ReopenProblem),
    cmd!("Next tab", "ctrl-tab", CycleTabs),
    cmd!("Previous tab", "ctrl-shift-tab", PreviousTab),
    cmd!("Search everything", "ctrl-k|ctrl-shift-p|ctrl-t", Search),
    cmd!("Go to problem", "ctrl-p", FindProblem),
    cmd!("Keyboard shortcut help", "ctrl-/", ShortcutHelp),
    cmd!("Guided tour", "", GuidedTour),
    cmd!("Codeforces contests", "ctrl-alt-c", ShowContests),
    cmd!("Show/hide past Codeforces contests", "", TogglePastContests),
    cmd!("Settings", "ctrl-,", OpenSettings),
    cmd!("Run tests", "ctrl-enter", RunTests),
    cmd!("Run on LeetCode", "ctrl-shift-enter", JudgeRun),
    cmd!("Submit to LeetCode", "ctrl-alt-enter", Submit),
    cmd!("Toggle explorer", "alt-s", ToggleLeft),
    cmd!("Toggle AI", "alt-d", ToggleRight),
    cmd!("Toggle description", "alt-a", ToggleDescription),
    cmd!("Toggle reference solution", "ctrl-alt-v", ToggleReference),
    cmd!("Show/hide problem tags", "alt-t", ToggleTags),
    cmd!("Toggle results", "alt-x", ToggleBottom),
    cmd!("Solution history", "ctrl-g", ToggleHistory),
    cmd!("Roadmap", "alt-r", ToggleRoadmap),
    cmd!("Zen mode", "alt-z", ToggleZen),
    cmd!("Focus sidebar", "ctrl-0", FocusSidebar),
    cmd!("Focus editor", "ctrl-1", FocusEditor),
    cmd!("Focus problem description", "ctrl-2", FocusStatement),
    cmd!("Editor completions", "ctrl-space", Complete),
    cmd!("Go to definition", "ctrl-f12", GoToDefinition),
    cmd!("Editor code actions", "alt-enter", ToggleCodeActions),
    cmd!("Next problem", "ctrl-down|alt-n", NextProblem),
    cmd!("Previous problem", "ctrl-up|alt-p", PrevProblem),
    cmd!("Next test case", "alt-.", NextCase),
    cmd!("Previous test case", "alt-,", PrevCase),
    cmd!("Add custom test", "ctrl-alt-t", AddCustomTest),
    cmd!("Edit selected test case", "ctrl-alt-shift-t", EditTestCase),
    cmd!("Restore question test cases", "", ResetTestCases),
    cmd!("AI: Hints", "ctrl-alt-1", AssistHints),
    cmd!("AI: I'm stuck", "ctrl-alt-2", AssistStuck),
    cmd!("AI: Find bugs", "ctrl-alt-3", AssistBugs),
    cmd!("AI: Edge cases", "ctrl-alt-4", AssistTests),
    cmd!("AI: Complexity", "ctrl-alt-5", AssistAnalyze),
    cmd!("AI: Optimize", "ctrl-alt-6", AssistOptimize),
    cmd!("AI: Visualize", "ctrl-alt-7", AssistVisualize),
    cmd!("AI: Dry run", "ctrl-alt-8", AssistDryRun),
    cmd!("AI: Pattern and similar problems", "ctrl-alt-9", AssistPattern),
    cmd!("AI: Explain", "ctrl-alt-0", AssistExplain),
    cmd!("AI: Solve until accepted", "", AssistSolve),
    cmd!("AI: Stop runs", "", StopAssist),
    cmd!("Debugger", "ctrl-`|alt-b", ToggleDebug),
    cmd!("AI: Choose agent and model", "ctrl-shift-e", ConfigureAi),
    cmd!("AI: Switch agent", "ctrl-alt-a", CycleAgent),
    cmd!("Cycle NeetCode list", "ctrl-alt-l", CycleList),
    cmd!("Switch practice source", "ctrl-alt-o", CycleSource),
    cmd!("Toggle NeetCode completion", "ctrl-alt-m", MarkNeetCode),
    cmd!("Cycle hints", "ctrl-alt-h", RevealHint),
    cmd!("Hide hints", "", HideHints),
    cmd!("Open in external editor", "ctrl-e", OpenExternal),
    cmd!("Open problem in browser", "ctrl-o", OpenInBrowser),
    cmd!("Change theme", "", PickTheme),
    cmd!("Refresh catalog and progress", "ctrl-alt-r", RefreshCatalog),
    cmd!("Reset Solution to starter code", "", ResetSolution),
    cmd!("Zoom in", "ctrl-=|ctrl-+|ctrl-shift-=", ZoomIn),
    cmd!("Zoom out", "ctrl--", ZoomOut),
    cmd!("Reset zoom", "", ZoomReset),
    cmd!("Restart into new build", "ctrl-shift-r", Restart),
    cmd!("Quit", "ctrl-q", Quit),
];

pub struct ComponentBindings(pub Vec<KeyBinding>);
impl Global for ComponentBindings {}

pub fn shortcut_search_text(label: &str, keys: &str) -> String {
    format!("{label} keyboard shortcut keybinding command {keys} {} {}", keys.replace('-', "+"), keys.replace('-', " "))
}

pub struct ContextShortcut { pub label: String, pub keys: String, pub context: String }

pub fn contextual_shortcuts(bindings: &[KeyBinding], config: &practice::config::Config) -> Vec<ContextShortcut> {
    let command_names: Vec<_> = COMMANDS.iter().map(|command| (command.action)().name()).collect();
    let overridden: Vec<_> = COMMANDS.iter().flat_map(|command| command.effective_key(config).split('|'))
        .filter_map(|keys| keys.split_whitespace().map(Keystroke::parse).collect::<Result<Vec<_>, _>>().ok())
        .map(|keys| keys.iter().map(ToString::to_string).collect::<Vec<_>>().join(" ")).collect();
    let mut groups = std::collections::BTreeMap::<(String, String), Vec<String>>::new();
    for binding in bindings {
        let name = binding.action().name();
        if command_names.contains(&name) || name.ends_with("NoAction") || name.ends_with("Unbind") { continue; }
        let context = binding.predicate().map_or("Global".into(), |predicate| predicate.to_string());
        let keys = binding.keystrokes().iter().map(ToString::to_string).collect::<Vec<_>>().join(" ");
        if keys.is_empty() || (context.contains("Input") && overridden.contains(&keys)) { continue; }
        let name = name.rsplit("::").next().unwrap_or(name);
        let mut label = String::new();
        for (index, ch) in name.chars().enumerate() {
            if index > 0 && ch.is_uppercase() { label.push(' '); }
            label.push(ch);
        }
        let group = groups.entry((label, context)).or_default();
        if !group.contains(&keys) { group.push(keys); }
    }
    groups.into_iter().map(|((label, context), keys)| ContextShortcut { label, context, keys: keys.join("|") }).collect()
}

impl Command {
    pub fn effective_key<'a>(&'a self, config: &'a practice::config::Config) -> &'a str {
        config.keybindings.get(self.id).map(String::as_str).unwrap_or(self.key)
    }
}

pub fn key_for<'a>(id: &str, config: &'a practice::config::Config) -> &'a str {
    COMMANDS.iter().find(|command| command.id == id).map_or("", |command| command.effective_key(config))
}

pub fn reload_keys(config: &practice::config::Config, cx: &mut App) {
    let base = cx.global::<ComponentBindings>().0.clone();
    cx.clear_key_bindings();
    cx.bind_keys(base);
    bind_keys(config, cx);
    crate::omnibar::bind_keys(cx);
    crate::accounts::bind_keys(cx);
    crate::ai::bind_keys(cx);
    crate::assist::bind_keys(cx);
    crate::debug_view::bind_keys(cx);
    crate::onboarding::bind_keys(cx);
    crate::statement::bind_keys(cx);
}

pub fn bind_keys(config: &practice::config::Config, cx: &mut App) {
    let mut bindings: Vec<KeyBinding> = COMMANDS
        .iter()
        .flat_map(|c| c.effective_key(config).split('|').rev().filter(|k| !k.is_empty()).map(move |k| (c, k)))
        .filter_map(|(c, key)| {
            KeyBinding::load(key, (c.action)(), None, false, None, cx.keyboard_mapper().as_ref())
                .map_err(|error| eprintln!("leet: invalid shortcut for {}: {error}", c.label)).ok()
        })
        .collect();
    // Apply the same keys in inputs, where component bindings take precedence.
    for command in COMMANDS {
        for key in command.effective_key(config).split('|').rev().filter(|key| !key.is_empty()) {
            let context = KeyBindingContextPredicate::parse("Workspace > Input").expect("valid context");
            if let Ok(binding) = KeyBinding::load(key, (command.action)(), Some(context.into()), false, None, cx.keyboard_mapper().as_ref()) {
                bindings.push(binding);
            }
        }
    }
    // The editor binds these itself; a deeper context wins, so re-bind them inside it.
    bindings.extend([
        KeyBinding::new("up", Up, Some(NAV)),
        KeyBinding::new("down", Down, Some(NAV)),
        KeyBinding::new("left", Left, Some(NAV)),
        KeyBinding::new("right", Right, Some(NAV)),
        KeyBinding::new("enter", Confirm, Some(NAV)),
        KeyBinding::new("escape", Back, Some(NAV)),
        // Escape while editing a setting cancels the edit instead of clearing the field.
        KeyBinding::new("escape", Back, Some("Settings > Input")),
        KeyBinding::new("enter", Confirm, Some("Settings > Input")),
        KeyBinding::new("tab", crate::settings::NextSettingsTab, Some("Settings && !Input")),
        KeyBinding::new("shift-tab", crate::settings::PreviousSettingsTab, Some("Settings && !Input")),
    ]);
    cx.bind_keys(bindings);
}

/// Parse with GPUI, require GUI modifiers, and reject ambiguous chords.
pub fn validate_key(index: usize, value: &str, config: &practice::config::Config) -> Result<(), String> {
    let parse = |value: &str| -> Result<Vec<Vec<Keystroke>>, String> {
        if value.is_empty() { return Ok(Vec::new()); }
        value.split('|').map(|alternative| {
            let keys: Vec<Keystroke> = alternative.split_whitespace()
                .map(|key| Keystroke::parse(key).map_err(|error| error.to_string()))
                .collect::<Result<_, _>>()?;
            if keys.is_empty() || keys.iter().any(|key| {
                let m = key.modifiers;
                !(m.control || m.alt || m.platform)
            }) {
                return Err("Use Ctrl, Alt or Super with each key.".into());
            }
            Ok(keys)
        }).collect()
    };
    let proposed = parse(value)?;
    for (index, keys) in proposed.iter().enumerate() {
        if proposed[..index].iter().any(|existing| keys.starts_with(existing) || existing.starts_with(keys)) {
            return Err("Primary and alternatives must use distinct, non-overlapping shortcuts.".into());
        }
    }
    for (i, command) in COMMANDS.iter().enumerate() {
        if i == index { continue; }
        let existing = parse(command.effective_key(config))?;
        if proposed.iter().any(|a| existing.iter().any(|b| a.starts_with(b) || b.starts_with(a))) {
            return Err(format!("Shortcut conflicts with {}.", command.label));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[::core::prelude::v1::test]
    fn shortcut_catalog_uses_overrides_and_preserves_context_without_duplicates() {
        let mut config = practice::config::Config::default();
        let bindings = [KeyBinding::new("ctrl-k", Down, Some("Input")), KeyBinding::new("down", Down, Some("Statement")), KeyBinding::new("down", Down, Some("Statement")), KeyBinding::new("pagedown", Down, Some("Statement"))];
        let shortcuts = contextual_shortcuts(&bindings, &config);
        assert_eq!(shortcuts.len(), 1);
        assert_eq!(shortcuts[0].keys, "down|pagedown");
        config.keybindings.insert("Search".into(), "ctrl-alt-j".into());
        assert_eq!(contextual_shortcuts(&bindings, &config).len(), 2);
        let command = COMMANDS.iter().find(|command| command.id == "Search").unwrap();
        let text = shortcut_search_text(command.label, command.effective_key(&config));
        assert!(text.contains("ctrl+alt+j")); assert!(!text.contains("ctrl-k"));
        let index = practice::search::Index::new([text]);
        assert_eq!(practice::search::Searcher::default().rank(&index, "ctrl+alt+j", 10, |_| true).len(), 1);
    }

    #[::core::prelude::v1::test]
    fn rejects_conflicts_and_bare_keys() {
        let config = practice::config::Config::default();
        let search = COMMANDS.iter().position(|c| c.id == "Search").unwrap();
        assert_eq!(COMMANDS[search].effective_key(&config), "ctrl-k|ctrl-shift-p|ctrl-t");
        let reopen = COMMANDS.iter().position(|c| c.id == "ReopenProblem").unwrap();
        assert_eq!(COMMANDS[reopen].key, "ctrl-shift-t");
        assert!(validate_key(reopen, COMMANDS[reopen].key, &config).is_ok());
        for (id, key) in [("CloseAllProblems", "ctrl-shift-w"), ("ToggleTags", "alt-t")] {
            let index = COMMANDS.iter().position(|c| c.id == id).unwrap();
            assert_eq!(COMMANDS[index].key, key);
            assert!(validate_key(index, key, &config).is_ok());
        }
        assert!(validate_key(search, COMMANDS[search].key, &config).is_ok());
        assert!(validate_key(COMMANDS.iter().position(|c| c.id == "FindProblem").unwrap(), "ctrl-t", &config).is_err());
        assert!(validate_key(COMMANDS.iter().position(|c| c.id == "Search").unwrap(), "ctrl-p", &config).is_err());
        assert!(validate_key(COMMANDS.iter().position(|c| c.id == "Search").unwrap(), "ctrl-p ctrl-k", &config).is_err());
        assert!(validate_key(COMMANDS.iter().position(|c| c.id == "Search").unwrap(), "k", &config).is_err());
        assert!(validate_key(COMMANDS.iter().position(|c| c.id == "Search").unwrap(), "shift-k", &config).is_err());
        assert!(validate_key(COMMANDS.iter().position(|c| c.id == "Search").unwrap(), "ctrl-alt-k", &config).is_ok());
        assert!(validate_key(COMMANDS.iter().position(|c| c.id == "Search").unwrap(), "", &config).is_ok());
    }

    #[::core::prelude::v1::test]
    fn home_primary_and_alternative_are_valid_and_conflict_checked() {
        let config = practice::config::Config::default();
        let home = COMMANDS.iter().position(|c| c.id == "ShowHome").unwrap();
        let search = COMMANDS.iter().position(|c| c.id == "Search").unwrap();
        assert_eq!(COMMANDS[home].effective_key(&config), "ctrl-h|ctrl-.");
        assert!(validate_key(home, COMMANDS[home].key, &config).is_ok());
        for keys in ["ctrl-h", "ctrl-.", "ctrl-alt-j|ctrl-h"] {
            assert!(validate_key(search, keys, &config).is_err());
        }
        assert!(validate_key(home, "ctrl-h|ctrl-h", &config).is_err());
        assert!(validate_key(home, "ctrl-h|ctrl-h ctrl-j", &config).is_err());
    }

    #[::core::prelude::v1::test]
    fn overrides_replace_all_default_alternatives() {
        let mut config = practice::config::Config::default();
        config.keybindings.insert("Search".into(), "ctrl-alt-k".into());
        assert_eq!(COMMANDS.iter().find(|c| c.id == "Search").unwrap().effective_key(&config), "ctrl-alt-k");
        assert!(validate_key(COMMANDS.iter().position(|c| c.id == "FindProblem").unwrap(), "ctrl-k", &config).is_ok());
        assert!(validate_key(COMMANDS.iter().position(|c| c.id == "FindProblem").unwrap(), "ctrl-alt-k", &config).is_err());
    }
}
