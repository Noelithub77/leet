//! The same native setup view is used on first launch and from Settings.
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Icon, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use practice::creds::Account;
use practice::language::{Language, Source};
use crate::workspace::{Center, Workspace};

gpui_kit::actions!(onboarding, [ContinueSetup, SetupPython, SetupCpp, SetupGo, SetupC, SetupJava]);
pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("ctrl-enter", ContinueSetup, Some("Onboarding")),
        KeyBinding::new("ctrl-enter", ContinueSetup, Some("Onboarding > Input")),
        KeyBinding::new("ctrl-1", SetupPython, Some("Onboarding")),
        KeyBinding::new("ctrl-2", SetupCpp, Some("Onboarding")),
        KeyBinding::new("ctrl-3", SetupGo, Some("Onboarding")),
        KeyBinding::new("ctrl-4", SetupC, Some("Onboarding")),
        KeyBinding::new("ctrl-5", SetupJava, Some("Onboarding")),
    ]);
}

pub struct Setup {
    pub focus: FocusHandle,
    workspace: WeakEntity<Workspace>,
    step: usize,
    language: Language,
    source: Source,
    python: String,
    handle: Entity<InputState>,
    busy: bool,
    error: Option<String>,
    requirements: Option<practice::toolchain::Setup>,
    checking: bool,
    check_epoch: u64,
    check_task: Task<()>,
}

impl Workspace {
    pub fn begin_onboarding(&mut self, accounts: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.save_now(cx);
        self.suspend_language_servers(cx);
        let workspace = cx.weak_entity();
        let config = self.config.clone();
        let setup = cx.new(|cx| Setup {
            focus: cx.focus_handle(), workspace, step: usize::from(accounts),
            language: config.preferred_language, source: config.source,
            python: config.python.clone(),
            handle: cx.new(|cx| InputState::new(window, cx).placeholder("Codeforces handle").default_value(config.codeforces_handle)),
            busy: false, error: None, requirements: None, checking: false, check_epoch: 0, check_task: Task::ready(()),
        });
        if !accounts { setup.update(cx, |setup, cx| setup.check_requirements(window, cx)); }
        let focus = setup.read(cx).focus.clone();
        self.onboarding = Some(setup);
        self.center = Center::Onboarding;
        window.defer(cx, move |window, cx| focus.focus(window, cx));
        cx.notify();
    }
}

impl Setup {
    fn choose_language(&mut self, language: Language, window: &mut Window, cx: &mut Context<Self>) {
        if self.language == language && self.requirements.is_some() { return; }
        self.language = language;
        self.check_requirements(window, cx);
    }

