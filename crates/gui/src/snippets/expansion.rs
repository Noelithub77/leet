//! Source-editor expansion, placeholder navigation, and native completions.
use gpui_kit::*;
use gpui_kit::component::input::{Editor, EditorState, InputEvent, CompletionProvider};
use gpui_kit::base::input::{EditorMode, InputModeKind};
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::prelude::FluentBuilder as _;
use practice::{language::Language, snippets::{Snippet, body, session::Session, search::Library}};
use std::{ops::Range, rc::Rc, time::Duration};

gpui_kit::actions!(snippet_expansion, [NextStop, PreviousStop, CancelSnippet, PickerUp, PickerDown, InsertPicked]);
pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("tab", NextStop, Some("SnippetSource > Input")),
        KeyBinding::new("shift-tab", PreviousStop, Some("SnippetSource > Input")),
        KeyBinding::new("escape", CancelSnippet, Some("SnippetSource > Input")),
        KeyBinding::new("tab", NextStop, Some("SnippetPicker > Input")),
        KeyBinding::new("up", PickerUp, Some("SnippetPicker > Input")),
        KeyBinding::new("down", PickerDown, Some("SnippetPicker > Input")),
        KeyBinding::new("enter", InsertPicked, Some("SnippetPicker > Input")),
        KeyBinding::new("escape", CancelSnippet, Some("SnippetPicker > Input")),
    ]);
}
struct AcceptedPrediction {
    before: String,
    after: String,
    selection: Range<usize>,
    suggestion: practice::copilot::Suggestion,
}
pub struct EditorPane {
    pub state: Entity<EditorState>,
    pub language: Language,
    pub library: Rc<Library>,
    pub tab_expand: bool,
    pub path: Option<std::path::PathBuf>,
    pub workspace: Option<std::path::PathBuf>,
    session: Option<Session>,
    pub(super) completions: Rc<super::completion::Completions>,
    pub(super) menu: Option<super::menu::Menu>,
    pub(crate) changing: bool,
    pub(crate) notes: crate::notes::Notes,
    epoch: u64,
    pub(crate) predictions: Option<Rc<crate::copilot::Predictions>>,
    accepted_prediction: Option<AcceptedPrediction>,
    pub(super) language_document: Option<std::sync::Arc<practice::lsp::Document>>,
    pub(super) signature: super::signature_help::Hints,
    detail_task: Task<()>,
    _subscriptions: Vec<Subscription>,
}
impl EditorPane {
    pub fn new(state: Entity<EditorState>, language: Language, library: Rc<Library>, notes_view: practice::notes::View, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let completions = Rc::new(super::completion::Completions::new(language, cx.weak_entity(), library.clone()));
        state.update(cx, |editor, _| editor.lsp_mut().completion_provider = Some(completions.clone()));
        let observe = cx.observe(&state, |this, editor, cx| {
            let content = editor.read(cx).completion_menu_state();
            if content.open {
                let mut content = content.clone();
                if let Some((source, offset, items)) = this.completions.published.borrow().as_ref() {
                    if editor.read(cx).cursor() == *offset && editor.read(cx).value().as_str() == source { content.items.clone_from(items); }
                }
                this.set_menu(content.clone(), editor.read(cx).cursor());
                this.resolve_detail(cx);
                editor.update(cx, |editor, cx| editor.dismiss_completion_overlay(cx));
            }
            this.refresh_signature(false, cx);
            cx.notify();
        });
        let changes = cx.subscribe_in(&state, window, |this, editor, event, window, cx| {
            if matches!(event, InputEvent::Focus) { this.signature.focused = true; this.signature.input = None; this.refresh_signature(false, cx); }
            if matches!(event, InputEvent::Blur) { this.signature.focused = false; this.hide_signature(false, cx); }
            if matches!(event, InputEvent::VimChanged) {
                if editor.read(cx).vim_mode().is_some_and(|mode| mode != gpui_kit::base::input::VimMode::Insert) {
                    this.session = None;
                    this.completions.cancel();
                    this.menu = None;
                    this.detail_task = Task::ready(());
                    this.hide_prediction(cx);
                    this.hide_signature(false, cx);
                    editor.update(cx, |editor, cx| editor.dismiss_lsp_overlays(cx));
                }
                cx.notify();
                return;
            }
            if !matches!(event, InputEvent::Change) || this.changing { return; }
            let text = editor.read(cx).value().to_string();
            if this.accepted_prediction.as_ref().is_some_and(|accepted| accepted.after != text) { this.accepted_prediction = None; }
            if let Some(predictions) = &this.predictions { predictions.invalidate(); }
            this.menu = None;
            this.detail_task = Task::ready(());
            let selection = editor.read(cx).selected_range();
            if let Some(predictions) = &this.predictions {
                predictions.configure(this.path.clone(), this.language);
                predictions.warm(text.clone(), editor.read(cx).cursor(), cx.weak_entity(), cx);
            }
            if let Some(session) = &mut this.session {
                match session.changed(&text) {
                    Some(edits) => {
                        this.changing = true;
                        let mut selection = selection;
                        for (range, value) in edits {
                            let delta = value.len() as isize - (range.end - range.start) as isize;
                            if range.end <= selection.start { selection.start = selection.start.saturating_add_signed(delta); selection.end = selection.end.saturating_add_signed(delta); }
                            editor.update(cx, |e, cx| { e.set_selected_range(range, cx); e.replace(value, window, cx); });
                        }
                        editor.update(cx, |e, cx| e.set_selected_range(selection, cx));
                        this.changing = false;
                    }
                    None => this.session = None,
                }
            }
            cx.notify();
        });
        let notes = cx.subscribe(&state, |this, _, event, cx| if matches!(event, InputEvent::Change) { this.notes_changed(cx); });
        let focused = state.read(cx).focus_handle(cx).is_focused(window);
        Self { state, language, library, tab_expand: true, path: None, workspace: None, session: None, completions, menu: None, changing: false, notes: crate::notes::Notes::new(notes_view, language), epoch: 0, predictions: None, accepted_prediction: None, language_document: None, signature: super::signature_help::Hints::new(language, focused), detail_task: Task::ready(()), _subscriptions: vec![observe, changes, notes] }
    }

