//! Production setup with deterministic prerequisite results and no tool detection.
use super::*;
use anyhow::{Result, ensure};
use std::{path::Path, sync::Arc};
use gpui_kit::test::TestWindowExt as _;
use practice::{config::Config, db::Db, toolchain::Requirement};

struct Fixture {
    workspace: Entity<Workspace>,
    setup: Option<Entity<Setup>>,
}
impl Render for Fixture {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let status = self.workspace.update(cx, |workspace, cx| workspace.render_status(window, cx).into_any_element());
        gpui_kit::component::v_flex().size_full().bg(cx.theme().background)
            .child(div().flex_1().children(self.setup.clone()))
            .child(status)
    }
}

pub(crate) fn native(cx: &mut HeadlessAppContext, output: &Path, pixels: bool) -> Result<serde_json::Value> {
    let db = Arc::new(Db::open(&output.join("session/onboarding.sqlite"))?);
    let config = Config { onboarding_completed: true, workspace: output.join("session/solutions"), ..Config::default() };
    for (name, compiler_ready, checking) in [("optional-server", true, false), ("missing-compiler", false, false), ("checking-tools", false, true)] {
        let (handle, fixture) = cx.update(|cx| gpui_kit::open_window(WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds { origin: point(px(0.), px(0.)), size: size(px(1000.), px(900.)) })),
            show: false, focus: false, ..Default::default()
        }, cx, |window, cx| {
            let workspace = cx.new(|cx| Workspace::from_storage(config.clone(), db.clone(), [None, None], window, cx));
            workspace.update(cx, |workspace, _| workspace.tool_install.fixture = true);
            let setup = cx.new(|cx| Setup {
                focus: cx.focus_handle(), workspace: workspace.downgrade(), step: 0,
                copilot: workspace.read(cx).copilot.clone(),
                _copilot_subscription: cx.observe(&workspace.read(cx).copilot.clone(), |_, _, cx| cx.notify()),
                language: Language::Cpp, source: Source::NeetCode, python: config.python.clone(),
                snippet_found: Vec::new(), snippet_scanning: false, snippet_status: String::new(),
                handle: cx.new(|cx| InputState::new(window, cx)), busy: false, error: None,
                requirements: (!checking).then_some(practice::toolchain::Setup { requirements: vec![
                    Requirement { label: "C++ compiler".into(), required: true, ready: compiler_ready, detail: "Fixture compiler".into(), setup_url: "https://gcc.gnu.org/install/" },
                    Requirement { label: "clangd".into(), required: false, ready: false, detail: "Fixture missing server".into(), setup_url: "https://clangd.llvm.org/installation" },
                ] }), checking, check_epoch: 0, check_task: Task::ready(()),
            });
            cx.new(|cx| {
                cx.observe(&workspace, |_, _, cx| cx.notify()).detach();
                cx.observe(&setup, |_, _, cx| cx.notify()).detach();
                Fixture { workspace, setup: Some(setup) }
            })
        }))?;
        let setup = cx.update(|cx| fixture.read(cx).setup.as_ref().unwrap().clone());
        let workspace = cx.update(|cx| fixture.read(cx).workspace.clone());
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
            ensure!(window.find("setup-snippet-guide").visible(), "Snippet onboarding explanation is hidden");
            window.click("setup-continue", cx);
            ensure!(setup.read(cx).step == 2, "Snippet onboarding could not be skipped");
            window.render_frame(cx);
            ensure!(window.find("copilot-setup").visible(), "Copilot setup step is hidden");
            ensure!(!workspace.read(cx).config.copilot_enabled, "Copilot must remain optional");
            Ok(())
        })??;
        if pixels && name == "optional-server" { cx.capture_screenshot(handle)?.save(output.join("onboarding-copilot.png"))?; }
        cx.update_window(handle, |_, window, cx| -> Result<()> {
            window.click("setup-continue", cx);
            ensure!(setup.read(cx).step == 3, "Copilot onboarding could not be skipped");
            ensure!(!workspace.read(cx).copilot.read(cx).busy, "Skipping Copilot started authentication");
            fixture.update(cx, |fixture, cx| { fixture.setup = None; cx.notify(); });
            Ok(())
        })??;
        drop(setup);
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| -> Result<()> {
            ensure!(workspace.read(cx).tool_install.busy(Language::Cpp), "Continue did not start background setup");
            workspace.update(cx, |workspace, cx| workspace.tool_install_event(Language::Cpp, crate::tool_install::Event::Progress("Downloading clangd".into()), window, cx));
            window.render_frame(cx);
            ensure!(window.find("tool-setup-label").label() == Some("C/C++ tools · Downloading"), "Background progress disappeared after onboarding");
            Ok(())
        })??;
        if pixels && name == "optional-server" { cx.capture_screenshot(handle)?.save(output.join("tool-setup-downloading.png"))?; }
        cx.update_window(handle, |_, window, cx| -> Result<()> {
            workspace.update(cx, |workspace, cx| workspace.tool_install_event(Language::Cpp, crate::tool_install::Event::Finished(Err("Fixture download failed".into())), window, cx));
            window.render_frame(cx);
            window.click("tool-setup-retry", cx);
            ensure!(workspace.read(cx).tool_install.busy(Language::Cpp), "Retry did not restart private setup");
            workspace.update(cx, |workspace, cx| workspace.tool_install_event(Language::Cpp, crate::tool_install::Event::Finished(Ok(())), window, cx));
            window.render_frame(cx);
            ensure!(window.find("tool-setup-label").label() == Some("C/C++ tools · Ready"), "Successful setup did not show Ready");
            Ok(())
        })??;
        if pixels && name == "optional-server" { cx.capture_screenshot(handle)?.save(output.join("tool-setup-ready.png"))?; }
        cx.update_window(handle, |_, window, _| window.remove_window())?;
    }
    Ok(serde_json::json!({"fixture":"onboarding","passed":true,"pixels":pixels,"checks":["missing clangd permits Continue","missing compiler permits Continue","pending checks permit Continue","snippet step is skippable","Continue starts background setup","progress survives leaving onboarding","failed setup retries through pointer input","successful setup shows Ready"]}))
}
