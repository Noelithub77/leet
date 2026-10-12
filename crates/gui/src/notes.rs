//! Notes in the solution editor: the Rich, Raw and Off views, their decorations,
//! hovers and code actions, and the edits that write AI notes into the file.
use std::{cell::{Cell, RefCell}, ops::Range, rc::Rc, time::Duration};
use anyhow::Result;
use gpui_kit::*;
use gpui_kit::component::{ActiveTheme as _, Theme};
use gpui_kit::component::input::{CodeActionProvider, EditorState, HoverProvider, Position, RangeDecoration, RangeDecorationCollection,
    RangeDecorationStyle, Rope, RopeExt as _};
use practice::{language::Language, notes::{self, Hidden, Kind, Note, View}};
use crate::actions::{AddQuestion, AskInPlace, ClearAiNotes, ResolveNote};
use crate::note_card::{Card, Item, Key};
use crate::snippets::expansion::EditorPane;
use crate::workspace::Workspace;

#[cfg(feature = "gui-test")]
#[path = "gui_test/notes.rs"]
pub(crate) mod fixture;

/// The full explanation behind a note's text, when the chat still has it.
pub(crate) type Detail = Rc<dyn Fn(&str, &App) -> Option<String>>;

pub(crate) struct Notes {
    view: View,
    /// Note lines the view leaves out of the editor.
    hidden: Hidden,
    /// The editor text the hidden lines were last placed against.
    shown: String,
    marks: Option<RangeDecorationCollection>,
    flash: Option<RangeDecorationCollection>,
    flash_task: Task<()>,
    badges: Vec<Badge>,
    pub(crate) provider: Rc<Provider>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum BadgeKind { Cost, Trace }

/// A hover detail tied to a line of earlier code, found again after edits.
struct Badge {
    kind: BadgeKind,
    seen: Rc<str>,
    /// `seen` is a whole file; otherwise it is editor text.
    file: bool,
    line: usize,
    text: SharedString,
}

/// A note with the editor lines it shows on.
#[derive(Clone)]
struct Placed {
    note: Note,
    /// Editor line of the comment; `None` while the view hides it.
    line: Option<usize>,
    /// Editor line of the code the note is about.
    target: Option<usize>,
}

impl Notes {
    pub(crate) fn new(view: View, language: Language) -> Self {
        let provider = Rc::new(Provider { view: Cell::new(view), language: Cell::new(language), ai: Cell::new(false), placed: RefCell::default(), badges: RefCell::default(), detail: RefCell::new(None), upstream: RefCell::new(None) });
        Self { view, hidden: Hidden::default(), shown: String::new(), marks: None, flash: None, flash_task: Task::ready(()), badges: Vec::new(), provider }
    }
}

/// Hovers and code actions for notes, merged with the language server's hover.
pub(crate) struct Provider {
    view: Cell<View>,
    language: Cell<Language>,
    /// The file has AI notes, including hidden ones.
    ai: Cell<bool>,
    placed: RefCell<Vec<Placed>>,
    /// Cost and debugger values by editor line, shown in Rich.
    badges: RefCell<Vec<(usize, BadgeKind, SharedString)>>,
    pub(crate) detail: RefCell<Option<Detail>>,
    upstream: RefCell<Option<Rc<dyn HoverProvider>>>,
}

impl HoverProvider for Provider {
    fn hover(&self, text: &Rope, offset: usize, window: &mut Window, cx: &mut App) -> Task<Result<Option<lsp_types::Hover>>> {
        let upstream = self.upstream.borrow().clone().map(|upstream| upstream.hover(text, offset, window, cx));
        let code = text.to_string();
        let line = line_of(&code, offset);
        let own = (self.view.get() == View::Rich).then(|| code_range(&code, line)).flatten()
            .filter(|range| range.start <= offset && offset <= range.end)
            .and_then(|range| Some((range, self.line_details(line, cx)?)));
        let Some((range, value)) = own else { return upstream.unwrap_or_else(|| Task::ready(Ok(None))) };
        let range = lsp_types::Range { start: text.offset_to_position(range.start), end: text.offset_to_position(range.end) };
        let hover = move |value: String, range| lsp_types::Hover { contents: lsp_types::HoverContents::Markup(lsp_types::MarkupContent { kind: lsp_types::MarkupKind::Markdown, value }), range: Some(range) };
        let Some(upstream) = upstream else { return Task::ready(Ok(Some(hover(value, range)))) };
        cx.spawn(async move |_| {
            let Some(found) = upstream.await.ok().flatten() else { return Ok(Some(hover(value, range))) };
            Ok(Some(hover(format!("{value}\n\n---\n\n{}", markdown(&found.contents)), found.range.unwrap_or(range))))
        })
    }
}

impl Provider {
    /// The hover card for an editor line: its notes, a question's answers, and its values.
    fn line_details(&self, line: usize, cx: &App) -> Option<String> {
        let placed = self.placed.borrow();
        let detail = self.detail.borrow();
        let item = |note: &Note| card_item(note, detail.as_ref().and_then(|detail| detail(&note.text, cx)));
        let mut card = Card::default();
        match placed.iter().find(|placed| placed.line == Some(line) && placed.note.kind == Kind::Ask) {
            Some(question) => {
                card.items = placed.iter().filter(|placed| placed.note.kind == Kind::Answer && placed.target == question.target).map(|placed| item(&placed.note)).collect();
                if card.items.is_empty() { card.items.push(Item { kind: Kind::Ask, step: None, summary: question.note.text.clone(), detail: String::new(), outdated: false }); }
                if !question.note.text.is_empty() { card.keys.push(Key::Ask); }
            }
            None => {
                card.items = placed.iter().filter(|placed| placed.note.kind.ai() && placed.target == Some(line) && placed.line.is_none()).map(|placed| item(&placed.note)).collect();
                if !card.items.is_empty() { card.keys.push(Key::Resolve); }
                if card.items.iter().any(|item| item.step.is_some()) { card.keys.push(Key::NextStep); }
            }
        }
        card.facts = self.badges.borrow().iter().filter(|(at, ..)| *at == line).map(|(_, kind, text)| (match kind {
            BadgeKind::Cost => "Cost", BadgeKind::Trace => "Last values",
        }.to_owned(), text.to_string())).collect();
        if card.items.is_empty() && card.facts.is_empty() { return None; }
        if !card.keys.contains(&Key::Ask) { card.keys.push(Key::Question); }
        Some(card.markdown())
    }
}

fn markdown(contents: &lsp_types::HoverContents) -> String {
    use lsp_types::{HoverContents, MarkedString};
    let marked = |item: &MarkedString| match item {
        MarkedString::String(text) => text.clone(),
        MarkedString::LanguageString(code) => format!("```{}\n{}\n```", code.language, code.value),
    };
    match contents {
        HoverContents::Scalar(item) => marked(item),
        HoverContents::Array(items) => items.iter().map(marked).collect::<Vec<_>>().join("\n\n"),
        HoverContents::Markup(markup) => markup.value.clone(),
    }
}

fn card_item(note: &Note, detail: Option<String>) -> Item {
    // The highlight already shows the quoted code.
    let summary = note.snippet.as_ref().and_then(|snippet| note.text.strip_suffix(&format!(" (`{snippet}`)"))).unwrap_or(&note.text);
    Item { kind: note.kind, step: note.step, summary: summary.to_owned(), detail: detail.unwrap_or_default(), outdated: note.outdated() }
}

const ACTIONS: &str = "notes";

impl CodeActionProvider for Provider {
    fn id(&self) -> SharedString { ACTIONS.into() }
    fn code_actions(&self, editor: Entity<EditorState>, range: Range<usize>, _: &mut Window, cx: &mut App) -> Task<Result<Vec<lsp_types::CodeAction>>> {
        let text = editor.read(cx).value().to_string();
        let line = line_of(&text, range.start);
        let placed = self.placed.borrow();
        let action = |title: &str, id: &str| lsp_types::CodeAction { title: title.into(), data: Some(serde_json::Value::from(id)), ..Default::default() };
        let mut actions = Vec::new();
        match placed.iter().find(|placed| placed.line == Some(line)).map(|placed| placed.note.kind) {
            Some(Kind::Ask) => actions.push(action("Ask AI", "ask")),
            Some(kind) if kind.ai() => actions.push(action("Resolve note", "resolve")),
            Some(_) => {}
            None => if code_range(&text, line).is_some() {
                if placed.iter().any(|placed| placed.note.kind.ai() && placed.target == Some(line)) { actions.push(action("Resolve note", "resolve")); }
                actions.push(action("Ask about this line", "question"));
            },
        }
        if self.ai.get() { actions.push(action("Clear AI notes", "clear")); }
        Task::ready(Ok(actions))
    }
    fn perform_code_action(&self, _: Entity<EditorState>, action: lsp_types::CodeAction, _: bool, window: &mut Window, cx: &mut App) -> Task<Result<()>> {
        let command: Box<dyn Action> = match action.data.as_ref().and_then(|data| data.as_str()) {
            Some("ask") => Box::new(AskInPlace),
            Some("resolve") => Box::new(ResolveNote),
            Some("question") => Box::new(AddQuestion),
            Some("clear") => Box::new(ClearAiNotes),
            _ => return Task::ready(Ok(())),
        };
        window.dispatch_action(command, cx);
        Task::ready(Ok(()))
    }
}

pub(crate) fn color(kind: Kind, theme: &Theme) -> Option<Hsla> {
    match kind {
        Kind::Bug => Some(theme.danger),
        Kind::Risk | Kind::Slow => Some(theme.warning),
        Kind::Stuck => Some(theme.info),
        Kind::Hint => Some(theme.magenta),
        Kind::Answer => Some(theme.success),
        Kind::Ask | Kind::Comment => None,
    }
}

/// Which note colors a line when several are about it.
fn rank(kind: Kind) -> u8 {
    match kind { Kind::Bug => 0, Kind::Risk => 1, Kind::Slow => 2, Kind::Stuck => 3, Kind::Hint => 4, Kind::Answer => 5, Kind::Ask | Kind::Comment => 6 }
}

/// One fill over each code line that AI notes are about; fainter when every note there is outdated.
fn marks(placed: &[Placed], text: &str, theme: &Theme) -> Vec<RangeDecoration> {
    let mut lines: Vec<(usize, Kind, bool)> = Vec::new();
    for placed in placed.iter().filter(|placed| placed.note.kind.ai()) {
        let Some(target) = placed.target else { continue };
        let fresh = !placed.note.outdated();
        match lines.iter_mut().find(|(line, ..)| *line == target) {
            Some(entry) => { if rank(placed.note.kind) < rank(entry.1) { entry.1 = placed.note.kind; } entry.2 |= fresh; }
            None => lines.push((target, placed.note.kind, fresh)),
        }
    }
    lines.sort_by_key(|(line, ..)| *line);
    lines.into_iter().filter_map(|(line, kind, fresh)| {
        let range = code_range(text, line)?;
        let color = color(kind, theme)?;
        Some(RangeDecoration::new(range).with_style(RangeDecorationStyle::Fill).with_color(color.opacity(if fresh { 0.14 } else { 0.07 })))
    }).collect()
}

/// Byte range of a line's code without surrounding whitespace; `None` for a blank line.
fn code_range(text: &str, line: usize) -> Option<Range<usize>> {
    let range = line_range(text, line)?;
    let code = &text[range.clone()];
    let trimmed = code.trim_end();
    let lead = trimmed.len() - trimmed.trim_start().len();
    (lead < trimmed.len()).then(|| range.start + lead..range.start + trimmed.len())
}

/// Zero-based line of a byte offset.
fn line_of(text: &str, offset: usize) -> usize { text[..offset.min(text.len())].matches('\n').count() }

/// Byte range of a zero-based line, without its `\n`.
fn line_range(text: &str, line: usize) -> Option<Range<usize>> {
    let start = if line == 0 { 0 } else { text.match_indices('\n').nth(line - 1)?.0 + 1 };
    Some(start..text[start..].find('\n').map_or(text.len(), |end| start + end))
}

impl EditorPane {
    /// The solution file: the editor text with any hidden notes put back.
    pub(crate) fn file_text(&self, cx: &App) -> String { self.notes.hidden.restore(&self.state.read(cx).value()) }

