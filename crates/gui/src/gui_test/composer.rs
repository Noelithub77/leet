//! Production composer input and containment, without a live workspace or agent.
use std::{path::Path, sync::Arc};
use anyhow::{Result, ensure};
use gpui_kit::*;
use gpui_kit::component::{ActiveTheme as _, v_flex};
use gpui_kit::test::TestWindowExt as _;
use practice::{config::Config, db::Db};
use super::Assist;

struct Fixture {
    assist: Entity<Assist>,
    enabled: bool,
    busy: bool,
    threads: bool,
}

impl Render for Fixture {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.threads {
            let header = self.assist.update(cx, |assist, cx| assist.thread_header(window, cx));
            let composer = self.assist.read(cx).threads.is_empty().then(|| self.assist.update(cx, |assist, cx| assist.render_composer(true, false, false, cx).into_any_element()));
            return v_flex().key_context("AgentChat").size_full().bg(cx.theme().background)
                .on_action(cx.listener(|view, _: &super::NewChat, window, cx| view.assist.update(cx, |assist, cx| assist.new_thread(window, cx))))
                .child(header).children(composer);
        }
        let composer = self.assist.update(cx, |assist, cx| assist.render_composer(self.enabled, false, self.busy, cx).into_any_element());
        v_flex().size_full().bg(cx.theme().background).child(composer)
    }
}

