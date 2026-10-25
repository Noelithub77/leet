//! Real sheet controls with public metadata and disposable workspace state.
use super::*;
use gpui_kit::SharedString;
use practice::{config::Config, db::Db, striver::Sheet};
use crate::workspace::{Center, Workspace};

pub(super) fn native(cx: &mut HeadlessAppContext, output: &Path, pixels: bool) -> Result<serde_json::Value> {
    let db = Arc::new(Db::open(&output.join("session/striver.sqlite"))?);
    let config = Config { onboarding_completed: true, workspace: output.join("session/solutions"), ..Config::default() };
    cx.update(|cx| {
        let bindings = cx.key_bindings().borrow().bindings().cloned().collect();
        cx.set_global(crate::actions::ComponentBindings(bindings));
        crate::actions::reload_keys(&config, cx);
    });
    let (handle, workspace) = cx.update(|cx| gpui_kit::open_window(WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds { origin: point(px(0.), px(0.)), size: size(px(1280.), px(800.)) })),
        show: false, focus: false, ..Default::default()
    }, cx, |window, cx| cx.new(|cx| Workspace::from_storage(config.clone(), db.clone(), [None, None], window, cx))))?;
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("practice-source", cx);
    })?;
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("striver-source", cx);
    })?;
    cx.run_until_parked();
    std::thread::sleep(Duration::from_millis(400));
    cx.advance_clock(Duration::from_millis(450));
    cx.run_until_parked();
    let first = &Sheet::A2z.data().sections[0];
    cx.update_window(handle, |_, window, cx| -> Result<()> {
        ensure!(workspace.read(cx).center == Center::Sheets, "Source picker did not open Striver sheets");
        window.render_frame(cx);
        window.click(SharedString::from(format!("striver-section-{}", first.id)), cx);
        Ok(())
    })??;
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| -> Result<()> {
        window.render_frame(cx);
        ensure!(workspace.read(cx).center == Center::Sheets, "Section click left the sheet view");
        ensure!(window.find(SharedString::from(format!("striver-item-{}", first.items[0].id))).visible(), "Expanding a section did not expose its items");
        Ok(())
    })??;
    cx.run_until_parked();
    if pixels { cx.capture_screenshot(handle)?.save(output.join("striver-a2z.png"))?; }
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click(Sheet::Sde.slug(), cx);
    })?;
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| -> Result<()> {
        window.render_frame(cx);
        ensure!(workspace.read(cx).config.striver_sheet == Sheet::Sde, "Sheet selection did not change");
        ensure!(Config::load()?.striver_sheet == Sheet::Sde, "Sheet selection did not persist");
        window.click("striver-account", cx);
        Ok(())
    })??;
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        ensure!(window.find(if cfg!(any(target_os = "windows", target_os = "macos")) { "embedded-login" } else { "save-account" }).visible(), "TUF account dialog did not open");
        window.press("escape", cx);
        window.render_frame(cx);
        ensure!(window.try_find("save-account").is_none() && window.try_find("embedded-login").is_none(), "Account dialog did not dismiss");
        anyhow::Ok(())
    })??;
    cx.run_until_parked();
    if pixels { cx.capture_screenshot(handle)?.save(output.join("striver-sde.png"))?; }
    cx.update_window(handle, |_, window, cx| -> Result<()> {
        window.press("escape", cx);
        ensure!(workspace.read(cx).center == Center::Home, "Escape did not return to Home");
        window.remove_window();
        Ok(())
    })??;
    Ok(json!({"fixture":"striver","passed":true,"pixels":pixels,"checks":["source picker opens sheets","section expansion exposes real entries","sheet switch persists","TUF account dialog opens and dismisses"]}))
}
