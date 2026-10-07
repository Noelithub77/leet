//! The custom test dialog: input and expected output, saved with ctrl+enter.

use std::rc::Rc;

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{InputEvent, Textarea, TextareaState};
use gpui_kit::component::{ActiveTheme as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::*;
use gpui_kit::prelude::FluentBuilder as _;

use crate::view::key;
use crate::workspace::Workspace;

/// Closes the dialog and runs `f` on the workspace with editor focus restored.
fn after_close(
    ws: WeakEntity<Workspace>,
    window: &mut Window,
    cx: &mut App,
    f: impl FnOnce(&mut Workspace, &mut Window, &mut Context<Workspace>) + 'static,
) {
    window.close_dialog(cx);
    window.defer(cx, move |window, cx| {
        let _ = ws.update(cx, |ws, cx| {
            ws.focus_editor(window, cx);
            f(ws, window, cx);
        });
    });
}

pub fn open_custom_test(ws: &mut Workspace, window: &mut Window, cx: &mut Context<Workspace>) {
    open_test_case(ws, None, window, cx);
}

pub fn edit_test_case(ws: &mut Workspace, window: &mut Window, cx: &mut Context<Workspace>) {
    let Some(index) = ws.session.as_ref().map(|s| s.selected_case) else { return; };
    open_test_case(ws, Some(index), window, cx);
}

fn open_test_case(ws: &mut Workspace, index: Option<usize>, window: &mut Window, cx: &mut Context<Workspace>) {
    if ws.session.as_ref().and_then(|s| s.question.as_ref()).is_none() {
        return;
    }
    let placeholder = ws
        .session
        .as_ref()
        .and_then(|s| s.cases.first())
        .map(|c| c.input.clone())
        .unwrap_or_default();
    let input = cx.new(|cx| TextareaState::new(window, cx).rows(4).placeholder(placeholder));
    let expected = cx.new(|cx| TextareaState::new(window, cx).rows(2).placeholder("Expected output (optional)"));
    let Some(session) = ws.session.as_ref() else { return; };
    let origin = session.slug.clone();
    if let Some(case) = index.and_then(|index| session.cases.get(index)) {
        input.update(cx, |state, cx| state.set_value(case.input.clone(), window, cx));
        expected.update(cx, |state, cx| state.set_value(case.expected.clone().unwrap_or_default(), window, cx));
    }
    let original = index.and_then(|index| session.question.as_ref().and_then(|q| q.examples.get(index).map(|input| (input.clone(), q.outputs.get(index).cloned().unwrap_or_default()))));
    let is_stdin = session.source == practice::language::Source::Codeforces;
    let weak = cx.weak_entity();
    let save: Rc<dyn Fn(&mut Window, &mut App)> = {
        let (input, expected, weak) = (input.clone(), expected.clone(), weak.clone());
        let origin = origin.clone();
        Rc::new(move |window, cx| {
            let i = input.read(cx).value().trim().to_string();
            let e = expected.read(cx).value().trim().to_string();
            if i.is_empty() {
                return;
            }
            let origin = origin.clone();
            after_close(weak.clone(), window, cx, move |ws, _, cx| {
                if ws.session.as_ref().is_some_and(|session| session.slug == origin) {
                    ws.save_test_case(index, i, e, cx);
                }
            });
        })
    };
    for state in [&input, &expected] {
        let save = save.clone();
        cx.subscribe_in(state, window, move |_, _, event: &InputEvent, window, cx| {
            if let InputEvent::PressEnter { secondary: true, .. } = event {
                save(window, cx);
            }
        })
        .detach();
    }
    let focus = input.clone();
    window.defer(cx, move |window, cx| focus.update(cx, |s, cx| s.focus(window, cx)));
    window.open_dialog(cx, move |dialog, _, cx| {
        let muted = cx.theme().muted_foreground;
        dialog.title(if index.is_some() { "Edit test case" } else { "New test case" }).w(px(520.)).child(
            v_flex()
                .gap_2()
                .child(div().text_xs().text_color(muted).child(if is_stdin { "Input · stdin" } else { "Input · one argument per line" }))
                .child(Textarea::new(&input))
                .child(div().text_xs().text_color(muted).child("Expected"))
                .child(Textarea::new(&expected))
                .child(h_flex().gap_2().text_xs().text_color(muted).child(key("ctrl-enter")).child("save").child(key("escape")).child("cancel"))
                .child(h_flex().gap_2().justify_end()
                    .when_some(original.clone(), |row, (initial_input, initial_expected)| row.child(
                        Button::new("reset-selected-case").ghost().label("Use original example")
                            .on_click({ let input = input.clone(); let expected = expected.clone(); move |_, window, cx| {
                                input.update(cx, |state, cx| state.set_value(initial_input.clone(), window, cx));
                                expected.update(cx, |state, cx| state.set_value(initial_expected.clone(), window, cx));
                            }})))
                    .child(Button::new("save-test-case").label("Save").on_click({ let save = save.clone(); move |_, window, cx| save(window, cx) }))),
        )
    });
}
