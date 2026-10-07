//! VS Code-style settings page (`ctrl+,`). Choices change with left/right and apply at once;
//! text values edit inline. Every setting is also reachable from universal search.

use gpui_kit::base::{Tab, Tabs};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{ActiveTheme as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use practice::agents::AgentKind;

use crate::assist::Target;
use crate::workspace::{Center, Focus, Workspace};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Setting {
    Onboarding,
    Language,
    LanguageServer,
    Codeforces,
    CompanionEnabled,
    CompanionPort,
    LeetCode,
    NeetCode,
    Theme,
    Font,
    List,
    Python,
    ExternalEditor,
    TestTimeout,
    Workspace,
    OpenFile,
    Keybindings,
    Keybinding(usize),
    AiAgent,
    AiModel,
    AiReasoning,
    AiFast,
    WebChat,
    Install(AgentKind),
}

pub enum Kind {
    Choice,
    Toggle,
    Text,
    Action,
}

impl Setting {
    pub const ALL: [Setting; 24] = [
        Setting::Onboarding,
        Setting::Language,
        Setting::LanguageServer,
        Setting::Codeforces,
        Setting::CompanionEnabled,
        Setting::CompanionPort,
        Setting::LeetCode,
        Setting::NeetCode,
        Setting::Theme,
        Setting::Font,
        Setting::List,
        Setting::Python,
        Setting::ExternalEditor,
        Setting::TestTimeout,
        Setting::Workspace,
        Setting::Keybindings,
        Setting::OpenFile,
        Setting::AiAgent,
        Setting::AiModel,
        Setting::AiReasoning,
        Setting::AiFast,
        Setting::WebChat,
        Setting::Install(AgentKind::OpenCode),
        Setting::Install(AgentKind::Antigravity),
    ];

    pub fn label(self) -> &'static str {
        match self {
            Setting::Onboarding => "Run onboarding again",
            Setting::Language => "Preferred language",
            Setting::LanguageServer => "Restart language server",
            Setting::Codeforces => "Codeforces account",
            Setting::CompanionEnabled => "Competitive Companion",
            Setting::CompanionPort => "Browser import port",
            Setting::LeetCode => "LeetCode account",
            Setting::NeetCode => "NeetCode account",
            Setting::Theme => "Theme",
            Setting::Font => "Font",
            Setting::List => "Roadmap list",
            Setting::Python => "Python interpreter",
            Setting::ExternalEditor => "External editor",
            Setting::TestTimeout => "Test timeout (seconds)",
            Setting::Workspace => "Solutions folder",
            Setting::OpenFile => "Open config.toml",
            Setting::Keybindings => "Keyboard shortcuts",
            Setting::Keybinding(i) => crate::actions::COMMANDS[i].label,
            Setting::AiAgent => "Agent",
            Setting::AiModel => "Model",
            Setting::AiReasoning => "Reasoning",
            Setting::AiFast => "Fast tier",
            Setting::WebChat => "Web chat",
            Setting::Install(AgentKind::OpenCode) => "Install OpenCode",
            Setting::Install(_) => "Install Antigravity",
        }
    }

    /// Extra words universal search matches.
    pub fn keywords(self) -> &'static str {
        match self {
            Setting::Onboarding => "setup onboarding welcome language account sign in",
            Setting::Language => "language python cpp c++ go c java preferred",
            Setting::LanguageServer => "lsp intellisense completion diagnostics hover definitions restart",
            Setting::Codeforces => "codeforces handle account sign in",
            Setting::CompanionEnabled => "competitive companion browser import enable disable",
            Setting::CompanionPort => "browser import port localhost",
            Setting::LeetCode | Setting::NeetCode => "login session account sign in sync",
            Setting::Theme => "color appearance dark light vesper",
            Setting::Font => "fonts typography font family liberation sans system",
            Setting::List => "neetcode 150 250 all",
            Setting::Python => "python3 pypy interpreter",
            Setting::ExternalEditor => "zed code nvim",
            Setting::TestTimeout => "limit time",
            Setting::Workspace => "workspace directory path git",
            Setting::OpenFile => "toml config file",
            Setting::Keybindings | Setting::Keybinding(_) => "keyboard shortcut binding keys",
            Setting::AiAgent => "ai agent codex claude opencode antigravity gemini cursor local",
            Setting::AiModel => "ai model luna haiku free",
            Setting::AiReasoning => "ai reasoning effort thinking",
            Setting::AiFast => "ai fast tier speed",
            Setting::WebChat => "ai web chat chatgpt claude gemini browser",
            Setting::Install(_) => "ai install agent free models opencode antigravity",
        }
    }

    pub fn kind(self) -> Kind {
        match self {
            Setting::Language | Setting::Theme | Setting::List | Setting::TestTimeout | Setting::AiAgent | Setting::AiReasoning | Setting::WebChat => Kind::Choice,
            Setting::CompanionEnabled | Setting::AiFast => Kind::Toggle,
            Setting::AiModel | Setting::Install(_) => Kind::Action,
            Setting::Python | Setting::ExternalEditor | Setting::Workspace | Setting::CompanionPort | Setting::Keybinding(_) => Kind::Text,
            Setting::Onboarding | Setting::LanguageServer | Setting::Codeforces | Setting::OpenFile | Setting::Font | Setting::Keybindings | Setting::LeetCode | Setting::NeetCode => Kind::Action,
        }
    }

    pub fn value(self, ws: &Workspace, cx: &App) -> String {
        let c = &ws.config;
        match self {
            Setting::Onboarding => String::new(),
            Setting::Language => c.preferred_language.label().into(),
            Setting::LanguageServer => ws.intelligence.label(ws.session.as_ref().map_or(c.preferred_language, |session| session.language)),
            Setting::Codeforces => if c.codeforces_handle.is_empty() { "Not set".into() } else { c.codeforces_handle.clone() },
            Setting::CompanionEnabled => if c.companion_enabled { "On".into() } else { "Off".into() },
            Setting::CompanionPort => c.companion_port.to_string(),
            Setting::LeetCode => ws.account_names[0].clone(),
            Setting::NeetCode => ws.account_names[1].clone(),
            Setting::Theme => cx.theme().theme_name().to_string(),
            Setting::Font => c.font_family.clone(),
            Setting::List => c.roadmap_list.label().into(),
            Setting::Python => c.python.clone(),
            Setting::ExternalEditor => c.external_editor.clone(),
            Setting::TestTimeout => c.test_timeout_secs.to_string(),
            Setting::Workspace => dirs::home_dir().and_then(|home| c.workspace.strip_prefix(home).ok().map(|path| format!("~/{}", path.display()))).unwrap_or_else(|| c.workspace.display().to_string()),
            Setting::OpenFile | Setting::Keybindings => String::new(),
            Setting::Keybinding(i) => crate::actions::COMMANDS[i].effective_key(c).into(),
            Setting::AiAgent => match ws.assist.read(cx).target(c) { Target::Agent(agent, _) => agent.kind.label().into(), Target::Web(_) => "Web chat".into() },
            Setting::AiModel => ws.ai_selection(cx).map_or_else(|| "Default".into(), |s| s.model),
            Setting::AiReasoning => ws.ai_selection(cx).and_then(|s| s.effort).unwrap_or_else(|| "Default".into()),
            Setting::AiFast => if ws.ai_selection(cx).is_some_and(|s| s.fast) { "On".into() } else { "Off".into() },
            Setting::WebChat => c.web_chat.label().into(),
            Setting::Install(kind) => if ws.assist.read(cx).installing == Some(kind) { "Installing…".into() } else { "Install".into() },
        }
    }

    fn note(self) -> Option<&'static str> {
        match self {
            Setting::Workspace => Some("Applies after restart"),

            _ => None,
        }
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum SettingsTab { #[default] General, Editor, Appearance, Provider, Ai, Accounts, Keybindings }

impl SettingsTab {
    const ALL: [Self; 7] = [Self::General, Self::Editor, Self::Appearance, Self::Provider, Self::Ai, Self::Accounts, Self::Keybindings];
    fn label(self) -> &'static str {
        match self { Self::General => "General", Self::Editor => "Editor", Self::Appearance => "Appearance", Self::Provider => "Provider", Self::Ai => "AI", Self::Accounts => "Accounts", Self::Keybindings => "Keybindings" }
    }
    fn settings(self) -> &'static [Setting] {
        match self {
            Self::General => &[Setting::List, Setting::Workspace, Setting::Onboarding, Setting::OpenFile],
            Self::Editor => &[Setting::Language, Setting::Python, Setting::ExternalEditor, Setting::TestTimeout, Setting::LanguageServer],
            Self::Appearance => &[Setting::Theme, Setting::Font],
            Self::Provider => &[Setting::Codeforces, Setting::CompanionEnabled, Setting::CompanionPort],
            Self::Ai => &[Setting::AiAgent, Setting::AiModel, Setting::AiReasoning, Setting::AiFast, Setting::WebChat, Setting::Install(AgentKind::OpenCode), Setting::Install(AgentKind::Antigravity)],
            Self::Accounts => &[Setting::LeetCode, Setting::NeetCode],
            Self::Keybindings => &[],
        }
    }
    fn setting_visible(self, setting: Setting, codeforces_configured: bool) -> bool {
        self.settings().contains(&setting)
            && (!matches!(setting, Setting::CompanionEnabled | Setting::CompanionPort) || codeforces_configured)
    }
    fn for_setting(setting: Setting) -> Self {
        if matches!(setting, Setting::Keybinding(_) | Setting::Keybindings) { return Self::Keybindings; }
        Self::ALL.into_iter().find(|tab| tab.settings().contains(&setting)).unwrap_or_default()
    }
}