    /// Replaces the editor with a file, clearing undo history.
    pub(crate) fn load_file(&mut self, file: String, window: &mut Window, cx: &mut Context<Self>) {
        self.show_file(file, false, window, cx);
    }

    /// Replaces the editor with a file as one undoable edit.
    pub(crate) fn replace_file(&mut self, file: String, window: &mut Window, cx: &mut Context<Self>) {
        self.show_file(file, true, window, cx);
    }

    fn show_file(&mut self, file: String, undoable: bool, window: &mut Window, cx: &mut Context<Self>) {
        let (view, hidden) = notes::hide(&file, self.notes.view);
        self.notes.hidden = hidden;
        if self.state.read(cx).value().as_ref() != view {
            let text = view.clone();
            self.state.update(cx, |editor, cx| if undoable { editor.replace_all(text, window, cx) } else { editor.set_value(text, window, cx) });
        }
        self.notes.shown = view;
        self.refresh_notes(cx);
    }

    /// Applies a changed file, such as one with new AI notes, as the smallest edit.
    pub(crate) fn apply_file(&mut self, file: String, window: &mut Window, cx: &mut Context<Self>) {
        let current = self.state.read(cx).value().to_string();
        let (view, hidden) = notes::hide(&file, self.notes.view);
        self.notes.hidden = hidden;
        if view == current { self.refresh_notes(cx); return; }
        let (range, text) = notes::changed_bytes(&current, &view);
        let selection = self.state.read(cx).selected_range();
        let delta = text.len() as isize - range.len() as isize;
        let moved = |offset: usize| if offset >= range.end { offset.saturating_add_signed(delta) } else if offset > range.start { range.start + text.len() } else { offset };
        let selection = moved(selection.start)..moved(selection.end);
        self.changing = true;
        self.state.update(cx, |editor, cx| {
            editor.set_selected_range(range, cx);
            editor.replace(text, window, cx);
            editor.set_selected_range(selection, cx);
        });
        self.changing = false;
        self.notes.shown = view;
        self.refresh_notes(cx);
    }

