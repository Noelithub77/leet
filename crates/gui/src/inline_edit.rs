//! Focused prompt-only edits, reviewed in a card at the selection before a single undoable replacement.
use std::{ops::Range, path::PathBuf};
use gpui_kit::*;
use gpui_kit::assets::IconName;
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Icon, Selectable as _, Sizable as _, h_flex, v_flex};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputState, InputEvent, RangeDecoration, RangeDecorationCollection, RangeDecorationStyle};
use gpui_kit::component::spinner::Spinner;
use gpui_kit::prelude::FluentBuilder as _;
use practice::{agents, focused_edit::{Edit, Problem}};
use crate::workspace::{Workspace, Center, EditorPane};

pub struct InlineEdit {
    workspace: WeakEntity<Workspace>, pane: Entity<EditorPane>, path: PathBuf,
    input: Entity<InputState>, before: String, file_before: String, range: Range<usize>,
    language: practice::language::Language, notes_view: practice::notes::View, problem: Problem,
    /// Tints the edit target while the card has focus and the editor's own selection is hidden.
    mark: Option<RangeDecorationCollection>,
    busy: bool, proposal: Option<Edit>, proposed_for: String, error: Option<String>, cancel: agents::Cancel,
    /// The open recent-instructions list and the entry Up/Down last recalled.
    show_history: bool, recalled: Option<usize>,
    task: Task<()>, _subscription: Subscription,
    #[cfg(feature = "gui-test")] pub(crate) fixture_reply: Option<String>,
    #[cfg(feature = "gui-test")] pub(crate) fixture_note: String,
}
impl Drop for InlineEdit { fn drop(&mut self) { self.cancel.cancel(); } }

impl Workspace {
    pub(crate) fn open_inline_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.center != Center::Editor || self.debug_mode { return; }
        if let Some(edit) = &self.inline_edit { edit.read(cx).input.read(cx).focus_handle(cx).focus(window, cx); return; }
        let Some(session) = &self.session else { return; };
        let path = session.path.clone(); let language = session.language; let notes_view = self.config.notes_view;
        let before = self.editor.read(cx).value().to_string(); let selected = self.editor.read(cx).selected_range();
        let range = if selected.is_empty() { 0..before.len() } else { selected };
        let problem = self.assist_snapshot(cx).map(|snapshot| Problem {
            title: snapshot.title, statement: practice::prompts::statement_markdown(&snapshot.statement_html),
            interface: snapshot.starter, cases: snapshot.cases,
        }).unwrap_or_default();
        self.selection_menu = None;
        let tint = cx.theme().magenta;
        let mark = (range != (0..before.len())).then(|| self.editor.update(cx, |editor, cx| editor.create_range_decorations_collection(vec![
            RangeDecoration::new(range.clone()).with_style(RangeDecorationStyle::Fill).with_color(tint.opacity(0.16)),
            RangeDecoration::new(range.clone()).with_style(RangeDecorationStyle::Frame).with_color(tint.opacity(0.6)),
        ], cx)));
        let pane = self.editor_pane.clone(); let file_before = self.solution_text(cx); let workspace = cx.weak_entity();
        let whole = mark.is_none();
        let edit = cx.new(|cx| {
            let input = cx.new(|cx| InputState::new(window, cx).placeholder(if whole { "Edit the solution…" } else { "Edit the selection…" }));
            let subscription = cx.subscribe_in(&input, window, |this: &mut InlineEdit, _, event, window, cx| match event {
                InputEvent::PressEnter { secondary: false, shift: false } => this.submit(window, cx),
                InputEvent::Change => cx.notify(),
                _ => {}
            });
            input.read(cx).focus_handle(cx).focus(window, cx);
            InlineEdit { workspace, pane, path, input, before, file_before, range, language, notes_view, problem, mark, show_history: false, recalled: None, busy: false, proposal: None, proposed_for: String::new(), error: None, cancel: agents::Cancel::default(), task: Task::ready(()), _subscription: subscription, #[cfg(feature = "gui-test")] fixture_reply: None, #[cfg(feature = "gui-test")] fixture_note: String::new() }
        });
        self.inline_edit = Some(edit); cx.notify();
    }

