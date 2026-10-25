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
        let settings = self.0.update(cx, |ws, cx| (ws.center == crate::workspace::Center::Settings).then(|| ws.render_settings(window, cx).into_any_element()));
        let status = self.0.update(cx, |ws, cx| ws.render_status(window, cx).into_any_element());
        gpui_kit::component::v_flex().size_full().bg(cx.theme().background)
            .child(div().flex_1().children(settings)).child(status)
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
        let mut pane = crate::snippets::expansion::EditorPane::new(state.clone(), Language::Python, std::rc::Rc::new(practice::snippets::search::Library::new(vec![])), Default::default(), window, cx);
        let connection = cx.new(|_| Connection::new(true));
        pane.connect_predictions(connection, window, cx);

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
        let editor = pane.read(cx).state.clone();
        let predictions = pane.read(cx).predictions.as_ref().unwrap();
        let (text, styles) = predictions.inline_highlights(editor.read(cx).value().as_str(), editor.read(cx).cursor(), cx).unwrap();
        ensure!(styles.iter().all(|(_, style)| style.color.is_some_and(|color| color.a > 0. && color.a < 0.8)), "Ghost syntax colors are not faint");
        ensure!(styles.iter().filter_map(|(_, style)| style.color).any(|color| color != styles[0].1.color.unwrap()), "Ghost preview lost syntax color differences");
        editor.update(cx, |editor, cx| -> Result<()> {
            ensure!(!editor.set_inline_completion_highlights("stale", styles.clone(), cx), "Stale ghost styles were accepted");
            ensure!(!editor.set_inline_completion_highlights(&text, vec![(0..text.len() + 1, HighlightStyle::default())], cx), "Invalid ghost style bounds were accepted");
            ensure!(editor.set_inline_completion_highlights(&text, styles, cx), "Current ghost styles were rejected");
            Ok(())
        })?;
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
        ensure!(pane.read(cx).state.read(cx).value().as_str() == "pri", "Alt+Tab accepted a prediction");
        window.press("alt-l", cx);
        ensure!(pane.read(cx).state.read(cx).value().as_str() == "print(1)\n", "Alt+L did not accept the visible prediction");
        ensure!(!EditorMode::has_inline_completion(pane.read(cx).state.read(cx)), "Accepted prediction stayed visible");
        ensure!(pane.read(cx).can_revert_prediction(cx), "Accepted prediction cannot be reverted");
        window.render_frame(cx); window.press("alt-k", cx);
        ensure!(pane.read(cx).state.read(cx).value().as_str() == "pri", "Alt+K did not restore the original source");
        ensure!(pane.read(cx).state.read(cx).cursor() == 3, "Alt+K did not restore the caret");
        Ok(())
    })??;
    settle(cx);
    cx.update_window(handle, |_, window, cx| -> Result<()> {
        ensure!(EditorMode::has_inline_completion(pane.read(cx).state.read(cx)), "Revert did not restore the preview");
        window.render_frame(cx); window.press("alt-l", cx);
        ensure!(pane.read(cx).state.read(cx).value().as_str() == "print(1)\n", "Alt+L did not accept again after reverting");
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
        window.input("x", cx); window.press("alt-k", cx);
        ensure!(pane.read(cx).state.read(cx).value().as_str() == "print(1)\nx", "Alt+K removed later typing");
        let state = pane.read(cx).state.clone();
        state.update(cx, |editor, cx| editor.set_value("", window, cx));
        alt(window, false, cx); window.input("pri", cx); window.render_frame(cx); alt(window, true, cx); Ok(())
    })??;
    settle(cx);
    cx.update_window(handle, |_, window, cx| -> Result<()> {
        window.input("x", cx); window.render_frame(cx); window.press("alt-l", cx);
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
        let code = format!("nt(1)\n{}", (0..40).map(|i| format!("print(\"café {i}\")\n")).collect::<String>());
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
        let editor = pane.read(cx).state.clone();
        let (text, _) = pane.read(cx).predictions.as_ref().unwrap().inline_highlights(editor.read(cx).value().as_str(), editor.read(cx).cursor(), cx).unwrap();
        let boundary = text.find('é').unwrap() + 1;
        ensure!(!editor.update(cx, |editor, cx| editor.set_inline_completion_highlights(&text,
            vec![(boundary..boundary + 1, HighlightStyle::default())], cx)), "Mid-character ghost style boundary was accepted");
        let editor_scroll = pane.read(cx).state.read(cx).scroll_offset();
        window.scroll("copilot-full-preview", ScrollDelta::Lines(point(0., -12.)), cx);
        window.render_frame(cx);
        ensure!(window.find("copilot-preview-code").bounds().top() < before, "Long preview did not scroll");
        ensure!(pane.read(cx).state.read(cx).scroll_offset() == editor_scroll, "Preview wheel input scrolled the source editor");
        let after = window.find("copilot-preview-popover").bounds();
        ensure!(after.top() >= px(0.) && after.bottom() <= window.viewport_size().height, "Scrolled preview escaped the window");
        Ok(())
    })??;
    if pixels { cx.capture_screenshot(handle)?.save(output.join("copilot-full-preview-scrolled.png"))?; }
    cx.update_window(handle, |_, window, _| window.remove_window())?;
    let db = Arc::new(practice::db::Db::open(&output.join("session/copilot-status.sqlite"))?);
    let config = practice::config::Config { onboarding_completed: true, workspace: output.join("session/solutions"), ..Default::default() };
    let (status_handle, status_fixture) = cx.update(|cx| gpui_kit::open_window(WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds { origin: point(px(0.), px(0.)), size: size(px(1000.), px(700.)) })), show: false, focus: false, ..Default::default()
    }, cx, |window, cx| {
        let workspace = cx.new(|cx| crate::workspace::Workspace::from_storage(config, db, std::array::from_fn(|_| None), window, cx));
        workspace.read(cx).copilot.clone().update(cx, |connection, _| {
            connection.enabled = true;
            connection.fixture_no_transport = true;
            connection.account = Some(practice::copilot::Account { status: "OK".into(), user: Some("fixture-user".into()) });
            connection.quota = Some(serde_json::from_value(serde_json::json!({"copilotPlan":"free","completions":{"unlimited":false,"percentRemaining":42.5},"premium_interactions":{"unlimited":true}})).unwrap());
        });
        cx.new(|_| StatusFixture(workspace))
    }))?;
    let status_handle = status_handle.into(); settle(cx);
    cx.update_window(status_handle, |_, window, cx| -> Result<()> {
        window.activate_window(); window.render_frame(cx);
        window.click("status-copilot", cx);
        let ws = status_fixture.read(cx).0.read(cx);
        ensure!(!ws.copilot.read(cx).enabled() && !ws.config.copilot_enabled, "Single click did not disable and persist completions");
        ensure!(ws.copilot.read(cx).signed_in(), "Disabling completions signed the account out");
        ensure!(ws.copilot.read(cx).quota.is_some(), "Disabling completions discarded account usage");
        ensure!(ws.center != crate::workspace::Center::Settings, "The toggle opened Settings");
        window.render_frame(cx);
        ensure!(window.find("copilot-disabled-mark").visible(), "Disabled Copilot has no cross");
        Ok(())
    })??;
    if pixels { cx.capture_screenshot(status_handle)?.save(output.join("copilot-disabled-status.png"))?; }
    cx.update_window(status_handle, |_, window, cx| -> Result<()> {
        window.click("status-copilot", cx);
        ensure!(status_fixture.read(cx).0.read(cx).copilot.read(cx).enabled(), "Second click did not re-enable completions");
        window.render_frame(cx);
        ensure!(window.try_find("copilot-disabled-mark").is_none(), "Enabled Copilot kept the cross");
        window.click("copilot-options", cx); Ok(())
    })??;
    settle(cx);
    cx.update_window(status_handle, |_, window, cx| -> Result<()> {
        window.render_frame(cx);
        ensure!(window.find("popup-menu").visible(), "Copilot options did not open");
        let connection = status_fixture.read(cx).0.read(cx).copilot.read(cx);
        ensure!(connection.detail().contains("57.5% used · 42.5% remaining"), "Options lost the reported quota");
        Ok(())
    })??;
    if pixels { cx.capture_screenshot(status_handle)?.save(output.join("copilot-options.png"))?; }
    cx.update_window(status_handle, |_, window, cx| { window.press("escape", cx); })?;
    settle(cx);
    cx.update_window(status_handle, |_, window, cx| -> Result<()> {
        window.render_frame(cx); ensure!(window.try_find("popup-menu").is_none(), "Escape did not close Copilot options");
        window.click("copilot-options", cx); Ok(())
    })??;
    settle(cx);
    cx.update_window(status_handle, |_, window, cx| { window.press("enter", cx); })?;
    settle(cx);
    cx.update_window(status_handle, |_, window, cx| -> Result<()> {
        window.render_frame(cx);
        let ws = status_fixture.read(cx).0.read(cx);
        ensure!(ws.center == crate::workspace::Center::Settings && ws.settings.tab == crate::settings::SettingsTab::Editor, "Options did not open Copilot settings");
        ensure!(window.find("copilot-account").visible() && window.find("copilot-sign-out").visible(), "Signed-in Settings lacks account or Sign out");
        ensure!(window.try_find("copilot-connect").is_none(), "Signed-in Settings still shows connection toggle");
        Ok(())
    })??;
    if pixels { cx.capture_screenshot(status_handle)?.save(output.join("copilot-account-settings.png"))?; }
    cx.update_window(status_handle, |_, window, cx| -> Result<()> {
        window.click("copilot-sign-out", cx);
        let connection = status_fixture.read(cx).0.read(cx).copilot.read(cx);
        ensure!(connection.signed_in() && connection.error.is_some(), "Missing server reported a false successful sign-out");
        status_fixture.read(cx).0.read(cx).copilot.clone().update(cx, |connection, cx| {
            connection.account = None; connection.error = None; connection.busy = true;
            connection.progress = Some(practice::copilot::install::Progress::Downloading { received: 5 << 20, total: Some(20 << 20) }.label());
            connection.elapsed = 12; cx.notify();
        });
        status_fixture.read(cx).0.clone().update(cx, |workspace, cx| {
            workspace.tool_install.fixture = true;
            workspace.setup_tools(Language::Cpp, false, window, cx);
            workspace.tool_install_event(Language::Cpp, crate::tool_install::Event::Progress("Downloading compiler · 25% · 10.0 / 40.0 MiB".into()), window, cx);
        });
        window.render_frame(cx);
        ensure!(window.find("copilot-setup-progress").visible(), "Copilot setup has no progress indicator while waiting");
        ensure!(window.find("copilot-background-progress").visible(), "Copilot setup has no status-bar progress");
        let tools = window.find("tool-setup-status").bounds();
        let copilot = window.find("copilot-status-controls").bounds();
        ensure!(tools.right() <= copilot.left() && copilot.right() <= window.viewport_size().width, "Simultaneous setup indicators overlap or escape the status bar");
        ensure!(status_fixture.read(cx).0.read(cx).copilot.read(cx).label().contains("25% · 5.0 / 20.0 MiB"), "Setup lost actual transfer progress");
        Ok(())
    })??;
    if pixels { cx.capture_screenshot(status_handle)?.save(output.join("copilot-setup-download.png"))?; }
    cx.update_window(status_handle, |_, window, cx| -> Result<()> {
        let connection = status_fixture.read(cx).0.read(cx).copilot.clone();
        connection.update(cx, |connection, cx| { connection.code = Some("TEST-CODE".into()); cx.notify(); });
        window.render_frame(cx);
        ensure!(connection.read(cx).label() == "Finish sign-in in your browser", "Browser approval still reads as download progress");
        window.click("copilot-copy-code", cx);
        ensure!(cx.read_from_clipboard().and_then(|item| item.text()).as_deref() == Some("TEST-CODE"), "Device code could not be copied while setup was busy");
        Ok(())
    })??;
    if pixels { cx.capture_screenshot(status_handle)?.save(output.join("copilot-setup-sign-in.png"))?; }
    cx.update_window(status_handle, |_, window, cx| -> Result<()> {
        window.click("copilot-connect", cx);
        let connection = status_fixture.read(cx).0.read(cx).copilot.clone();
        ensure!(!connection.read(cx).busy && connection.read(cx).code.is_none() && connection.read(cx).progress.is_none(), "Cancel left setup activity or a device code behind");
        connection.update(cx, |connection, cx| { connection.error = Some("Download timed out. Check your connection and retry setup.".into()); cx.notify(); });
        window.render_frame(cx);
        ensure!(window.try_find("copilot-setup-progress").is_none(), "Failed setup kept spinning");
        ensure!(window.find("copilot-connect").visible(), "Failed setup has no retry control");
        Ok(())
    })??;
    if pixels { cx.capture_screenshot(status_handle)?.save(output.join("copilot-setup-failed.png"))?; }
    cx.update_window(status_handle, |_, window, _| window.remove_window())?;
    Ok(serde_json::json!({"fixture":"copilot","passed":true,"pixels":pixels,"checks":["hidden without Alt","Alt reveals native ghost text","faint syntax colors","stale and invalid highlight ranges rejected","preview preserves document","Alt release hides immediately","Alt+Tab does not accept","Alt+L accepts and Alt+K reverts","revert restores the preview and caret","revert preserves later typing","Escape dismisses","typing invalidates stale predictions","focus loss hides prediction","turning off clears preview","long Unicode preview scrolls without moving the editor"]}))
}