    pub(crate) fn connect_language_document(&mut self, language: Language, document: std::sync::Arc<practice::lsp::Document>, cx: &mut Context<Self>) {
        self.language = language;
        self.language_document = Some(document);
        self.hide_signature(false, cx); self.signature.input = None;
        self.sync_completions(cx);
        self.sync_note_providers(cx);
        self.refresh_signature(false, cx);
    }

    pub(crate) fn connect_predictions(&mut self, connection: Entity<crate::copilot::Connection>, window: &mut Window, cx: &mut Context<Self>) {
        let predictions = crate::copilot::Predictions::new(connection.clone(), self.language);
        *self.completions.predictions.borrow_mut() = Some(predictions.clone());
        self.predictions = Some(predictions);
        self._subscriptions.push(cx.observe(&connection, |this, connection, cx| {
            if !connection.read(cx).enabled() {
                this.hide_prediction(cx);
                if let Some(predictions) = &this.predictions { predictions.invalidate(); }
            }
            cx.notify();
        }));
        let focus = self.state.read(cx).focus_handle(cx);
        self._subscriptions.push(cx.on_focus_out(&focus, window, |this, _, _, cx| this.hide_prediction(cx)));
        self._subscriptions.push(cx.observe_window_activation(window, |this, window, cx| {
            if !window.is_window_active() { this.hide_prediction(cx); }
        }));
    }
    pub(super) fn hide_prediction(&mut self, cx: &mut Context<Self>) {
        if let Some(predictions) = &self.predictions { predictions.alt.set(false); }
        self.state.update(cx, |editor, cx| EditorMode::clear_inline_completion(editor, cx));
        cx.notify();
    }
    fn prediction_modifiers(&mut self, event: &ModifiersChangedEvent, window: &mut Window, cx: &mut Context<Self>) {
        let alt = event.modifiers.alt && !event.modifiers.control && !event.modifiers.platform && self.state.read(cx).focus_handle(cx).is_focused(window);
        let Some(predictions) = &self.predictions else { return; };
        if predictions.alt.replace(alt) == alt { return; }
        if !alt { self.hide_prediction(cx); return; }
        predictions.cancel_warm();
        predictions.configure(self.path.clone(), self.language);
        if self.menu.is_none() {
            self.completions.cancel();
            self.detail_task = Task::ready(());
        }
        self.state.update(cx, |editor, cx| {
            editor.dismiss_completion_overlay(cx);
            let offset = editor.cursor();
            EditorMode::on_text_typed(editor, &(offset..offset), "", window, cx);
        });
        cx.notify();
    }
    fn accept_prediction(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if !EditorMode::has_inline_completion(self.state.read(cx)) { return false; }
        let Some(predictions) = &self.predictions else { return false; };
        let editor = self.state.read(cx);
        let Some(suggestion) = predictions.current(editor.value().as_str(), editor.cursor(), cx) else { return false; };
        let before = editor.value().to_string();
        let selection = editor.selected_range();
        predictions.accepted(suggestion.clone(), cx);
        self.state.update(cx, |editor, cx| {
            EditorMode::clear_inline_completion(editor, cx);
            editor.set_selected_range(suggestion.range(), cx);
            editor.replace(suggestion.text(), window, cx);
        });
        self.accepted_prediction = Some(AcceptedPrediction { before, after: self.state.read(cx).value().to_string(), selection, suggestion });
        self.menu = None;
        cx.notify(); true
    }
    pub(crate) fn can_revert_prediction(&self, cx: &App) -> bool {
        self.accepted_prediction.as_ref().is_some_and(|accepted| self.state.read(cx).value().as_str() == accepted.after)
    }
    fn revert_prediction(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if !self.can_revert_prediction(cx) { return false; }
        let Some(accepted) = self.accepted_prediction.take() else { return false; };
        self.hide_prediction(cx);
        if let Some(predictions) = &self.predictions { predictions.invalidate(); }
        self.changing = true;
        self.state.update(cx, |editor, cx| {
            editor.set_selected_range(0..accepted.after.len(), cx);
            editor.replace(accepted.before.clone(), window, cx);
            editor.set_selected_range(accepted.selection, cx);
        });
        self.changing = false;
        self.menu = None;
        self.completions.cancel();
        self.detail_task = Task::ready(());
        if let Some(predictions) = &self.predictions {
            predictions.restore(accepted.suggestion, &accepted.before, self.state.read(cx).cursor());
            self.state.update(cx, |editor, cx| {
                let offset = editor.cursor();
                EditorMode::on_text_typed(editor, &(offset..offset), "", window, cx);
            });
        }
        cx.notify(); true
    }

