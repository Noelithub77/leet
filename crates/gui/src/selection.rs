//! AI commands scoped to highlighted text in the editor or rendered text.
use gpui_kit::*;
use gpui_kit::base::TextSelection;
use practice::assist::Action;
use crate::workspace::Workspace;

impl Workspace {
    pub(crate) fn selection_ai(&mut self, command: Option<&str>, window: &mut Window, cx: &mut Context<Self>) {
        let mut text = TextSelection::selected_text(window, cx);
        if text.trim().is_empty() && self.editor.read(cx).focus_handle(cx).is_focused(window) {
            let editor = self.editor.read(cx);
            text = editor.selected_text().to_string();
        }
        self.selection_ai_text(command, text, window, cx);
    }

    pub(crate) fn selection_ai_text(&mut self, command: Option<&str>, text: String, window: &mut Window, cx: &mut Context<Self>) {
        if text.trim().is_empty() { self.flash("Highlight some text first", cx); return; }
        let Some(snapshot) = self.assist_snapshot(cx) else { self.flash("Open a problem first", cx); return; };
        let quote = text.lines().map(|line| format!("> {line}")).collect::<Vec<_>>().join("\n");
        self.right = true; self.zen = false; self.save_layout();
        let target = self.assist.read(cx).target(&self.config);
        self.assist.update(cx, |assist, cx| {
            assist.set_problem(Some(&snapshot.slug), cx);
            assist.ai_tab = 2;
            if let Some(command) = command {
                assist.sync_composer(window, cx);
                let draft = assist.composer.read(cx).value().to_string();
                let editing = assist.editing.take();
                let instruction = if command == "Fix" {
                    "Fix the highlighted text below. Show the corrected snippet and briefly explain the change. Keep the fix scoped to this selection; do not edit files."
                } else { "Explain the highlighted text below in the context of this problem." };
                assist.composer.update(cx, |input, cx| input.set_value(format!("{instruction}\n\n{quote}"), window, cx));
                assist.start(Action::Ask, snapshot, target, window, cx);
                assist.editing = editing;
                assist.composer_thread = assist.selected_thread;
                assist.composer.update(cx, |input, cx| input.set_value(draft, window, cx));
                assist.persist_draft(cx);
            } else {
                assist.ask_in_chat(None, quote, window, cx);
                match assist.ensure_thread() {
                    Ok(_) => {
                        assist.composer_thread = assist.selected_thread;
                        assist.persist_draft(cx); assist.refresh_threads();
                    }
                    Err(error) => assist.conversation_error = Some(error.to_string()),
                }
            }
            cx.notify();
        });
        cx.notify();
    }
}

