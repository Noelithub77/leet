//! Workspace-owned private tool installation survives navigation and serializes languages.
use std::collections::{HashMap, VecDeque};
use futures::StreamExt as _;
use gpui_kit::*;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::assets::IconName;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::spinner::Spinner;
use practice::language::Language;
use crate::workspace::{Center, Workspace};

#[derive(Clone, Debug)]
enum Phase { Queued, Installing(String), Ready, Failed(String) }

#[derive(Default)]
pub struct State {
    phases: HashMap<Language, Phase>,
    queue: VecDeque<Language>,
    active: Option<Language>,
    last: Option<Language>,
    elapsed: u64,
    ticker: Option<Task<()>>,
    #[cfg(feature = "gui-test")]
    pub(crate) fixture: bool,
}

fn tool_language(language: Language) -> Language { if language == Language::C { Language::Cpp } else { language } }

impl State {
    fn request(&mut self, language: Language, retry: bool) {
        let language = tool_language(language);
        if !practice::tool_setup::supported(language) || self.active == Some(language) || self.queue.contains(&language) { return; }
        if !retry && self.phases.contains_key(&language) { return; }
        self.phases.insert(language, Phase::Queued);
        self.queue.push_back(language);
    }
    fn next(&mut self) -> Option<Language> {
        if self.active.is_some() { return None; }
        let language = self.queue.pop_front()?;
        self.active = Some(language);
        self.last = Some(language);
        self.elapsed = 0;
        self.phases.insert(language, Phase::Installing("Setting up".into()));
        Some(language)
    }
    pub(crate) fn busy(&self, language: Language) -> bool {
        matches!(self.phases.get(&tool_language(language)), Some(Phase::Queued | Phase::Installing(_)))
    }
}

pub(crate) enum Event { Progress(String), Finished(Result<(), String>) }

impl Workspace {
    pub(crate) fn setup_tools(&mut self, language: Language, retry: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.tool_install.request(language, retry);
        self.start_tool_install(window, cx);
        cx.notify();
    }
    fn start_tool_install(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(language) = self.tool_install.next() else { return };
        #[cfg(feature = "gui-test")]
        if self.tool_install.fixture { return; }
        self.tool_install.ticker = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(std::time::Duration::from_secs(1)).await;
                if !matches!(this.update(cx, |this, cx| {
                    if this.tool_install.active.is_none() { return false; }
                    this.tool_install.elapsed += 1; cx.notify(); true
                }), Ok(true)) { break; }
            }
        }));
        let (tx, mut rx) = futures::channel::mpsc::unbounded();
        std::thread::spawn(move || {
            let result = practice::tool_setup::install(language, &practice::tool_setup::directory(), |line| {
                let _ = tx.unbounded_send(Event::Progress(line));
            }).map_err(|error| error.to_string());
            let _ = tx.unbounded_send(Event::Finished(result));
        });
        cx.spawn_in(window, async move |this, cx| {
            while let Some(event) = rx.next().await {
                if this.update_in(cx, |this, window, cx| this.tool_install_event(language, event, window, cx)).is_err() { break; }
            }
        }).detach();
    }
    pub(crate) fn tool_install_event(&mut self, language: Language, event: Event, window: &mut Window, cx: &mut Context<Self>) {
        if self.tool_install.active != Some(language) { return; }
        match event {
            Event::Progress(line) => {
                if ["Downloading", "Installing", "Checking", "Verifying", "Unpacking", "Preparing"].iter().any(|prefix| line.starts_with(prefix)) {
                    self.tool_install.phases.insert(language, Phase::Installing(line.chars().filter(|character| !character.is_control()).take(180).collect()));
                }
            }
            Event::Finished(result) => {
                let installed = result.is_ok();
                self.tool_install.phases.insert(language, match result { Ok(()) => Phase::Ready, Err(error) => Phase::Failed(error) });
                self.tool_install.active = None;
                #[cfg(feature = "gui-test")]
                let installed = installed && !self.tool_install.fixture;
                if installed {
                    self.suspend_language_servers(cx);
                    self.warm_language_server(self.config.preferred_language, window, cx);
                    if self.center == Center::Editor { self.attach_language_server(window, cx); }
                    if self.debug_mode { self.debugger.update(cx, |debugger, cx| debugger.tools_installed(language, cx)); }
                    if let Some(setup) = self.onboarding.clone() { setup.update(cx, |setup, cx| setup.tools_installed(window, cx)); }
                }
                self.start_tool_install(window, cx);
            }
        }
        cx.notify();
    }
}