    fn prefix(&self, cx: &App) -> Range<usize> {
        let e = self.state.read(cx); let text = e.value(); let end = e.cursor();
        if let Some(prefix) = self.library.iter().filter(|s|s.applies_to(self.language)).flat_map(|s|&s.prefixes).filter(|p| {
            if p.is_empty() || !text[..end].ends_with(p.as_str()) { return false; }
            let before=&text[..end-p.len()];
            !p.chars().next().is_some_and(|c|c.is_alphanumeric()||c=='_') || before.chars().next_back().is_none_or(|c|!c.is_alphanumeric()&&c!='_')
        }).max_by_key(|p|p.len()) { return end-prefix.len()..end; }
        let start = text[..end].char_indices().rev().find(|(_,c)| !c.is_alphanumeric() && *c != '_').map_or(0, |(i,c)| i + c.len_utf8());
        start..end
    }
    fn matching_prefix(&self, cx: &App) -> Vec<Snippet> {
        let range = self.prefix(cx); let text = self.state.read(cx).value();
        if range.is_empty() { return Vec::new(); }
        self.library.iter().filter(|s| s.applies_to(self.language) && s.prefixes.iter().any(|p| p == &text[range.clone()])).cloned().collect()
    }
    pub fn insert(&mut self, snippet: &Snippet, range: Range<usize>, window: &mut Window, cx: &mut Context<Self>) {
        self.insert_body(&snippet.body, range, window, cx);
    }
    fn insert_body(&mut self, snippet_body: &str, range: Range<usize>, window: &mut Window, cx: &mut Context<Self>) {
        self.completions.cancel();
        self.menu = None;
        let text = self.state.read(cx).value().to_string();
        if range.end > text.len() { return; }
        let line_start = text[..range.start].rfind('\n').map_or(0, |i| i+1);
        let line = text[line_start..].split('\n').next().unwrap_or_default();
        let context = body::Context { path: self.path.clone(), workspace: self.workspace.clone(), selected: self.state.read(cx).selected_value().to_string(), clipboard: cx.read_from_clipboard().and_then(|c| c.text()).unwrap_or_default(), language: Some(self.language), line: line.into(), line_index: text[..line_start].bytes().filter(|b| *b == b'\n').count(), word: text[range.clone()].into(), indent: line.chars().take_while(|c| c.is_whitespace()).collect(), unit: "    ".into(), ..Default::default() };
        let expansion = body::expand(snippet_body, &context);
        if expansion.truncated { return; }
        self.changing = true;
        self.state.update(cx, |e, cx| { e.dismiss_lsp_overlays(cx); e.set_selected_range(range.clone(), cx); e.replace(expansion.text.clone(), window, cx); e.focus(window, cx); });
        let document = self.state.read(cx).value().to_string();
        self.activate(expansion, range.start, document, cx);
        self.state.update(cx, |e, cx| e.focus(window, cx));
    }
    pub fn activate(&mut self, expansion: body::Expansion, offset: usize, document: String, cx: &mut Context<Self>) {
        let session = Session::new(expansion, offset, document);
        let first = session.range();
        self.state.update(cx, |e, cx| e.set_selected_range(first, cx));
        self.session = (session.stops[0].index != 0).then_some(session);
        self.changing = false; self.epoch += 1; cx.notify();
    }
    fn navigate(&mut self, backwards: bool, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(session) = &mut self.session {
            self.menu = None;
            self.state.update(cx, |editor, cx| editor.dismiss_completion_overlay(cx));
            let selection = self.state.read(cx).selected_range(); let active = session.range();
            if selection.start < active.start || selection.end > active.end { self.session = None; cx.notify(); return; }
            let range = session.next(backwards);
            let done = session.stops[session.active].index == 0;
            if let Some(range) = range { self.state.update(cx, |e, cx| e.set_selected_range(range, cx)); }
            if done || self.session.as_ref().is_some_and(|s| s.active >= s.stops.len()) { self.session = None; }
            self.epoch += 1; cx.notify(); return;
        }
        if self.menu.is_some() && !backwards { self.accept_completion(window, cx); return; }
        let candidates = self.matching_prefix(cx);
        if candidates.len() == 1 { self.insert(&candidates[0], self.prefix(cx), window, cx); }
        else { self.open_completions(window, cx); }
    }
    pub fn open_completions(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.signature.focused = self.state.read(cx).focus_handle(cx).is_focused(window);
        self.refresh_signature(true, cx);
        self.sync_completions(cx);
        let editor = self.state.read(cx);
        let snapshot = editor.value().to_string(); let text = editor.text().clone(); let offset = editor.cursor();
        let range = self.prefix(cx);
        let task = self.completions.completions(&text, offset, lsp_types::CompletionContext {
            trigger_kind: lsp_types::CompletionTriggerKind::INVOKED, trigger_character: None
        }, window, cx);
        self.state.update(cx, |editor, cx| editor.focus(window, cx));
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(response) = task.await {
                let _ = this.update(cx, |this, cx| {
                    let editor = this.state.read(cx);
                    if editor.value().as_str() != snapshot || editor.cursor() != offset { return; }
                    let items = match response { lsp_types::CompletionResponse::Array(items) => items, lsp_types::CompletionResponse::List(list) => list.items };
                    this.state.update(cx, |editor, cx| editor.present_completion_items(range.start, &snapshot[range], items, cx));
                });
            }
        }).detach();
    }
    pub(super) fn accept_completion(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(menu) = self.menu.take() else { return; };
        let Some(item) = menu.content.items.get(menu.selected) else { return; };
        if self.state.read(cx).cursor() != menu.offset { return; }
        let expansion = self.completions.pending.borrow_mut().take().and_then(|pending| pending.accepted(item, self.state.read(cx).value().as_str()));
        self.completions.cancel();
        if item.kind == Some(lsp_types::CompletionItemKind::SNIPPET) && expansion.is_none() { cx.notify(); return; }
        if let Some((range, snippet_body)) = expansion {
            self.insert_body(&snippet_body, range, window, cx);
        } else {
            self.changing = true;
            self.state.update(cx, |editor, cx| editor.insert_completion(item, menu.content.trigger_start_offset.unwrap_or(menu.offset)..menu.offset, window, cx));
        }
        self.changing = false;
        cx.notify();
    }
    pub(super) fn close_completions(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.completions.cancel();
        self.detail_task = Task::ready(());
        self.menu = None;
        self.state.update(cx, |editor, cx| { editor.dismiss_lsp_overlays(cx); editor.focus(window, cx); });
        cx.notify();
    }
    fn set_menu(&mut self, content: gpui_kit::base::input::CompletionMenuState, offset: usize) {
        if let Some(menu) = self.menu.as_mut().filter(|menu| menu.offset == offset) {
            menu.update(content);
        } else { self.menu = Some(super::menu::Menu::new(content, offset)); }
    }
    fn resolve_detail(&mut self, cx: &mut Context<Self>) {
        self.detail_task = Task::ready(());
        let Some(menu) = &self.menu else { return; };
        let Some(item) = menu.content.items.get(menu.selected).filter(|item| item.kind != Some(lsp_types::CompletionItemKind::SNIPPET)
            && item.detail.as_deref() != Some("Defined in this file") && item.documentation.is_none()).cloned() else { return; };
        let Some(document) = self.language_document.clone().filter(|document| document.server.supports_completion_resolve()) else { return; };
        let text = self.state.read(cx).value().to_string(); let offset = menu.offset;
        self.detail_task = cx.spawn(async move |this, cx| {
            let original = item.clone();
            let result = cx.background_spawn(async move {
                let wire: lsp_wire::CompletionItem = serde_json::from_value(serde_json::to_value(item)?)?;
                let item = document.server.request::<lsp_wire::request::ResolveCompletionItem>(wire).await?;
                let item: lsp_types::CompletionItem = serde_json::from_value(serde_json::to_value(item)?)?;
                anyhow::Ok(item)
            }).await;
            if let Ok(resolved) = result {
                let _ = this.update(cx, |pane,cx| {
                    if pane.state.read(cx).value().as_str() != text || pane.state.read(cx).cursor() != offset { return; }
                    if let Some(menu) = &mut pane.menu {
                        if let Some(item) = menu.content.items.get_mut(menu.selected).filter(|item| **item == original) {
                            pane.completions.cache_documentation(&original, &resolved);
                            item.documentation = resolved.documentation;
                            item.detail = resolved.detail;
                            cx.notify();
                        }
                    }
                });
            }
        });
    }
    pub(super) fn merge_completions(&mut self, start: usize, query: &str, items: Vec<lsp_types::CompletionItem>, cx: &mut Context<Self>) {
        if items.is_empty() { return; }
        let offset = self.state.read(cx).cursor();
        let mut content = self.menu.as_ref().map(|menu| menu.content.clone()).unwrap_or_default();
        content.items = items;
        content.open = true;
        content.trigger_start_offset = Some(start);
        content.query = query.into();
        self.set_menu(content, offset);
        self.resolve_detail(cx);
        cx.notify();
    }
    fn sync_completions(&mut self, cx: &mut App) {
        if let Some(predictions) = &self.predictions { predictions.configure(self.path.clone(), self.language); }
        let mut catalog = self.completions.catalog.borrow_mut();
        if !Rc::ptr_eq(&catalog.library, &self.library) || catalog.language != self.language { self.completions.cancel(); self.menu = None; catalog.library = self.library.clone(); }
        catalog.language = self.language;
        catalog.selection = self.state.read(cx).selected_range();
        drop(catalog);
        let provider: Rc<dyn CompletionProvider> = self.completions.clone();
        let current = self.state.read(cx).lsp().completion_provider.clone();
        if current.as_ref().is_none_or(|current| !Rc::ptr_eq(current, &provider)) {
            if current.is_none() { self.language_document = None; }
            self.completions.set_upstream(current);
            self.state.update(cx, |editor, _| editor.lsp_mut().completion_provider = Some(provider));
        }
    }
}