pub struct SettingsState {
    pub selected: usize,
    pub tab: SettingsTab,
    pub scroll: ScrollHandle,
    pub editing: Option<(Setting, Entity<InputState>)>,
}

impl SettingsState {
    pub fn new() -> Self {
        Self { scroll: ScrollHandle::new(), tab: SettingsTab::General, selected: 0, editing: None }
    }
}

impl Workspace {
    pub fn open_settings(&mut self, focus: Option<Setting>, window: &mut Window, cx: &mut Context<Self>) {
        self.center = Center::Settings;
        self.settings.tab = focus.map(SettingsTab::for_setting).unwrap_or_default();
        self.settings.selected = 0;
        if let Some(Setting::Keybinding(index)) = focus {
            self.settings.tab = SettingsTab::Keybindings;
            self.settings.selected = index;
        } else if let Some(setting) = focus {
            self.settings.selected = self.setting_rows(cx).iter().position(|s| *s == setting).unwrap_or(0);
        }
        self.settings.editing = None;
        cx.on_next_frame(window, |this, _, cx| {
            if this.center == Center::Settings {
                this.settings.scroll.scroll_to_item(this.settings.selected);
                cx.notify();
            }
        });
        self.focus_nav(Focus::Settings, window, cx);
        self.flash("Settings", cx);
    }

