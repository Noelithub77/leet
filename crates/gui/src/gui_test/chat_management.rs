//! Archive from history and manage every problem's archive from General settings.
use std::{path::Path, sync::Arc};
use anyhow::{Result, ensure};
use gpui_kit::*;
use gpui_kit::component::{ActiveTheme as _, v_flex};
use gpui_kit::component::button::Button;
use gpui_kit::test::TestWindowExt as _;
use practice::{config::Config, db::Db};

struct Fixture { workspace: Entity<crate::workspace::Workspace>, settings: bool }
impl Render for Fixture {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content = if self.settings {
            self.workspace.update(cx, |ws, cx| ws.render_settings(window, cx).into_any_element())
        } else {
            let assist = self.workspace.read(cx).assist.clone();
            assist.update(cx, |assist, cx| v_flex().relative().w_full().h(px(420.)).child(assist.chat_header(cx)).children(assist.thread_popover(cx)).into_any_element())
        };
        v_flex().size_full().bg(cx.theme().background)
            .child(Button::new("fixture-settings").label("Settings").on_click(cx.listener(|this, _, window, cx| {
                this.settings = true;
                this.workspace.update(cx, |ws, cx| ws.open_settings(Some(crate::settings::Setting::ArchivedChats), window, cx));
                cx.notify();
            }))).child(content)
    }
}

pub(super) fn native(cx: &mut HeadlessAppContext, output: &Path, pixels: bool) -> Result<()> {
    for (name, width, theme, zoom) in [("dark", 720., "Vesper", 1.), ("light", 600., "Solarized Light", 1.2)] {
        cx.update(|cx| { crate::theme::apply(theme, cx); crate::theme::set_zoom(zoom, cx); });
        let db = Arc::new(Db::open(&output.join(format!("session/manage-{name}.sqlite")))?);
        let first = db.create_chat(Some("manage-fixture"), "Map hints")?;
        let second = db.create_chat(Some("other-fixture"), "Stack explanation")?;
        for (thread, problem, title) in [(first.id, "manage-fixture", "Two Sum"), (second.id, "other-fixture", "Valid Parentheses")] {
            db.append_chat(thread, &practice::chat::history::SavedTurn { citations: vec![], model: "fixture".into(), agent: None, answer: None,
                request_prompt: String::new(), instructions: "Help".into(), action: None, phase: Default::default(), problem: problem.into(), problem_title: title.into() })?;
        }
        db.save_chat_draft(first.id, "Keep my draft")?;
        db.set_chat_archived(second.id, true)?;
        let (handle, _) = cx.update(|cx| gpui_kit::open_window(WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::new(point(px(0.), px(0.)), size(px(width), px(760.))))),
            show: false, focus: false, ..Default::default()
        }, cx, |window, cx| cx.new(|cx| {
            let config = Config { onboarding_completed: true, workspace: output.join("session/manage-solutions"), ..Default::default() };
            let workspace = cx.new(|cx| crate::workspace::Workspace::from_storage(config, db.clone(), [None, None], window, cx));
            let assist = workspace.read(cx).assist.clone();
            cx.observe(&assist, |_, _, cx| cx.notify()).detach();
            assist.update(cx, |assist, cx| {
                assist.set_problem(Some("manage-fixture"), cx);
                assist.select_thread(first.id, window, cx); assist.open_thread_list(window, cx);
            });
            Fixture { workspace, settings: false }
        })))?;
        let handle = handle.into(); cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| { window.render_frame(cx); })?;
        if pixels { cx.capture_screenshot(handle)?.save(output.join(format!("chat-archive-icon-{name}.png")))?; }
        cx.update_window(handle, |_, window, cx| -> Result<()> {
            window.click(("chat-archive", first.id as u64), cx);
            ensure!(db.chat_thread(first.id)?.archived, "Archive icon did not archive its chat");
            window.click("chat-undo", cx);
            ensure!(!db.chat_thread(first.id)?.archived && db.chat_thread(first.id)?.draft == "Keep my draft", "Undo lost the archived draft");
            window.click("chat-thread-picker", cx);
            window.click(("chat-archive", first.id as u64), cx);
            window.click("fixture-settings", cx);
            window.click(("setting", 0usize), cx);
            ensure!(window.find(("restore-chat", first.id as u64)).visible() && window.find(("restore-chat", second.id as u64)).visible(), "General settings did not show archives from both problems");
            Ok(())
        })??;
        cx.run_until_parked();
        if pixels { cx.capture_screenshot(handle)?.save(output.join(format!("chat-archived-settings-{name}.png")))?; }
        cx.update_window(handle, |_, window, cx| -> Result<()> {
            window.click(("restore-chat", first.id as u64), cx);
            ensure!(!db.chat_thread(first.id)?.archived && db.chat_thread(second.id)?.archived, "Restore changed the wrong problem's chat");
            window.click(("delete-chat", second.id as u64), cx);
            ensure!(db.chat_thread(second.id).is_ok(), "Delete skipped confirmation");
            window.click("cancel-delete-chat", cx);
            ensure!(db.chat_thread(second.id).is_ok(), "Cancel deleted the chat");
            window.click(("delete-chat", second.id as u64), cx);
            Ok(())
        })??;
        cx.run_until_parked();
        if pixels { cx.capture_screenshot(handle)?.save(output.join(format!("chat-delete-confirm-{name}.png")))?; }
        cx.update_window(handle, |_, window, cx| -> Result<()> {
            window.click("confirm-delete-chat", cx);
            ensure!(db.chat_thread(second.id).is_err() && db.chat_messages(second.id)?.is_empty(), "Permanent deletion left the chat behind");
            ensure!(db.chat_thread(first.id)?.draft == "Keep my draft", "Permanent deletion affected a different chat");
            window.press("escape", cx); window.remove_window(); Ok(())
        })??;
        cx.run_until_parked();
    }
    cx.update(|cx| { crate::theme::apply("Vesper", cx); crate::theme::set_zoom(1., cx); }); Ok(())
}