impl Render for EditorPane {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.language = Language::ALL.into_iter().find(|l| l.id()==self.state.read(cx).language_name().as_str()).unwrap_or(self.language);
        self.sync_completions(cx);
        self.sync_note_providers(cx);
        if self.menu.as_ref().is_some_and(|menu| menu.offset != self.state.read(cx).cursor()) { self.menu = None; }
        if let Some(menu) = &mut self.menu { menu.prepare(self.language, &self.library); }
        let theme = cx.theme().clone();
        if let Some((text, styles)) = self.predictions.as_ref().and_then(|predictions|
            predictions.inline_highlights(self.state.read(cx).value().as_str(), self.state.read(cx).cursor(), cx)) {
            self.state.update(cx, |editor, cx| { editor.set_inline_completion_highlights(&text, styles, cx); });
        }
        let selection = self.state.read(cx).selected_range();
        let active = self.session.as_ref().filter(|s| { let r=s.range(); selection.start>=r.start && selection.end<=r.end });
        let expandable = active.is_some() || self.tab_expand && !self.matching_prefix(cx).is_empty();
        let editor = self.state.read(cx);
        let bounds = active.and_then(|session| editor.range_to_bounds(&session.range()))
            .filter(|bounds| bounds.intersects(&editor.input_bounds()))
            .map(|bounds| bounds.intersect(&editor.input_bounds()));
        let origin = editor.input_bounds().origin;
        let glide = bounds.map(|b| {
            let ease=gpui_kit::base::Transition::new(Duration::from_millis(95));
            let x=gpui_kit::base::transition("snippet-caret-x",b.left().as_f32(),ease.clone(),window,cx);
            let y=gpui_kit::base::transition("snippet-caret-y",b.top().as_f32(),ease,window,cx);
            (x,y,b.size.height)
        });
        let hover=self.state.read(cx).diagnostics().and_then(|set|set.iter().find_map(|entry|{
            let mut bounds=self.state.read(cx).range_to_bounds(&entry.range)?;bounds.size.width=bounds.size.width.max(px(8.));
            bounds.contains(&window.mouse_position()).then(||(entry.message.clone(),bounds))
        }));
        self.state.update(cx,|e,cx|e.clear_diagnostic_popover(cx));
        let choices = active.map(|s| s.stops[s.active].choices.clone()).unwrap_or_default();
        let mut context = KeyContext::parse(if self.menu.is_some() { "SnippetPicker" } else if expandable { "SnippetSource" } else { "SourceEditor" }).expect("valid editor context");
        if self.signature.has_overloads() { context.add("SignatureHints"); }
        div().id("snippet-source").test_support().relative().size_full().key_context(context)
            .on_modifiers_changed(cx.listener(Self::prediction_modifiers))
            .capture_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape" && (EditorMode::has_inline_completion(this.state.read(cx)) || this.predictions.as_ref().is_some_and(|predictions| predictions.alt.get())) {
                    this.hide_prediction(cx);
                    if let Some(predictions) = &this.predictions { predictions.invalidate(); }
                    cx.stop_propagation();
                } else if event.keystroke.key == "escape" && (this.menu.is_some() || this.completions.is_pending()) {
                    if this.menu.is_none() { this.session = None; }
                    this.close_completions(window, cx);
                    this.hide_signature(true, cx);
                    cx.stop_propagation();
                } else if event.keystroke.key == "escape" && this.hide_signature(true, cx) {
                    cx.stop_propagation();
                } else if !event.keystroke.modifiers.control && !event.keystroke.modifiers.platform && !event.keystroke.modifiers.shift
                    && event.keystroke.key == "k" && event.keystroke.modifiers.alt
                    && this.revert_prediction(window, cx) { cx.stop_propagation(); }
                else if !event.keystroke.modifiers.control && !event.keystroke.modifiers.platform && !event.keystroke.modifiers.shift
                    && event.keystroke.key == "l" && event.keystroke.modifiers.alt
                    && this.accept_prediction(window, cx) { cx.stop_propagation(); }
                else if this.menu.is_some() && !event.keystroke.modifiers.control && !event.keystroke.modifiers.platform && !event.keystroke.modifiers.shift && !event.keystroke.modifiers.alt {
                    match event.keystroke.key.as_str() {
                        "up" | "down" => {
                            if let Some(menu) = &mut this.menu { menu.step(if event.keystroke.key == "up" { -1 } else { 1 }); }
                            this.resolve_detail(cx); cx.notify(); cx.stop_propagation();
                        }
                        "enter" => { this.accept_completion(window,cx); cx.stop_propagation(); }
                        "tab" if this.session.is_none() => { this.accept_completion(window,cx); cx.stop_propagation(); }
                        _ => {}
                    }
                }
            }))
            .on_action(cx.listener(|this,_:&crate::actions::InsertSnippet,w,cx|this.open_completions(w,cx)))
            .on_action(cx.listener(|this,_:&NextStop,w,cx| this.navigate(false,w,cx)))
            .on_action(cx.listener(|this,_:&PreviousStop,w,cx| this.navigate(true,w,cx)))
            .on_action(cx.listener(|this,_:&CancelSnippet,w,cx| { if this.menu.is_none() { this.session=None; } this.close_completions(w,cx); this.hide_signature(true, cx); }))
            .on_action(cx.listener(|this,_:&PickerDown,_,cx| { if let Some(menu) = &mut this.menu { menu.step(1); } this.resolve_detail(cx); cx.notify(); }))
            .on_action(cx.listener(|this,_:&PickerUp,_,cx| { if let Some(menu) = &mut this.menu { menu.step(-1); } this.resolve_detail(cx); cx.notify(); }))
            .on_action(cx.listener(|this,_:&InsertPicked,w,cx| this.accept_completion(w,cx)))
            .on_action(cx.listener(|this,_:&crate::language_server::Complete,w,cx| this.open_completions(w,cx)))
            .on_action(cx.listener(|this, _: &super::signature_help::PreviousSignature, window, cx| this.step_signature(-1, window, cx)))
            .on_action(cx.listener(|this, _: &super::signature_help::NextSignature, window, cx| this.step_signature(1, window, cx)))
            .on_mouse_move(cx.listener(|this,_,_,cx|{this.state.update(cx,|e,cx|e.clear_diagnostic_popover(cx));cx.notify();}))
            .when_some(hover,|view,(message,bounds)|view.child(deferred(div().absolute().left((bounds.left()-origin.x).max(px(0.))).top(bounds.bottom()-origin.y+px(8.)).max_w(px(420.)).max_h(px(180.)).id("hovered-diagnostic").overflow_y_scroll().p_3().rounded_lg().bg(theme.popover).border_1().border_color(theme.danger.opacity(0.4)).shadow_md().text_sm().text_color(theme.danger).child(message))))
            .child(Editor::new(&self.state).text_size(theme.mono_font_size * (12. / 13.)).bordered(false).h_full().context_menu({
                let capabilities = self.state.read(cx).context_menu_capabilities();
                move |menu, _, _| {
                    use gpui_kit::base::input::{GoToDefinition, ToggleCodeActions, Cut, Copy, Paste, SelectAll};
                    let editable = capabilities.is_editable();
                    let menu = menu.menu_with_disabled("Go to definition", !(!capabilities.is_disabled() && capabilities.has_definition()), Box::new(GoToDefinition))
                        .menu_with_disabled("Show code actions", !(editable && capabilities.has_code_actions()), Box::new(ToggleCodeActions))
                        .separator().menu_with_disabled("Cut", !(editable && capabilities.is_copyable()), Box::new(Cut))
                        .menu_with_disabled("Copy", !capabilities.is_copyable(), Box::new(Copy))
                        .menu_with_disabled("Paste", !editable, Box::new(Paste)).separator().menu("Select all", Box::new(SelectAll));
                    if !capabilities.has_selection() { menu } else {
                        menu.separator().menu("Quote", Box::new(crate::actions::QuoteSelection))
                            .menu("Explain", Box::new(crate::actions::ExplainSelection))
                            .menu("Fix", Box::new(crate::actions::FixSelection))
                    }
                }
            }))
            .when_some(bounds, |view,bounds| view.child(deferred(anchored().position(bounds.origin).child(div().id("snippet-placeholder").test_support().w(bounds.size.width.max(px(3.))).h(bounds.size.height).rounded_sm().border_1().border_color(theme.primary).bg(theme.primary.opacity(0.08))
                .with_animation(("snippet-stop-glide",self.epoch), Animation::new(Duration::from_millis(100)), |view,t|view.opacity(0.35+0.65*t))))))
            .when_some(glide, |view,(x,y,height)|view.child(deferred(anchored().position(point(px(x),px(y))).child(div().w(px(2.)).h(height).bg(theme.primary)))))
            .when(!choices.is_empty(),|view|view.child(deferred(h_flex().absolute().bottom_2().left_2().gap_1().p_1().bg(theme.popover).rounded_md().children(choices.into_iter().map(|choice| {
                Button::new(SharedString::from(format!("snippet-choice-{choice}"))).ghost().small().label(choice.clone()).on_click(cx.listener(move|this,_,w,cx| {
                    if let Some(s)=&this.session {let range=s.range();this.state.update(cx,|e,cx|{e.set_selected_range(range,cx);e.replace(choice.clone(),w,cx);});}
                }))
            })))))
            .when_some(crate::modal_editing::status(self.state.read(cx)), |view, status| {
                view.child(div().absolute().bottom_1().right_3().max_w_full().px_2().py_1()
                    .rounded_sm().bg(theme.popover).text_xs().text_color(theme.muted_foreground)
                    .truncate().child(status))
            })
            .when_some(super::menu::render(self, window, cx), |view, menu| view.child(menu))
            .when_some(super::signature_help::render_popup(self, window, cx), |view, hints| view.child(hints))
            .when_some(if self.predictions.as_ref().is_some_and(|predictions| predictions.alt.get()) || (self.menu.is_none() && !(self.signature.focused && super::signature_help::height(self, window) > px(0.))) { crate::copilot::preview_overlay(self, window, cx) } else { None }, |view, hint| view.child(hint))
    }
}

#[cfg(feature = "gui-test")]
#[path = "cursor_fixture.rs"]
pub(crate) mod cursor_fixture;
