//! Source-editor expansion, navigation, and a searchable completion picker.
use gpui_kit::*;
use gpui_kit::component::input::{Editor, EditorState, Input, InputState, InputEvent};
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::assets::IconName;
use gpui_kit::prelude::FluentBuilder as _;
use practice::{language::Language, snippets::{Snippet, body, session::Session}};
use gpui_kit::component::RopeExt as _;
use std::{ops::Range, time::Duration};

gpui_kit::actions!(snippet_expansion, [NextStop, PreviousStop, CancelSnippet, PickerUp, PickerDown, InsertPicked]);
pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("tab", NextStop, Some("SnippetSource > Input")),
        KeyBinding::new("shift-tab", PreviousStop, Some("SnippetSource > Input")),
        KeyBinding::new("escape", CancelSnippet, Some("SnippetSource > Input")),
        KeyBinding::new("up", PickerUp, Some("SnippetPicker > Input")),
        KeyBinding::new("down", PickerDown, Some("SnippetPicker > Input")),
        KeyBinding::new("enter", InsertPicked, Some("SnippetPicker > Input")),
        KeyBinding::new("escape", CancelSnippet, Some("SnippetPicker > Input")),
    ]);
}
#[derive(Clone)]
enum Candidate { Snippet(Snippet), Completion(lsp_types::CompletionItem) }
impl Candidate {
    fn label(&self) -> String { match self { Self::Snippet(s) => format!("{} · {}", s.prefix(), s.name), Self::Completion(c) => c.label.clone() } }
    fn preview(&self) -> String { match self { Self::Snippet(s) => body::preview(&s.body).text, Self::Completion(c) => c.detail.clone().unwrap_or_else(|| c.label.clone()) } }
}
pub struct EditorPane {
    pub state: Entity<EditorState>,
    pub language: Language,
    pub library: Vec<Snippet>,
    pub tab_expand: bool,
    pub path: Option<std::path::PathBuf>,
    pub workspace: Option<std::path::PathBuf>,
    session: Option<Session>,
    query: Entity<InputState>,
    picker: Option<(Range<usize>, Vec<Candidate>)>,
    selected: usize,
    scroll: UniformListScrollHandle,
    changing: bool,
    epoch: u64,
    _subscriptions: Vec<Subscription>,
}
impl EditorPane {
    pub fn new(state: Entity<EditorState>, language: Language, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let query = cx.new(|cx| InputState::new(window, cx).placeholder("Find a snippet…"));
        let observe = cx.observe(&state, |_, _, cx| cx.notify());
        let changes = cx.subscribe_in(&state, window, |this, editor, event, window, cx| {
            if !matches!(event, InputEvent::Change) || this.changing { return; }
            let text = editor.read(cx).value().to_string();
            let selection = editor.read(cx).selected_range();
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
        let search = cx.subscribe(&query, |this, _, _: &InputEvent, cx| { this.selected = 0; this.scroll.scroll_to_item(0, ScrollStrategy::Top); cx.notify(); });
        Self { state, language, library: practice::snippets::builtin(), tab_expand: true, path: None, workspace: None, session: None, query, picker: None, selected: 0, scroll: UniformListScrollHandle::new(), changing: false, epoch: 0, _subscriptions: vec![observe, changes, search] }
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
        self.picker = None; self.changing = false; self.epoch += 1; cx.notify();
    }
    fn navigate(&mut self, backwards: bool, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(session) = &mut self.session {
            let selection = self.state.read(cx).selected_range(); let active = session.range();
            if selection.start < active.start || selection.end > active.end { self.session = None; cx.notify(); return; }
            let range = session.next(backwards);
            let done = session.stops[session.active].index == 0;
            if let Some(range) = range { self.state.update(cx, |e, cx| e.set_selected_range(range, cx)); }
            if done || self.session.as_ref().is_some_and(|s| s.active >= s.stops.len()) { self.session = None; }
            self.epoch += 1; cx.notify(); return;
        }
        let candidates = self.matching_prefix(cx);
        if candidates.len() == 1 { self.insert(&candidates[0], self.prefix(cx), window, cx); }
        else { self.open_picker(false, window, cx); }
    }
    pub fn open_picker(&mut self, completions: bool, window: &mut Window, cx: &mut Context<Self>) {
        let range = if self.state.read(cx).selected_range().is_empty() { self.prefix(cx) } else { self.state.read(cx).selected_range() };
        let items = self.library.iter().filter(|s| s.applies_to(self.language)).cloned().map(Candidate::Snippet).collect();
        self.picker = Some((range.clone(), items)); self.selected = 0;
        self.query.update(cx, |q,cx| { q.set_value("", window, cx); q.focus(window,cx); });
        if completions {
            let state = self.state.read(cx); let snapshot = state.value().to_string(); let text = state.text().clone(); let offset = state.cursor();
            if let Some(provider) = state.lsp().completion_provider.clone() {
                let task = provider.completions(&text, offset, lsp_types::CompletionContext { trigger_kind: lsp_types::CompletionTriggerKind::INVOKED, trigger_character: None }, window, cx);
                cx.spawn_in(window, async move |this,cx| {
                    if let Ok(response) = task.await { let _ = this.update(cx, |this,cx| {
                        if this.state.read(cx).value().as_str() != snapshot { return; }
                        if let Some((current, items)) = &mut this.picker && *current == range {
                            let more = match response { lsp_types::CompletionResponse::Array(items) => items, lsp_types::CompletionResponse::List(list) => list.items };
                            items.extend(more.into_iter().map(Candidate::Completion)); cx.notify();
                        }
                    }); }
                }).detach();
            }
        }
        cx.notify();
    }
    fn filtered(&self, cx: &App) -> Vec<Candidate> {
        let query = self.query.read(cx).value().to_lowercase();
        self.picker.as_ref().map(|(_,items)| items.iter().filter(|item| {
            let label = item.label().to_lowercase(); let mut chars = label.chars(); query.chars().all(|c| chars.by_ref().any(|x| x==c))
        }).cloned().collect()).unwrap_or_default()
    }
    fn confirm(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(item) = self.filtered(cx).get(self.selected).cloned() else { return; };
        let range = self.picker.as_ref().unwrap().0.clone();
        match item {
            Candidate::Snippet(s) => self.insert(&s, range, window,cx),
            Candidate::Completion(item) => {
                if item.insert_text_format == Some(lsp_types::InsertTextFormat::SNIPPET) {
                    let (body, range) = match item.text_edit.as_ref() {
                        Some(lsp_types::CompletionTextEdit::Edit(edit)) => {
                            let text = self.state.read(cx).text();
                            (edit.new_text.clone(), text.position_to_offset(&edit.range.start)..text.position_to_offset(&edit.range.end))
                        },
                        Some(lsp_types::CompletionTextEdit::InsertAndReplace(edit)) => {
                            let text = self.state.read(cx).text();
                            (edit.new_text.clone(), text.position_to_offset(&edit.replace.start)..text.position_to_offset(&edit.replace.end))
                        },
                        None => (item.insert_text.clone().unwrap_or_else(|| item.label.clone()), range),
                    };
                    let s = Snippet {name:item.label, prefixes:vec!["lsp".into()],body,description:String::new(),scope:Some(self.language),template:false,origin:practice::snippets::Origin::User};
                    self.insert(&s,range,window,cx);
                } else { self.state.update(cx, |e,cx| { e.insert_completion(&item,range,window,cx); e.focus(window,cx); }); self.picker=None; cx.notify(); }
            }
        }
    }
}
impl Render for EditorPane {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.language = Language::ALL.into_iter().find(|l| l.id()==self.state.read(cx).language_name().as_str()).unwrap_or(self.language);
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
        let items = self.filtered(cx); let preview = items.get(self.selected).map(Candidate::preview);
        div().id("snippet-source").test_support().relative().size_full().key_context(if self.picker.is_some() { "SnippetPicker" } else if expandable { "SnippetSource" } else { "SourceEditor" })
            .on_action(cx.listener(|this,_:&crate::actions::InsertSnippet,w,cx|this.open_picker(false,w,cx)))
            .on_action(cx.listener(|this,_:&NextStop,w,cx| this.navigate(false,w,cx)))
            .on_action(cx.listener(|this,_:&PreviousStop,w,cx| this.navigate(true,w,cx)))
            .on_action(cx.listener(|this,_:&CancelSnippet,w,cx| { this.session=None;this.picker=None;this.state.update(cx,|e,cx|e.focus(w,cx));cx.notify(); }))
            .on_action(cx.listener(|this,_:&PickerDown,_,cx| { let len=this.filtered(cx).len(); if len>0 {this.selected=(this.selected+1)%len;this.scroll.scroll_to_item(this.selected,ScrollStrategy::Nearest);} cx.notify(); }))
            .on_action(cx.listener(|this,_:&PickerUp,_,cx| { let len=this.filtered(cx).len(); if len>0 {this.selected=(this.selected+len-1)%len;this.scroll.scroll_to_item(this.selected,ScrollStrategy::Nearest);} cx.notify(); }))
            .on_action(cx.listener(|this,_:&InsertPicked,w,cx|this.confirm(w,cx)))
            .on_action(cx.listener(|this,_:&crate::language_server::Complete,w,cx| this.open_picker(true,w,cx)))
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
            .when(self.picker.is_some(), |view|view.child(deferred(v_flex().id("snippet-picker").test_support().absolute().inset_4().p_3().gap_3().bg(theme.popover).border_1().border_color(theme.border).rounded_lg().shadow_lg()
                .child(h_flex().gap_2().child(div().flex_1().child(Input::new(&self.query))).child(Button::new("close-snippet-picker").ghost().small().icon(IconName::X).tooltip("Close · Esc").on_click(cx.listener(|this,_,w,cx|{this.picker=None;this.state.update(cx,|e,cx|e.focus(w,cx));cx.notify();}))))
                .child(h_flex().flex_1().min_h_0().items_stretch().gap_3()
                    .child(uniform_list("snippet-picker-list",items.len(),cx.processor(move|this,range:Range<usize>,_,cx|{
                        range.filter_map(|ix|items.get(ix).map(|item|(ix,item))).map(|(ix,item)| {
                            crate::theme::selected_choice(Button::new(SharedString::from(format!("pick-{}",item.label()))).ghost(),this.selected==ix,cx).w_full().label(item.label()).on_click(cx.listener(move|this,_,w,cx|{this.selected=ix;this.confirm(w,cx);}))
                        }).collect::<Vec<_>>()
                    })).track_scroll(&self.scroll).flex_1().min_w_0())
                    .child(div().id("snippet-picker-preview").overflow_y_scroll().flex_1().min_w_0().p_3().bg(theme.muted).rounded_md().font_family(theme.mono_font_family.clone()).text_sm().child(preview.unwrap_or_else(||"No matches".into())))))) )
    }
}
