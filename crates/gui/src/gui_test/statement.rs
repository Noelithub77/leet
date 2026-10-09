//! Long statement fixtures exercise bounded cards without accounts or saved state.
use super::*;
use crate::statement::{Sections, Statement};
use practice::description::{Block, Part};

fn fixture(window: &mut gpui_kit::Window, cx: &mut App) -> Entity<Statement> {
    if cx.try_global::<Sections>().is_none() { cx.set_global(Sections::default()); }
    crate::statement::bind_keys(cx);
    let statement = cx.new(|_| Statement {
        slug: "statement-fixture".into(), title: "Two Sum".into(),
        blocks: vec![
            Block::Markdown("Given an array of integers `nums` and an integer `target`, return the indices of the two numbers that add up to `target`.\n\n".repeat(70)),
            Block::Example { title: "Example 1".into(), parts: vec![
                Part::Field { label: "Input".into(), value: "nums = [2, 7, 11, 15], target = 9".into() },
                Part::Field { label: "Output".into(), value: "[0, 1]".into() },
                Part::Field { label: "Explanation".into(), value: "Because nums[0] + nums[1] == 9, we return [0, 1].\n\n".repeat(20) },
            ] },
            Block::Constraints("- `2 <= nums.length <= 10⁴`\n- `-10⁹ <= nums[i] <= 10⁹`\n- Exactly one valid answer exists.".into()),
        ], ..Default::default()
    });
    statement.update(cx, |view, cx| view.focus(window, cx));
    statement
}

fn headers(window: &gpui_kit::Window) -> Result<()> {
    let first = window.find(("statement-section", 0usize)).bounds();
    for section in 0..3usize {
        let header = window.find(("statement-section", section));
        ensure!(header.visible(), "Statement header {section} is hidden");
        let bounds = header.bounds();
        ensure!(bounds.left() == first.left() && bounds.right() == first.right(), "Statement header {section} does not span the same width");
        ensure!(bounds.top() >= px(0.) && bounds.bottom() <= window.viewport_size().height, "Statement header {section} escaped the pane");
    }
    Ok(())
}