    pub fn settings_move(&mut self, delta: isize, cx: &mut Context<Self>) {
        let len = self.setting_rows(cx).len() as isize;
        self.settings.selected = (self.settings.selected as isize + delta).clamp(0, len - 1) as usize;
        self.settings.scroll.scroll_to_item(self.settings.selected);
        cx.notify();
    }

    fn setting_rows(&self, cx: &App) -> Vec<Setting> {
        if self.settings.tab == SettingsTab::Keybindings {
            (0..crate::actions::COMMANDS.len()).map(Setting::Keybinding).collect()
        } else {
            self.settings.tab.settings().iter().copied().filter(|setting| {
                self.settings.tab.setting_visible(*setting, !self.config.codeforces_handle.trim().is_empty()) && self.ai_setting_visible(*setting, cx)
            }).collect()
        }
    }

    fn selected_setting(&self, cx: &App) -> Setting {
        self.setting_rows(cx)[self.settings.selected]
    }

    /// AI rows that only apply to a local agent, a model with options, or a missing installable agent.
    fn ai_setting_visible(&self, setting: Setting, cx: &App) -> bool {
        let assist = self.assist.read(cx);
        let local = matches!(assist.target(&self.config), Target::Agent(..));
        let model = self.ai_model(cx);
        match setting {
            Setting::AiModel => local,
            Setting::AiReasoning => local && model.is_some_and(|m| !m.efforts.is_empty()),
            Setting::AiFast => local && model.is_some_and(|m| m.fast),
            Setting::Install(kind) => assist.detected(kind).is_none() && !assist.detecting,
            _ => true,
        }
    }

