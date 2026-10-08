//! Changelog popover interaction and scrolling without application startup.
use std::{path::Path, sync::Arc};
use anyhow::{Result, ensure};
use gpui_kit::*;
use gpui_kit::component::{ActiveTheme as _, v_flex};
use gpui_kit::test::TestWindowExt as _;
use practice::{config::Config, db::Db};

struct Fixture {
    workspace: Entity<crate::workspace::Workspace>,
}

impl Render for Fixture {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let button = self.workspace.update(cx, |workspace, cx| crate::update::button(workspace, cx).into_any_element());
        let panel = self.workspace.update(cx, |workspace, cx| crate::update::panel(workspace, window, cx));
        div().id("update-fixture").test_support().size_full().bg(cx.theme().background)
            .child(v_flex().size_full().justify_end().items_end().p_3().child(button))
            .children(panel)
    }
}

pub(crate) fn native(cx: &mut HeadlessAppContext, output: &Path, pixels: bool) -> Result<serde_json::Value> {
    let db = Arc::new(Db::open(&output.join("session/update.sqlite"))?);
    let config = Config { onboarding_completed: true, workspace: output.join("session/solutions"), ..Config::default() };
    let (handle, fixture) = cx.update(|cx| gpui_kit::open_window(WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds { origin: point(px(0.), px(0.)), size: size(px(520.), px(600.)) })),
        show: false, focus: false, ..Default::default()
    }, cx, |window, cx| cx.new(|cx| {
        let workspace = cx.new(|cx| crate::workspace::Workspace::from_storage(config, db, [None, None], window, cx));
        cx.observe(&workspace, |_, _, cx| cx.notify()).detach();
        workspace.update(cx, |workspace, _| workspace.release_update.latest = Some(practice::updates::Latest {
            version: env!("LEET_VERSION").into(),
            notes: include_str!("../../../../docs/changelog.md").into(),
            available: None,
        }));
        Fixture { workspace }
    })))?;
    let handle = handle.into();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| -> Result<()> {
        window.render_frame(cx);
        window.hover("release-updates", cx);
        ensure!(!fixture.read(cx).workspace.read(cx).release_update.is_open(), "Hover opened the changelog state");
        ensure!(window.try_find("release-update-panel").is_none(), "Hover opened the changelog panel");
        window.click("release-updates", cx);
        window.render_frame(cx);
        ensure!(fixture.read(cx).workspace.read(cx).release_update.is_open(), "Click did not open the changelog state");
        ensure!(window.find("release-notes-scroll").visible(), "Click did not show release notes");
        window.scroll("release-notes-scroll", ScrollDelta::Lines(point(0., -12.)), cx);
        window.render_frame(cx);
        ensure!(fixture.read(cx).workspace.read(cx).release_update.scroll_offset() < px(0.), "Release notes did not scroll");
        window.click("release-updates", cx);
        window.render_frame(cx);
        ensure!(!fixture.read(cx).workspace.read(cx).release_update.is_open(), "Trigger click did not close the panel");
        window.click("release-updates", cx);
        window.render_frame(cx);
        window.click_at("update-fixture", point(px(24.), px(24.)), cx);
        window.render_frame(cx);
        ensure!(!fixture.read(cx).workspace.read(cx).release_update.is_open(), "Outside click did not close the panel");
        Ok(())
    })??;
    if pixels {
        cx.update_window(handle, |_, window, cx| window.click("release-updates", cx))?;
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| window.render_frame(cx))?;
        cx.capture_screenshot(handle)?.save(output.join("update-changelog.png"))?;
    }
    cx.update_window(handle, |_, window, _| window.remove_window())?;
    cx.run_until_parked();
    Ok(serde_json::json!({"fixture":"update-changelog","passed":true,"pixels":pixels,"checks":["hover does not open","trigger toggles panel","release notes scroll","outside click dismisses"]}))
}
