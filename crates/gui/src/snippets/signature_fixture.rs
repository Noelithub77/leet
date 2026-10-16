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
        Task::ready(Ok(lsp_types::CompletionResponse::Array(["first", "second"].map(|label| lsp_types::CompletionItem { label: label.into(), kind: Some(lsp_types::CompletionItemKind::VARIABLE), documentation: (label == "first").then(|| lsp_types::Documentation::String(String::new())), ..Default::default() }).into())))
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
        {"label":"combine(first: str, second: str) -> str", "parameters":[{"label":"first: str","documentation":"First text value"},{"label":"second: str","documentation":"Second text value"}]}
    ]})).unwrap()
}

pub(crate) fn native(cx: &mut HeadlessAppContext, output: &Path, pixels: bool) -> Result<()> {
    for (name, width, height, theme, zoom) in [("dark", 900., 550., "Vesper", 1.), ("compact", 380., 350., "Vesper", 1.), ("light-zoom", 640., 420., "Catppuccin Latte", 1.4)] {
        let delayed = Rc::new(Cell::new(false)); let requests = Rc::new(RefCell::new(Vec::new()));
        let (handle, view) = cx.update(|cx| {
            crate::theme::apply(theme, cx); crate::theme::set_zoom(zoom, cx);
            let config = practice::config::Config::default();
            crate::actions::bind_keys(&config, cx);
            let requests = requests.clone(); let delayed = delayed.clone();
            gpui_kit::open_window(WindowOptions { window_bounds: Some(WindowBounds::Windowed(Bounds { origin: point(px(0.), px(0.)), size: size(px(width), px(height)) })), show: false, focus: false, ..Default::default() }, cx, |window, cx| cx.new(|cx| {
                let editor = cx.new(|cx| EditorState::new(window, cx).language("python").line_number(true));
                let pane = cx.new(|cx| {
                    let mut pane = EditorPane::new(editor.clone(), Language::Python, Rc::new(practice::snippets::search::Library::new(vec![])), Default::default(), window, cx);
                    let connection = cx.new(|_| crate::copilot::Connection::new(false));
                    pane.connect_predictions(connection, window, cx);
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
            let before = pane.read(cx).state.read(cx).value().to_string();
            window.press("alt-down", cx);
            ensure!(pane.read(cx).signature.help.as_ref().is_some_and(|help| signature::active_signature(help) == 0), "Alt+Down did not wrap to the first overload");
            window.press("alt-up", cx);
            ensure!(pane.read(cx).signature.help.as_ref().is_some_and(|help| signature::active_signature(help) == 1), "Alt+Up did not wrap to the last overload");
            ensure!(pane.read(cx).state.read(cx).value().as_str() == before, "Overload shortcuts edited source");
            ensure!(pane.read(cx).state.read(cx).focus_handle(cx).is_focused(window), "Overload shortcuts stole focus");
            let mut config = practice::config::Config::default(); config.keybindings.insert("NextSignature".into(), "ctrl-alt-shift-u".into());
            crate::actions::bind_keys(&config, cx);
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
            ensure!(window.find("source-completion-preview").visible(), "Parameter documentation must stay available beside or below completions");
            ensure!(window.find("source-completion-preview").bounds().size.height <= rems(4.).to_pixels(window.rem_size()) + px(1.), "Short parameter documentation must remain compact");
            let strip = window.find("completion-signature").bounds();
            let list = window.find("source-completion-list").bounds();
            ensure!(list.size.height >= rems(2.75).to_pixels(window.rem_size()) - px(1.), "Short parameter details unnecessarily hid completion rows");
            ensure!(strip.bottom() <= list.top() + px(1.), "Parameter strip must sit above completions");
            let label = window.find("signature-label").bounds();
            ensure!(label.size.width >= strip.size.width - rems(1.5).to_pixels(window.rem_size()), "Signature text is squeezed beside overload controls: label={label:?}, strip={strip:?}");
            window.dispatch_event(PlatformInput::ModifiersChanged(ModifiersChangedEvent { modifiers: Modifiers { alt: true, ..Default::default() }, capslock: Default::default() }), cx);
            ensure!(pane.read(cx).menu.is_some(), "Alt modifier discarded the completion menu before overload navigation");
            window.press("alt-up", cx);
            window.render_frame(cx);
            ensure!(pane.read(cx).signature.help.as_ref().is_some_and(|help| signature::active_signature(help) == 1), "Overload shortcut failed with completions open");
            ensure!(!pane.read(cx).predictions.as_ref().unwrap().alt.get(), "Overload navigation left Copilot preview active");
            window.dispatch_event(PlatformInput::ModifiersChanged(ModifiersChangedEvent { modifiers: Modifiers::default(), capslock: Default::default() }), cx);
            ensure!(window.find("source-completions").visible(), "Overload shortcut dismissed completions");
            let selected = pane.read(cx).menu.as_ref().unwrap().selected;
            window.press("down", cx);
            ensure!(pane.read(cx).menu.as_ref().unwrap().selected != selected, "Normal arrows no longer navigate completions");
            ensure!(pane.read(cx).signature.help.as_ref().is_some_and(|help| signature::active_signature(help) == 1), "Completion arrows changed the overload");
            window.press("up", cx);
            Ok(())
        })??;
        if pixels { cx.capture_screenshot(handle)?.save(output.join(format!("parameter-hints-{name}.png")))?; }
        if name == "dark" {
            cx.update_window(handle, |_, window, cx| window.hover("previous-signature", cx))?;
            cx.advance_clock(Duration::from_millis(700)); cx.run_until_parked();
            std::thread::sleep(Duration::from_millis(700));
            cx.update_window(handle, |_, window, cx| -> Result<()> {
                window.render_frame(cx);
                ensure!(window.find("tooltip-keybinding").visible(), "Overload arrow tooltip does not show its shortcut");
                Ok(())
            })??;
            if pixels { cx.capture_screenshot(handle)?.save(output.join("parameter-shortcut.png"))?; }
            cx.update_window(handle, |_, window, cx| window.hover("signature-other-control", cx))?;
            cx.advance_clock(Duration::from_millis(700)); cx.run_until_parked();
            std::thread::sleep(Duration::from_millis(700));
        }
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
        cx.update_window(handle, |_, _, cx| -> Result<()> { ensure!(pane.read(cx).signature.help.is_none(), "Selection did not invalidate pending hints"); Ok(()) })??;
        if name == "dark" {
            delayed.set(false);
            cx.update_window(handle, |_, window, cx| {
                pane.read(cx).state.clone().update(cx, |editor, cx| {
                    editor.set_value("combine()", window, cx); editor.set_selected_range(8..8, cx);
                    editor.set_vim_enabled(true, cx); editor.focus(window, cx);
                });
                window.render_frame(cx); window.press("i", cx); window.press("ctrl-space", cx);
            })?;
            cx.run_until_parked(); cx.advance_clock(Duration::from_millis(150)); cx.run_until_parked();
            cx.update_window(handle, |_, window, cx| -> Result<()> {
                window.render_frame(cx);
                ensure!(pane.read(cx).signature.help.is_some(), "Vim Insert mode did not allow parameter hints");
                window.press("escape", cx); window.render_frame(cx); window.press("escape", cx); window.render_frame(cx);
                ensure!(pane.read(cx).state.read(cx).vim_mode() == Some(gpui_kit::base::input::VimMode::Normal), "Escape did not return to Vim Normal mode");
                window.press("ctrl-space", cx); Ok(())
            })??;
            cx.run_until_parked(); cx.advance_clock(Duration::from_millis(150)); cx.run_until_parked();
            cx.update_window(handle, |_, _, cx| -> Result<()> { ensure!(pane.read(cx).signature.help.is_none(), "Vim Normal mode reopened parameter hints"); Ok(()) })??;
        }
        cx.update_window(handle, |_, window, _| window.remove_window())?;
        cx.run_until_parked();
    }
    cx.update(|cx| { crate::theme::apply("Vesper", cx); crate::theme::set_zoom(1., cx); });
    Ok(())
}