    /// The effective model choice: remembered, else the catalog default.
    pub fn ai_selection(&self, cx: &App) -> Option<practice::agents::Selection> {
        let assist = self.assist.read(cx);
        let Target::Agent(agent, selection) = assist.target(&self.config) else { return None };
        selection.or_else(|| {
            let catalog = assist.catalog(agent.kind)?;
            let model = catalog.models.iter().find(|m| Some(&m.id) == catalog.default_model.as_ref())?;
            Some(practice::agents::Selection { agent: agent.kind, model: model.id.clone(), effort: model.default_effort.clone(), fast: false })
        })
    }

    fn ai_model(&self, cx: &App) -> Option<practice::agents::Model> {
        let selection = self.ai_selection(cx)?;
        self.assist.read(cx).catalog(selection.agent)?.models.iter().find(|m| m.id == selection.model).cloned()
    }

    fn cycle_ai(&mut self, setting: Setting, delta: isize, window: &mut Window, cx: &mut Context<Self>) {
        let step = |len: usize, i: usize| (i as isize + delta).rem_euclid(len.max(1) as isize) as usize;
        match setting {
            Setting::AiAgent => {
                let options: Vec<Option<AgentKind>> = self.assist.read(cx).agents.iter().map(|a| Some(a.kind)).chain([None]).collect();
                let current = match self.assist.read(cx).target(&self.config) { Target::Agent(agent, _) => Some(agent.kind), Target::Web(_) => None };
                let index = options.iter().position(|o| *o == current).unwrap_or(0);
                self.config.agent = options[step(options.len(), index)];
                self.config.ai_web = self.config.agent.is_none();
                if let Some(kind) = self.config.agent { self.assist.update(cx, |assist, cx| assist.load_catalog(kind, cx)); }
            }
            Setting::AiReasoning | Setting::AiFast => {
                let (Some(mut selection), Some(model)) = (self.ai_selection(cx), self.ai_model(cx)) else { return };
                if setting == Setting::AiFast { selection.fast = !selection.fast; } else {
                    let index = model.efforts.iter().position(|e| Some(&e.id) == selection.effort.as_ref()).unwrap_or(0);
                    selection.effort = model.efforts.get(step(model.efforts.len(), index)).map(|e| e.id.clone());
                }
                self.config.agent = Some(selection.agent);
                self.config.ai_web = false;
                self.config.remember(selection);
            }
            Setting::WebChat => {
                let all = practice::prompts::Provider::ALL;
                let index = all.iter().position(|p| *p == self.config.web_chat).unwrap_or(0);
                self.config.web_chat = all[step(all.len(), index)];
            }
            _ => return,
        }
        self.save_config(window, cx);
        self.ai_chip.update(cx, |_, cx| cx.notify());
        cx.notify();
    }

