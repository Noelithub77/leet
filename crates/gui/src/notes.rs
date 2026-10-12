//! Notes in the solution editor: the Rich, Raw and Off views, their decorations,
//! hovers and code actions, and the edits that write AI notes into the file.
use std::{cell::{Cell, RefCell}, ops::Range, rc::Rc, time::Duration};
use anyhow::Result;
use gpui_kit::*;
use gpui_kit::component::{ActiveTheme as _, Theme};
use gpui_kit::base::input::LineEndHint;
use gpui_kit::component::input::{CodeActionProvider, EditorState, HoverProvider, Position, RangeDecoration, RangeDecorationCollection,
    RangeDecorationStyle, Rope, RopeExt as _, TextDecoration, TextDecorationCollection};
use practice::{language::Language, notes::{self, Hidden, Kind, Note, View}};
use crate::actions::{AddQuestion, AskInPlace, ClearAiNotes, ResolveNote};
use crate::snippets::expansion::EditorPane;
use crate::workspace::Workspace;

#[cfg(feature = "gui-test")]
#[path = "gui_test/notes.rs"]
pub(crate) mod fixture;

/// The full explanation behind a note's text, when the chat still has it.
pub(crate) type Detail = Rc<dyn Fn(&str, &App) -> Option<String>>;

pub(crate) struct Notes {
    view: View,
    /// Note lines removed from the editor in the Off view.
    hidden: Hidden,
    /// The editor text the hidden lines were last placed against.
    shown: String,
    styles: Option<TextDecorationCollection>,
    marks: Option<RangeDecorationCollection>,
    flash: Option<RangeDecorationCollection>,
    flash_task: Task<()>,
    badges: Vec<Badge>,
    pub(crate) provider: Rc<Provider>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum BadgeKind { Cost, Trace }

/// A line-end label tied to a line of earlier code, found again after edits.
struct Badge {
    kind: BadgeKind,
    seen: Rc<str>,
    /// `seen` is a whole file; otherwise it is editor text.
    file: bool,
    line: usize,
    text: SharedString,
}

impl Notes {
    pub(crate) fn new(view: View, language: Language) -> Self {
        let provider = Rc::new(Provider { view: Cell::new(view), language: Cell::new(language), ai: Cell::new(false), detail: RefCell::new(None), upstream: RefCell::new(None) });
        Self { view, hidden: Hidden::default(), shown: String::new(), styles: None, marks: None, flash: None, flash_task: Task::ready(()), badges: Vec::new(), provider }
    }
}

/// Hovers and code actions for notes. Hovers elsewhere go to the language server.
pub(crate) struct Provider {
    view: Cell<View>,
    language: Cell<Language>,
    /// The file has AI notes, including hidden ones.
    ai: Cell<bool>,
    pub(crate) detail: RefCell<Option<Detail>>,
    upstream: RefCell<Option<Rc<dyn HoverProvider>>>,
}

impl HoverProvider for Provider {
    fn hover(&self, text: &Rope, offset: usize, window: &mut Window, cx: &mut App) -> Task<Result<Option<lsp_types::Hover>>> {
        if self.view.get() == View::Rich {
            let code = text.to_string();
            let found = notes::parse(&code, self.language.get()).into_iter().filter(|note| note.kind != Kind::Comment).find_map(|note| {
                if let Some(anchor) = note.anchor.clone().filter(|anchor| anchor.contains(&offset)) { return Some((anchor, note)); }
                note.comment.contains(&offset).then(|| (note.comment.clone(), note))
            });
            if let Some((range, note)) = found {
                let detail = self.detail.borrow().as_ref().and_then(|detail| detail(&note.text, cx));
                let value = hover_text(&note, detail.as_deref());
                let range = lsp_types::Range { start: text.offset_to_position(range.start), end: text.offset_to_position(range.end) };
                let contents = lsp_types::HoverContents::Markup(lsp_types::MarkupContent { kind: lsp_types::MarkupKind::Markdown, value });
                return Task::ready(Ok(Some(lsp_types::Hover { contents, range: Some(range) })));
            }
        }
        let upstream = self.upstream.borrow().clone();
        match upstream { Some(upstream) => upstream.hover(text, offset, window, cx), None => Task::ready(Ok(None)) }
    }
}

fn hover_text(note: &Note, detail: Option<&str>) -> String {
    let title = match (note.kind, note.step) {
        (Kind::Ask, _) => return "**Your question** · alt+enter asks the AI".into(),
        (Kind::Stuck, Some((step, of))) => format!("Stuck · step {step} of {of}"),
        (kind, _) => { let tag = kind.tag(); format!("{}{}", tag[..1].to_uppercase(), &tag[1..]) }
    };
    let mut text = format!("**{title}** · {}", note.text);
    if let Some(detail) = detail.filter(|detail| !detail.trim().is_empty()) { text.push_str("\n\n"); text.push_str(detail); }
    if note.outdated() { text.push_str("\n\n_The quoted code has changed._"); }
    text
}

const ACTIONS: &str = "notes";

impl CodeActionProvider for Provider {
    fn id(&self) -> SharedString { ACTIONS.into() }
    fn code_actions(&self, editor: Entity<EditorState>, range: Range<usize>, _: &mut Window, cx: &mut App) -> Task<Result<Vec<lsp_types::CodeAction>>> {
        let text = editor.read(cx).value().to_string();
        let line = text[..range.start.min(text.len())].matches('\n').count();
        let parsed = notes::parse(&text, self.language.get());
        let action = |title: &str, id: &str| lsp_types::CodeAction { title: title.into(), data: Some(serde_json::Value::from(id)), ..Default::default() };
        let mut actions = Vec::new();
        match parsed.iter().find(|note| note.line == line).map(|note| note.kind) {
            Some(Kind::Ask) => actions.push(action("Ask AI", "ask")),
            Some(kind) if kind.ai() => actions.push(action("Resolve note", "resolve")),
            Some(_) => {}
            None => if text.split('\n').nth(line).is_some_and(|code| !code.trim().is_empty()) { actions.push(action("Ask about this line", "question")); },
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

fn color(kind: Kind, theme: &Theme) -> Option<Hsla> {
    match kind {
        Kind::Bug => Some(theme.danger),
        Kind::Risk | Kind::Slow => Some(theme.warning),
        Kind::Stuck => Some(theme.info),
        Kind::Hint => Some(theme.magenta),
        Kind::Answer => Some(theme.success),
        Kind::Ask => Some(theme.cyan),
        Kind::Comment => None,
    }
}

/// Text styles for markers and comments, and fills for the code each note is about.
fn decorations(parsed: &[Note], theme: &Theme) -> (Vec<TextDecoration>, Vec<RangeDecoration>) {
    let (mut styles, mut marks) = (Vec::new(), Vec::new());
    for note in parsed {
        let Some(color) = color(note.kind, theme) else { continue };
        let fade = note.outdated().then_some(0.5);
        styles.push(TextDecoration::new(note.marker.clone(), HighlightStyle { color: Some(color), font_weight: Some(FontWeight::SEMIBOLD), fade_out: fade, ..Default::default() }));
        styles.push(TextDecoration::new(note.marker.end..note.comment.end, HighlightStyle { color: Some(theme.foreground.opacity(0.78)), font_style: Some(FontStyle::Italic), fade_out: fade, ..Default::default() }));
        let Some(anchor) = note.anchor.clone().filter(|_| note.kind.ai() && note.snippet.is_some()) else { continue };
        if note.kind == Kind::Bug {
            styles.push(TextDecoration::new(anchor.clone(), HighlightStyle { underline: Some(UnderlineStyle { thickness: px(1.), color: Some(color), wavy: true }), ..Default::default() }));
        }
        marks.push(RangeDecoration::new(anchor).with_style(RangeDecorationStyle::Fill).with_color(color.opacity(0.14)));
    }
    (styles, marks)
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
        let (view, hidden) = if self.notes.view == View::Off { notes::hide(&file) } else { (file, Hidden::default()) };
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
        let view = if self.notes.view == View::Off {
            let (view, hidden) = notes::hide(&file);
            if view == current { self.notes.hidden = hidden; self.refresh_notes(cx); return; }
            self.notes.hidden = hidden;
            view
        } else { file };
        if view == current { return; }
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
        if (previous == View::Off) == (view == View::Off) { self.refresh_notes(cx); return; }
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
        let parsed = notes::parse(&text, self.language);
        let provider = &self.notes.provider;
        provider.language.set(self.language);
        provider.ai.set(if self.notes.view == View::Off { notes::parse(&self.file_text(cx), self.language).iter().any(|note| note.kind.ai()) } else { parsed.iter().any(|note| note.kind.ai()) });
        let (styles, marks) = if self.notes.view == View::Rich { decorations(&parsed, cx.theme()) } else { (Vec::new(), Vec::new()) };
        match &self.notes.styles {
            Some(collection) => collection.set(styles, cx),
            None => self.notes.styles = Some(self.state.update(cx, |editor, cx| editor.create_decorations_collection(styles, cx))),
        }
        match &self.notes.marks {
            Some(collection) => collection.set(marks, cx),
            None => self.notes.marks = Some(self.state.update(cx, |editor, cx| editor.create_range_decorations_collection(marks, cx))),
        }
        let hints = if self.notes.view == View::Rich { self.badge_hints(&text, cx) } else { Vec::new() };
        if hints.as_slice() != self.state.read(cx).line_end_hints() { self.state.update(cx, |editor, cx| editor.set_line_end_hints(hints, cx)); }
    }

    /// Replaces one kind of badge. Lines are zero-based in `seen`.
    pub(crate) fn set_badges(&mut self, kind: BadgeKind, seen: &str, file: bool, items: Vec<(usize, String)>, cx: &mut Context<Self>) {
        let seen: Rc<str> = seen.into();
        self.notes.badges.retain(|badge| badge.kind != kind);
        self.notes.badges.extend(items.into_iter().map(|(line, text)| Badge { kind, seen: seen.clone(), file, line, text: text.into() }));
        self.refresh_notes(cx);
    }

    fn badge_hints(&self, view: &str, cx: &App) -> Vec<LineEndHint> {
        if self.notes.badges.is_empty() { return Vec::new(); }
        let file = self.notes.hidden.restore(view);
        let theme = cx.theme();
        let mut hints: Vec<LineEndHint> = self.notes.badges.iter().filter_map(|badge| {
            let line = if badge.file { self.view_line(notes::relocate(&badge.seen, &file, badge.line, None)?) } else { notes::relocate(&badge.seen, view, badge.line, None)? };
            let color = match badge.kind { BadgeKind::Cost => theme.warning.opacity(0.85), BadgeKind::Trace => theme.info.opacity(0.85) };
            Some(LineEndHint { line, text: badge.text.clone(), color })
        }).collect();
        hints.sort_by_key(|hint| hint.line);
        hints
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

    /// The `#?` question on the cursor line.
    pub(crate) fn question_at_cursor(&self, cx: &App) -> Option<Note> {
        let editor = self.state.read(cx);
        let text = editor.value().to_string();
        let line = line_of(&text, editor.cursor());
        notes::parse(&text, self.language).into_iter().find(|note| note.kind == Kind::Ask && note.line == line && !note.text.is_empty())
    }

    /// Steps through a reasoning trail, wrapping at either end.
    pub(crate) fn step_trail(&mut self, forward: bool, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let text = self.state.read(cx).value().to_string();
        let mut trail: Vec<Note> = notes::parse(&text, self.language).into_iter().filter(|note| note.step.is_some()).collect();
        if trail.is_empty() { return false; }
        trail.sort_by_key(|note| note.step);
        let line = line_of(&text, self.state.read(cx).cursor());
        let current = trail.iter().position(|note| note.line == line || note.target == Some(line));
        let next = match (current, forward) {
            (Some(index), true) => (index + 1) % trail.len(),
            (Some(index), false) => (index + trail.len() - 1) % trail.len(),
            (None, true) => 0,
            (None, false) => trail.len() - 1,
        };
        let note = &trail[next];
        self.reveal_line(note.target.unwrap_or(note.line), window, cx);
        true
    }

    /// Adds a `#?` line above the cursor line and puts the cursor on it.
    pub(crate) fn add_question(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.state.read(cx).value().to_string();
        let line = line_of(&text, self.state.read(cx).cursor());
        let Some(range) = line_range(&text, line) else { return };
        let code = &text[range.clone()];
        let indent = &code[..code.len() - code.trim_start().len()];
        let marker = format!("{indent}{}? ", notes::comment_prefix(self.language));
        let at = range.start + marker.len();
        self.state.update(cx, |editor, cx| {
            editor.set_selected_range(range.start..range.start, cx);
            editor.replace(format!("{marker}\n"), window, cx);
            editor.set_selected_range(at..at, cx);
            editor.focus(window, cx);
        });
    }

    /// Removes the AI note on the cursor line.
    pub(crate) fn resolve_note(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.state.read(cx).value().to_string();
        let line = line_of(&text, self.state.read(cx).cursor());
        if !notes::parse(&text, self.language).iter().any(|note| note.line == line && note.kind.ai()) { return; }
        let Some(range) = line_range(&text, line) else { return };
        let range = if range.end < text.len() { range.start..range.end + 1 } else { range.start.saturating_sub(1)..range.end };
        self.state.update(cx, |editor, cx| { editor.set_selected_range(range, cx); editor.replace("", window, cx); });
    }

    /// Removes every AI note, keeping questions and comments. Returns how many.
    pub(crate) fn clear_ai_notes(&mut self, window: &mut Window, cx: &mut Context<Self>) -> usize {
        let file = self.file_text(cx);
        let parsed = notes::parse(&file, self.language);
        let removed: Vec<usize> = parsed.iter().filter(|note| note.kind.ai()).map(|note| note.line).collect();
        if removed.is_empty() { return 0; }
        let kept: Vec<&str> = file.split('\n').enumerate().filter(|(index, _)| !removed.contains(index)).map(|(_, line)| line).collect();
        self.apply_file(kept.join("\n"), window, cx);
        removed.len()
    }
}

impl Workspace {
    /// The solution file, including notes hidden in the Off view.
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

    /// Shows an analysis's per-line costs at the end of those lines.
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