    pub(crate) fn open_inline_history(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_inline_edit(window, cx);
        if let Some(edit) = &self.inline_edit { edit.update(cx, |edit, cx| { edit.show_history = true; cx.notify(); }); }
    }

    pub(crate) fn close_inline_edit(&mut self, cx: &mut App) {
        let Some(edit) = self.inline_edit.take() else { return; };
        let edit = edit.read(cx);
        edit.cancel.cancel();
        if let Some(mark) = edit.mark.clone() { mark.dispose(cx); }
    }
}

const HISTORY_KEY: &str = "inline-edit-history";
const HISTORY_LIMIT: usize = 20;

pub(crate) fn load_history(db: &practice::db::Db) -> Vec<String> {
    db.get(HISTORY_KEY).ok().flatten().map(|text| text.lines().map(str::to_owned).take(HISTORY_LIMIT).collect()).unwrap_or_default()
}

fn remember(ws: &mut Workspace, instruction: &str) {
    ws.inline_history.retain(|entry| entry != instruction);
    ws.inline_history.insert(0, instruction.to_owned());
    ws.inline_history.truncate(HISTORY_LIMIT);
    let _ = ws.db.set(HISTORY_KEY, &ws.inline_history.join("\n"));
}

/// Places the card under the edit target, above it near the bottom, or at the top for whole-solution edits.
pub(crate) fn overlay(ws: &Workspace, cx: &App) -> Option<AnyElement> {
    let edit = ws.inline_edit.clone()?;
    let (range, whole) = { let edit = edit.read(cx); (edit.range.clone(), edit.mark.is_none()) };
    let editor = ws.editor.read(cx);
    let viewport = editor.input_bounds();
    let width = (viewport.size.width - px(16.)).min(px(560.)).max(px(240.));
    let text = editor.value();
    let margin = px(8.); let room = px(64.);
    let target = (!whole).then(|| {
        let start = text.get(..range.start)?.rfind('\n').map_or(0, |index| index + 1);
        let end = if text.get(..range.end)?.ends_with('\n') && range.end > range.start { range.end - 1 } else { range.end };
        Some((editor.range_to_bounds(&(start..start))?, editor.range_to_bounds(&(end..end))?))
    }).flatten();
    let left = target.map_or(viewport.left() + margin, |(start, _)| start.left())
        .min(viewport.right() - width - margin).max(viewport.left() + margin);
    let (y, anchor) = match target {
        Some((_, end)) if end.bottom() >= viewport.top() && end.bottom() <= viewport.bottom() - room => (end.bottom() + px(6.), Anchor::TopLeft),
        Some((start, _)) if start.top() >= viewport.top() + room && start.top() <= viewport.bottom() => (start.top() - px(6.), Anchor::BottomLeft),
        _ => (viewport.top() + margin, Anchor::TopLeft),
    };
    Some(deferred(anchored().position(point(left, y)).anchor(anchor).snap_to_window_with_margin(margin)
        .child(div().w(width).child(edit))).with_priority(1).into_any_element())
}