pub(crate) fn native(cx: &mut HeadlessAppContext, output: &Path, pixels: bool) -> Result<serde_json::Value> {
    let db = Arc::new(Db::open(&output.join("session/composer.sqlite"))?);
    cx.update(super::bind_keys);
    for (name, width, height, theme, zoom) in [("regular", 420., 200., "Vesper", 1.), ("compact", 260., 200., "Vesper", 1.), ("light", 420., 200., "Solarized Light", 1.), ("zoom", 360., 240., "Vesper", 1.4)] {
        cx.update(|cx| { crate::theme::apply(theme, cx); crate::theme::set_zoom(zoom, cx); });
        let (handle, fixture) = cx.update(|cx| gpui_kit::open_window(WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds { origin: point(px(0.), px(0.)), size: size(px(width), px(height)) })),
            show: false, focus: false, ..Default::default()
        }, cx, |window, cx| cx.new(|cx| {
            let config = Config { onboarding_completed: true, workspace: output.join("session/solutions"), ..Config::default() };
            let workspace = cx.new(|cx| crate::workspace::Workspace::from_storage(config, db.clone(), [None, None], window, cx));
            let assist = workspace.read(cx).assist.clone();
            cx.observe(&assist, |_, _, cx| cx.notify()).detach();
            // Dropping the fixture workspace leaves the real send handler without transport side effects.
            drop(workspace);
            Fixture { assist, enabled: true, busy: false, threads: false }
        })))?;
        let handle = handle.into();
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| -> Result<()> {
            window.render_frame(cx);
            window.click_at("chat-composer", point(px(16.), px(16.)), cx);
            window.press("enter", cx);
            Ok(())
        })??;
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| -> Result<()> {
            let assist = fixture.read(cx).assist.clone();
            ensure!(assist.read(cx).ai_tab == 0, "Empty Enter sent a chat");
            window.input("tell me more", cx);
            window.press("shift-enter", cx);
            window.input("about maps", cx);
            ensure!(assist.read(cx).composer.read(cx).value().as_ref() == "tell me more\nabout maps", "Shift+Enter did not add a line");
            ensure!(assist.read(cx).ai_tab == 0, "Shift+Enter sent a chat");
            window.press("ctrl-enter", cx);
            ensure!(assist.read(cx).ai_tab == 0, "Removed Ctrl+Enter binding still sends");
            Ok(())
        })??;
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| -> Result<()> {
            ensure!(fixture.read(cx).assist.read(cx).ai_tab == 0, "Line break or removed shortcut sent a chat");
            window.press("enter", cx);
            Ok(())
        })??;
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| -> Result<()> {
            let assist = fixture.read(cx).assist.clone();
            ensure!(assist.read(cx).ai_tab == 2, "Enter did not invoke send");
            ensure!(assist.read(cx).composer.read(cx).value().as_ref() == "tell me more\nabout maps", "Enter inserted a newline");
            assist.update(cx, |assist, _| assist.ai_tab = 0);
            window.click("chat-send", cx);
            Ok(())
        })??;
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| -> Result<()> {
            let assist = fixture.read(cx).assist.clone();
            ensure!(assist.read(cx).ai_tab == 2, "Send button did not invoke send");
            let group = window.find("chat-composer").bounds();
            let button = window.find("chat-send").bounds();
            ensure!(button.left() >= group.left() && button.right() < group.right() && button.top() >= group.top() && button.bottom() < group.bottom(), "Send escaped the prompt box");
            ensure!(group.right() <= window.viewport_size().width && group.bottom() <= window.viewport_size().height, "Composer escaped the window");
            Ok(())
        })??;
        if pixels {
            cx.run_until_parked();
            cx.update_window(handle, |_, window, cx| window.render_frame(cx))?;
            cx.capture_screenshot(handle)?.save(output.join(format!("composer-{name}.png")))?;
        }
        cx.update_window(handle, |_, window, cx| -> Result<()> {
            let assist = fixture.read(cx).assist.clone();
            fixture.update(cx, |view, cx| { view.busy = true; cx.notify(); });
            window.render_frame(cx);
            let group = window.find("chat-composer").bounds();
            let stop = window.find("chat-send-stop").bounds();
            ensure!(stop.right() < group.right() && stop.bottom() < group.bottom(), "Stop escaped the prompt box");
            fixture.update(cx, |view, cx| { view.busy = false; view.enabled = false; cx.notify(); });
            assist.update(cx, |assist, _| assist.ai_tab = 0);
            window.render_frame(cx);
            window.click("chat-send", cx);
            ensure!(assist.read(cx).ai_tab == 0, "Disabled composer sent a chat");
            fixture.update(cx, |view, cx| { view.threads = true; cx.notify(); });
            assist.update(cx, |assist, cx| {
                assist.slug = Some("composer-fixture".into());
                if assist.db.visible_chat_threads(assist.slug.as_deref(), "").unwrap().is_empty() {
                    for title in ["New chat", "Previous conversation", "Maps"] {
                        let thread = assist.db.create_chat(assist.slug.as_deref(), title).unwrap();
                        if title != "New chat" {
                            assist.db.append_chat(thread.id, &practice::chat::history::SavedTurn {
                                model: "fixture".into(), agent: None, answer: None, request_prompt: String::new(), instructions: title.into(),
                                action: Some(practice::assist::Action::Ask), phase: practice::chat::history::SavedPhase::Done,
                                problem: "composer-fixture".into(), problem_title: String::new(),
                            }).unwrap();
                        }
                    }
                }
                assist.refresh_threads();
                assist.selected_thread = Some(assist.threads[1].id);
                assist.open_thread_list(window, cx);
            });
            window.render_frame(cx);
            window.hover("chat-thread-picker", cx);
            let toolbar = window.find("chat-thread-toolbar").bounds();
            let search = window.find("chat-thread-search").bounds();
            ensure!(toolbar.left() == search.left() && toolbar.right() == search.right(), "History toolbar and search edges do not align");
            ensure!(assist.read(cx).threads.len() == 2 && assist.read(cx).threads.iter().all(|thread| thread.title != "New chat"), "Empty New chat leaked into the list");
            Ok(())
        })??;
        cx.run_until_parked();
        if pixels { cx.capture_screenshot(handle)?.save(output.join(format!("threads-{name}.png")))?; }
        cx.update_window(handle, |_, window, cx| -> Result<()> {
            window.render_frame(cx);
            let assist = fixture.read(cx).assist.clone();
            let selected = assist.read(cx).threads[0].id;
            window.press("up", cx);
            window.press("enter", cx);
            ensure!(assist.read(cx).selected_thread == Some(selected) && !assist.read(cx).show_threads, "Keyboard selection did not open the selected chat");
            for shortcut in ["ctrl-t", "ctrl-shift-o", "ctrl-n"] {
                assist.update(cx, |assist, cx| assist.open_thread_list(window, cx));
                window.render_frame(cx);
                window.press(shortcut, cx);
                ensure!(assist.read(cx).ai_tab == 2 && !assist.read(cx).show_threads, "{shortcut} did not start a chat");
                let id = assist.read(cx).selected_thread.unwrap();
                ensure!(assist.read(cx).db.chat_messages(id)?.is_empty(), "{shortcut} kept the existing conversation");
                ensure!(assist.read(cx).composer.read(cx).value().is_empty(), "{shortcut} kept the previous prompt");
                ensure!(assist.read(cx).composer.focus_handle(cx).is_focused(window), "{shortcut} did not focus the prompt");
                ensure!(assist.read(cx).threads.len() == 2, "{shortcut} added an empty list option");
            }
            assist.update(cx, |assist, cx| {
                assist.set_problem(Some("empty-fixture"), cx);
                assist.sync_composer(window, cx);
                assist.open_thread_list(window, cx);
            });
            window.render_frame(cx);
            ensure!(window.try_find("chat-thread-picker").is_none() && window.try_find("chat-thread-search").is_none(), "Empty history still renders a picker or search");
            window.click("chat-new-thread", cx);
            ensure!(assist.read(cx).composer.focus_handle(cx).is_focused(window), "Plus did not focus the empty prompt");
            window.render_frame(cx);
            Ok(())
        })??;
        cx.run_until_parked();
        if pixels { cx.capture_screenshot(handle)?.save(output.join(format!("threads-empty-{name}.png")))?; }
        cx.update_window(handle, |_, window, _cx| -> Result<()> {
            window.remove_window();
            Ok(())
        })??;
        cx.run_until_parked();
    }
    cx.update(|cx| { crate::theme::apply("Vesper", cx); crate::theme::set_zoom(1., cx); });
    Ok(serde_json::json!({"fixture":"chat-composer","passed":true,"pixels":pixels,"checks":["Enter sends","Shift+Enter adds a line","Ctrl+Enter removed","empty and disabled guards","send button input","send and stop containment","compact/light/zoom","thread borders and keyboard selection","empty chats hidden","Ctrl+T / Ctrl+Shift+O / Ctrl+N","aligned toolbar and search","empty history has only Plus"]}))
}