    /// Steps a choice setting; `delta` is ±1.
    pub fn settings_cycle(&mut self, delta: isize, window: &mut Window, cx: &mut Context<Self>) {
        let setting = self.selected_setting(cx);
        if matches!(setting, Setting::AiAgent | Setting::AiReasoning | Setting::AiFast | Setting::WebChat) { self.cycle_ai(setting, delta, window, cx); return; }
        let step = |len: usize, i: usize| (i as isize + delta).rem_euclid(len as isize) as usize;
        match setting {
            Setting::Theme => {
                let names = crate::theme::names(cx);
                let current = names.iter().position(|n| n == cx.theme().theme_name()).unwrap_or(0);
                let name = names[step(names.len(), current)].clone();
                crate::theme::apply(&name, cx);
                self.config.theme = name.to_string();
            }
            Setting::List => {
                self.config.roadmap_list = if delta > 0 { self.config.roadmap_list.next() } else { self.config.roadmap_list.next().next() };
                self.rebuild_library();
            }
            Setting::Language => self.config.preferred_language = self.config.preferred_language.next(),
            Setting::CompanionEnabled => {
                self.config.companion_enabled = !self.config.companion_enabled;
                self.save_config(window, cx);
                self.start_companion(window, cx);
                cx.notify();
                return;
            }
            Setting::TestTimeout => {
                self.config.test_timeout_secs = (self.config.test_timeout_secs as i64 + delta as i64).clamp(1, 120) as u64;
            }
            _ => return,
        }
        self.save_config(window, cx);
        cx.notify();
    }

    pub fn settings_confirm(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some((setting, input)) = self.settings.editing.as_ref() {
            let setting = *setting;
            let value = input.read(cx).value().trim().to_owned();
            self.apply_text_setting(setting, value, window, cx);
            return;
        }
        let setting = self.selected_setting(cx);
        match setting.kind() {
            Kind::Choice => self.settings_cycle(1, window, cx),
            Kind::Toggle => self.settings_cycle(1, window, cx),
            Kind::Action if setting == Setting::LanguageServer => self.restart_language_server(window, cx),
            Kind::Action if setting == Setting::Onboarding => self.begin_onboarding(false, window, cx),
            Kind::Action if setting == Setting::Codeforces => self.begin_onboarding(true, window, cx),
            Kind::Action if setting == Setting::Font => {
                self.omni_open(crate::omnibar::Scope::Fonts, window, cx);
            }
            Kind::Action if matches!(setting, Setting::LeetCode | Setting::NeetCode) => {
                let account = if setting == Setting::LeetCode { practice::creds::Account::LeetCode } else { practice::creds::Account::NeetCode };
                crate::accounts::open(account, self.account_names[account.index()] != "Signed out", window, cx);
            }
            Kind::Action if setting == Setting::AiModel => self.ai_chip.update(cx, |chip, cx| chip.open(window, cx)),
            Kind::Action if matches!(setting, Setting::Install(_)) => {
                if let Setting::Install(kind) = setting { self.assist.update(cx, |assist, cx| assist.install(kind, window, cx)); }
            }
            Kind::Action if setting == Setting::Keybindings => {
                self.settings.tab = SettingsTab::Keybindings;
                self.settings.selected = 0;
                self.settings.scroll.scroll_to_item(0);
                cx.notify();
            }
            Kind::Action => {
                let _ = self.config.save();
                let path = practice::config::Config::path();
                let editor = self.config.external_editor.clone();
                if std::process::Command::new(&editor).arg(&path).spawn().is_err() {
                    let _ = open::that_detached(&path);
                }
                self.flash("Opened config.toml", cx);
            }
            Kind::Text => {
                let value = setting.value(self, cx);
                let input = cx.new(|cx| InputState::new(window, cx).default_value(value));
                input.update(cx, |i, cx| i.focus(window, cx));
                self.settings.editing = Some((setting, input));
                cx.notify();
            }
        }
    }

