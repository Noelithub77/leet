//! Production setup with deterministic prerequisite results and no tool detection.
use super::*;
use anyhow::{Result, ensure};
use std::{path::Path, sync::Arc};
use gpui_kit::test::TestWindowExt as _;
use practice::{config::Config, db::Db, toolchain::Requirement};

pub(crate) fn native(cx: &mut HeadlessAppContext, output: &Path, pixels: bool) -> Result<serde_json::Value> {
    let db = Arc::new(Db::open(&output.join("session/onboarding.sqlite"))?);
    let config = Config { onboarding_completed: true, workspace: output.join("session/solutions"), ..Config::default() };
    for (name, compiler_ready, checking) in [("optional-server", true, false), ("missing-compiler", false, false), ("checking-tools", false, true)] {
        let (handle, setup) = cx.update(|cx| gpui_kit::open_window(WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds { origin: point(px(0.), px(0.)), size: size(px(1000.), px(900.)) })),
            show: false, focus: false, ..Default::default()
        }, cx, |window, cx| {
            let workspace = cx.new(|cx| Workspace::from_storage(config.clone(), db.clone(), [None, None], window, cx));
            cx.new(|cx| Setup {
                focus: cx.focus_handle(), workspace: workspace.downgrade(), step: 0,
                language: Language::Cpp, source: Source::NeetCode, python: config.python.clone(),
                snippet_found: Vec::new(), snippet_scanning: false, snippet_status: String::new(),
                handle: cx.new(|cx| InputState::new(window, cx)), busy: false, error: None,
                requirements: (!checking).then_some(practice::toolchain::Setup { requirements: vec![
                    Requirement { label: "C++ compiler".into(), required: true, ready: compiler_ready, detail: "Fixture compiler".into(), setup_url: "https://gcc.gnu.org/install/" },
                    Requirement { label: "clangd".into(), required: false, ready: false, detail: "Fixture missing server".into(), setup_url: "https://clangd.llvm.org/installation" },
                ] }), checking, installing: false, check_epoch: 0, check_task: Task::ready(()),
            })
        }))?;
        let handle = handle.into();
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| window.render_frame(cx))?;
        if pixels { cx.capture_screenshot(handle)?.save(output.join(format!("onboarding-{name}.png")))?; }
        cx.update_window(handle, |_, window, cx| -> Result<()> {
            window.click("setup-continue", cx);
            ensure!(setup.read(cx).step == 1, "Onboarding prerequisite gate is incorrect for {name}");
            Ok(())
        })??;
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| window.render_frame(cx))?;
        if pixels && name == "optional-server" { cx.capture_screenshot(handle)?.save(output.join("onboarding-snippets.png"))?; }
        cx.update_window(handle, |_, window, cx| -> Result<()> {
            window.click("setup-continue", cx);
            ensure!(setup.read(cx).step == 2, "Snippet onboarding could not be skipped");
            window.remove_window();
            Ok(())
        })??;
    }
    Ok(serde_json::json!({"fixture":"onboarding","passed":true,"pixels":pixels,"checks":["missing clangd permits Continue","missing compiler permits Continue","pending checks permit Continue","snippet step is skippable"]}))
}
