//! Deterministic signature responses through the production caret and popup lifecycle.
use super::*;
use std::{cell::{Cell, RefCell}, rc::Rc, path::Path};
use anyhow::{Result, ensure};
use gpui_kit::component::input::{EditorState, Input, InputState, CompletionProvider};
use gpui_kit::test::TestWindowExt as _;

struct Fixture {
    pane: Entity<EditorPane>,
    other: Entity<InputState>,
}
struct CompletionFixture;
impl CompletionProvider for CompletionFixture {
    fn is_completion_trigger(&self, _: usize, _: &str, _: &mut App) -> bool { false }
    fn completions(&self, _: &ropey::Rope, _: usize, _: lsp_types::CompletionContext, _: &mut Window, _: &mut App) -> Task<Result<lsp_types::CompletionResponse>> {
        Task::ready(Ok(lsp_types::CompletionResponse::Array(["first", "second"].map(|label| lsp_types::CompletionItem { label: label.into(), kind: Some(lsp_types::CompletionItemKind::VARIABLE), ..Default::default() }).into())))
    }
}
impl Render for Fixture {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        v_flex().size_full().key_context(crate::actions::WORKSPACE)
            .child(div().flex_1().min_h_0().child(self.pane.clone()))
            .child(div().id("signature-other-control").test_support().child(Input::new(&self.other)))
    }
}

fn response(second: bool) -> SignatureHelp {
    serde_json::from_value(serde_json::json!({"activeSignature":0,"activeParameter":usize::from(second),"signatures":[
        {"label":"combine(first: int, second: int) -> int", "parameters":[{"label":"first: int","documentation":"First input value"},{"label":"second: int","documentation":"Second input value"}]},
        {"label":"combine(first: str, second: str) -> str", "parameters":[{"label":"first: str"},{"label":"second: str"}]}
    ]})).unwrap()
}