    fn apply_text_setting(&mut self, setting: Setting, value: String, window: &mut Window, cx: &mut Context<Self>) {
        match setting {
            Setting::Keybinding(i) => {
                let command = &crate::actions::COMMANDS[i];
                let key = if value.is_empty() { command.key } else if value == "none" { "" } else { &value };
                if let Err(error) = crate::actions::validate_key(i, key, &self.config) {
                    self.toast(gpui_kit::component::notification::Notification::error(error), window, cx);
                    return;
                }
                let mut config = self.config.clone();
                if value.is_empty() { config.keybindings.remove(command.id); }
                else { config.keybindings.insert(command.id.into(), key.into()); }
                if let Err(error) = config.save() {
                    self.toast(gpui_kit::component::notification::Notification::error(error.to_string()), window, cx);
                    return;
                }
                self.config = config;
                crate::actions::reload_keys(&self.config, cx);
                self.omni.stale = true;
            }
            Setting::Python if !value.is_empty() => self.config.python = value,
            Setting::CompanionPort => match value.parse::<u16>() {
                Ok(port) if port != 0 => {
                    self.config.companion_port = port;
                    self.save_config(window, cx);
                    self.settings_cancel_edit(window, cx);
                    self.start_companion(window, cx);
                    self.flash("Browser import port saved", cx);
                    return;
                }
                _ => {
                    self.toast(gpui_kit::component::notification::Notification::error("Enter a port from 1 to 65535"), window, cx);
                    return;
                }
            },
            Setting::ExternalEditor if !value.is_empty() => self.config.external_editor = value,
            Setting::Workspace if !value.is_empty() => {
                let expanded = value.strip_prefix("~/").map_or(value.clone().into(), |rest| dirs::home_dir().unwrap_or_default().join(rest));
                self.config.workspace = expanded;
            }
            _ => {}
        }
        self.save_config(window, cx);
        self.settings_cancel_edit(window, cx);
        self.flash(format!("{} saved", setting.label()), cx);
    }

