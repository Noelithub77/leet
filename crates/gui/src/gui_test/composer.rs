//! Production composer input and containment, without a live workspace or agent.
use std::{path::Path, sync::Arc};
use anyhow::{Result, ensure};
use gpui_kit::*;
use gpui_kit::component::{ActiveTheme as _, v_flex};
use gpui_kit::test::TestWindowExt as _;
use practice::{config::Config, db::Db};
use super::Assist;

#[path = "tooltip.rs"]
mod tooltip;
#[path = "chat_management.rs"]
mod management;

fn stuck_answer() -> practice::assist::Answer {
    practice::assist::Answer::Stuck(practice::assist::Stuck {
        diagnosis: "The range never shrinks".into(), line: Some(1), snippet: None, steps: vec![], concept: "Binary search bounds".into(),
        next_step: "Trace a two-item list".into(), question: "What stops lo from reaching hi?".into(),
    })
}

struct LaunchFixture {
    workspace: Entity<crate::workspace::Workspace>,
    snapshot: super::Snapshot,
}
impl Render for LaunchFixture {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        gpui_kit::component::button::Button::new("fixture-ai-run").label("Start AI")
            .on_click(cx.listener(|fixture, _, window, cx| {
                let snapshot = fixture.snapshot.clone();
                fixture.workspace.update(cx, |ws, cx| {
                    ws.assist.update(cx, |assist, cx| assist.start(super::Action::Hints, snapshot,
                        super::Target::Agent(practice::agents::Detected {
                            kind: practice::agents::AgentKind::OpenCode,
                            path: "/fixture/no-agent".into(), version: None,
                        }, None), window, cx));
                });
            }))
    }
}

fn launch_inside_workspace(cx: &mut HeadlessAppContext, output: &Path) -> Result<()> {
    let db = Arc::new(Db::open(&output.join("session/launch.sqlite"))?);
    let (handle, fixture) = cx.update(|cx| gpui_kit::open_window(WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds { origin: point(px(0.), px(0.)), size: size(px(400.), px(200.)) })),
        show: false, focus: false, ..Default::default()
    }, cx, |window, cx| cx.new(|cx| {
        let config = Config { onboarding_completed: true, workspace: output.join("session/launch-solutions"), ..Default::default() };
        let workspace = cx.new(|cx| crate::workspace::Workspace::from_storage(config, db, [None, None], window, cx));
        workspace.read(cx).assist.clone().update(cx, |assist, _| assist.fixture_no_transport = true);
        LaunchFixture { workspace, snapshot: super::Snapshot {
            slug: "launch-fixture".into(), title: "Launch fixture".into(), difficulty: "Easy".into(), url: String::new(),
            statement_html: String::new(), code: "pass\n".into(), starter: String::new(), language: practice::language::Language::Python,
            cases: vec![], focus_case: 0, solution_path: output.join("session/launch-solutions/solution.py"),
            workspace: output.join("session/launch-solutions"), trace: None,
        } }
    })))?;
    let handle = handle.into(); cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| { window.render_frame(cx); window.click("fixture-ai-run", cx); })?;
    cx.run_until_parked();
    cx.update_window(handle, |_, _, cx| -> Result<()> {
        let ws = fixture.read(cx).workspace.read(cx);
        let assist = ws.assist.read(cx);
        let run = assist.runs.first().ok_or_else(|| anyhow::anyhow!("AI run was not created"))?;
        ensure!(ws.db.chat_thread(run.thread_id)?.title == "Hints", "First quick action remained an unnamed draft");
        ensure!(run.generation == 1, "Deferred launch did not prepare the run");
        ensure!(run.snippet_before.as_ref().is_some_and(|(dir, _)| dir == &ws.snippet_dir), "Launch did not capture workspace context");
        Ok(())
    })??;
    // An action card and the user's text travel as one message; other sends leave the card in place.
    cx.update_window(handle, |_, window, cx| -> Result<()> {
        let fixture = fixture.read(cx);
        let (assist, snapshot) = (fixture.workspace.read(cx).assist.clone(), fixture.snapshot.clone());
        let target = || super::Target::Agent(practice::agents::Detected { kind: practice::agents::AgentKind::OpenCode, path: "/fixture/no-agent".into(), version: None }, None);
        assist.update(cx, |assist, cx| {
            assist.prefill_action(super::Action::Explain, window, cx);
            assist.composer.update(cx, |input, cx| input.set_value("Focus on the map", window, cx));
            assist.start(super::Action::Ask, snapshot.clone(), target(), window, cx);
        });
        ensure!(assist.read(cx).composer_action == Some(super::Action::Explain), "A plain send consumed the draft's action card");
        assist.update(cx, |assist, cx| {
            assist.composer.update(cx, |input, cx| input.set_value("Focus on the map", window, cx));
            assist.start(super::Action::Explain, snapshot, target(), window, cx);
        });
        let assist = assist.read(cx);
        let run = assist.runs.first().ok_or_else(|| anyhow::anyhow!("Card message was not created"))?;
        ensure!(run.action == super::Action::Explain && run.request_prompt == super::Action::Explain.instructions() && run.instructions == "Focus on the map", "Card message lost its prompt or the user's text");
        ensure!(assist.composer_action.is_none() && assist.composer.read(cx).value().is_empty(), "Sending kept the action card in the prompt");
        window.remove_window(); Ok(())
    })??;
    cx.run_until_parked(); Ok(())
}