impl InlineEdit {
    fn close(&self, window: &mut Window, cx: &mut Context<Self>) {
        self.cancel.cancel(); let workspace = self.workspace.clone();
        window.defer(cx, move |window, cx| { let _ = workspace.update(cx, |ws, cx| { ws.close_inline_edit(cx); ws.focus_editor(window, cx); cx.notify(); }); });
    }
    /// Enter generates, or applies a proposal that still matches the instruction.
    fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let instruction = self.input.read(cx).value().trim().to_owned();
        if self.proposal.is_some() && instruction == self.proposed_for { self.apply(window, cx); } else { self.send(cx); }
    }
    fn stop(&mut self, cx: &mut Context<Self>) {
        self.cancel.cancel(); self.task = Task::ready(()); self.busy = false; cx.notify();
    }
    fn send(&mut self, cx: &mut Context<Self>) {
        if self.busy { return; }
        let instruction = self.input.read(cx).value().trim().to_owned(); if instruction.is_empty() { return; }
        let Some(workspace) = self.workspace.upgrade() else { return; };
        let ws = workspace.read(cx); let target = ws.assist.read(cx).target(&ws.config);
        let crate::assist::Target::Agent(agent, selection) = target else { self.error = Some("Choose an installed agent in the status bar.".into()); cx.notify(); return; };
        let catalog = ws.assist.read(cx).catalog(agent.kind).cloned();
        workspace.update(cx, |ws, _| remember(ws, &instruction));
        self.show_history = false; self.recalled = None;
        let source = self.before.clone(); let range = self.range.clone(); let language = self.language; let problem = self.problem.clone();
        self.cancel.cancel(); self.cancel = agents::Cancel::default(); let cancel = self.cancel.clone();
        self.busy = true; self.error = None; self.proposal = None; self.proposed_for = instruction.clone();
        #[cfg(feature = "gui-test")]
        if let Some(reply) = self.fixture_reply.clone() {
            self.busy = false;
            match Edit::new(source, range, reply) { Ok(mut edit) => { edit.note = self.fixture_note.clone(); self.proposal = Some(edit) } Err(error) => self.error = Some(error.to_string()) }
            cx.notify(); return;
        }
        let job = cx.background_spawn(async move { practice::focused_edit::run(agent, selection, catalog, &source, range, &instruction, language.id(), &problem, &cancel, &mut |_| {}) });
        self.task = cx.spawn(async move |this, cx| {
            let result = job.await;
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                match result { Ok(edit) => this.proposal = Some(edit), Err(error) => this.error = Some(error.to_string()) }
                cx.notify();
            });
        }); cx.notify();
    }
    fn apply(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(proposal) = &self.proposal else { return; };
        let Some(workspace) = self.workspace.upgrade() else { return; };
        let result = workspace.update(cx, |ws, cx| -> anyhow::Result<()> {
            anyhow::ensure!(ws.editor_pane == self.pane && ws.session.as_ref().is_some_and(|session| session.path == self.path) && ws.solution_text(cx) == self.file_before, "Code changed. Discard this suggestion and try again.");
            let after = proposal.apply(ws.editor.read(cx).value().as_str())?;
            let (_, mut hidden) = practice::notes::hide(&self.file_before, self.notes_view);
            hidden.edited(&self.before, &after);
            let after = hidden.restore(&after);
            self.pane.update(cx, |pane, cx| pane.replace_file(after, window, cx));
            Ok(())
        });
        match result { Ok(()) => self.close(window, cx), Err(error) => { self.error = Some(error.to_string()); cx.notify(); } }
    }
    fn history(&self, cx: &App) -> Vec<String> {
        self.workspace.upgrade().map(|workspace| workspace.read(cx).inline_history.clone()).unwrap_or_default()
    }
    fn pick(&mut self, instruction: String, window: &mut Window, cx: &mut Context<Self>) {
        self.input.update(cx, |input, cx| { input.set_value(instruction, window, cx); input.focus_handle(cx).focus(window, cx); });
        self.show_history = false; cx.notify();
    }
    /// Up/Down walk recent instructions like a shell; Down past the newest clears the input.
    fn recall(&mut self, step: isize, window: &mut Window, cx: &mut Context<Self>) {
        let history = self.history(cx);
        if history.is_empty() { return; }
        let next = match (self.recalled, step) { (None, 1) => return, (None, _) => Some(0), (Some(0), 1) => None, (Some(index), step) => Some((index as isize - step).clamp(0, history.len() as isize - 1) as usize) };
        self.recalled = next;
        let text = next.map(|index| history[index].clone()).unwrap_or_default();
        self.input.update(cx, |input, cx| input.set_value(text, window, cx));
        cx.notify();
    }
    #[cfg(feature = "gui-test")]
    pub(crate) fn mark(&self) -> Option<RangeDecorationCollection> { self.mark.clone() }
    #[cfg(feature = "gui-test")]
    pub(crate) fn error(&self) -> Option<String> { self.error.clone() }
    #[cfg(feature = "gui-test")]
    pub(crate) fn input_focused(&self, window: &Window, cx: &App) -> bool { self.input.read(cx).focus_handle(cx).is_focused(window) }
    #[cfg(feature = "gui-test")]
    pub(crate) fn instruction(&self, cx: &App) -> String { self.input.read(cx).value().to_string() }
    fn target_label(&self) -> String {
        if self.mark.is_none() { return "Solution".into(); }
        let first = self.before[..self.range.start].matches('\n').count() + 1;
        let last = first + self.before[self.range.clone()].trim_end_matches('\n').matches('\n').count();
        if first == last { format!("Line {first}") } else { format!("Lines {first}–{last}") }
    }
    fn agent(&self, cx: &App) -> Option<(agents::AgentKind, String)> {
        let workspace = self.workspace.upgrade()?; let ws = workspace.read(cx); let assist = ws.assist.read(cx);
        let crate::assist::Target::Agent(agent, selection) = assist.target(&ws.config) else { return None; };
        let model = assist.catalog(agent.kind).and_then(|catalog| {
            let id = selection.map(|selection| selection.model).or_else(|| catalog.default_model.clone())?;
            catalog.models.iter().find(|model| model.id == id).map(|model| model.label.clone())
        });
        Some((agent.kind, model.unwrap_or_else(|| agent.kind.label().to_owned())))
    }
}

