//! VS Code-style settings page (`ctrl+,`). Choices change with left/right and apply at once;
//! text values edit inline. Every setting is also reachable from universal search.

use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{ActiveTheme as _, Selectable as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use practice::prompts::Style;

use crate::workspace::{Center, Focus, Workspace};

gpui_kit::actions!(settings, [SavePrompt]);

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([KeyBinding::new("ctrl-enter", SavePrompt, Some("PromptForm > Input"))]);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Setting {
    Onboarding,
    Language,
    LanguageServer,
    Codeforces,
    LeetCode,
    NeetCode,
    Theme,
    Font,
    List,
    Provider,
    PromptStyle,
    Python,
    ExternalEditor,
    TestTimeout,
    Workspace,
    OpenFile,
    Keybindings,
    Keybinding(usize),
    Prompt(Style),
}

pub enum Kind {
    Choice,
    Text,
    Action,
}

impl Setting {
    pub const ALL: [Setting; 21] = [
        Setting::Onboarding,
        Setting::Language,
        Setting::LanguageServer,
        Setting::Codeforces,
        Setting::LeetCode,
        Setting::NeetCode,
        Setting::Theme,
        Setting::Font,
        Setting::List,
        Setting::Provider,
        Setting::PromptStyle,
        Setting::Python,
        Setting::ExternalEditor,
        Setting::TestTimeout,
        Setting::Workspace,
        Setting::Keybindings,
        Setting::Prompt(Style::Hints),
        Setting::Prompt(Style::Guided),
        Setting::Prompt(Style::Full),
        Setting::Prompt(Style::SolutionOnly),
        Setting::OpenFile,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Setting::Onboarding => "Run onboarding again",
            Setting::Language => "Preferred language",
            Setting::LanguageServer => "Restart language server",
            Setting::Codeforces => "Codeforces account",
            Setting::LeetCode => "LeetCode account",
            Setting::NeetCode => "NeetCode account",
            Setting::Theme => "Theme",
            Setting::Font => "Font",
            Setting::List => "Roadmap list",
            Setting::Provider => "AI assistant",
            Setting::PromptStyle => "Default prompt style",
            Setting::Python => "Python interpreter",
            Setting::ExternalEditor => "External editor",
            Setting::TestTimeout => "Test timeout (seconds)",
            Setting::Workspace => "Solutions folder",
            Setting::OpenFile => "Open config.toml",
            Setting::Keybindings => "Keyboard shortcuts",
            Setting::Keybinding(i) => crate::actions::COMMANDS[i].label,
            Setting::Prompt(style) => style.label(),
        }
    }

    /// Extra words universal search matches.
    pub fn keywords(self) -> &'static str {
        match self {
            Setting::Onboarding => "setup onboarding welcome language account sign in",
            Setting::Language => "language python cpp c++ go c preferred",
            Setting::LanguageServer => "lsp intellisense completion diagnostics hover definitions restart",
            Setting::Codeforces => "codeforces handle account sign in",
            Setting::LeetCode | Setting::NeetCode => "login session account sign in sync",
            Setting::Theme => "color appearance dark light vesper",
            Setting::Font => "fonts typography font family liberation sans system",
            Setting::List => "neetcode 150 250 all",
            Setting::Provider => "chatgpt claude gemini ai",
            Setting::PromptStyle => "hints guided explanation solution ai",
            Setting::Python => "python3 pypy interpreter",
            Setting::ExternalEditor => "zed code nvim",
            Setting::TestTimeout => "limit time",
            Setting::Workspace => "workspace directory path git",
            Setting::OpenFile => "toml config file",
            Setting::Keybindings | Setting::Keybinding(_) => "keyboard shortcut binding keys",
            Setting::Prompt(_) => "ai instructions custom prompt",
        }
    }

    pub fn kind(self) -> Kind {
        match self {
            Setting::Language | Setting::Theme | Setting::List | Setting::Provider | Setting::PromptStyle | Setting::TestTimeout => Kind::Choice,
            Setting::Python | Setting::ExternalEditor | Setting::Workspace | Setting::Keybinding(_) | Setting::Prompt(_) => Kind::Text,
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
            Setting::LeetCode => ws.account_names[0].clone(),
            Setting::NeetCode => ws.account_names[1].clone(),
            Setting::Theme => cx.theme().theme_name().to_string(),
            Setting::Font => c.font_family.clone(),
            Setting::List => c.roadmap_list.label().into(),
            Setting::Provider => c.prompt_provider.label().into(),
            Setting::PromptStyle => c.prompt_style.label().into(),
            Setting::Python => c.python.clone(),
            Setting::ExternalEditor => c.external_editor.clone(),
            Setting::TestTimeout => c.test_timeout_secs.to_string(),
            Setting::Workspace => c.workspace.display().to_string(),
            Setting::OpenFile | Setting::Keybindings => String::new(),
            Setting::Keybinding(i) => crate::actions::COMMANDS[i].effective_key(c).into(),
            Setting::Prompt(style) => c.prompt_instructions.get(style.id()).cloned().unwrap_or_else(|| style.instructions().into()),
        }
    }

    fn note(self) -> Option<&'static str> {
        match self {
            Setting::Workspace => Some("Applies after restart"),

            _ => None,
        }
    }
}

