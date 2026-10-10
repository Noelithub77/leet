//! Optional prediction connection shared by setup, settings, and source editors.
use std::{cell::{Cell, RefCell}, path::PathBuf, rc::Rc, sync::Arc, time::Duration};
use anyhow::Result;
use gpui_kit::*;
use gpui_kit::prelude::FluentBuilder as _;
use practice::copilot::{Client, Status, Suggestion};

#[cfg(feature = "gui-test")]
#[path = "gui_test/copilot.rs"]
pub(crate) mod fixture;

pub struct Enabled(pub bool);
impl EventEmitter<Enabled> for Connection {}
pub struct Connection {
    client: Option<Arc<Client>>,
    enabled: bool,
    pub busy: bool,
    pub code: Option<String>,
    pub error: Option<String>,
    quota: Option<practice::copilot::Quota>,
    task: Task<()>,
}
impl Connection {
    pub fn new(enabled: bool) -> Self { Self { client: None, enabled, busy: false, code: None, error: None, quota: None, task: Task::ready(()) } }
    pub fn enabled(&self) -> bool { self.enabled }
    pub fn detail(&self) -> String {
        let mut detail = self.label();
        if self.enabled {
            detail.push('\n');
            detail.push_str(&self.quota.as_ref().map_or_else(|| "Usage not reported by Copilot".into(), |quota| quota.detail()));
            if let Some(message) = self.client.as_ref().and_then(|client| client.message()) { detail.push('\n'); detail.push_str(&message); }
        }
        detail.push_str("\nHold Alt to preview · Tab or Alt+L to accept"); detail
    }
    pub fn label(&self) -> String {
        if self.busy { if self.code.is_some() { "Finish sign-in in your browser" } else { "Connecting…" }.into() }
        else if let Some(error) = &self.error { error.clone() }
        else if !self.enabled { "Off".into() }
        else { match self.client.as_ref().map(|client| client.status()) {
            Some(Status::Ready) => "Connected · hold Alt to preview".into(),
            Some(Status::Unavailable(message)) => if message.is_empty() { "Sign-in required".into() } else { message },
            _ => "Starting…".into(),
        } }
    }
    pub fn disable(&mut self, cx: &mut Context<Self>) {
        self.task = Task::ready(()); self.busy = false; self.code = None; self.error = None; self.quota = None; self.enabled = false;
        if let Some(client) = self.client.take() { client.stop(); }
        cx.emit(Enabled(false)); cx.notify();
    }
    pub fn connect(&mut self, root: PathBuf, sign_in: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy { return; }
        self.busy = true; self.error = None; self.code = None;
        let existing = if sign_in { if let Some(client) = self.client.take() { client.stop(); } None } else { self.client.clone() };
        self.task = cx.spawn_in(window, async move |this, cx| {
            let result = cx.background_spawn(async move {
                if let Some(client) = existing { return Ok(client); }
                std::fs::create_dir_all(&root)?;
                let binary = if sign_in { practice::copilot::install::install(&practice::copilot::install::directory())? }
                    else { practice::copilot::install::binary().ok_or_else(|| anyhow::anyhow!("Connect Copilot in Settings to install its server"))? };
                Client::start(&binary, &root)
            }).await;
            let client = match result {
                Ok(client) => client,
                Err(error) => { let _ = this.update(cx, |this, cx| { this.busy = false; this.error = Some(error.to_string()); cx.notify(); }); return; }
            };
            let _ = this.update(cx, |this, cx| { this.client = Some(client.clone()); cx.notify(); });
            if sign_in {
                let login = client.clone();
                let result = cx.background_spawn(async move { login.sign_in().await }).await;
                match result {
                    Ok(login) => {
                        let _ = this.update(cx, |this, cx| { this.code = login.user_code; cx.notify(); });
                        if let Some(command) = login.command {
                            let login = client.clone();
                            let result = cx.background_spawn(async move { login.finish_sign_in(command).await }).await;
                            if let Err(error) = result { let _ = this.update(cx, |this, cx| { this.busy = false; this.error = Some(error.to_string()); cx.notify(); }); return; }
                        }
                    }
                    Err(error) => { let _ = this.update(cx, |this, cx| { this.busy = false; this.error = Some(error.to_string()); cx.notify(); }); return; }
                }
            }
            // Device flow may finish before the account status notification arrives.
            for _ in 0..100 {
                if matches!(client.status(), Status::Ready) { break; }
                cx.background_executor().timer(Duration::from_millis(100)).await;
            }
            let _ = this.update(cx, |this, cx| {
                this.busy = false; this.code = None;
                this.quota = client.quota();
                if sign_in {
                    match client.status() {
                        Status::Ready => { this.enabled = true; cx.emit(Enabled(true)); }
                        _ => this.error = Some(client.message().unwrap_or_else(|| "Copilot could not connect. Check your GitHub access and retry.".into())),
                    }
                }
                cx.notify();
            });
            let mut status = client.status();
            let mut quota = client.quota();
            let mut message = client.message();
            loop {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                let next = client.status();
                let next_quota = client.quota(); let next_message = client.message();
                if next != status || next_quota != quota || next_message != message {
                    status = next; quota = next_quota; message = next_message;
                    if this.update(cx, |this, cx| { this.quota = quota.clone(); cx.notify(); }).is_err() { break; }
                }
            }
        });
        cx.notify();
    }
}

