//! Source-editor expansion, placeholder navigation, and native completions.
use gpui_kit::*;
use gpui_kit::component::input::{Editor, EditorState, InputEvent, CompletionProvider};
use gpui_kit::base::input::{EditorMode, InputModeKind};
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::prelude::FluentBuilder as _;
use practice::{language::Language, snippets::{Snippet, body, session::Session}};
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
pub struct EditorPane {
    pub state: Entity<EditorState>,
    pub language: Language,
    pub library: Vec<Snippet>,
    pub tab_expand: bool,
    pub path: Option<std::path::PathBuf>,
    pub workspace: Option<std::path::PathBuf>,
    session: Option<Session>,
    completions: Rc<super::completion::Completions>,
    pub(super) menu: Option<super::menu::Menu>,
    changing: bool,
    epoch: u64,
    pub(crate) predictions: Option<Rc<crate::copilot::Predictions>>,
    _subscriptions: Vec<Subscription>,
}
impl EditorPane {
    pub fn new(state: Entity<EditorState>, language: Language, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let completions = Rc::new(super::completion::Completions::new(language));
        state.update(cx, |editor, _| editor.lsp_mut().completion_provider = Some(completions.clone()));
        let observe = cx.observe(&state, |this, editor, cx| {
            let content = editor.read(cx).completion_menu_state();
            if content.open {
                this.menu = Some(super::menu::Menu::new(content.clone(), editor.read(cx).cursor()));
                editor.update(cx, |editor, cx| editor.dismiss_completion_overlay(cx));
            }
            cx.notify();
        });
        let changes = cx.subscribe_in(&state, window, |this, editor, event, window, cx| {
            if !matches!(event, InputEvent::Change) || this.changing { return; }
            let text = editor.read(cx).value().to_string();
            if let Some(predictions) = &this.predictions { predictions.invalidate(); }
            this.menu = None;
            let selection = editor.read(cx).selected_range();
            if let Some(predictions) = &this.predictions {
                predictions.configure(this.path.clone(), this.language);
                predictions.warm(text.clone(), editor.read(cx).cursor(), cx);
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
        Self { state, language, library: practice::snippets::builtin(), tab_expand: true, path: None, workspace: None, session: None, completions, menu: None, changing: false, epoch: 0, predictions: None, _subscriptions: vec![observe, changes] }
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
    fn hide_prediction(&mut self, cx: &mut Context<Self>) {
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
        self.menu = None;
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
        predictions.accepted(suggestion.clone(), cx);
        self.state.update(cx, |editor, cx| {
            EditorMode::clear_inline_completion(editor, cx);
            editor.set_selected_range(suggestion.range(), cx);
            editor.replace(suggestion.text(), window, cx);
        });
        self.menu = None;
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
        let text = self.state.read(cx).value().to_string();
        if range.end > text.len() { return; }
        let line_start = text[..range.start].rfind('\n').map_or(0, |i| i+1);
        let line = text[line_start..].split('\n').next().unwrap_or_default();
        let context = body::Context { path: self.path.clone(), workspace: self.workspace.clone(), selected: self.state.read(cx).selected_value().to_string(), clipboard: cx.read_from_clipboard().and_then(|c| c.text()).unwrap_or_default(), language: Some(self.language), line: line.into(), line_index: text[..line_start].bytes().filter(|b| *b == b'\n').count(), word: text[range.clone()].into(), indent: line.chars().take_while(|c| c.is_whitespace()).collect(), unit: "    ".into(), ..Default::default() };
        let expansion = body::expand(&snippet.body, &context);
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
        self.changing = true;
        self.state.update(cx, |editor, cx| editor.insert_completion(item, menu.content.trigger_start_offset.unwrap_or(menu.offset)..menu.offset, window, cx));
        if let Some((offset, expansion)) = expansion {
            self.activate(expansion, offset, self.state.read(cx).value().to_string(), cx);
        }
        self.changing = false;
        cx.notify();
    }
    pub(super) fn close_completions(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.menu = None;
        self.state.update(cx, |editor, cx| { editor.dismiss_lsp_overlays(cx); editor.focus(window, cx); });
        cx.notify();
    }
    fn sync_completions(&mut self, cx: &mut App) {
        if let Some(predictions) = &self.predictions { predictions.configure(self.path.clone(), self.language); }
        let mut catalog = self.completions.catalog.borrow_mut();
        if catalog.library != self.library { catalog.library = self.library.clone(); }
        catalog.language = self.language;
        catalog.path.clone_from(&self.path);
        catalog.workspace.clone_from(&self.workspace);
        catalog.selected = self.state.read(cx).selected_value().to_string();
        catalog.selection = self.state.read(cx).selected_range();
        drop(catalog);
        let provider: Rc<dyn CompletionProvider> = self.completions.clone();
        let current = self.state.read(cx).lsp().completion_provider.clone();
        if current.as_ref().is_none_or(|current| !Rc::ptr_eq(current, &provider)) {
            *self.completions.upstream.borrow_mut() = current;
            self.state.update(cx, |editor, _| editor.lsp_mut().completion_provider = Some(provider));
        }
    }
}

impl Render for EditorPane {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.language = Language::ALL.into_iter().find(|l| l.id()==self.state.read(cx).language_name().as_str()).unwrap_or(self.language);
        self.sync_completions(cx);
        if self.menu.as_ref().is_some_and(|menu| menu.offset != self.state.read(cx).cursor()) { self.menu = None; }
        if let Some(menu) = &mut self.menu { menu.prepare(self.language); }
        let theme = cx.theme().clone();
        let selection = self.state.read(cx).selected_range();
        let active = self.session.as_ref().filter(|s| { let r=s.range(); selection.start>=r.start && selection.end<=r.end });
        let expandable = active.is_some() || self.tab_expand && !self.matching_prefix(cx).is_empty();
        let bounds = active.and_then(|s| self.state.read(cx).range_to_bounds(&s.range()));
        let origin = self.state.read(cx).input_bounds().origin;
        let glide = bounds.map(|b| {
            let ease=gpui_kit::base::Transition::new(Duration::from_millis(95));
            let x=gpui_kit::base::transition("snippet-caret-x",(b.left()-origin.x).as_f32(),ease.clone(),window,cx);
            let y=gpui_kit::base::transition("snippet-caret-y",(b.top()-origin.y).as_f32(),ease,window,cx);
            (x,y,b.size.height)
        });
        let hover=self.state.read(cx).diagnostics().and_then(|set|set.iter().find_map(|entry|{
            let mut bounds=self.state.read(cx).range_to_bounds(&entry.range)?;bounds.size.width=bounds.size.width.max(px(8.));
            bounds.contains(&window.mouse_position()).then(||(entry.message.clone(),bounds))
        }));
        self.state.update(cx,|e,cx|e.clear_diagnostic_popover(cx));
        let choices = active.map(|s| s.stops[s.active].choices.clone()).unwrap_or_default();
        div().id("snippet-source").test_support().relative().size_full().key_context(if self.menu.is_some() { "SnippetPicker" } else if expandable { "SnippetSource" } else { "SourceEditor" })
            .on_modifiers_changed(cx.listener(Self::prediction_modifiers))
            .capture_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape" && (EditorMode::has_inline_completion(this.state.read(cx)) || this.predictions.as_ref().is_some_and(|predictions| predictions.alt.get())) {
                    this.hide_prediction(cx);
                    if let Some(predictions) = &this.predictions { predictions.invalidate(); }
                    cx.stop_propagation();
                } else if !event.keystroke.modifiers.control && !event.keystroke.modifiers.platform && !event.keystroke.modifiers.shift
                    && (event.keystroke.key == "tab" || event.keystroke.key == "l" && event.keystroke.modifiers.alt)
                    && this.accept_prediction(window, cx) { cx.stop_propagation(); }
            }))
            .on_action(cx.listener(|this,_:&crate::actions::InsertSnippet,w,cx|this.open_completions(w,cx)))
            .on_action(cx.listener(|this,_:&NextStop,w,cx| this.navigate(false,w,cx)))
            .on_action(cx.listener(|this,_:&PreviousStop,w,cx| this.navigate(true,w,cx)))
            .on_action(cx.listener(|this,_:&CancelSnippet,w,cx| { if this.menu.is_none() { this.session=None; } this.close_completions(w,cx); }))
            .on_action(cx.listener(|this,_:&PickerDown,_,cx| { if let Some(menu) = &mut this.menu { menu.step(1); } cx.notify(); }))
            .on_action(cx.listener(|this,_:&PickerUp,_,cx| { if let Some(menu) = &mut this.menu { menu.step(-1); } cx.notify(); }))
            .on_action(cx.listener(|this,_:&InsertPicked,w,cx| this.accept_completion(w,cx)))
            .on_action(cx.listener(|this,_:&crate::language_server::Complete,w,cx| this.open_completions(w,cx)))
            .on_mouse_move(cx.listener(|this,_,_,cx|{this.state.update(cx,|e,cx|e.clear_diagnostic_popover(cx));cx.notify();}))
            .when_some(hover,|view,(message,bounds)|view.child(deferred(div().absolute().left((bounds.left()-origin.x).max(px(0.))).top(bounds.bottom()-origin.y+px(8.)).max_w(px(420.)).max_h(px(180.)).id("hovered-diagnostic").overflow_y_scroll().p_3().rounded_lg().bg(theme.popover).border_1().border_color(theme.danger.opacity(0.4)).shadow_md().text_sm().text_color(theme.danger).child(message))))
            .child(Editor::new(&self.state).bordered(false).h_full())
            .when_some(bounds, |view,bounds| view.child(div().absolute().left(bounds.left()-origin.x).top(bounds.top()-origin.y).w(bounds.size.width.max(px(3.))).h(bounds.size.height).rounded_sm().border_1().border_color(theme.primary).bg(theme.primary.opacity(0.08))
                .with_animation(("snippet-stop-glide",self.epoch), Animation::new(Duration::from_millis(100)), |view,t|view.opacity(0.35+0.65*t))))
            .when_some(glide, |view,(x,y,height)|view.child(div().absolute().left(px(x)).top(px(y)).w(px(2.)).h(height).bg(theme.primary)))
            .when(!choices.is_empty(),|view|view.child(deferred(h_flex().absolute().bottom_2().left_2().gap_1().p_1().bg(theme.popover).rounded_md().children(choices.into_iter().map(|choice| {
                Button::new(SharedString::from(format!("snippet-choice-{choice}"))).ghost().small().label(choice.clone()).on_click(cx.listener(move|this,_,w,cx| {
                    if let Some(s)=&this.session {let range=s.range();this.state.update(cx,|e,cx|{e.set_selected_range(range,cx);e.replace(choice.clone(),w,cx);});}
                }))
            })))))
            .when_some(super::menu::render(self, window, cx), |view, menu| view.child(menu))
    }
}