pub struct SettingsState {
    pub selected: usize,
    pub keybindings: bool,
    pub scroll: ScrollHandle,
    pub editing: Option<(Setting, Entity<InputState>)>,
}

impl SettingsState {
    pub fn new() -> Self {
        Self { scroll: ScrollHandle::new(), keybindings: false, selected: 0, editing: None }
    }
}

impl Workspace {
    pub fn open_settings(&mut self, focus: Option<Setting>, window: &mut Window, cx: &mut Context<Self>) {
        self.center = Center::Settings;
        self.settings.keybindings = false;
        if let Some(Setting::Keybinding(index)) = focus {
            self.settings.keybindings = true;
            self.settings.selected = index;
        } else if let Some(setting) = focus {
            self.settings.selected = Setting::ALL.iter().position(|s| *s == setting).unwrap_or(0);
        }
        self.settings.editing = None;
        cx.on_next_frame(window, |this, _, cx| {
            if this.center == Center::Settings {
                this.settings.scroll.scroll_to_item(this.settings.selected + 1);
                cx.notify();
            }
        });
        self.focus_nav(Focus::Settings, window, cx);
        self.flash("Settings", cx);
    }

    pub fn settings_move(&mut self, delta: isize, cx: &mut Context<Self>) {
        let len = self.setting_rows().len() as isize;
        self.settings.selected = (self.settings.selected as isize + delta).clamp(0, len - 1) as usize;
        self.settings.scroll.scroll_to_item(self.settings.selected + 1);
        cx.notify();
    }

    fn setting_rows(&self) -> Vec<Setting> {
        if self.settings.keybindings {
            (0..crate::actions::COMMANDS.len()).map(Setting::Keybinding).collect()
        } else {
            Setting::ALL.to_vec()
        }
    }

    fn selected_setting(&self) -> Setting {
        self.setting_rows()[self.settings.selected]
    }