    pub(crate) fn notes_changed(&mut self, cx: &mut Context<Self>) {
        let text = self.state.read(cx).value().to_string();
        if text == self.notes.shown { return; }
        self.notes.hidden.edited(&self.notes.shown, &text);
        self.notes.shown = text;
        self.refresh_notes(cx);
    }

    pub(crate) fn set_notes_view(&mut self, view: View, window: &mut Window, cx: &mut Context<Self>) {
        let previous = self.notes.view;
        if previous == view { return; }
        self.notes.view = view;
        self.notes.provider.view.set(view);
        let file = self.notes.hidden.restore(&self.state.read(cx).value());
        let cursor = self.state.read(cx).cursor_position();
        let file_line = self.notes.hidden.file_line(cursor.line as usize);
        self.load_file(file, window, cx);
        let line = self.view_line(file_line);
        let text = self.state.read(cx).value().to_string();
        let Some(range) = line_range(&text, line) else { return };
        let at = text[range.clone()].char_indices().nth(cursor.character as usize).map_or(range.end, |(index, _)| range.start + index);
        self.state.update(cx, |editor, cx| editor.set_selected_range(at..at, cx));
    }

    /// The editor line showing a file line; a hidden note shows as the code below it.
    pub(crate) fn view_line(&self, file_line: usize) -> usize {
        (file_line..=file_line + self.notes.hidden.len()).find_map(|line| self.notes.hidden.view_line(line)).unwrap_or(file_line)
    }