    fn check_requirements(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.checking = true;
        self.requirements = None;
        self.error = None;
        self.check_epoch = self.check_epoch.wrapping_add(1);
        let epoch = self.check_epoch;
        let language = self.language;
        // Setup can start while Workspace is still being updated.
        let python = self.python.clone();
        self.check_task = cx.spawn_in(window, async move |this, cx| {
            let requirements = cx.background_spawn(async move { practice::toolchain::check(language, &python) }).await;
            let _ = this.update(cx, |this, cx| {
                if this.check_epoch != epoch || this.language != language { return; }
                this.checking = false;
                this.requirements = Some(requirements);
                cx.notify();
            });
        });
        cx.notify();
    }
    fn advance(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy { return; }
        if self.step == 0 {
            if self.checking || !self.requirements.as_ref().is_some_and(|requirements| requirements.ready()) {
                self.error = Some("Install the missing tools, then recheck.".into());
                cx.notify();
                return;
            }
            self.step = 1;
            self.handle.update(cx, |handle, cx| handle.focus(window, cx));
            cx.notify();
            return;
        }
        let handle = self.handle.read(cx).value().trim().to_owned();
        if self.source == Source::Codeforces && handle.is_empty() {
            self.error = Some("Enter your Codeforces handle.".into());
            self.handle.update(cx, |handle, cx| handle.focus(window, cx));
            cx.notify();
            return;
        }
        self.busy = true;
        self.error = None;
        let (language, source) = (self.language, self.source);
        let workspace = self.workspace.clone();
        cx.spawn_in(window, async move |this, cx| {
            let result = cx.background_spawn(async move {
                if handle.is_empty() { Ok(handle) } else { practice::codeforces::user(&handle).map(|user| user.handle) }
            }).await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.busy = false;
                match result {
                    Ok(handle) => {
                        let result = workspace.update(cx, |ws, cx| {
                            let mut config = ws.config.clone();
                            config.preferred_language = language;
                            config.source = source;
                            config.codeforces_handle = handle;
                            config.onboarding_completed = true;
                            config.save()?;
                            ws.config = config;
                            ws.onboarding = None;
                            ws.rebuild_rows();
                            ws.omni.stale = true;
                            ws.show_home(window, cx);
                            ws.start_default_tour(window, cx);
                            if source == Source::Codeforces { ws.refresh_codeforces(false, window, cx); }
                            anyhow::Ok(())
                        });
                        if let Err(error) = result.and_then(|result| result) { this.error = Some(error.to_string()); }
                    }
                    Err(error) => this.error = Some(error.to_string()),
                }
                cx.notify();
            });
        }).detach();
        cx.notify();
    }
}

