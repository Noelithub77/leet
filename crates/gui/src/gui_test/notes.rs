//! Notes in the production editor pane: views, copy without notes, hovers, actions, and note writes.
use super::*;
use anyhow::{Result, ensure};
use std::path::Path;
use gpui_kit::component::input::ToggleCodeActions;
use gpui_kit::test::TestWindowExt as _;

const CODE: &str = "class Solution:\n    def search(self, nums, target):\n        lo, hi = 0, len(nums)\n        # hi is exclusive\n        while lo < hi:\n            mid = (lo + hi) // 2\n            #! bug: The range never shrinks (`lo = mid`)\n            #! stuck 1/3: Follow lo on a two-item list (`lo = mid`)\n            #? why does this loop forever\n            lo = mid\n        return -1\n";

fn settle(cx: &HeadlessAppContext) { cx.run_until_parked(); cx.advance_clock(Duration::from_millis(300)); cx.run_until_parked(); }

pub(crate) fn native(cx: &mut HeadlessAppContext, output: &Path, pixels: bool) -> Result<serde_json::Value> {
    cx.update(|cx| {
        cx.bind_keys([KeyBinding::new("alt-enter", ToggleCodeActions, None)]);
        // The notes shortcuts the hover card shows as keycaps.
        let shown = ["ResolveNote", "AddQuestion", "AskInPlace", "NextNoteStep"];
        let bindings: Vec<KeyBinding> = crate::actions::COMMANDS.iter().filter(|command| shown.contains(&command.id))
            .filter_map(|command| KeyBinding::load(command.key, (command.action)(), None, false, None, cx.keyboard_mapper().as_ref()).ok()).collect();
        cx.bind_keys(bindings);
    });
    let (handle, pane) = cx.update(|cx| gpui_kit::open_window(WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds { origin: point(px(0.), px(0.)), size: size(px(900.), px(640.)) })),
        show: false, focus: false, ..Default::default()
    }, cx, |window, cx| cx.new(|cx| {
        let state = cx.new(|cx| EditorState::new(window, cx).language("python").line_number(true));
        let mut pane = EditorPane::new(state.clone(), Language::Python, Rc::new(practice::snippets::search::Library::new(vec![])), View::Rich, window, cx);
        pane.load_file(CODE.into(), window, cx);
        *pane.notes.provider.detail.borrow_mut() = Some(Rc::new(|text: &str, _: &App| if text.contains("never shrinks") { Some("Set `lo = mid + 1`.".to_owned()) } else { text.contains("two-item").then(|| "With two items mid equals lo.\n\nNext step: Trace `nums = [1, 3]`.\n\n_What stops lo from reaching hi?_".to_owned()) }));
        state.update(cx, |editor, cx| editor.focus(window, cx));
        pane
    })))?;
    let handle = handle.into();
    let text = |cx: &App| pane.read(cx).state.read(cx).value().to_string();
    settle(cx);

    let rich = notes::hide(CODE, View::Rich).0;
    let line = rich.lines().position(|code| code == "            lo = mid").unwrap_or_default();
    let anchor = rich.find("            lo = mid\n").map(|at| at + 12..at + 20).unwrap_or_default();
    pane.update(cx, |pane, cx| pane.set_badges(BadgeKind::Cost, &rich, false, vec![(line, "O(1)".into())], cx));
    cx.update_window(handle, |_, window, cx| -> Result<()> {
        window.activate_window();
        window.render_frame(cx);
        ensure!(text(cx) == rich && rich.contains("#? why") && rich.contains("# hi is exclusive"), "Rich view did not hide only AI notes");
        ensure!(pane.read(cx).file_text(cx) == CODE, "Rich view lost hidden notes");
        let marks = pane.read(cx).notes.marks.as_ref().map(|marks| marks.get_ranges(cx)).unwrap_or_default();
        ensure!(marks == [anchor.clone()], "Rich view did not highlight the noted line: {marks:?}");
        let bounds = pane.read(cx).state.read(cx).range_to_bounds(&anchor).ok_or_else(|| anyhow::anyhow!("Noted code has no bounds"))?;
        window.dispatch_event(MouseMoveEvent { position: bounds.center(), pressed_button: None, modifiers: Default::default() }.to_platform_input(), cx);
        window.render_frame(cx);
        Ok(())
    })??;
    settle(cx);
    cx.update_window(handle, |_, window, cx| -> Result<()> {
        window.render_frame(cx);
        let hover = pane.read(cx).state.read(cx).hover_popover().map(|popover| format!("{:?}", popover.hover.contents)).unwrap_or_default();
        ensure!(hover.contains("leet-note") && hover.contains(r#"\"kind\":\"bug\""#) && hover.contains("lo = mid + 1") && hover.contains("O(1)") && hover.contains("Resolve") && hover.contains(r#"\"step\":[1,3]"#) && hover.contains("What stops lo"), "Hovering the noted line did not explain it: {hover}");
        Ok(())
    })??;
    if pixels { cx.capture_screenshot(handle)?.save(output.join("notes-rich-hover.png"))?; }

    let actions = |cx: &mut HeadlessAppContext, at: usize| -> Result<Vec<String>> {
        cx.update_window(handle, |_, window, cx| {
            window.press("escape", cx);
            pane.read(cx).state.clone().update(cx, |editor, cx| editor.set_selected_range(at..at, cx));
            window.press("alt-enter", cx);
        })?;
        settle(cx);
        cx.update_window(handle, |_, _, cx| pane.read(cx).state.read(cx).code_action_menu_state().items.iter().map(|item| item.action.title.clone()).collect())
    };
    let titles = actions(cx, rich.find("#? why").unwrap_or_default() + 3)?;
    ensure!(titles == ["Ask AI", "Clear AI notes"], "Unexpected question actions: {titles:?}");
    let titles = actions(cx, anchor.start + 2)?;
    ensure!(titles == ["Resolve note", "Ask about this line", "Clear AI notes"], "Unexpected noted-line actions: {titles:?}");
    if pixels {
        cx.update_window(handle, |_, window, cx| window.render_frame(cx))?;
        cx.capture_screenshot(handle)?.save(output.join("notes-actions.png"))?;
    }

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
        pane.update(cx, |pane, cx| pane.set_notes_view(View::Raw, window, cx));
        let file = pane.read(cx).file_text(cx);
        ensure!(text(cx) == file && file.contains("#! bug:"), "Raw view did not show every note");
        let cleared = pane.update(cx, |pane, cx| pane.clear_ai_notes(window, cx));
        ensure!(cleared == 2 && !text(cx).contains("#!") && text(cx).contains("#? why"), "Clearing AI notes removed the wrong lines");
        window.press("ctrl-z", cx);
        ensure!(text(cx) == file, "Undo did not restore the cleared note in one step");
        pane.update(cx, |pane, cx| pane.set_notes_view(View::Rich, window, cx));
        let marks = pane.read(cx).notes.marks.as_ref().map(|marks| marks.get_ranges(cx)).unwrap_or_default();
        ensure!(!text(cx).contains("#!") && marks.len() == 1, "Rich view did not return to highlights");
        // The cursor stays on line 0; the open hover decides which line resolves.
        pane.read(cx).state.clone().update(cx, |editor, cx| editor.set_selected_range(0..0, cx));
        let bounds = pane.read(cx).state.read(cx).range_to_bounds(&marks[0]).ok_or_else(|| anyhow::anyhow!("Noted code has no bounds"))?;
        window.dispatch_event(MouseMoveEvent { position: point(px(890.), px(630.)), pressed_button: None, modifiers: Default::default() }.to_platform_input(), cx);
        window.render_frame(cx);
        window.dispatch_event(MouseMoveEvent { position: bounds.center(), pressed_button: None, modifiers: Default::default() }.to_platform_input(), cx);
        window.render_frame(cx);
        Ok(())
    })??;
    settle(cx);
    cx.update_window(handle, |_, window, cx| -> Result<()> {
        ensure!(pane.read(cx).state.read(cx).hover_popover().is_some(), "Hovering the noted line again did not open its card");
        pane.update(cx, |pane, cx| pane.resolve_note(window, cx));
        let file = pane.read(cx).file_text(cx);
        let marks = pane.read(cx).notes.marks.as_ref().map(|marks| marks.get_ranges(cx)).unwrap_or_default();
        ensure!(marks.is_empty() && !file.contains("#!") && file.contains("#? why"), "Resolving a noted line left the note:\n{file}");
        ensure!(pane.read(cx).state.read(cx).hover_popover().is_none(), "Resolving left the stale card open");
        Ok(())
    })??;
    cx.update_window(handle, |_, window, _| { window.remove_window(); })?;
    Ok(serde_json::json!({"fixture":"notes","passed":true,"pixels":pixels,"checks":["rich view hides only AI notes","rich view highlights the noted line","hover explains the note and its cost","question and noted-line code actions","off view hides notes and renumbers","copy excludes hidden notes","edits keep hidden notes beside their code","raw view shows every note","clearing AI notes is one undo step","resolving acts on the hovered line"]}))
}