    pub(crate) fn refresh_notes(&mut self, cx: &mut Context<Self>) {
        let text = self.state.read(cx).value().to_string();
        let hidden = &self.notes.hidden;
        let parsed = notes::parse(&hidden.restore(&text), self.language);
        let provider = &self.notes.provider;
        provider.language.set(self.language);
        provider.ai.set(parsed.iter().any(|note| note.kind.ai()));
        let placed: Vec<Placed> = parsed.into_iter().map(|note| Placed { line: hidden.view_line(note.line), target: note.target.and_then(|line| hidden.view_line(line)), note }).collect();
        let rich = self.notes.view == View::Rich;
        let marks = if rich { marks(&placed, &text, cx.theme()) } else { Vec::new() };
        *provider.badges.borrow_mut() = if rich { self.badge_lines(&text) } else { Vec::new() };
        *provider.placed.borrow_mut() = placed;
        match &self.notes.marks {
            Some(collection) => collection.set(marks, cx),
            None => self.notes.marks = Some(self.state.update(cx, |editor, cx| editor.create_range_decorations_collection(marks, cx))),
        }
    }

    /// Replaces one kind of badge. Lines are zero-based in `seen`.
    pub(crate) fn set_badges(&mut self, kind: BadgeKind, seen: &str, file: bool, items: Vec<(usize, String)>, cx: &mut Context<Self>) {
        let seen: Rc<str> = seen.into();
        self.notes.badges.retain(|badge| badge.kind != kind);
        self.notes.badges.extend(items.into_iter().map(|(line, text)| Badge { kind, seen: seen.clone(), file, line, text: text.into() }));
        self.refresh_notes(cx);
    }