pub struct Predictions {
    connection: Entity<Connection>,
    path: RefCell<Option<PathBuf>>,
    language: Cell<practice::language::Language>,
    pub alt: Cell<bool>,
    cache: RefCell<Option<Suggestion>>,
    preview: RefCell<Option<(String, gpui_kit::component::highlighter::SyntaxHighlighter)>>,
    generation: Cell<u64>,
    warm_task: RefCell<Task<()>>,
    #[cfg(feature = "gui-test")] pub fixture: RefCell<Option<Suggestion>>,
}
impl Predictions {
    pub fn new(connection: Entity<Connection>, language: practice::language::Language) -> Rc<Self> {
        Rc::new(Self { connection, path: RefCell::new(None), language: Cell::new(language), alt: Cell::new(false), cache: RefCell::new(None), preview: RefCell::new(None), generation: Cell::new(0), warm_task: RefCell::new(Task::ready(())),
            #[cfg(feature = "gui-test")] fixture: RefCell::new(None) })
    }
    pub fn configure(&self, path: Option<PathBuf>, language: practice::language::Language) {
        if *self.path.borrow() != path || self.language.get() != language { self.invalidate(); *self.path.borrow_mut() = path; self.language.set(language); }
    }
    pub fn invalidate(&self) { self.generation.set(self.generation.get().wrapping_add(1)); self.cache.borrow_mut().take(); self.preview.borrow_mut().take(); *self.warm_task.borrow_mut() = Task::ready(()); }
    fn cache(&self, suggestion: Suggestion, source: &str, offset: usize) {
        let prefix = source[..offset].rsplit('\n').next().unwrap_or("");
        let code = format!("{prefix}{}", suggestion.text());
        let mut highlighter = gpui_kit::component::highlighter::SyntaxHighlighter::new(self.language.get().id());
        highlighter.update(None, &ropey::Rope::from_str(&code), None);
        *self.preview.borrow_mut() = Some((code, highlighter));
        *self.cache.borrow_mut() = Some(suggestion);
    }
    pub fn warm(self: &Rc<Self>, text: String, offset: usize, pane: WeakEntity<crate::snippets::expansion::EditorPane>, cx: &mut App) {
        if self.alt.get() || !self.connection.read(cx).enabled() { return; }
        let this = self.clone(); let executor = cx.background_executor().clone();
        *self.warm_task.borrow_mut() = cx.spawn(async move |cx| {
            executor.timer(Duration::from_millis(150)).await;
            let task = cx.update(|cx| this.fetch(text, offset, cx));
            let _ = task.await;
            let _ = pane.update(cx, |_, cx| cx.notify());
        });
    }
    pub fn cancel_warm(&self) { *self.warm_task.borrow_mut() = Task::ready(()); }
    pub fn current(&self, text: &str, offset: usize, cx: &App) -> Option<Suggestion> {
        (self.alt.get() && self.connection.read(cx).enabled()).then(|| self.cache.borrow().as_ref().filter(|item| item.matches(text, offset)).cloned()).flatten()
    }
    pub fn accepted(&self, suggestion: Suggestion, cx: &mut App) {
        if let Some(client) = self.connection.read(cx).client.clone() { cx.background_spawn(async move { let _ = client.accepted(suggestion).await; }).detach(); }
        self.invalidate();
    }
    pub fn fetch(self: &Rc<Self>, text: String, offset: usize, cx: &mut App) -> Task<Result<lsp_types::InlineCompletionResponse>> {
        let empty = || Task::ready(Ok(lsp_types::InlineCompletionResponse::Array(vec![])));
        if !self.connection.read(cx).enabled() { return empty(); }
        let cached = self.cache.borrow().as_ref().filter(|item| item.matches(&text, offset)).cloned();
        if let Some(suggestion) = cached {
            if self.alt.get() {
                if let Some(client) = self.connection.read(cx).client.as_ref() { client.shown(&suggestion); }
                return Task::ready(Ok(inline(&suggestion)));
            }
            return empty();
        }
        #[cfg(feature = "gui-test")]
        if let Some(suggestion) = self.fixture.borrow().as_ref().filter(|item| item.matches(&text, offset)).cloned() {
            self.cache(suggestion.clone(), &text, offset);
            return Task::ready(Ok(if self.alt.get() { inline(&suggestion) } else { lsp_types::InlineCompletionResponse::Array(vec![]) }));
        }
        let Some(client) = self.connection.read(cx).client.clone() else { return empty(); };
        let Some(path) = self.path.borrow().clone() else { return empty(); };
        let language = self.language.get(); let this = self.clone();
        let generation = self.generation.get().wrapping_add(1); self.generation.set(generation);
        let task = cx.background_spawn(async move { client.predict(&path, language, &text, offset).await.map(|suggestion| (client, suggestion, text)) });
        cx.spawn(async move |cx| {
            let (client, suggestion, text) = task.await?;
            if this.generation.get() != generation { return Ok(lsp_types::InlineCompletionResponse::Array(vec![])); }
            let response = match suggestion {
                Some(suggestion) => {
                    let visible = this.alt.get() && cx.update(|cx| this.connection.read(cx).enabled());
                    if visible { client.shown(&suggestion); }
                    let response = if visible { inline(&suggestion) } else { lsp_types::InlineCompletionResponse::Array(vec![]) };
                    this.cache(suggestion, &text, offset); response
                }
                None => lsp_types::InlineCompletionResponse::Array(vec![]),
            };
            Ok(response)
        })
    }
}

pub(crate) fn preview_hint(pane: &crate::snippets::expansion::EditorPane, window: &Window, cx: &App) -> Option<AnyElement> {
    use gpui_kit::component::{ActiveTheme as _, Icon, h_flex};
    let editor = pane.state.read(cx);
    if !editor.focus_handle(cx).is_focused(window) || !window.is_window_active() { return None; }
    let predictions = pane.predictions.as_ref()?;
    if !predictions.connection.read(cx).enabled() || !editor.selected_range().is_empty() { return None; }
    predictions.cache.borrow().as_ref().filter(|item| item.matches(editor.value().as_str(), editor.cursor()))?;
    let theme = cx.theme();
    Some(h_flex().id("copilot-preview-hint").test_support().gap_2().px_2().py_1().rounded_md()
        .bg(theme.popover).text_xs().text_color(theme.muted_foreground)
        .child(Icon::default().path("providers/copilot.svg").size(px(14.)))
        .child(if predictions.alt.get() { "Tab or Alt+L" } else { "Hold Alt" }).into_any_element())
}

pub(crate) fn preview_overlay(pane: &crate::snippets::expansion::EditorPane, window: &Window, cx: &App) -> Option<AnyElement> {
    use gpui_kit::component::ActiveTheme as _;
    let hint = preview_hint(pane, window, cx)?;
    let editor = pane.state.read(cx);
    let (cursor, line_height) = editor.cursor_layout()?;
    let input = editor.input_bounds();
    let position = editor.scroll_offset() + cursor.origin - input.origin;
    let predictions = pane.predictions.as_ref()?;
    let expanded = predictions.alt.get();
    let width = if expanded { px(480.).min(input.size.width) } else { px(150.) };
    let left = position.x.max(px(0.)).min((input.size.width - width).max(px(0.)));
    let below = position.y + line_height + px(4.);
    let height = if expanded { px(240.).min(input.size.height) } else { px(30.) };
    let top = if below + height > input.size.height { (position.y - height - px(4.)).max(px(0.)) } else { below };
    let theme = cx.theme();
    let preview = predictions.preview.borrow();
    let code = if expanded {
        preview.as_ref().map(|(code, highlighter)| div().id("copilot-full-preview").test_support()
            .overflow_y_scroll().max_h((height - px(30.)).max(px(30.))).p_2().font_family(theme.mono_font_family.clone()).text_xs()
            .child(div().id("copilot-preview-code").test_support()
                .child(StyledText::new(code.clone()).with_highlights(highlighter.styles(&(0..code.len()), theme.highlight_theme.as_ref())))))
    } else { None };
    Some(deferred(div().absolute().left(left).top(top).rounded_md().bg(theme.popover)
        .when(expanded, |view| view.w(width).border_1().border_color(theme.border).shadow_md())
        .child(hint).children(code)).into_any_element())
}
fn inline(suggestion: &Suggestion) -> lsp_types::InlineCompletionResponse {
    lsp_types::InlineCompletionResponse::Array(vec![lsp_types::InlineCompletionItem { insert_text: suggestion.text().into(), filter_text: None, range: None, command: None, insert_text_format: None }])
}

pub fn status(ws: &crate::workspace::Workspace, cx: &mut Context<crate::workspace::Workspace>) -> impl IntoElement {
    use gpui_kit::component::{Sizable as _, button::{Button, ButtonVariants as _}};
    Button::new("status-copilot").ghost().xsmall()
        .icon(gpui_kit::component::Icon::default().path("providers/copilot.svg").xsmall())
        .accessibility_label("Copilot").tooltip(ws.copilot.read(cx).detail())
        .on_click(cx.listener(|ws, _, window, cx| ws.open_settings(Some(crate::settings::Setting::Copilot), window, cx)))
}

pub fn controls(connection: &Entity<Connection>, root: PathBuf, cx: &App) -> AnyElement {
    use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
    use gpui_kit::component::button::{Button, ButtonVariants as _};
    use gpui_kit::prelude::FluentBuilder as _;
    let state = connection.read(cx);
    let busy = state.busy; let enabled = state.enabled; let code = state.code.clone();
    let label = state.label(); let error = state.error.is_some();
    let connection = connection.downgrade();
    v_flex().id("copilot-setup").test_support().gap_3()
        .child(div().text_sm().text_color(if error { cx.theme().danger } else { cx.theme().muted_foreground }).child(label))
        .when_some(code, |view, code| view.child(h_flex().gap_3()
            .child(div().font_family(cx.theme().mono_font_family.clone()).child(code.clone()))
            .child(Button::new("copilot-copy-code").ghost().small().label("Copy code").on_click(move |_, _, cx| { cx.stop_propagation(); cx.write_to_clipboard(ClipboardItem::new_string(code.clone())); }))))
        .child(Button::new("copilot-connect").outline().label(if busy { "Cancel" } else if enabled { "Turn off" } else { "Connect Copilot" })
            .on_click(move |_, window, cx| { cx.stop_propagation(); let _ = connection.update(cx, |connection, cx| {
                if busy || enabled { connection.disable(cx); } else { connection.connect(root.clone(), true, window, cx); }
            }); }))
        .into_any_element()
}
