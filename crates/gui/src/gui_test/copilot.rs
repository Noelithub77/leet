//! Production editor input with deterministic predictions and no account or process.
use super::*;
use anyhow::{Result, ensure};
use std::path::Path;
use gpui_kit::base::input::{EditorMode, InputModeKind};
use gpui_kit::component::input::EditorState;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::test::TestWindowExt as _;
use practice::language::Language;
struct StatusFixture(Entity<crate::workspace::Workspace>);
impl Render for StatusFixture {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let status = self.0.update(cx, |ws, cx| ws.render_status(window, cx).into_any_element());
        gpui_kit::component::v_flex().size_full().bg(cx.theme().background)
            .child(div().flex_1()).child(status)
    }
}

fn alt(window: &mut Window, held: bool, cx: &mut App) {
    window.dispatch_event(PlatformInput::ModifiersChanged(ModifiersChangedEvent { modifiers: Modifiers { alt: held, ..Default::default() }, capslock: Default::default() }), cx);
}
fn settle(cx: &HeadlessAppContext) { cx.run_until_parked(); cx.advance_clock(Duration::from_millis(200)); cx.run_until_parked(); }
pub(crate) fn native(cx: &mut HeadlessAppContext, output: &Path, pixels: bool) -> Result<serde_json::Value> {
    let (handle, pane) = cx.update(|cx| gpui_kit::open_window(WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds { origin: point(px(0.), px(0.)), size: size(px(1000.), px(700.)) })),
        show: false, focus: false, ..Default::default()
    }, cx, |window, cx| cx.new(|cx| {
        let state = cx.new(|cx| EditorState::new(window, cx).language("python").line_number(true));
        let mut pane = crate::snippets::expansion::EditorPane::new(state.clone(), Language::Python, window, cx);
        let connection = cx.new(|_| Connection::new(true));
        pane.connect_predictions(connection, window, cx);
        pane.library.clear();
        let item = serde_json::from_value(serde_json::json!({"insertText":"nt(1)\n"})).unwrap();
        *pane.predictions.as_ref().unwrap().fixture.borrow_mut() = Suggestion::from_item("pri", 3, item);
        state.update(cx, |editor, cx| editor.focus(window, cx));
        pane
    })))?;
    let handle = handle.into();
    settle(cx);
    cx.update_window(handle, |_, window, cx| { window.activate_window(); window.render_frame(cx); window.input("pri", cx); })?;
    settle(cx);
    cx.update_window(handle, |_, window, cx| -> Result<()> {
        ensure!(!EditorMode::has_inline_completion(pane.read(cx).state.read(cx)), "Prediction appeared without Alt");
        ensure!(pane.read(cx).predictions.as_ref().unwrap().cache.borrow().is_some(), "Typing did not eagerly prepare a hidden prediction");
        window.render_frame(cx);
        ensure!(window.find("copilot-preview-hint").visible(), "Ready prediction has no Alt cue");
        Ok(())
    })??;
    if pixels { cx.capture_screenshot(handle)?.save(output.join("copilot-ready-cue.png"))?; }
    cx.update_window(handle, |_, window, cx| -> Result<()> {
        alt(window, true, cx); ensure!(pane.read(cx).predictions.as_ref().unwrap().alt.get(), "Alt event did not reach focused editor"); Ok(())
    })??;
    settle(cx);
    cx.update_window(handle, |_, window, cx| -> Result<()> {
        ensure!(EditorMode::has_inline_completion(pane.read(cx).state.read(cx)), "Holding Alt did not reveal prediction");
        ensure!(pane.read(cx).state.read(cx).value().as_str() == "pri", "Preview changed source text");
        window.render_frame(cx);
        ensure!(window.find("copilot-full-preview").visible(), "Full Copilot preview hidden while holding Alt");
        ensure!(pane.read(cx).predictions.as_ref().unwrap().preview.borrow().as_ref().unwrap().0 == "print(1)\n", "Full preview omitted the typed prefix");
        Ok(())
    })??;
    if pixels { cx.capture_screenshot(handle)?.save(output.join("copilot-alt-preview.png"))?; }
    cx.update_window(handle, |_, window, cx| -> Result<()> {
        alt(window, false, cx);
        ensure!(!EditorMode::has_inline_completion(pane.read(cx).state.read(cx)), "Alt release did not hide prediction");
        window.render_frame(cx); alt(window, true, cx); Ok(())
    })??;
    settle(cx);
    cx.update_window(handle, |_, window, cx| -> Result<()> {
        window.render_frame(cx); window.press("alt-tab", cx);
        ensure!(pane.read(cx).state.read(cx).value().as_str() == "print(1)\n", "Tab did not accept the visible prediction");
        ensure!(!EditorMode::has_inline_completion(pane.read(cx).state.read(cx)), "Accepted prediction stayed visible");
        let state = pane.read(cx).state.clone();
        state.update(cx, |editor, cx| { editor.set_value("", window, cx); editor.focus(window, cx); });
        alt(window, false, cx); window.input("pri", cx); window.render_frame(cx); alt(window, true, cx); Ok(())
    })??;
    settle(cx);
    cx.update_window(handle, |_, window, cx| -> Result<()> {
        window.render_frame(cx); window.press("escape", cx);
        ensure!(!EditorMode::has_inline_completion(pane.read(cx).state.read(cx)), "Escape did not dismiss prediction");
        ensure!(pane.read(cx).state.read(cx).value().as_str() == "pri", "Escape changed source text");
        alt(window, false, cx); window.render_frame(cx); alt(window, true, cx); Ok(())
    })??;
    settle(cx);
    cx.update_window(handle, |_, window, cx| -> Result<()> {
        window.render_frame(cx); window.press("alt-l", cx);
        ensure!(pane.read(cx).state.read(cx).value().as_str() == "print(1)\n", "Alt+L did not accept prediction");
        let state = pane.read(cx).state.clone();
        state.update(cx, |editor, cx| editor.set_value("", window, cx));
        alt(window, false, cx); window.input("pri", cx); window.render_frame(cx); alt(window, true, cx); Ok(())
    })??;
    settle(cx);
    cx.update_window(handle, |_, window, cx| -> Result<()> {
        window.input("x", cx); window.render_frame(cx); window.press("alt-tab", cx);
        ensure!(!pane.read(cx).state.read(cx).value().contains("print(1)"), "Stale prediction was accepted after typing");
        ensure!(preview_hint(pane.read(cx), window, cx).is_none(), "Stale prediction kept its cue");
        alt(window, false, cx); window.render_frame(cx); Ok(())
    })??;
    cx.update_window(handle, |_, window, cx| {
        let state = pane.read(cx).state.clone();
        state.update(cx, |editor, cx| editor.set_value("", window, cx));
        window.input("pri", cx); window.render_frame(cx);
        alt(window, true, cx);
    })?;
    settle(cx);
    cx.update_window(handle, |_, window, cx| -> Result<()> {
        ensure!(EditorMode::has_inline_completion(pane.read(cx).state.read(cx)), "Prediction did not refresh");
        window.blur(cx); window.render_frame(cx);
        Ok(())
    })??;
    settle(cx);
    cx.update_window(handle, |_, window, cx| -> Result<()> {
        ensure!(!EditorMode::has_inline_completion(pane.read(cx).state.read(cx)), "Focus loss did not hide prediction");
        ensure!(preview_hint(pane.read(cx), window, cx).is_none(), "Unfocused editor kept its cue");
        pane.read(cx).state.clone().update(cx, |editor, cx| editor.focus(window, cx));
        window.render_frame(cx); alt(window, true, cx); Ok(())
    })??;
    settle(cx);
    cx.update_window(handle, |_, _, cx| -> Result<()> {
        let connection = pane.read(cx).predictions.as_ref().unwrap().connection.clone();
        connection.update(cx, |connection, cx| connection.disable(cx));
        Ok(())
    })??;
    settle(cx);
    cx.update_window(handle, |_, window, cx| -> Result<()> {
        ensure!(!EditorMode::has_inline_completion(pane.read(cx).state.read(cx)), "Disabling Copilot left a visible prediction");
        ensure!(preview_hint(pane.read(cx), window, cx).is_none(), "Disabled Copilot kept its cue");
        window.render_frame(cx); Ok(())
    })??;
    if pixels { cx.capture_screenshot(handle)?.save(output.join("copilot-hidden.png"))?; }
    cx.update_window(handle, |_, window, cx| {
        let predictions = pane.read(cx).predictions.as_ref().unwrap().clone();
        predictions.connection.update(cx, |connection, cx| { connection.enabled = true; cx.notify(); });
        let code = format!("nt(1)\n{}", (0..40).map(|i| format!("print({i})\n")).collect::<String>());
        let item = serde_json::from_value(serde_json::json!({"insertText":code})).unwrap();
        *predictions.fixture.borrow_mut() = Suggestion::from_item("pri", 3, item);
        pane.read(cx).state.clone().update(cx, |editor, cx| { editor.set_value("", window, cx); editor.focus(window, cx); });
        alt(window, false, cx); window.input("pri", cx);
    })?;
    settle(cx);
    cx.update_window(handle, |_, window, cx| alt(window, true, cx))?;
    settle(cx);
    cx.update_window(handle, |_, window, cx| -> Result<()> {
        window.render_frame(cx);
        let bounds = window.find("copilot-full-preview").bounds();
        ensure!(bounds.top() >= px(0.) && bounds.bottom() <= window.viewport_size().height, "Long preview escaped the window");
        let before = window.find("copilot-preview-code").bounds().top();
        window.scroll("copilot-full-preview", ScrollDelta::Lines(point(0., -12.)), cx);
        window.render_frame(cx);
        ensure!(window.find("copilot-preview-code").bounds().top() < before, "Long preview did not scroll");
        Ok(())
    })??;
    if pixels { cx.capture_screenshot(handle)?.save(output.join("copilot-full-preview-scrolled.png"))?; }
    cx.update_window(handle, |_, window, _| window.remove_window())?;
    let db = Arc::new(practice::db::Db::open(&output.join("session/copilot-status.sqlite"))?);
    let config = practice::config::Config { onboarding_completed: true, workspace: output.join("session/solutions"), ..Default::default() };
    let (status_handle, status_fixture) = cx.update(|cx| gpui_kit::open_window(WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds { origin: point(px(0.), px(0.)), size: size(px(1200.), px(700.)) })), show: false, focus: false, ..Default::default()
    }, cx, |window, cx| {
        let workspace = cx.new(|cx| crate::workspace::Workspace::from_storage(config, db, [None, None], window, cx));
        workspace.read(cx).copilot.clone().update(cx, |connection, _| {
            connection.enabled = true;
            connection.quota = Some(serde_json::from_value(serde_json::json!({"copilotPlan":"free","completions":{"unlimited":false,"percentRemaining":42.5},"premium_interactions":{"unlimited":true}})).unwrap());
        });
        cx.new(|_| StatusFixture(workspace))
    }))?;
    let status_handle = status_handle.into(); settle(cx);
    cx.update_window(status_handle, |_, window, cx| -> Result<()> {
        window.render_frame(cx);
        ensure!(window.find("status-copilot").visible(), "Copilot status button is hidden");
        let connection = status_fixture.read(cx).0.read(cx).copilot.read(cx);
        ensure!(connection.detail().contains("57.5% used · 42.5% remaining"), "Status tooltip lost server quota");
        window.hover("status-copilot", cx); Ok(())
    })??;
    cx.advance_clock(Duration::from_secs(1)); cx.run_until_parked();
    cx.update_window(status_handle, |_, window, cx| window.render_frame(cx))?;
    cx.advance_clock(Duration::from_millis(200)); cx.run_until_parked();
    if pixels { std::thread::sleep(Duration::from_millis(200)); }
    cx.update_window(status_handle, |_, window, cx| window.render_frame(cx))?;
    if pixels { cx.capture_screenshot(status_handle)?.save(output.join("copilot-usage-hover.png"))?; }
    cx.update_window(status_handle, |_, window, cx| -> Result<()> {
        window.click("status-copilot", cx);
        ensure!(status_fixture.read(cx).0.read(cx).settings.tab == crate::settings::SettingsTab::Editor, "Copilot status button did not open Editor settings");
        window.remove_window(); Ok(())
    })??;
    Ok(serde_json::json!({"fixture":"copilot","passed":true,"pixels":pixels,"checks":["hidden without Alt","Alt reveals native ghost text","preview preserves document","Alt release hides immediately","Tab and Alt+L accept visible prediction","Escape dismisses","typing invalidates stale predictions","focus loss hides prediction","turning off clears preview"]}))
}