pub(crate) fn native(cx: &mut HeadlessAppContext, output: &Path, pixels: bool) -> Result<()> {
    for (name, width, height, theme, zoom) in [("dark", 900., 550., "Vesper", 1.), ("compact", 380., 350., "Vesper", 1.), ("light-zoom", 640., 420., "Catppuccin Latte", 1.4)] {
        let delayed = Rc::new(Cell::new(false)); let requests = Rc::new(RefCell::new(Vec::new()));
        let (handle, view) = cx.update(|cx| {
            crate::theme::apply(theme, cx); crate::theme::set_zoom(zoom, cx);
            let mut config = practice::config::Config::default(); config.keybindings.insert("NextSignature".into(), "ctrl-alt-shift-u".into());
            crate::actions::bind_keys(&config, cx);
            let requests = requests.clone(); let delayed = delayed.clone();
            gpui_kit::open_window(WindowOptions { window_bounds: Some(WindowBounds::Windowed(Bounds { origin: point(px(0.), px(0.)), size: size(px(width), px(height)) })), show: false, focus: false, ..Default::default() }, cx, |window, cx| cx.new(|cx| {
                let editor = cx.new(|cx| EditorState::new(window, cx).language("python").line_number(true));
                let pane = cx.new(|cx| {
                    let mut pane = EditorPane::new(editor.clone(), Language::Python, Rc::new(practice::snippets::search::Library::new(vec![])), Default::default(), window, cx);
                    pane.signature.fixture = Some(Rc::new(move |text, offset, context, cx| {
                        requests.borrow_mut().push((text.clone(), offset, context));
                        let help = response(text[..offset].contains(',')); let delay = delayed.get();
                        cx.spawn(async move |cx| { if delay { cx.background_executor().timer(Duration::from_millis(400)).await; } Ok(Some(help)) })
                    }));
                    pane
                });
                let other = cx.new(|cx| InputState::new(window, cx));
                editor.update(cx, |editor, cx| { editor.lsp_mut().completion_provider = Some(Rc::new(CompletionFixture)); editor.focus(window, cx); });
                Fixture { pane, other }
            }))
        })?;
        let pane = cx.update(|cx| view.read(cx).pane.clone()); let handle = handle.into(); cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| { window.activate_window(); window.render_frame(cx); window.input("combine", cx); window.input("(", cx); })?;
        cx.run_until_parked(); cx.advance_clock(Duration::from_millis(150)); cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| -> Result<()> {
            window.render_frame(cx);
            ensure!(window.find("parameter-hints").visible(), "Typing ( did not open parameter hints");
            ensure!(pane.read(cx).state.read(cx).focus_handle(cx).is_focused(window), "Hints stole editor focus");
            let bounds = window.find("signature-popup").bounds(); let editor = pane.read(cx).state.read(cx);
            ensure!(bounds.left() >= editor.input_bounds().left() && bounds.right() <= editor.input_bounds().right(), "Hints exceed editor width");
            ensure!(bounds.top() >= editor.input_bounds().top() && bounds.bottom() <= editor.input_bounds().bottom(), "Hints exceed editor height");
            let caret = editor.cursor_layout().unwrap().0;
            ensure!(bounds.bottom() <= caret.top() || bounds.top() >= caret.bottom(), "Hints cover the typing line");
            window.input("1, ", cx); Ok(())
        })??;
        cx.run_until_parked(); cx.advance_clock(Duration::from_millis(150)); cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| -> Result<()> {
            window.render_frame(cx);
            let help = pane.read(cx).signature.help.as_ref().unwrap();
            ensure!(signature::active_parameter(help, &help.signatures[0]).unwrap().label == lsp_wire::ParameterLabel::Simple("second: int".into()), "Comma did not update the active parameter");
            window.click("next-signature", cx); Ok(())
        })??;
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| -> Result<()> {
            window.render_frame(cx);
            ensure!(pane.read(cx).signature.help.as_ref().is_some_and(|help| signature::active_signature(help) == 1), "Overload navigation failed");
            window.press("ctrl-alt-shift-u", cx);
            ensure!(pane.read(cx).signature.help.as_ref().is_some_and(|help| signature::active_signature(help) == 0), "Configured overload shortcut failed");
            window.press("ctrl-space", cx); Ok(())
        })??;
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| -> Result<()> {
            window.render_frame(cx);
            ensure!(window.find("source-completions").visible() && window.find("completion-signature").visible(), "Ctrl+Space did not show completions with hints");
            ensure!(requests.borrow().iter().any(|(_, _, context)| context.trigger_kind == SignatureHelpTriggerKind::INVOKED), "Ctrl+Space did not invoke signature help");
            ensure!(window.try_find("signature-popup").is_none(), "Ctrl+Space opened overlapping signature surfaces");
            Ok(())
        })??;
        if pixels { cx.capture_screenshot(handle)?.save(output.join(format!("parameter-hints-{name}.png")))?; }
        cx.update_window(handle, |_, window, cx| { window.press("escape", cx); })?; cx.run_until_parked();
        cx.advance_clock(Duration::from_millis(500)); cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| -> Result<()> {
            window.render_frame(cx); ensure!(pane.read(cx).signature.help.is_none() && pane.read(cx).menu.is_none(), "Escape reopened signature help: hint={}, pending={}, menu={}, dismissed={:?}, call={:?}, requests={}", pane.read(cx).signature.help.is_some(), pane.read(cx).signature.pending, pane.read(cx).menu.is_some(), pane.read(cx).signature.dismissed, pane.read(cx).signature.input.as_ref().and_then(|input| input.call), requests.borrow().len());
            window.press("ctrl-space", cx); Ok(())
        })??; cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| { window.render_frame(cx); window.click("signature-other-control", cx); })?; cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| -> Result<()> {
            ensure!(pane.read(cx).signature.help.is_none(), "Focus loss did not dismiss hints");
            let editor = pane.read(cx).state.clone(); editor.update(cx, |editor, cx| editor.focus(window, cx));
            window.press("ctrl-a", cx); window.input("combine(1, 2)", cx); window.press("left", cx); window.press("ctrl-space", cx); Ok(())
        })??; cx.run_until_parked();
        delayed.set(true);
        cx.update_window(handle, |_, window, cx| { window.press("ctrl-space", cx); })?; cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| { window.press("escape", cx); window.press("end", cx); })?;
        cx.run_until_parked(); cx.advance_clock(Duration::from_millis(600)); cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| -> Result<()> {
            ensure!(pane.read(cx).signature.help.is_none(), "Late result reopened a dismissed/outside-call hint");
            window.press("left", cx); window.press("ctrl-space", cx); Ok(())
        })??; cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| { window.press("ctrl-a", cx); })?; cx.run_until_parked(); cx.advance_clock(Duration::from_millis(600)); cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| -> Result<()> { ensure!(pane.read(cx).signature.help.is_none(), "Selection did not invalidate pending hints"); window.remove_window(); Ok(()) })??;
        cx.run_until_parked();
    }
    cx.update(|cx| { crate::theme::apply("Vesper", cx); crate::theme::set_zoom(1., cx); });
    Ok(())
}