#[cfg(feature = "gui-test")]
pub(crate) fn fixture(cx: &mut HeadlessAppContext, output: &std::path::Path, pixels: bool) -> anyhow::Result<()> {
    use std::sync::Arc;
    use anyhow::{ensure, Context as _};
    use gpui_kit::test::TestWindowExt as _;
    use practice::{config::Config, db::Db, language::{Language, Source}};
    use crate::workspace::{Center, Session};
    let db = Arc::new(Db::open(&output.join("session/selection.sqlite"))?);
    let (handle, workspace) = cx.update(|cx| {
        crate::actions::bind_keys(&Config::default(), cx);
        gpui_kit::open_window(WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds { origin: point(px(0.), px(0.)), size: size(px(1100.), px(700.)) })),
            show: false, focus: false, ..Default::default()
        }, cx, |window, cx| cx.new(|cx| {
            let config = Config { onboarding_completed: true, workspace: output.join("session/selection-solutions"), ..Default::default() };
            let mut ws = Workspace::from_storage(config, db.clone(), [None, None], window, cx);
            let question = db.question("two-sum").expect("fixture database").expect("bundled problem");
            ws.session = Some(Session {
                slug: question.slug.clone(), title: question.title.clone(), frontend_id: 1, question: Some(question),
                language: Language::Python, source: Source::NeetCode, rel: "selection.py".into(), path: output.join("session/selection-solutions/selection.py"),
                disk_mtime: None, cases: vec![], results: vec![], compile_error: None, selected_case: 0,
                running: false, run_id: 0, judge: None, hints_shown: 0, history: vec![], selected_commit: 0,
                remote_versions: vec![], history_status: None, history_loading: false, history_loaded: false,
                version_sequence: 0, history_sequence: 0,
            });
            ws.center = Center::Editor; ws.left = false; ws.description = false;
            ws.editor_pane.update(cx, |pane, cx| pane.load_file("answer = 42\n".into(), window, cx));
            ws.assist.update(cx, |assist, _| {
                assist.fixture_no_transport = true;
                assist.agents = vec![practice::agents::Detected { kind: practice::agents::AgentKind::OpenCode, path: "/fixture/no-agent".into(), version: None }];
            });
            ws
        }))
    })?;
    let handle = handle.into(); cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| -> anyhow::Result<()> {
        window.activate_window(); window.render_frame(cx);
        workspace.read(cx).editor.clone().update(cx, |editor, cx| editor.focus(window, cx));
        window.press("ctrl-a", cx); window.press("alt-l", cx);
        let assist = workspace.read(cx).assist.read(cx);
        ensure!(assist.composer.read(cx).value().contains("> answer = 42"), "Alt+L did not quote selected code");
        ensure!(assist.runs.is_empty(), "Quote sent a request");
        Ok(())
    })??;
    cx.run_until_parked(); cx.advance_clock(std::time::Duration::from_millis(500)); cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| -> anyhow::Result<()> {
        let ws = workspace.read(cx); let editor = ws.editor.clone(); let assist = ws.assist.clone();
        assist.update(cx, |assist, cx| assist.composer.update(cx, |input, cx| input.set_value("Menu draft", window, cx)));
        editor.update(cx, |editor, cx| editor.focus(window, cx));
        window.render_frame(cx); window.press("ctrl-a", cx); window.render_frame(cx);
        let bounds = editor.read(cx).range_to_bounds(&(0..11)).context("Selected code has no bounds")?;
        window.dispatch_event(MouseDownEvent { position: bounds.center(), button: MouseButton::Right, modifiers: Default::default(), click_count: 1, first_mouse: false }.to_platform_input(), cx);
        window.render_frame(cx);
        window.dispatch_event(MouseUpEvent { position: bounds.center(), button: MouseButton::Right, modifiers: Default::default(), click_count: 1 }.to_platform_input(), cx);
        window.render_frame(cx);
        Ok(())
    })??;
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| -> anyhow::Result<()> {
        window.render_frame(cx);
        ensure!(window.find("popup-menu").visible(), "Selection context menu did not open");
        Ok(())
    })??;
    if pixels { cx.capture_screenshot(handle)?.save(output.join("selection-menu.png"))?; }
    cx.update_window(handle, |_, window, cx| {
        for _ in 0..3 { window.press("up", cx); }
        window.press("enter", cx);
    })?;
    cx.run_until_parked();
    cx.update_window(handle, |_, _, cx| -> anyhow::Result<()> {
        let assist = workspace.read(cx).assist.read(cx);
        ensure!(assist.composer.read(cx).value().as_str() == "Menu draft\n\n> answer = 42", "Menu Quote lost its selection or replaced the draft: {:?}", assist.composer.read(cx).value());
        ensure!(assist.runs.is_empty(), "Menu Quote sent a request");
        Ok(())
    })??;
    for (key, instruction) in [("alt-e", "Explain the highlighted text"), ("alt-f", "Fix the highlighted text")] {
        cx.update_window(handle, |_, window, cx| -> anyhow::Result<()> {
            let ws = workspace.read(cx);
            let editor = ws.editor.clone(); let assist = ws.assist.clone();
            assist.update(cx, |assist, cx| {
                assist.composer.update(cx, |input, cx| input.set_value("My unsent question", window, cx));
            });
            editor.update(cx, |editor, cx| editor.focus(window, cx));
            window.render_frame(cx); window.press("ctrl-a", cx); window.press(key, cx);
            let assist = assist.read(cx);
            let run = assist.runs.first().context("Selection action did not send")?;
            ensure!(run.instructions.starts_with(instruction) && run.instructions.contains("> answer = 42"), "Wrong selection prompt: {}", run.instructions);
            ensure!(!run.instructions.contains("My unsent question"), "Selection action sent the composer draft");
            ensure!(assist.composer.read(cx).value().as_str() == "My unsent question", "Selection action lost the composer draft");
            ensure!(workspace.read(cx).editor.read(cx).value().as_str() == "answer = 42\n", "Selection action edited code");
            Ok(())
        })??;
        cx.run_until_parked();
    }
    let answer_id = cx.update_window(handle, |_, _, cx| {
        let assist = workspace.read(cx).assist.clone();
        assist.update(cx, |assist, cx| {
            let run = &mut assist.runs[0]; run.phase = super::Phase::Done;
            run.set_answer(Some(practice::assist::Answer::Chat("Answer selection fixture".into())));
            let id = run.id; assist.scroll.scroll_to_bottom(); cx.notify(); id
        })
    })?;
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))?;
    if pixels { cx.capture_screenshot(handle)?.save(output.join("selection-rendered-answer.png"))?; }
    cx.update_window(handle, |_, window, cx| -> anyhow::Result<()> {
        let assist = workspace.read(cx).assist.clone();
        let bounds = window.find(("chat-answer", answer_id)).bounds();
        window.drag(bounds.origin + point(px(1.), px(8.)), point(bounds.right() - px(1.), bounds.top() + px(8.)), cx);
        let selected = TextSelection::selected_text(window, cx);
        ensure!(selected.contains("selection fixture"), "Rendered answer did not select text: {selected:?}");
        let count = assist.read(cx).runs.len();
        window.press("alt-l", cx);
        ensure!(assist.read(cx).composer.read(cx).value().contains("> Answer selection fixture"), "Alt+L did not quote rendered text: {:?}, selected: {selected:?}", assist.read(cx).composer.read(cx).value());
        ensure!(assist.read(cx).runs.len() == count, "Quoting rendered text sent a request");
        Ok(())
    })??;
    if pixels { cx.capture_screenshot(handle)?.save(output.join("selection-ai.png"))?; }
    cx.update_window(handle, |_, window, _| window.remove_window())?;
    cx.run_until_parked(); Ok(())
}
