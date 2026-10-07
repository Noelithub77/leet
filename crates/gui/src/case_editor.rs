//! Edit test values in the results panel without leaving the current case.

use gpui_kit::assets::IconName;
use gpui_kit::base::Disableable as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Textarea, TextareaState};
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::*;
use practice::runner::Case;
use crate::workspace::{Judge, Workspace};

gpui_kit::actions!(case_editor, [SaveCaseField, CancelCaseField]);
pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("ctrl-enter", SaveCaseField, Some("CaseField")),
        KeyBinding::new("ctrl-enter", SaveCaseField, Some("CaseField > Input")),
        KeyBinding::new("escape", CancelCaseField, Some("CaseField")),
        KeyBinding::new("escape", CancelCaseField, Some("CaseField > Input")),
    ]);
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Field { Input, Expected }
impl Field {
    fn label(self) -> &'static str { match self { Self::Input => "Input", Self::Expected => "Expected" } }
    fn id(self) -> &'static str { match self { Self::Input => "case-input", Self::Expected => "case-expected" } }
}

pub struct Draft {
    slug: String,
    index: usize,
    original: Case,
    field: Field,
    input: Entity<TextareaState>,
}
impl Draft {
    fn matches(&self, ws: &Workspace) -> bool {
        ws.session.as_ref().is_some_and(|session| session.slug == self.slug
            && session.selected_case == self.index
            && session.cases.get(self.index).is_some_and(|case| same_case(case, &self.original)))
    }
}

fn same_case(case: &Case, original: &Case) -> bool {
    case.id == original.id && case.input == original.input
        && case.expected == original.expected && case.custom == original.custom
}

pub fn begin(ws: &mut Workspace, field: Field, window: &mut Window, cx: &mut Context<Workspace>) {
    if !save(ws, window, cx) { return; }
    let Some(session) = &ws.session else { return; };
    if session.running || matches!(session.judge, Some(Judge::Running { .. })) { return; }
    let Some(case) = session.cases.get(session.selected_case) else { return; };
    let value = match field { Field::Input => case.input.clone(), Field::Expected => case.expected.clone().unwrap_or_default() };
    let input = cx.new(|cx| TextareaState::new(window, cx).rows(value.lines().count().clamp(1, 6)).default_value(value));
    ws.case_edit = Some(Draft { slug: session.slug.clone(), index: session.selected_case, original: case.clone(), field, input: input.clone() });
    input.update(cx, |input, cx| input.focus(window, cx));
    cx.notify();
}

pub fn save(ws: &mut Workspace, window: &mut Window, cx: &mut Context<Workspace>) -> bool {
    let Some(draft) = &ws.case_edit else { return true; };
    if !draft.matches(ws) {
        ws.case_edit = None;
        cx.notify();
        return true;
    }
    let value = draft.input.read(cx).value().to_string();
    let (input, expected) = match draft.field {
        Field::Input => (value, draft.original.expected.clone().unwrap_or_default()),
        Field::Expected => (draft.original.input.clone(), value),
    };
    if input.trim().is_empty() {
        ws.flash("Input cannot be empty", cx);
        return false;
    }
    let index = draft.index;
    if input != draft.original.input || expected != draft.original.expected.clone().unwrap_or_default() {
        if !ws.save_test_case(Some(index), input, expected, cx) { return false; }
    }
    ws.case_edit = None;
    ws.focus_editor(window, cx);
    cx.notify();
    true
}

pub fn field(ws: &Workspace, field: Field, value: String, cx: &mut Context<Workspace>) -> AnyElement {
    let theme = cx.theme().clone();
    let busy = ws.session.as_ref().is_some_and(|s| s.running || matches!(s.judge, Some(Judge::Running { .. })));
    let draft = ws.case_edit.as_ref().filter(|draft| draft.field == field && draft.matches(ws));
    let mut body = h_flex().id(field.id()).min_w_0().items_start().gap_1().p_2().rounded_md().bg(theme.muted)
        .font_family(theme.mono_font_family.clone()).text_xs();
    if let Some(draft) = draft {
        body = body.key_context("CaseField")
            .on_action(cx.listener(|ws, _: &SaveCaseField, window, cx| { save(ws, window, cx); }))
            .on_action(cx.listener(|ws, _: &CancelCaseField, window, cx| {
                ws.case_edit = None; ws.focus_editor(window, cx); cx.notify();
            }))
            .child(div().min_w_0().flex_1().child(Textarea::new(&draft.input).bordered(false).appearance(false).small()))
            .child(Button::new((field.id(), 0usize)).ghost().xsmall().icon(IconName::Check)
                .accessibility_label("Save test value").tooltip("Save · Ctrl+Enter").disabled(busy)
                .on_click(cx.listener(|ws, _, window, cx| { save(ws, window, cx); })));
    } else {
        body = body.cursor_text()
            .on_click(cx.listener(move |ws, event: &ClickEvent, window, cx| {
                if event.click_count() == 2 { begin(ws, field, window, cx); }
            }))
            .child(div().min_w_0().flex_1().child(if value.is_empty() { "—".to_owned() } else { value }))
            .child(Button::new((field.id(), 1usize)).ghost().xsmall().icon(IconName::Pencil)
                .accessibility_label(format!("Edit {}", field.label())).tooltip("Edit · double-click")
                .disabled(busy).on_click(cx.listener(move |ws, _, window, cx| {
                    cx.stop_propagation(); begin(ws, field, window, cx);
                })));
    }
    v_flex().gap_1().min_w_0().flex_1()
        .child(div().text_xs().text_color(theme.muted_foreground).child(field.label()))
        .child(body).into_any_element()
}

#[cfg(test)]
mod tests {
    use super::same_case;
    use practice::runner::Case;
    #[test]
    fn a_draft_does_not_overwrite_replaced_cases() {
        let original = Case { id: 0, input: "[1,2]".into(), expected: Some("3".into()), custom: false };
        assert!(same_case(&original, &original));
        let mut replaced = original.clone();
        replaced.input = "[3,4]".into();
        assert!(!same_case(&replaced, &original));
        replaced = original.clone(); replaced.expected = None;
        assert!(!same_case(&replaced, &original));
        replaced = original.clone(); replaced.custom = true;
        assert!(!same_case(&replaced, &original));
    }
}