    /// Steps a choice setting; `delta` is ±1.
    pub fn settings_cycle(&mut self, delta: isize, window: &mut Window, cx: &mut Context<Self>) {
        let setting = self.selected_setting();
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
            Setting::Provider => self.config.prompt_provider = self.config.prompt_provider.toggle(),
            Setting::PromptStyle => {
                let i = Style::ALL.iter().position(|s| *s == self.config.prompt_style).unwrap_or(0);
                self.config.prompt_style = Style::ALL[step(Style::ALL.len(), i)];
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
        let setting = self.selected_setting();
        if let Setting::Prompt(style) = setting {
            self.edit_prompt(style, window, cx);
            return;
        }
        match setting.kind() {
            Kind::Choice => self.settings_cycle(1, window, cx),
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
            Kind::Action if setting == Setting::Keybindings => {
                self.settings.keybindings = true;
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
            Setting::Prompt(style) => {
                if value.is_empty() { self.config.prompt_instructions.remove(style.id()); }
                else { self.config.prompt_instructions.insert(style.id().into(), value); }
            }
            Setting::Python if !value.is_empty() => self.config.python = value,
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
        v_flex()
            .flex_1()
            .size_full()
            .items_center()
            .child(
                v_flex()
                    .id("settings-scroll")
                    .overflow_y_scroll()
                    .track_scroll(&self.settings.scroll)
                    .min_h_0()
                    .h_full()
                    .flex_1()
                    .w_full()
                    .max_w(px(780.))
                    .px_4()
                    .pb_8()
                    .pt_10()
                    .gap_1()
                    .child(
                        h_flex()
                            .flex_shrink_0()
                            .pb_4()
                            .justify_between()
                            .child(v_flex().gap_1()
                                .child(div().text_xl().font_weight(FontWeight::SEMIBOLD).child(if self.settings.keybindings { "Keyboard shortcuts" } else { "Settings" }))
                                .when(self.settings.keybindings, |heading| heading.child(div().text_xs().text_color(theme.muted_foreground).child("Empty resets · none disables · | separates keys"))))
                            .child(
                                h_flex()
                                    .gap_3()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child(h_flex().gap_1().child(crate::view::key("left")).child(crate::view::key("right")).child("change"))
                                    .child(h_flex().gap_1().child(crate::view::key("enter")).child("edit"))
                                    .child(h_flex().gap_1().child(crate::view::key("escape")).child("close")),
                            ),
                    )
                    .children(self.setting_rows().iter().enumerate().map(|(i, &setting)| {
                        let selected = i == self.settings.selected;
                        let editing = self.settings.editing.as_ref().filter(|(s, _)| *s == setting);
                        let value = setting.value(self, cx);
                        let control = match (editing, setting.kind()) {
                            (Some((_, input)), _) => div().w(px(320.)).child(Input::new(input)).into_any_element(),
                            (None, Kind::Choice) if setting == Setting::Provider => h_flex().gap_1().children(
                                practice::prompts::Provider::ALL.into_iter().enumerate().map(|(index, provider)| {
                                    use gpui_kit::component::button::{Button, ButtonVariants as _};
                                    Button::new(("settings-provider", index)).ghost().small().icon(crate::brand::icon(provider))
                                        .selected(provider == self.config.prompt_provider)
                                        .accessibility_label(provider.label()).tooltip(provider.label())
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            this.config.prompt_provider = provider;
                                            this.save_config(window, cx);
                                            cx.notify();
                                        }))
                                })
                            ).into_any_element(),
                            (None, Kind::Choice) => h_flex()
                                .gap_2()
                                .child(div().text_color(theme.muted_foreground).child("‹"))
                                .child(div().min_w(px(140.)).text_center().child(value))
                                .child(div().text_color(theme.muted_foreground).child("›"))
                                .into_any_element(),
                            (None, Kind::Text) => div()
                                .w(px(320.))
                                .truncate()
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
                    })),
            )
    }
}

impl Workspace {
    fn edit_prompt(&mut self, style: Style, window: &mut Window, cx: &mut Context<Self>) {
        use std::rc::Rc;
        use gpui_kit::component::button::{Button, ButtonVariants as _};
        use gpui_kit::component::input::{Textarea, TextareaState};
        use gpui_kit::component::WindowExt as _;
        let value = self.config.prompt_instructions.get(style.id()).cloned().unwrap_or_else(|| style.instructions().into());
        let input = cx.new(|cx| TextareaState::new(window, cx).rows(8).default_value(value));
        let weak = cx.weak_entity();
        let save: Rc<dyn Fn(&mut Window, &mut App)> = {
            let input = input.clone();
            Rc::new(move |window, cx| {
                let value = input.read(cx).value().trim().to_string();
                let mut saved = false;
                let _ = weak.update(cx, |ws, cx| {
                    let mut config = ws.config.clone();
                    if value.is_empty() { config.prompt_instructions.remove(style.id()); }
                    else { config.prompt_instructions.insert(style.id().into(), value); }
                    match config.save() {
                        Ok(()) => { ws.config = config; saved = true; ws.focus_nav(Focus::Settings, window, cx); }
                        Err(error) => ws.toast(gpui_kit::component::notification::Notification::error(error.to_string()), window, cx),
                    }
                    cx.notify();
                });
                if saved { window.close_dialog(cx); }
            })
        };
        let focus = input.clone();
        window.open_dialog(cx, move |dialog, _, cx| {
            let save = save.clone();
            let save_key = save.clone();
            dialog.title(format!("{} instructions", style.label())).w(px(640.)).child(
                v_flex().key_context("PromptForm").gap_3()
                    .on_action(move |_: &SavePrompt, window, cx| save_key(window, cx))
                    .child(Textarea::new(&input))
                    .child(div().text_xs().text_color(cx.theme().muted_foreground)
                        .child("Problem context is added automatically. Empty restores defaults. Ctrl+Enter saves."))
                    .child(Button::new("save-prompt").primary().label("Save").on_click(move |_, window, cx| save(window, cx)))
            )
        });
        window.defer(cx, move |window, cx| focus.update(cx, |state, cx| state.focus(window, cx)));
    }
}