    pub fn settings_cancel_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.settings.editing = None;
        self.focus_nav(Focus::Settings, window, cx);
    }

    pub fn render_settings(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let focused = self.focus_area == Focus::Settings && self.nav_focus.is_focused(window);
        v_flex().size_full().items_center()
            .child(v_flex().w_full().max_w(px(780.)).h_full().min_h_0().px_4().pt_10().gap_4()
                .child(div().flex_shrink_0().text_xl().font_weight(FontWeight::SEMIBOLD).child("Settings"))
                .child(Tabs::new("settings-tabs").flex().flex_row().flex_nowrap().flex_shrink_0().gap_1()
                    .children(SettingsTab::ALL.into_iter().enumerate().map(|(index, tab)| {
                        Tab::new(("settings-tab", index)).selected(self.settings.tab == tab).set_position(index + 1, SettingsTab::ALL.len())
                            .h_9().px_3().rounded_lg().child(div().line_height(relative(1.4)).py_1().child(tab.label()))
                            .styles(|styles| styles.selected(|style| style.bg(theme.list_active).text_color(theme.primary)))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.settings.tab = tab; this.settings.selected = 0; this.settings.editing = None;
                                this.settings.scroll.scroll_to_item(0); this.focus_nav(Focus::Settings, window, cx); cx.notify();
                            }))
                    })))
                .child(v_flex().id("settings-scroll").overflow_y_scroll().track_scroll(&self.settings.scroll)
                    .min_h_0().flex_1().w_full().pb_8().gap_1()
                    .children(self.setting_rows(cx).iter().enumerate().map(|(i, &setting)| {
                        let selected = i == self.settings.selected;
                        let editing = self.settings.editing.as_ref().filter(|(s, _)| *s == setting);
                        let value = setting.value(self, cx);
                        let control = match (editing, setting.kind()) {
                            (Some((_, input)), _) => div().w(px(320.)).child(Input::new(input)).into_any_element(),
                            (None, Kind::Choice | Kind::Toggle) => h_flex()
                                .gap_2()
                                .child(div().text_color(theme.muted_foreground).child("‹"))
                                .child(div().text_right().child(value))
                                .child(div().text_color(theme.muted_foreground).child("›"))
                                .into_any_element(),
                            (None, Kind::Text) if matches!(setting, Setting::Keybinding(_)) => crate::view::shortcut_keys(&value).into_any_element(),
                            (None, Kind::Text) => div()
                                .w(px(320.))
                                .truncate()
                                .text_right()
                                .font_family(theme.mono_font_family.clone())
                                .text_xs()
                                .child(if value.is_empty() { "—".into() } else { value })
                                .into_any_element(),
                            (None, Kind::Action) => div().text_color(theme.link).child(if value.is_empty() { "Open".into() } else { value }).into_any_element(),
                        };
                        h_flex()
                            .id(("setting", i))
                            .flex_shrink_0()
                            .px_4()
                            .py_3()
                            .gap_4()
                            .rounded_lg()
                            .justify_between()
                            .when(selected, |el| {
                                el.bg(if focused || editing.is_some() { theme.list_active } else { theme.list_hover })
                            })
                            .child(
                                v_flex()
                                    .gap_0p5()
                                    .child(div().font_weight(FontWeight::MEDIUM).child(setting.label()))
                                    .when_some(setting.note(), |el, note| el.child(div().text_xs().text_color(theme.muted_foreground).child(note))),
                            )
                            .child(control)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.settings.selected = i;
                                this.settings_confirm(window, cx);
                            }))
                    }))
                    .when(self.settings.tab == SettingsTab::Keybindings, |view| {
                        let bindings: Vec<_> = cx.key_bindings().borrow().bindings().cloned().collect();
                        view.children(crate::actions::contextual_shortcuts(&bindings, &self.config).into_iter().map(|shortcut| {
                            h_flex().px_4().py_3().gap_4().justify_between()
                                .child(v_flex().gap_1().child(shortcut.label).child(div().text_xs().text_color(theme.muted_foreground).child(shortcut.context)))
                                .child(crate::view::shortcut_keys(&shortcut.keys))
                        }))
                    })))
    }
}


#[cfg(test)]
mod tests {
    use super::{Setting, SettingsTab};

    #[test]
    fn searchable_settings_route_to_one_tab_and_keybindings_use_their_own_tab() {
        for setting in Setting::ALL {
            let tab = SettingsTab::for_setting(setting);
            if setting == Setting::Keybindings { assert!(tab == SettingsTab::Keybindings); continue; }
            assert_eq!(SettingsTab::ALL.into_iter().filter(|tab| tab.settings().contains(&setting)).count(), 1);
            assert!(tab.settings().contains(&setting));
        }
        assert!(SettingsTab::for_setting(Setting::Keybinding(0)) == SettingsTab::Keybindings);
        assert!(SettingsTab::for_setting(Setting::Theme) == SettingsTab::Appearance);
        assert!(SettingsTab::for_setting(Setting::LeetCode) == SettingsTab::Accounts);
        assert!(SettingsTab::for_setting(Setting::Codeforces) == SettingsTab::Provider);
    }

    #[test]
    fn codeforces_companion_settings_require_a_configured_account() {
        assert!(!SettingsTab::Provider.setting_visible(Setting::CompanionEnabled, false));
        assert!(!SettingsTab::Provider.setting_visible(Setting::CompanionPort, false));
        assert!(SettingsTab::Provider.setting_visible(Setting::CompanionEnabled, true));
        assert!(SettingsTab::Provider.setting_visible(Setting::CompanionPort, true));
        assert!(SettingsTab::Provider.setting_visible(Setting::Codeforces, false));
    }
}