impl Render for Setup {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let accounts = self.workspace.upgrade().map(|ws| ws.read(cx).account_names.clone()).unwrap_or(["Signed out".into(), "Signed out".into()]);
        let current_step = self.step;
        v_flex().size_full().items_center().justify_center().key_context("Onboarding").track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &ContinueSetup, window, cx| this.advance(window, cx)))
            .on_action(cx.listener(|this, _: &SetupPython, window, cx| this.choose_language(Language::Python, window, cx)))
            .on_action(cx.listener(|this, _: &SetupCpp, window, cx| this.choose_language(Language::Cpp, window, cx)))
            .on_action(cx.listener(|this, _: &SetupGo, window, cx| this.choose_language(Language::Go, window, cx)))
            .on_action(cx.listener(|this, _: &SetupC, window, cx| this.choose_language(Language::C, window, cx)))
            .on_action(cx.listener(|this, _: &SetupJava, window, cx| this.choose_language(Language::Java, window, cx)))
            .child(v_flex().w(px(600.)).max_w_full().p_6().gap_5().rounded_lg().border_1().border_color(theme.border).bg(theme.sidebar)
                .child(h_flex().justify_between().items_center()
                    .child(div().text_2xl().font_weight(FontWeight::SEMIBOLD).child(if current_step == 0 { "Set up leet" } else { "Connect your accounts" }))
                    .child(div().text_xs().text_color(theme.muted_foreground).child(format!("{} / 2", current_step + 1))))
                .when(current_step == 0, |view| view
                    .child(v_flex().gap_3().child(div().text_color(theme.muted_foreground).child("Preferred language"))
                        .child(h_flex().gap_3().children(Language::ALL.into_iter().enumerate().map(|(index, language)| {
                            crate::theme::selected_choice(Button::new(("setup-language", index)).outline(), self.language == language, cx).flex_1().h(px(76.))
                                .icon(Icon::default().path(format!("languages/{}.svg", language.id())).size(px(24.)))
                                .label(language.label()).tooltip(format!("{} · Ctrl+{}", language.label(), index + 1))
                                .on_click(cx.listener(move |this, _, window, cx| this.choose_language(language, window, cx)))
                        }))))
                    .child(v_flex().gap_2()
                        .when(self.checking, |view| view.child(div().text_sm().text_color(theme.muted_foreground).child("Checking requirements…")))
                        .when_some(self.requirements.as_ref(), |view, setup| view.children(setup.requirements.iter().enumerate().map(|(index, requirement)| {
                            let url = requirement.setup_url;
                            h_flex().gap_2().items_center()
                                .child(Icon::new(if requirement.ready { IconName::Check } else { IconName::X }).small()
                                    .text_color(if requirement.ready { theme.success } else { theme.warning }))
                                .child(div().id(("setup-requirement-detail", index)).flex_1().text_sm().child(requirement.label.clone()).tooltip({ let detail = requirement.detail.clone(); move |window, cx| gpui_kit::component::tooltip::Tooltip::new(detail.clone()).build(window, cx) }))
                                .when(!requirement.ready, |row| row.child(Button::new(("setup-requirement", index)).ghost().small().label("Set up")
                                    .on_click(move |_, _, _| { let _ = open::that(url); })))
                        })))
                        .when(self.requirements.as_ref().is_some_and(|setup| !setup.ready()), |view| view.child(div().text_xs().text_color(theme.muted_foreground).child("Install missing tools, then recheck.")))
                        .child(h_flex().justify_end().child(Button::new("recheck-tools").ghost().small().label("Recheck").disabled(self.checking)
                            .on_click(cx.listener(|this, _, window, cx| this.check_requirements(window, cx))))))
                    .child(v_flex().gap_3().child(div().text_color(theme.muted_foreground).child("Practice source"))
                        .child(h_flex().gap_3().children(Source::ALL.into_iter().enumerate().map(|(index, source)| {
                            crate::theme::selected_choice(Button::new(("setup-source", index)).outline(), self.source == source, cx).flex_1()
                                .label(source.label()).on_click(cx.listener(move |this, _, _, cx| { this.source = source; cx.notify(); }))
                        })))))
                .when(current_step == 1, |view| view
                    .child(v_flex().gap_3().children([Account::LeetCode, Account::NeetCode].into_iter().map(|account| {
                        let workspace = self.workspace.clone();
                        let status = accounts[account.index()].clone();
                        let signed_in = status != "Signed out";
                        h_flex().w_full().p_3().gap_3().items_center().rounded_lg().border_1().border_color(theme.border)
                            .child(v_flex().flex_1().gap_1().child(account.label()).child(div().text_xs().text_color(theme.muted_foreground).child(status)))
                            .child(Button::new(("setup-account", account.index())).outline().label(if signed_in { "Log out" } else { "Log in" })
                                .on_click(move |_, window, cx| { let _ = workspace.update(cx, |_, cx| crate::accounts::open(account, signed_in, window, cx)); }))
                    })))
                    .child(v_flex().gap_2().child(div().child("Codeforces handle")).child(Input::new(&self.handle))
                        .when(self.source != Source::Codeforces, |view| view.child(div().text_xs().text_color(theme.muted_foreground).child("Optional; you can add it later.")))))
                .when_some(self.error.clone(), |view, error| view.child(div().text_color(theme.danger).child(error)))
                .child(h_flex().w_full().justify_between().items_center()
                    .child(Button::new("setup-back").ghost().small().icon(IconName::ArrowLeft).disabled(current_step == 0 || self.busy)
                        .accessibility_label("Previous setup step").tooltip("Back")
                        .on_click(cx.listener(|this, _, window, cx| { this.step = 0; this.check_requirements(window, cx); this.focus.focus(window, cx); cx.notify(); })))
                    .child(Button::new("setup-continue").primary().disabled(self.busy || (self.step == 0 && (self.checking || !self.requirements.as_ref().is_some_and(|requirements| requirements.ready()))))
                        .label(if self.busy { "Checking…" } else if current_step == 0 { "Continue" } else { "Start practicing" })
                        .tooltip("Continue · Ctrl+Enter").on_click(cx.listener(|this, _, window, cx| this.advance(window, cx)))))
                .child(div().text_xs().text_color(theme.muted_foreground).child(if current_step == 0 { "Ctrl+1…5 language · Ctrl+Enter continue" } else { "Connect accounts for judging and progress sync." })))
    }
}
