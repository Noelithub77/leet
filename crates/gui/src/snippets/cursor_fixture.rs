//! Placeholder geometry through the production editor and keyboard expansion.
use super::*;
use anyhow::{Result, ensure};
use std::path::Path;
use gpui_kit::test::TestWindowExt as _;

struct Fixture { pane: Entity<EditorPane> }
impl Render for Fixture {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().p_8().child(self.pane.clone())
    }
}

pub(crate) fn native(cx: &mut HeadlessAppContext, output: &Path, pixels: bool) -> Result<()> {
    for (name, zoom, lines) in [("regular", 1., 0), ("zoom-scroll", 1.4, 35)] {
        cx.update(|cx| { crate::theme::apply("Vesper", cx); crate::theme::set_zoom(zoom, cx); });
        let (handle, view) = cx.update(|cx| gpui_kit::open_window(WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds { origin: point(px(0.), px(0.)), size: size(px(700.), px(430.)) })),
            show: false, focus: false, ..Default::default()
        }, cx, |window, cx| cx.new(|cx| {
            let state = cx.new(|cx| EditorState::new(window, cx).language("cpp").line_number(true));
            let library = Rc::new(Library::new(vec![Snippet { name: "Range loop".into(), prefixes: vec!["loopfixture".into()], body: "for (auto &${1:value} : ${2:a}) {\n\t$0\n}".into(), description: String::new(), scope: Some(Language::Cpp), template: false, origin: practice::snippets::Origin::User }]));
            let pane = cx.new(|cx| EditorPane::new(state.clone(), Language::Cpp, library, Default::default(), window, cx));
            state.update(cx, |editor, cx| {
                let text = format!("{}class Solution {{\npublic:\n    void run() {{\n        ", "// preceding line\n".repeat(lines));
                editor.set_value(text.clone(), window, cx); editor.set_selected_range(text.len()..text.len(), cx); editor.focus(window, cx);
            });
            Fixture { pane }
        })))?;
        let handle = handle.into(); let pane = cx.update(|cx| view.read(cx).pane.clone()); cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| { window.activate_window(); window.render_frame(cx); window.input("loopfixture", cx); })?;
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| { window.render_frame(cx); window.press("tab", cx); })?;
        cx.run_until_parked();
        for (key, expected) in [(None, "value"), (Some("tab"), "a"), (Some("shift-tab"), "value")] {
            if let Some(key) = key { cx.update_window(handle, |_, window, cx| window.press(key, cx))?; }
            cx.run_until_parked(); cx.advance_clock(Duration::from_millis(150)); cx.run_until_parked();
            cx.update_window(handle, |_, window, cx| -> Result<()> {
                window.render_frame(cx); window.render_frame(cx);
                let editor = pane.read(cx).state.read(cx);
                ensure!(editor.selected_value().as_str() == expected, "Placeholder navigation selected the wrong text");
                if lines > 0 { ensure!(editor.scroll_offset().y < px(0.), "Scrolled placeholder case did not scroll"); }
                let selected = editor.range_to_bounds(&editor.selected_range()).unwrap();
                let caret = editor.cursor_layout().unwrap().0;
                ensure!((selected.center().y - caret.center().y - editor.scroll_offset().y).abs() < px(1.), "Selected text bounds must follow the painted caret while scrolling: selected={selected:?}, caret={caret:?}, scroll={:?}", editor.scroll_offset());
                let marker = window.find("snippet-placeholder").bounds();
                ensure!((marker.left() - selected.left()).abs() < px(1.) && (marker.top() - selected.top()).abs() < px(1.), "Placeholder marker is offset from selected text: marker={marker:?}, selected={selected:?}");
                ensure!(marker.bottom() <= editor.input_bounds().bottom() && marker.top() >= editor.input_bounds().top(), "Placeholder escaped the editor viewport");
                Ok(())
            })??;
        }
        if pixels { cx.capture_screenshot(handle)?.save(output.join(format!("snippet-cursor-{name}.png")))?; }
        cx.update_window(handle, |_, window, cx| -> Result<()> {
            window.press("tab", cx); window.press("tab", cx);
            ensure!(pane.read(cx).state.read(cx).selected_range().is_empty(), "Final snippet stop did not place the caret");
            let editor = pane.read(cx).state.read(cx);
            let source = editor.value();
            ensure!(source[..editor.cursor()].rsplit('\n').next().unwrap().trim().is_empty() && source[editor.cursor()..].trim_start().starts_with('}'), "Final caret must land in the empty loop body");
            window.remove_window(); Ok(())
        })??;
    }
    cx.update(|cx| { crate::theme::set_zoom(1., cx); });
    Ok(())
}
