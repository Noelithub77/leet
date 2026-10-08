//! Long statement fixtures exercise bounded cards without accounts or saved state.
use super::*;
use crate::statement::{Sections, Statement};
use practice::description::{Block, Part};

fn fixture(window: &mut gpui_kit::Window, cx: &mut App) -> Entity<Statement> {
    cx.set_global(Sections::default());
    crate::statement::bind_keys(cx);
    let statement = cx.new(|_| Statement {
        slug: "statement-fixture".into(), title: "Two Sum".into(),
        blocks: vec![
            Block::Markdown("Given an array of integers `nums` and an integer `target`, return the indices of the two numbers that add up to `target`.\n\n".repeat(35)),
            Block::Example { title: "Example 1".into(), parts: vec![
                Part::Field { label: "Input".into(), value: "nums = [2, 7, 11, 15], target = 9".into() },
                Part::Field { label: "Output".into(), value: "[0, 1]".into() },
                Part::Field { label: "Explanation".into(), value: "Because nums[0] + nums[1] == 9, we return [0, 1].".into() },
            ] },
            Block::Constraints("- `2 <= nums.length <= 10⁴`\n- `-10⁹ <= nums[i] <= 10⁹`\n- Exactly one valid answer exists.".into()),
        ], ..Default::default()
    });
    statement.update(cx, |view, cx| view.focus(window, cx));
    statement
}

fn headers(window: &gpui_kit::Window) -> Result<()> {
    for section in 0..3usize {
        let header = window.find(("statement-section", section));
        ensure!(header.visible(), "Statement header {section} is hidden");
        let bounds = header.bounds();
        ensure!(bounds.top() >= px(0.) && bounds.bottom() <= window.viewport_size().height, "Statement header {section} escaped the pane");
    }
    Ok(())
}

pub(super) fn native(cx: &mut HeadlessAppContext, output: &Path, pixels: bool) -> Result<serde_json::Value> {
    for (name, width, height, theme, zoom) in [("regular", 1280., 800., "Vesper", 1.), ("compact", 420., 400., "Vesper", 1.), ("light", 1280., 800., "Solarized Light", 1.), ("zoom", 520., 600., "Vesper", 1.4)] {
        cx.update(|cx| { crate::theme::apply(theme, cx); crate::theme::set_zoom(zoom, cx); });
        let (handle, statement) = cx.update(|cx| gpui_kit::open_window(WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds { origin: point(px(0.), px(0.)), size: size(px(width), px(height)) })),
            show: false, focus: false, ..Default::default()
        }, cx, fixture))?;
        let handle = handle.into();
        cx.update_window(handle, |_, window, cx| -> Result<()> {
            window.render_frame(cx); headers(window)?;
            window.scroll(("statement-section", 0usize), gpui_kit::ScrollDelta::Pixels(point(px(0.), px(-600.))), cx);
            ensure!(statement.read(cx).scroll.offset().y < px(0.), "Description did not scroll");
            headers(window)?;
            window.click(("statement-section", 1usize), cx);
            ensure!(cx.global::<Sections>().0 == [false, true, false], "Examples did not replace Description");
            headers(window)?;
            window.click(("statement-section", 2usize), cx);
            ensure!(cx.global::<Sections>().0 == [false, false, true], "Constraints did not replace Examples");
            headers(window)?;
            window.click(("statement-section", 0usize), cx);
            ensure!(cx.global::<Sections>().0 == [true, false, false], "Description did not replace Constraints");
            statement.update(cx, |view, cx| view.focus(window, cx));
            window.press("pagedown", cx);
            ensure!(statement.read(cx).scroll.offset().y < px(0.), "PageDown did not scroll the open card");
            headers(window)?;
            // Focus starts at the statement; toolbar commands precede the card headers.
            statement.update(cx, |view, cx| view.focus(window, cx));
            window.press("tab", cx); window.press("tab", cx); window.press("tab", cx);
            window.press("space", cx);
            ensure!(cx.global::<Sections>().0 == [false; 3], "Keyboard did not collapse Description");
            headers(window)?;
            window.press("tab", cx); window.press("space", cx);
            ensure!(cx.global::<Sections>().0 == [false, true, false], "Keyboard did not open Examples");
            headers(window)
        })??;
        if pixels {
            cx.run_until_parked();
            cx.capture_screenshot(handle)?.save(output.join(format!("statement-{name}.png")))?;
        }
        cx.update_window(handle, |_, window, _| window.remove_window())?;
        cx.run_until_parked();
    }
    cx.update(|cx| { crate::theme::apply("Vesper", cx); crate::theme::set_zoom(1., cx); });
    Ok(json!({"fixture":"statement-cards","passed":true,"checks":["long description scroll","visible headers at regular and compact sizes","mutually exclusive cards","PageDown","Tab/Space card activation","all collapsed"],"pixels":pixels}))
}

pub(super) fn explore(output: PathBuf) -> Result<()> {
    gpui_kit::application().with_assets(crate::assets::Assets).run(move |cx| {
        init(cx);
        gpui_kit::open_window(WindowOptions {
            app_id: Some("leet-statement-fixture".into()),
            window_bounds: Some(WindowBounds::Windowed(Bounds { origin: point(px(0.), px(0.)), size: size(px(480.), px(680.)) })),
            ..Default::default()
        }, cx, fixture).expect("Open isolated statement fixture");
        cx.spawn(async move |cx| {
            cx.background_executor().timer(Duration::from_millis(500)).await;
            let _ = std::fs::write(output.join("ready.json"), "{\"ready\":true,\"fixture\":\"statement\"}");
            cx.background_executor().timer(Duration::from_secs(180)).await;
            let _ = cx.update(|cx| cx.quit());
        }).detach();
    });
    Ok(())
}