impl Render for InlineEdit {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let empty = self.input.read(cx).value().trim().is_empty();
        let history = if self.show_history { self.history(cx) } else { Vec::new() };
        let has_history = self.show_history || self.workspace.upgrade().is_some_and(|workspace| !workspace.read(cx).inline_history.is_empty());
        let action = if self.busy {
            Button::new("inline-edit-stop").ghost().xsmall().icon(IconName::CircleStop).tooltip("Stop").accessibility_label("Stop edit")
                .on_click(cx.listener(|this, _, _, cx| this.stop(cx)))
        } else {
            Button::new("inline-edit-send").primary().xsmall().icon(IconName::ArrowUp).disabled(empty)
                .tooltip(if self.proposal.is_some() { "Regenerate · Enter" } else { "Send · Enter" }).accessibility_label("Send edit")
                .on_click(cx.listener(|this, _, _, cx| this.send(cx)))
        };
        let note = self.proposal.as_ref().map(|proposal| proposal.note.clone()).filter(|note| !note.is_empty());
        let diff = self.proposal.as_ref().map(Edit::diff).unwrap_or_default();
        let added = diff.iter().filter(|(kind, _)| *kind == '+').count();
        let removed = diff.iter().filter(|(kind, _)| *kind == '-').count();
        v_flex().id("inline-edit").test_support().w_full().p_2().gap_2().rounded_lg().occlude()
            .border_1().border_color(theme.magenta.opacity(0.45)).bg(theme.popover).shadow_lg().text_xs()
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if event.keystroke.key != "escape" { return; }
                if this.show_history { this.show_history = false; cx.notify(); } else { this.close(window, cx); }
                cx.stop_propagation();
            }))
            .capture_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                let step = match event.keystroke.key.as_str() { "up" => -1, "down" => 1, _ => return };
                if event.keystroke.modifiers.modified() || !this.input.read(cx).focus_handle(cx).is_focused(window) { return; }
                this.recall(step, window, cx); cx.stop_propagation();
            }))
            .child(h_flex().gap_1p5().items_center().min_w_0()
                .child(Icon::new(IconName::Sparkles).size_3p5().text_color(theme.magenta))
                .child(div().flex_shrink_0().text_color(theme.foreground).child(self.target_label()))
                .child(match self.agent(cx) {
                    Some((kind, label)) => h_flex().gap_1().items_center().min_w_0().text_color(theme.muted_foreground)
                        .child(crate::brand::agent_icon(kind).size_3()).child(div().truncate().child(label)).into_any_element(),
                    None => div().text_color(theme.danger).child("Choose an agent in the status bar").into_any_element(),
                })
                .child(div().flex_1())
                .child(Button::new("inline-edit-history-toggle").ghost().xsmall().icon(IconName::Clock).selected(self.show_history).disabled(!has_history)
                    .tooltip("Recent edits · Ctrl+Shift+I").accessibility_label("Recent edits")
                    .on_click(cx.listener(|this, _, _, cx| { this.show_history = !this.show_history; cx.notify(); })))
                .child(Button::new("inline-edit-discard").ghost().xsmall().icon(IconName::X).tooltip("Discard · Esc").accessibility_label("Discard edit")
                    .on_click(cx.listener(|this, _, window, cx| this.close(window, cx)))))
            .when_some(note, |view, note| view.child(h_flex().id("inline-edit-note").test_support().gap_1p5().items_start().px_1().text_color(theme.foreground)
                .child(Icon::new(IconName::MessageCircle).size_3p5().flex_shrink_0().text_color(theme.magenta)).child(div().min_w_0().child(note))))
            .child(Input::new(&self.input).small().suffix(action))
            .when(self.show_history, |view| view.child(v_flex().id("inline-edit-history").test_support().max_h(rems(12.)).overflow_y_scroll().p_0p5().rounded_md()
                .border_1().border_color(theme.border).bg(theme.background)
                .when(history.is_empty(), |list| list.child(div().px_2().py_1().text_color(theme.muted_foreground).child("No recent edits")))
                .children(history.into_iter().enumerate().map(|(index, instruction)| {
                    let recalled = self.recalled == Some(index);
                    div().id(("inline-edit-recent", index)).test_support().px_2().py_1().rounded_sm().truncate().cursor_pointer()
                        .when(recalled, |row| row.bg(theme.secondary)).hover(|row| row.bg(theme.secondary))
                        .child(instruction.clone())
                        .on_click(cx.listener(move |this, _, window, cx| this.pick(instruction.clone(), window, cx)))
                }))))
            .when(self.busy, |view| view.child(h_flex().gap_1p5().items_center().text_color(theme.muted_foreground).child(Spinner::new().xsmall()).child("Writing edit…")))
            .when_some(self.error.clone(), |view, error| view.child(h_flex().gap_1p5().items_start().text_color(theme.danger)
                .child(Icon::new(IconName::CircleX).size_3p5().flex_shrink_0()).child(div().min_w_0().child(error))))
            .when(self.proposal.is_some(), |view| view
                .child(v_flex().id("inline-edit-diff").test_support().max_h(rems(14.)).overflow_y_scroll().py_1().rounded_md()
                    .border_1().border_color(theme.border).bg(theme.background).font_family(theme.mono_font_family.clone())
                    .children(diff.into_iter().map(|(kind, line)| {
                        let (color, tint) = match kind { '-' => (theme.danger, theme.danger.opacity(0.1)), '+' => (theme.success, theme.success.opacity(0.1)), _ => (theme.muted_foreground, transparent_black()) };
                        h_flex().items_start().gap_2().px_2().bg(tint)
                            .child(div().w(px(8.)).flex_shrink_0().text_color(color).child(kind.to_string()))
                            .child(div().flex_1().min_w_0().whitespace_normal().text_color(if kind == ' ' { theme.muted_foreground } else { theme.foreground }).child(line))
                    })))
                .child(h_flex().gap_2().items_center()
                    .child(div().text_color(theme.success).child(format!("+{added}"))).child(div().text_color(theme.danger).child(format!("−{removed}")))
                    .child(div().flex_1())
                    .child(Button::new("inline-edit-apply").primary().xsmall().icon(IconName::Check).tooltip("Apply · Enter").accessibility_label("Apply edit")
                        .on_click(cx.listener(|this, _, window, cx| this.apply(window, cx))))))
    }
}