    /// Badges on the editor lines they now belong to.
    fn badge_lines(&self, view: &str) -> Vec<(usize, BadgeKind, SharedString)> {
        if self.notes.badges.is_empty() { return Vec::new(); }
        let file = self.notes.hidden.restore(view);
        self.notes.badges.iter().filter_map(|badge| {
            let line = if badge.file { self.view_line(notes::relocate(&badge.seen, &file, badge.line, None)?) } else { notes::relocate(&badge.seen, view, badge.line, None)? };
            Some((line, badge.kind, badge.text.clone()))
        }).collect()
    }

    /// Keeps the notes provider first, wrapping the language server's hover.
    pub(crate) fn sync_note_providers(&mut self, cx: &mut App) {
        let provider = self.notes.provider.clone();
        let hover: Rc<dyn HoverProvider> = provider.clone();
        let actions: Rc<dyn CodeActionProvider> = provider.clone();
        let lsp = self.state.read(cx).lsp();
        let current = lsp.hover_provider.clone();
        let has_actions = lsp.code_action_providers.iter().any(|existing| existing.id() == ACTIONS);
        let wrapped = current.as_ref().is_some_and(|current| Rc::ptr_eq(current, &hover));
        if wrapped && has_actions { return; }
        if !wrapped { *provider.upstream.borrow_mut() = current; }
        self.state.update(cx, |editor, _| {
            editor.lsp_mut().hover_provider = Some(hover);
            if !has_actions { editor.lsp_mut().code_action_providers.insert(0, actions); }
        });
    }