struct Fixture {
    assist: Entity<Assist>,
    enabled: bool,
    busy: bool,
    threads: bool,
    full_view: bool,
}

impl Render for Fixture {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.full_view {
            return v_flex().size_full().bg(cx.theme().background).child(self.assist.clone());
        }
        if self.threads {
            let header = self.assist.update(cx, |assist, cx| assist.chat_header(cx));
            let popover = self.assist.update(cx, |assist, cx| assist.thread_popover(cx));
            let composer = self.assist.read(cx).threads.is_empty().then(|| self.assist.update(cx, |assist, cx| { assist.fit_composer(window, cx); assist.render_composer(true, false, false, cx).into_any_element() }));
            return v_flex().key_context("AgentChat").relative().size_full().bg(cx.theme().background)
                .on_action(cx.listener(|view, _: &super::NewChat, window, cx| view.assist.update(cx, |assist, cx| assist.new_thread(window, cx))))
                .child(header).child(div().flex_1()).children(composer).children(popover);
        }
        let composer = self.assist.update(cx, |assist, cx| { assist.fit_composer(window, cx); assist.render_composer(self.enabled, false, self.busy, cx).into_any_element() });
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
            assist.update(cx, |assist, _| assist.ai_tab = 1);
            Fixture { assist, enabled: true, busy: false, threads: false, full_view: false }
        })))?;
        let handle = handle.into();
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            let search = fixture.read(cx).assist.read(cx).thread_search.focus_handle(cx);
            search.focus(window, cx); window.render_frame(cx);
        })?;
        cx.run_until_parked();
        let idle = cx.update_window(handle, |_, window, cx| -> Result<Pixels> {
            window.render_frame(cx);
            let idle = window.find("chat-composer").bounds().size.height;
            window.click_at("chat-composer", point(px(16.), px(12.)), cx);
            window.press("enter", cx);
            Ok(idle)
        })??;
        cx.run_until_parked();
        // Let the focus change settle before measuring the expanded prompt.
        cx.update_window(handle, |_, window, cx| window.render_frame(cx))?;
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| -> Result<()> {
            let assist = fixture.read(cx).assist.clone();
            window.render_frame(cx);
            ensure!(window.find("chat-composer").bounds().size.height > idle * 1.5, "Focused prompt did not expand from its one-line pill");
            ensure!(assist.read(cx).ai_tab == 1, "Empty Enter sent a chat");
            window.input("tell me more", cx);
            window.press("shift-enter", cx);
            window.input("about maps", cx);
            ensure!(assist.read(cx).composer.read(cx).value().as_ref() == "tell me more\nabout maps", "Shift+Enter did not add a line");
            ensure!(assist.read(cx).ai_tab == 1, "Shift+Enter sent a chat");
            window.press("ctrl-enter", cx);
            ensure!(assist.read(cx).ai_tab == 1, "Removed Ctrl+Enter binding still sends");
            Ok(())
        })??;
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| -> Result<()> {
            ensure!(fixture.read(cx).assist.read(cx).ai_tab == 1, "Line break or removed shortcut sent a chat");
            window.press("enter", cx);
            Ok(())
        })??;
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| -> Result<()> {
            let assist = fixture.read(cx).assist.clone();
            ensure!(assist.read(cx).ai_tab == 2, "Enter did not invoke send");
            ensure!(assist.read(cx).composer.read(cx).value().as_ref() == "tell me more\nabout maps", "Enter inserted a newline");
            assist.update(cx, |assist, _| assist.ai_tab = 1);
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
            let runs = assist.read(cx).runs.len();
            assist.update(cx, |assist, cx| { assist.composer.update(cx, |input, cx| input.set_value("", window, cx)); assist.prefill_action(super::Action::Hints, window, cx); });
            window.render_frame(cx);
            let (group, card) = (window.find("chat-composer").bounds(), window.find("composer-action").bounds());
            ensure!(card.left() >= group.left() && card.right() < group.right() && card.top() >= group.top() && card.bottom() < group.bottom(), "Action card escaped the prompt box");
            ensure!(assist.read(cx).runs.len() == runs && assist.read(cx).composer.focus_handle(cx).is_focused(window), "Quick action sent instead of prefilling the prompt");
            ensure!(!assist.read(cx).composer_empty(cx), "A prompt with only an action card counts as empty");
            window.click("remove-composer-action", cx);
            ensure!(assist.read(cx).composer_action.is_none() && assist.read(cx).composer_empty(cx), "Action card could not be removed");
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
            assist.update(cx, |assist, _| assist.ai_tab = 1);
            window.render_frame(cx);
            window.click("chat-send", cx);
            ensure!(assist.read(cx).ai_tab == 1, "Disabled composer sent a chat");
            fixture.update(cx, |view, cx| { view.threads = true; cx.notify(); });
            assist.update(cx, |assist, cx| {
                assist.slug = Some("composer-fixture".into());
                if assist.db.visible_chat_threads(assist.slug.as_deref(), "").unwrap().is_empty() {
                    for title in ["New chat", "Previous conversation", "Maps", "I'm stuck"] {
                        let thread = assist.db.create_chat(assist.slug.as_deref(), title).unwrap();
                        let stuck = title == "I'm stuck";
                        if title != "New chat" {
                            assist.db.append_chat(thread.id, &practice::chat::history::SavedTurn { citations: vec![],
                                model: "fixture".into(), agent: None, answer: stuck.then(stuck_answer), request_prompt: String::new(), instructions: if stuck { String::new() } else { title.into() },
                                action: Some(if stuck { practice::assist::Action::Stuck } else { practice::assist::Action::Ask }), phase: practice::chat::history::SavedPhase::Done,
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
            ensure!(!assist.read(cx).thread_selection_visible, "Opening history highlighted a row before navigation");
            let header = window.find("chat-header").bounds();
            let container = window.find("chat-history-container").bounds();
            let search = window.find("chat-thread-search").bounds();
            ensure!(container.top() >= header.bottom() && container.right() <= window.viewport_size().width, "History dropdown covers the header or escapes the panel");
            ensure!(container.left() < search.left() && container.right() > search.right() && container.top() < search.top() && container.bottom() > search.bottom(), "History search escaped its dropdown");
            ensure!(assist.read(cx).threads.len() == 3 && assist.read(cx).threads.iter().all(|thread| thread.title != "New chat"), "Empty New chat leaked into the list");
            Ok(())
        })??;
        cx.run_until_parked();
        if pixels { cx.capture_screenshot(handle)?.save(output.join(format!("threads-{name}.png")))?; }
        cx.update_window(handle, |_, window, cx| -> Result<()> {
            window.render_frame(cx);
            let assist = fixture.read(cx).assist.clone();
            // Asking about a noted line continues the unopened chat that wrote the note.
            let stuck = assist.read(cx).threads.iter().find(|thread| thread.title == "I'm stuck").map(|thread| thread.id);
            let notes: Vec<String> = practice::notes::specs(&stuck_answer()).iter().map(practice::notes::body).collect();
            assist.update(cx, |assist, cx| {
                assist.composer.update(cx, |input, cx| input.set_value("", window, cx));
                let thread = assist.note_thread(&notes);
                assist.ask_in_chat(thread, "About `lo = mid`: ".into(), window, cx);
            });
            ensure!(stuck.is_some() && assist.read(cx).selected_thread == stuck, "Asking about a note did not open the chat that wrote it");
            ensure!(assist.read(cx).composer.read(cx).value().as_ref() == "About `lo = mid`: " && assist.read(cx).composer.focus_handle(cx).is_focused(window), "Asking about a note did not prepare the prompt");
            ensure!(assist.read(cx).ai_tab == 2 && !assist.read(cx).show_threads, "Asking about a note did not show the chat");
            assist.update(cx, |assist, cx| { assist.composer.update(cx, |input, cx| input.set_value("", window, cx)); assist.open_thread_list(window, cx); });
            window.render_frame(cx);
            let selected = assist.read(cx).threads[0].id;
            window.press("up", cx);
            ensure!(assist.read(cx).thread_selection_visible && assist.read(cx).thread_selection == 0, "Keyboard navigation lost its visible selection");
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
                ensure!(assist.read(cx).threads.len() == 3, "{shortcut} added an empty list option");
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
        cx.update_window(handle, |_, window, _| window.remove_window())?;
        let handle = cx.update(|cx| gpui_kit::open_window(WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds { origin: point(px(0.), px(0.)), size: size(px(width.max(320.)), px(640.)) })),
            show: false, focus: false, ..Default::default()
        }, cx, |_, _| fixture.clone()))?.0.into();
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| -> Result<()> {
            let assist = fixture.read(cx).assist.clone();
            assist.update(cx, |assist, cx| {
                assist.set_problem(None, cx);
                assist.ai_tab = 2;
                cx.notify();
            });
            fixture.update(cx, |view, cx| { view.full_view = true; cx.notify(); });
            window.render_frame(cx);
            ensure!(window.try_find("ai-tab-0").is_none(), "General tab still exists");
            let new_chat = window.find("chat-new-thread").bounds();
            let close = window.find("chat-hide").bounds();
            ensure!(new_chat.right() <= close.left() && new_chat.center().y == close.center().y, "Plus is not immediately before Close in the header");
            ensure!(window.try_find("chat-thread-picker").is_none(), "Empty chat offered a history picker");
            let header = window.find("chat-header").bounds();
            ensure!(header.size.height <= px(40.) && window.find("ai-tab-1").bounds().center().y == close.center().y, "Header is not a single slim row");
            ensure!(window.find("ai-action-grid").visible(), "Empty Chats lost the quick actions");
            window.click("ai-tab-1", cx);
            window.render_frame(cx);
            ensure!(window.find("ai-review").visible() && window.try_find("ai-action-grid").is_none(), "Analysis did not stay separate");
            window.click("ai-tab-1", cx);
            window.render_frame(cx);
            ensure!(window.find("ai-action-grid").visible() && !assist.read(cx).show_threads, "Chats did not return to its empty state");
            Ok(())
        })??;
        cx.run_until_parked();
        if pixels { cx.capture_screenshot(handle)?.save(output.join(format!("chats-merged-{name}.png")))?; }
        cx.update_window(handle, |_, window, cx| -> Result<()> {
            let assist = fixture.read(cx).assist.clone();
            let thread = db.visible_chat_threads(Some("composer-fixture"), "Maps")?.remove(0);
            let message = db.chat_messages(thread.id)?.remove(0);
            assist.update(cx, |assist, cx| {
                assist.restore_message(message);
                assist.selected_thread = Some(thread.id);
                cx.notify();
            });
            window.render_frame(cx);
            ensure!(window.try_find("ai-action-grid").is_none(), "Quick actions crowd the existing conversation");
            ensure!(window.find("assist-cards").visible(), "Chats lost its conversation");
            let run = assist.read(cx).runs.iter().find(|run| run.thread_id == thread.id).map(|run| run.id).ok_or_else(|| anyhow::anyhow!("Restored turn is missing"))?;
            let (list, bubble) = (window.find("assist-cards").bounds(), window.find(("chat-request", run)).bounds());
            ensure!(list.right() - bubble.right() < px(24.) && bubble.left() > list.left() + px(48.), "Request is not a right-aligned bubble");
            Ok(())
        })??;
        cx.run_until_parked();
        if pixels { cx.capture_screenshot(handle)?.save(output.join(format!("chats-populated-{name}.png")))?; }
        cx.update_window(handle, |_, window, _cx| -> Result<()> {
            window.remove_window();
            Ok(())
        })??;
        cx.run_until_parked();
    }
    cx.update(|cx| { crate::theme::apply("Vesper", cx); crate::theme::set_zoom(1., cx); });
    launch_inside_workspace(cx, output)?;
    super::selection::fixture(cx, output, pixels)?;
    tooltip::native(cx, output, pixels)?;
    management::native(cx, output, pixels)?;
    Ok(serde_json::json!({"fixture":"chat-composer","passed":true,"pixels":pixels,"checks":["archive icon, General settings restore and confirmed permanent deletion","Enter sends","quick actions prefill a removable action card","action card and text send as one message","Shift+Enter adds a line","Ctrl+Enter removed","empty and disabled guards","one-line prompt expands on focus","send button input","send and stop containment","compact/light/zoom","wrapped tooltip width, viewport containment, shortcuts and dismissal","neutral history rows and visible keyboard selection","empty chats hidden","Ctrl+T / Ctrl+Shift+O / Ctrl+N","history dropdown below the slim header","header Plus and no empty history row","merged Chats quick actions and Analysis toggle","quick-action tags and note follow-ups","first message names hidden chats","right-aligned request bubble","Alt+Q adds citation pills without sending","automatic selection menu Quote preserves the draft","Alt+E and Alt+F send selection prompts and preserve drafts","selection actions preserve editor content","citation source previews","normal replacement typing dismisses selection menu","workspace Alt+L accepts Copilot without quoting","annotation dropdown choices","long code wraps without source changes"]}))
}