pub(crate) fn status(ws: &Workspace, cx: &mut Context<Workspace>) -> Option<AnyElement> {
    let selected = tool_language(ws.session.as_ref().map_or(ws.config.preferred_language, |session| session.language));
    let language = ws.tool_install.active
        .or_else(|| Language::ALL.into_iter().find(|language| matches!(ws.tool_install.phases.get(language), Some(Phase::Failed(_)))))
        .or_else(|| ws.tool_install.phases.contains_key(&selected).then_some(selected)).or(ws.tool_install.last)?;
    let phase = ws.tool_install.phases.get(&language)?;
    let name = if language == Language::Cpp { "C/C++" } else { language.label() };
    let (text, icon, color, detail) = match phase {
        Phase::Queued => ("Queued", IconName::Clock, cx.theme().muted_foreground, "Waiting for the current tool setup".to_owned()),
        Phase::Installing(stage) => (stage.as_str(), IconName::Download, cx.theme().muted_foreground, format!("{name} · {stage}")),
        Phase::Ready => ("Ready", IconName::Check, cx.theme().success, format!("{name} tools are ready")),
        Phase::Failed(error) => ("Retry", IconName::TriangleAlert, cx.theme().warning, error.clone()),
    };
    let label = format!("{name} tools · {text}");
    if matches!(phase, Phase::Failed(_)) {
        return Some(Button::new("tool-setup-retry").ghost().xsmall().icon(icon).label(label).tooltip(detail)
            .on_click(cx.listener(move |this, _, window, cx| this.setup_tools(language, true, window, cx))).into_any_element());
    }
    Some(h_flex().id("tool-setup-status").test_support().min_w_0().gap_1().text_color(color)
        .child(if matches!(phase, Phase::Installing(_)) { Spinner::new().xsmall().into_any_element() }
            else { gpui_kit::component::Icon::new(icon).xsmall().into_any_element() })
        .child(div().id("tool-setup-label").test_support().min_w_0().max_w(rems(20.)).truncate().aria_label(label.clone()).child(label))
        .when(matches!(phase, Phase::Installing(_)), |view| view.child(format!("{} s", ws.tool_install.elapsed)))
        .when(!ws.tool_install.queue.is_empty(), |view| view.child(format!("+{} queued", ws.tool_install.queue.len())))
        .tooltip(move |window, cx| gpui_kit::component::tooltip::Tooltip::new(detail.clone()).build(window, cx)).into_any_element())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[::core::prelude::v1::test]
    fn requests_serialize_deduplicate_and_retry_only_explicitly() {
        let mut state = State::default();
        state.request(Language::Python, false);
        state.request(Language::Python, false);
        assert_eq!(state.next(), Some(Language::Python));
        state.request(Language::Cpp, false);
        state.request(Language::C, false);
        assert_eq!(state.queue.len(), 1);
        assert_eq!(state.next(), None);
        state.active = None;
        state.phases.insert(Language::Python, Phase::Failed("offline".into()));
        state.request(Language::Python, false);
        assert_eq!(state.next(), Some(Language::Cpp));
        state.request(Language::Python, true);
        assert_eq!(state.queue.front(), Some(&Language::Python));
        state.request(Language::Java, false);
        assert_eq!(state.queue.len(), 2);
    }
}