    /// Briefly highlights an editor line.
    pub(crate) fn flash_line(&mut self, line: usize, cx: &mut Context<Self>) {
        let text = self.state.read(cx).value().to_string();
        let Some(range) = line_range(&text, line) else { return };
        let range = range.start..(range.end + 1).min(text.len());
        let mark = vec![RangeDecoration::new(range).with_style(RangeDecorationStyle::Fill).with_color(cx.theme().info.opacity(0.22))];
        match &self.notes.flash {
            Some(collection) => collection.set(mark, cx),
            None => self.notes.flash = Some(self.state.update(cx, |editor, cx| editor.create_range_decorations_collection(mark, cx))),
        }
        self.notes.flash_task = cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_millis(900)).await;
            let _ = this.update(cx, |this, cx| if let Some(collection) = &this.notes.flash { collection.clear(cx); });
        });
    }

    /// Moves the cursor to a line, scrolls it into view, and flashes it.
    pub(crate) fn reveal_line(&mut self, line: usize, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.state.read(cx).value().to_string();
        let column = text.split('\n').nth(line).map_or(0, |code| code.len() - code.trim_start().len());
        self.state.update(cx, |editor, cx| editor.set_cursor_position(Position::new(line as u32, column as u32), window, cx));
        self.flash_line(line, cx);
    }

    /// The line note actions apply to: the hovered line while its hover is open, else the cursor's.
    fn acting_line(&self, cx: &App) -> usize {
        let editor = self.state.read(cx);
        line_of(&editor.value().to_string(), editor.hover_popover().map_or(editor.cursor(), |hover| hover.symbol_range.start))
    }

    /// The `#?` question on the acting line.
    pub(crate) fn question_at_cursor(&self, cx: &App) -> Option<Note> {
        let text = self.state.read(cx).value().to_string();
        let line = self.acting_line(cx);
        notes::parse(&text, self.language).into_iter().find(|note| note.kind == Kind::Ask && note.line == line && !note.text.is_empty())
    }

    /// Steps through a reasoning trail, wrapping at either end.
    pub(crate) fn step_trail(&mut self, forward: bool, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let mut trail: Vec<Placed> = self.notes.provider.placed.borrow().iter().filter(|placed| placed.note.step.is_some()).cloned().collect();
        if trail.is_empty() { return false; }
        trail.sort_by_key(|placed| placed.note.step);
        let line = self.acting_line(cx);
        let current = trail.iter().position(|placed| placed.line == Some(line) || placed.target == Some(line));
        let next = match (current, forward) {
            (Some(index), true) => (index + 1) % trail.len(),
            (Some(index), false) => (index + trail.len() - 1) % trail.len(),
            (None, true) => 0,
            (None, false) => trail.len() - 1,
        };
        let Some(line) = trail[next].target.or(trail[next].line) else { return false };
        self.reveal_line(line, window, cx);
        true
    }

    /// Adds a `#?` line above the acting line and puts the cursor on it.
    pub(crate) fn add_question(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.state.read(cx).value().to_string();
        let line = self.acting_line(cx);
        let Some(range) = line_range(&text, line) else { return };
        let code = &text[range.clone()];
        let indent = &code[..code.len() - code.trim_start().len()];
        let marker = format!("{indent}{}? ", notes::comment_prefix(self.language));
        let at = range.start + marker.len();
        self.state.update(cx, |editor, cx| {
            editor.set_selected_range(range.start..range.start, cx);
            editor.replace(format!("{marker}\n"), window, cx);
            editor.set_selected_range(at..at, cx);
            editor.clear_hover_state(cx);
            editor.focus(window, cx);
        });
    }

    /// Removes the AI note on the acting line, or every AI note about the code there.
    pub(crate) fn resolve_note(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let line = self.acting_line(cx);
        let file_line = self.notes.hidden.file_line(line);
        let file = self.file_text(cx);
        let parsed = notes::parse(&file, self.language);
        let on_note = parsed.iter().any(|note| note.line == file_line);
        let removed: Vec<usize> = parsed.iter().filter(|note| note.kind.ai() && if on_note { note.line == file_line } else { note.target == Some(file_line) }).map(|note| note.line).collect();
        if removed.is_empty() { return; }
        self.apply_file(without_lines(&file, &removed), window, cx);
        self.state.update(cx, |editor, cx| editor.clear_hover_state(cx));
    }

    /// Removes every AI note, keeping questions and comments. Returns how many.
    pub(crate) fn clear_ai_notes(&mut self, window: &mut Window, cx: &mut Context<Self>) -> usize {
        let file = self.file_text(cx);
        let removed: Vec<usize> = notes::parse(&file, self.language).iter().filter(|note| note.kind.ai()).map(|note| note.line).collect();
        if removed.is_empty() { return 0; }
        self.apply_file(without_lines(&file, &removed), window, cx);
        removed.len()
    }
}

fn without_lines(text: &str, removed: &[usize]) -> String {
    text.split('\n').enumerate().filter(|(index, _)| !removed.contains(index)).map(|(_, line)| line).collect::<Vec<_>>().join("\n")
}

impl Workspace {
    /// The solution file, including notes the view hides.
    pub fn solution_text(&self, cx: &App) -> String { self.editor_pane.read(cx).file_text(cx) }

    /// The solution without `#!` and `#?` lines, for judges.
    pub fn submission_text(&self, cx: &App) -> String { notes::strip(&self.solution_text(cx)) }

    pub fn load_solution(&mut self, text: String, window: &mut Window, cx: &mut Context<Self>) {
        self.editor_pane.update(cx, |pane, cx| pane.load_file(text, window, cx));
    }

    pub fn set_notes_view(&mut self, view: View, window: &mut Window, cx: &mut Context<Self>) {
        self.config.notes_view = view;
        self.save_config(window, cx);
        for pane in self.tab_panes().into_iter().chain([self.editor_pane.clone()]) { pane.update(cx, |pane, cx| pane.set_notes_view(view, window, cx)); }
        self.flash(format!("Notes: {}", view.label()), cx);
        cx.notify();
    }

    pub(crate) fn step_trail(&mut self, forward: bool, window: &mut Window, cx: &mut Context<Self>) {
        if !self.editor_pane.update(cx, |pane, cx| pane.step_trail(forward, window, cx)) { self.flash("No reasoning trail", cx); }
    }

    pub(crate) fn clear_ai_notes(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let removed = self.editor_pane.update(cx, |pane, cx| pane.clear_ai_notes(window, cx));
        if removed == 0 { self.flash("No AI notes", cx); return; }
        self.save_now(cx);
        self.flash(if removed == 1 { "Cleared 1 AI note".to_owned() } else { format!("Cleared {removed} AI notes") }, cx);
    }

