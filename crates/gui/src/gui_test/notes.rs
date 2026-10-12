//! Notes in the production editor pane: views, copy in Off, hovers, actions, and note writes.
use super::*;
use anyhow::{Result, ensure};
use std::path::Path;
use gpui_kit::component::input::ToggleCodeActions;
use gpui_kit::test::TestWindowExt as _;

const CODE: &str = "class Solution:\n    def search(self, nums, target):\n        lo, hi = 0, len(nums)\n        # hi is exclusive\n        while lo < hi:\n            mid = (lo + hi) // 2\n            #! bug: The range never shrinks (`lo = mid`)\n            #? why does this loop forever\n            lo = mid\n        return -1\n";

fn settle(cx: &HeadlessAppContext) { cx.run_until_parked(); cx.advance_clock(Duration::from_millis(300)); cx.run_until_parked(); }

pub(crate) fn native(cx: &mut HeadlessAppContext, output: &Path, pixels: bool) -> Result<serde_json::Value> {
    cx.update(|cx| cx.bind_keys([KeyBinding::new("alt-enter", ToggleCodeActions, None)]));
    let (handle, pane) = cx.update(|cx| gpui_kit::open_window(WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds { origin: point(px(0.), px(0.)), size: size(px(900.), px(420.)) })),
        show: false, focus: false, ..Default::default()
    }, cx, |window, cx| cx.new(|cx| {
        let state = cx.new(|cx| EditorState::new(window, cx).language("python").line_number(true));
        let mut pane = EditorPane::new(state.clone(), Language::Python, Rc::new(practice::snippets::search::Library::new(vec![])), View::Rich, window, cx);
        pane.load_file(CODE.into(), window, cx);
        *pane.notes.provider.detail.borrow_mut() = Some(Rc::new(|text: &str, _: &App| text.contains("never shrinks").then(|| "Set `lo = mid + 1`.".to_owned())));
        state.update(cx, |editor, cx| editor.focus(window, cx));
        pane
    })))?;
    let handle = handle.into();
    let text = |cx: &App| pane.read(cx).state.read(cx).value().to_string();
    settle(cx);

    let anchor = CODE.find("            lo = mid\n").map(|at| at + 12..at + 20).unwrap_or_default();
    cx.update_window(handle, |_, window, cx| -> Result<()> {
        window.activate_window();
        window.render_frame(cx);
        let marks = pane.read(cx).notes.marks.as_ref().map(|marks| marks.get_ranges(cx)).unwrap_or_default();
        ensure!(marks == [anchor.clone()], "Rich view did not highlight the quoted code: {marks:?}");
        let styles = pane.read(cx).notes.styles.as_ref().map(|styles| styles.get_ranges(cx)).unwrap_or_default();
        ensure!(styles.iter().any(|range| &CODE[range.clone()] == "#! bug:") && styles.iter().any(|range| &CODE[range.clone()] == "#?"), "Note markers are not styled");
        ensure!(!styles.iter().any(|range| CODE[range.clone()].contains("hi is exclusive")), "A plain comment was styled as a note");
        let bounds = pane.read(cx).state.read(cx).range_to_bounds(&anchor).ok_or_else(|| anyhow::anyhow!("Quoted code has no bounds"))?;
        window.dispatch_event(MouseMoveEvent { position: bounds.center(), pressed_button: None, modifiers: Default::default() }.to_platform_input(), cx);
        window.render_frame(cx);
        Ok(())
    })??;
    settle(cx);
    cx.update_window(handle, |_, window, cx| -> Result<()> {
        window.render_frame(cx);
        let hover = pane.read(cx).state.read(cx).hover_popover().map(|popover| format!("{:?}", popover.hover.contents)).unwrap_or_default();
        ensure!(hover.contains("Bug") && hover.contains("lo = mid + 1"), "Hovering the quoted code did not explain the note: {hover}");
        Ok(())
    })??;
    if pixels { cx.capture_screenshot(handle)?.save(output.join("notes-rich-hover.png"))?; }

    cx.update_window(handle, |_, window, cx| -> Result<()> {
        let question = CODE.find("#? why").unwrap_or_default() + 3;
        pane.read(cx).state.clone().update(cx, |editor, cx| editor.set_selected_range(question..question, cx));
        window.press("alt-enter", cx);
        Ok(())
    })??;
    settle(cx);
    cx.update_window(handle, |_, _, cx| -> Result<()> {
        let titles: Vec<String> = pane.read(cx).state.read(cx).code_action_menu_state().items.iter().map(|item| item.action.title.clone()).collect();
        ensure!(titles == ["Ask AI", "Clear AI notes"], "Unexpected question actions: {titles:?}");
        Ok(())
    })??;

    cx.update_window(handle, |_, window, cx| -> Result<()> {
        window.press("escape", cx);
        pane.update(cx, |pane, cx| pane.set_notes_view(View::Off, window, cx));
        let view = text(cx);
        ensure!(!view.contains("#!") && !view.contains("#?"), "Off view still shows notes");
        ensure!(view.contains("# hi is exclusive"), "Off view hid a plain comment");
        ensure!(view.lines().nth(6) == Some("            lo = mid"), "Off view did not renumber visible lines");
        ensure!(pane.read(cx).file_text(cx) == CODE, "Off view lost hidden notes");
        window.press("ctrl-a", cx);
        window.press("ctrl-c", cx);
        let copied = cx.read_from_clipboard().and_then(|item| item.text()).unwrap_or_default();
        ensure!(copied == view, "Copy in Off view included hidden notes");
        let end = view.find("lo = mid").map(|at| at + 8).unwrap_or_default();
        pane.read(cx).state.clone().update(cx, |editor, cx| editor.set_selected_range(end..end, cx));
        window.input(" + 1", cx);
        Ok(())
    })??;
    settle(cx);
    cx.update_window(handle, |_, window, cx| -> Result<()> {
        let file = pane.read(cx).file_text(cx);
        ensure!(file == CODE.replace("            lo = mid\n", "            lo = mid + 1\n"), "Editing in Off view misplaced hidden notes:\n{file}");
        window.render_frame(cx);
        Ok(())
    })??;
    if pixels { cx.capture_screenshot(handle)?.save(output.join("notes-off.png"))?; }

    cx.update_window(handle, |_, window, cx| -> Result<()> {
        pane.update(cx, |pane, cx| pane.set_notes_view(View::Rich, window, cx));
        let file = pane.read(cx).file_text(cx);
        ensure!(text(cx) == file && file.contains("#! bug:"), "Rich view did not restore notes");
        let cleared = pane.update(cx, |pane, cx| pane.clear_ai_notes(window, cx));
        ensure!(cleared == 1 && !text(cx).contains("#!") && text(cx).contains("#? why"), "Clearing AI notes removed the wrong lines");
        window.press("ctrl-z", cx);
        ensure!(text(cx) == file, "Undo did not restore the cleared note in one step");
        Ok(())
    })??;
    cx.update_window(handle, |_, window, cx| { window.remove_window(); })?;
    Ok(serde_json::json!({"fixture":"notes","passed":true,"pixels":pixels,"checks":["rich view highlights quoted code","markers styled, plain comments untouched","hover explains the note","question code actions","off view hides notes and renumbers","copy excludes hidden notes","edits keep hidden notes beside their code","rich view restores notes","clearing AI notes is one undo step"]}))
}