pub(super) fn native(cx: &mut HeadlessAppContext, output: &Path, pixels: bool) -> Result<serde_json::Value> {
    for (name, width, height, theme, zoom) in [("regular", 1280., 800., "Vesper", 1.), ("compact", 420., 400., "Vesper", 1.), ("light", 1280., 800., "Solarized Light", 1.), ("zoom", 520., 600., "Vesper", 1.4)] {
        cx.update(|cx| { cx.set_global(Sections::default()); crate::theme::apply(theme, cx); crate::theme::set_zoom(zoom, cx); });
        let (handle, statement) = cx.update(|cx| gpui_kit::open_window(WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds { origin: point(px(0.), px(0.)), size: size(px(width), px(height)) })),
            show: false, focus: false, ..Default::default()
        }, cx, fixture))?;
        let handle = handle.into();
        let db_path = output.join(format!("statement-{name}.sqlite"));
        let db = Arc::new(practice::db::Db::open_unseeded(&db_path)?);
        cx.update(|cx| statement.update(cx, |view, _| view.db = Some(db.clone())));
        cx.update_window(handle, |_, window, cx| window.render_frame(cx))?;
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| -> Result<()> {
            window.render_frame(cx); headers(window)?;
            ensure!(cx.global::<Sections>().0 == [true, true, false], "First load did not expand Description and Examples");
            ensure!(db.statement_sections()? == [true, true, false], "Fresh storage has incorrect card defaults");
            window.click(("statement-section", 1usize), cx);
            ensure!(cx.global::<Sections>().0 == [true, false, false], "Examples did not collapse independently");
            window.scroll(("statement-section", 0usize), gpui_kit::ScrollDelta::Pixels(point(px(0.), px(-10.))), cx);
            ensure!(statement.read(cx).section_scroll[0].offset().y <= px(-40.), "Description wheel speed was not increased: offset {:?}, viewport {:?}, max {:?}", statement.read(cx).section_scroll[0].offset(), statement.read(cx).section_scroll[0].bounds(), statement.read(cx).section_scroll[0].max_offset());
            window.scroll(("statement-section", 0usize), gpui_kit::ScrollDelta::Pixels(point(px(0.), px(600.))), cx);
            let viewport = statement.read(cx).section_scroll[0].bounds();
            let examples_offset = statement.read(cx).section_scroll[1].offset();
            window.drag(point(viewport.right() - px(8.), viewport.top() + px(10.)), point(viewport.right() - px(8.), viewport.bottom() - px(10.)), cx);
            ensure!(statement.read(cx).section_scroll[0].offset().y < px(0.), "Description scrollbar thumb did not drag");
            ensure!(statement.read(cx).section_scroll[1].offset() == examples_offset, "Description scrollbar moved Examples");
            window.scroll(("statement-section", 0usize), gpui_kit::ScrollDelta::Pixels(point(px(0.), px(-600.))), cx);
            ensure!(statement.read(cx).section_scroll[0].offset().y < px(0.), "Description did not scroll");
            headers(window)?;
            window.click(("statement-section", 1usize), cx);
            ensure!(cx.global::<Sections>().0 == [true, true, false], "Examples closed Description");
            headers(window)?;
            window.click(("statement-section", 2usize), cx);
            ensure!(cx.global::<Sections>().0 == [true; 3], "Constraints closed another section");
            headers(window)?;
            window.click(("statement-section", 0usize), cx);
            ensure!(cx.global::<Sections>().0 == [false, true, true], "Closing Description changed other sections");
            let description_offset = statement.read(cx).section_scroll[0].offset();
            window.scroll(("statement-section", 1usize), gpui_kit::ScrollDelta::Pixels(point(px(0.), px(-200.))), cx);
            ensure!(statement.read(cx).section_scroll[1].offset().y < px(0.), "Examples did not scroll");
            ensure!(statement.read(cx).section_scroll[0].offset() == description_offset, "Examples moved Description's scroll position");
            window.click(("statement-section", 0usize), cx);
            let description_scroll = &statement.read(cx).section_scroll[0];
            ensure!(description_scroll.offset().y == description_offset.y.max(-description_scroll.max_offset().y), "Reopening Description lost its scroll position: {:?} from {:?}, max {:?}", description_scroll.offset(), description_offset, description_scroll.max_offset());
            window.press("pagedown", cx);
            ensure!(statement.read(cx).section_scroll[0].offset().y < px(0.), "PageDown did not scroll the open card");
            headers(window)?;
            // Focus starts at the statement; toolbar commands precede the card headers.
            statement.update(cx, |view, cx| view.focus(window, cx));
            window.press("tab", cx); window.press("tab", cx); window.press("tab", cx);
            window.press("space", cx);
            ensure!(cx.global::<Sections>().0 == [false, true, true], "Keyboard did not collapse only Description");
            headers(window)?;
            window.press("tab", cx); window.press("space", cx);
            ensure!(cx.global::<Sections>().0 == [false, false, true], "Keyboard did not collapse only Examples");
            ensure!(db.statement_sections()? == [false, false, true], "Expanded flags were not saved");
            window.click(("statement-section", 2usize), cx);
            ensure!(cx.global::<Sections>().0 == [false; 3], "Cards did not all collapse");
            headers(window)?;
            window.click(("statement-section", 0usize), cx);
            window.click(("statement-section", 1usize), cx);
            window.click(("statement-section", 2usize), cx);
            headers(window)
        })??;
        let (other_handle, other_statement) = cx.update(|cx| gpui_kit::open_window(WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds { origin: point(px(0.), px(0.)), size: size(px(width), px(height)) })),
            show: false, focus: false, ..Default::default()
        }, cx, fixture))?;
        cx.update_window(other_handle.into(), |_, window, cx| -> Result<()> {
            other_statement.update(cx, |view, cx| { view.slug = "another-question".into(); cx.notify(); });
            window.render_frame(cx); headers(window)?;
            ensure!(cx.global::<Sections>().0 == [true; 3], "New question did not retain all expanded flags");
            window.remove_window(); Ok(())
        })??;
        let restored = practice::db::Db::open_unseeded(&db_path)?.statement_sections()?;
        ensure!(restored == [true; 3], "Reopening storage did not restore all expanded flags");
        cx.update_window(handle, |_, window, cx| -> Result<()> {
            statement.update(cx, |view, cx| { view.hints = vec!["Keep track of values already visited.".into(), "Use the target to find the missing value.".into()]; cx.notify(); });
            window.render_frame(cx);
            let hint = window.find("cycle-hint").bounds();
            let question = window.find("statement-question").bounds();
            ensure!(hint.top() >= question.top() && hint.bottom() <= question.bottom(), "Hint consumes a separate toolbar row");
            headers(window)
        })??;
        if pixels {
            cx.run_until_parked();
            cx.capture_screenshot(handle)?.save(output.join(format!("statement-{name}.png")))?;
        }
        cx.update_window(handle, |_, window, cx| -> Result<()> {
            statement.update(cx, |view, cx| { view.blocks[0] = Block::Markdown("Return two matching indices.".into()); view.section_scroll[0].set_offset(point(px(0.), px(0.))); cx.notify(); });
            window.click(("statement-section", 1usize), cx);
            window.click(("statement-section", 2usize), cx);
            statement.update(cx, |view, cx| view.focus(window, cx));
            window.render_frame(cx); headers(window)?;
            let description = window.find(("statement-section", 0usize)).bounds();
            let examples = window.find(("statement-section", 1usize)).bounds();
            ensure!(examples.top() - description.bottom() < gpui_kit::rems(8.).to_pixels(window.rem_size()), "Short Description retained a large empty body");
            ensure!(statement.read(cx).section_scroll[0].max_offset().y == px(0.), "Short Description incorrectly needs a scrollbar");
            Ok(())
        })??;
        if pixels {
            cx.run_until_parked();
            cx.capture_screenshot(handle)?.save(output.join(format!("statement-{name}-short.png")))?;
        }
        cx.update_window(handle, |_, window, _| window.remove_window())?;
        cx.run_until_parked();
    }
    cx.update(|cx| { crate::theme::apply("Vesper", cx); crate::theme::set_zoom(1., cx); });
    Ok(json!({"fixture":"statement-cards","passed":true,"checks":["long description scroll","visible headers at regular and compact sizes","independent expansion and scrolling","faster wheel scrolling","scrollbar thumb dragging","shared persisted flags","natural short content without overflow","PageDown","Tab/Space card activation","all collapsed"],"pixels":pixels}))
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