    pub(crate) fn has_ai_notes(&self, cx: &App) -> bool {
        let Some(session) = &self.session else { return false };
        notes::parse(&self.solution_text(cx), session.language).iter().any(|note| note.kind.ai())
    }

    /// Writes an answer's notes into the open solution, replacing notes from earlier runs.
    pub(crate) fn write_notes(&mut self, slug: &str, language: Language, seen: &str, answer: &practice::assist::Answer, window: &mut Window, cx: &mut Context<Self>) {
        if !self.session.as_ref().is_some_and(|session| session.slug == slug && session.language == language && session.question.is_some()) { return; }
        let current = self.solution_text(cx);
        let written = notes::write(&current, seen, &notes::specs(answer), notes::replaces(answer), language);
        if written == current { return; }
        self.editor_pane.update(cx, |pane, cx| pane.apply_file(written, window, cx));
        self.save_now(cx);
    }

    /// Adds an analysis's per-line costs to those lines' hovers.
    pub(crate) fn show_costs(&mut self, slug: &str, language: Language, seen: &str, answer: &practice::assist::Answer, cx: &mut Context<Self>) {
        use practice::assist::Answer;
        let lines = match answer { Answer::Analyze(analysis) => &analysis.lines, Answer::Review(review) => &review.complexity.lines, _ => return };
        if !self.session.as_ref().is_some_and(|session| session.slug == slug && session.language == language) { return; }
        let items = lines.iter().filter_map(|cost| Some((cost.line.checked_sub(1)? as usize, cost.cost.clone()))).collect();
        self.editor_pane.update(cx, |pane, cx| pane.set_badges(BadgeKind::Cost, seen, true, items, cx));
    }

    /// Hooks note hovers up to the chat's explanations.
    pub(crate) fn connect_notes(&self, cx: &mut App) {
        let assist = self.assist.downgrade();
        let detail: Detail = Rc::new(move |text, cx| assist.upgrade()?.read(cx).note_detail(text));
        *self.editor_pane.read(cx).notes.provider.detail.borrow_mut() = Some(detail);
    }

    /// Writes a reply below the `#?` question it answers.
    pub(crate) fn answer_question(&mut self, slug: &str, language: Language, question: &str, reply: &str, window: &mut Window, cx: &mut Context<Self>) {
        if !self.session.as_ref().is_some_and(|session| session.slug == slug && session.language == language) { return; }
        let Some(written) = notes::answer(&self.solution_text(cx), question, reply, language) else { return };
        self.editor_pane.update(cx, |pane, cx| pane.apply_file(written, window, cx));
        self.save_now(cx);
    }

    /// Sends the `#?` question on the cursor line to the AI; the reply lands below it.
    pub(crate) fn ask_in_place(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(question) = self.editor_pane.read(cx).question_at_cursor(cx) else { self.flash("Put the cursor on a question", cx); return };
        let target = self.assist.read(cx).target(&self.config);
        if matches!(target, crate::assist::Target::Web(_)) { self.flash("Asking in place needs a local agent", cx); return; }
        self.save_now(cx);
        let Some(snapshot) = self.assist_snapshot(cx) else { return };
        let view = self.editor_pane.read(cx).state.read(cx).value().to_string();
        let code = question.target.and_then(|line| view.split('\n').nth(line)).map(str::trim).unwrap_or_default().to_owned();
        let marker = format!("{}?", notes::comment_prefix(snapshot.language));
        let prompt = format!("{}\n\nThis is my `{marker}` question written above `{code}` in the code. Answer in at most two plain sentences without code blocks.", question.text);
        self.right = true;
        self.save_layout();
        self.flash("AI: answering in place", cx);
        self.assist.update(cx, |assist, cx| assist.ask_in_place(question.text, prompt, snapshot, target, window, cx));
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::{line_of, line_range};

    #[test]
    fn line_ranges_exclude_newlines() {
        let text = "a\nbc\n\nd";
        assert_eq!(line_range(text, 0), Some(0..1));
        assert_eq!(line_range(text, 1), Some(2..4));
        assert_eq!(line_range(text, 2), Some(5..5));
        assert_eq!(line_range(text, 3), Some(6..7));
        assert_eq!(line_range(text, 4), None);
        assert_eq!(line_of(text, 3), 1);
    }
}
