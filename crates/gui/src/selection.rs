//! Selection commands, citation previews, and the automatic selection menu.
use gpui_kit::*;
use gpui_kit::base::TextSelection;
use gpui_kit::component::menu::{PopupMenu, PopupMenuItem};
use gpui_kit::component::{ActiveTheme as _, h_flex, Icon};
use practice::assist::Action;
use practice::chat::history::Citation;
use crate::workspace::Workspace;

pub(crate) struct SelectionMenu {
    menu: Entity<PopupMenu>,
    position: Point<Pixels>,
    citation: Citation,
    return_focus: Option<FocusHandle>,
    _dismiss: Subscription,
}

pub(super) fn citation_key(citation: &Citation) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hash = std::hash::DefaultHasher::new();
    citation.label.hash(&mut hash); citation.text.trim_end().hash(&mut hash); hash.finish()
}

pub(super) fn citation_pill(id: impl Into<ElementId>, citation: &Citation, theme: &gpui_kit::component::Theme) -> impl IntoElement {
    let preview = citation.preview.clone();
    h_flex().id(id).test_support().gap_1().items_center().px_2().py_0p5().rounded_full().max_w(rems(24.))
        .border_1().border_color(theme.border).bg(theme.secondary.opacity(0.45)).text_color(theme.foreground).text_xs().child(Icon::new(gpui_kit::assets::IconName::MessageCircle).size_3())
        .child(div().min_w_0().truncate().child(citation.label.clone()))
        .tooltip(move |window, cx| {
            let preview = preview.clone();
            gpui_kit::component::tooltip::Tooltip::element(move |_, cx| {
                div().id("citation-preview").test_support().max_w(rems(36.)).max_h(rems(24.)).overflow_y_scroll()
                    .text_sm().text_color(cx.theme().foreground).children(preview.lines().map(|line|
                        div().font_family(cx.theme().mono_font_family.clone()).child(line.to_owned())))
            }).build(window, cx)
        })
}

pub(crate) fn render_menu(ws: &Workspace) -> Option<AnyElement> {
    let selection = ws.selection_menu.as_ref()?;
    Some(deferred(anchored().position(selection.position).child(div().id("selection-ai-menu").test_support().child(selection.menu.clone()))).into_any_element())
}

impl Workspace {
    fn selected_citation(&self, window: &mut Window, cx: &mut App) -> Option<Citation> {
        let text = TextSelection::selected_text(window, cx);
        if !text.trim().is_empty() {
            let label = text.lines().find(|line| !line.trim().is_empty()).unwrap_or("Quote").trim().chars().take(60).collect();
            return Some(Citation { label, preview: text.clone(), text });
        }
        let editor = self.editor.read(cx);
        if !editor.focus_handle(cx).is_focused(window) || editor.selected_range().is_empty() { return None; }
        let range = editor.selected_range(); let source = editor.value().to_string();
        let start = source[..range.start].matches('\n').count();
        let prefix = &source[..range.end];
        let end = prefix.strip_suffix('\n').unwrap_or(prefix).matches('\n').count();
        let file = self.session.as_ref()?.path.file_name()?.to_string_lossy();
        let label = if start == end { format!("{file}:{}", start + 1) } else { format!("{file}:{}-{}", start + 1, end + 1) };
        let preview = source.lines().enumerate().skip(start).take(end - start + 1).map(|(line, text)| format!("{}  {text}", line + 1)).collect::<Vec<_>>().join("\n");
        Some(Citation { label, text: editor.selected_text().to_string(), preview })
    }

    pub(crate) fn open_selection_menu(&mut self, position: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        if self.session.is_none() || self.center != crate::workspace::Center::Editor || self.tour.is_some() || self.omni.open || self.debug_mode || self.composer_focused(window, cx) { self.selection_menu = None; cx.notify(); return; }
        let Some(citation) = self.selected_citation(window, cx) else { self.selection_menu = None; cx.notify(); return; };
        if self.selection_menu.as_ref().is_some_and(|selection| selection.citation == citation) { return; }
        let workspace = cx.weak_entity();
        let quoted = citation.clone();
        let return_focus = window.focused(cx);
        let action_context = return_focus.clone();
        let menu = PopupMenu::build(window, cx, move |menu, _, _| {
            let menu = if let Some(focus) = action_context.clone() { menu.action_context(focus) } else { menu };
            [None, Some("Explain"), Some("Fix")].into_iter().fold(menu, |menu, command| {
                let action: Box<dyn gpui_kit::Action> = match command {
                    Some("Explain") => Box::new(crate::actions::ExplainSelection),
                    Some("Fix") => Box::new(crate::actions::FixSelection),
                    _ => Box::new(crate::actions::QuoteSelection),
                };
                let workspace = workspace.clone(); let citation = quoted.clone();
                menu.item(PopupMenuItem::new(command.unwrap_or("Quote")).action(action).on_click(move |_, window, cx| {
                    let _ = workspace.update(cx, |ws, cx| ws.selection_ai_citation(command, citation.clone(), window, cx));
                }))
            })
        });
        let dismiss = cx.subscribe(&menu, |ws, _, _: &DismissEvent, cx| { ws.selection_menu = None; cx.notify(); });
        self.selection_menu = Some(SelectionMenu { menu, position: position + point(px(0.), px(8.)), citation, return_focus, _dismiss: dismiss });
        cx.notify();
    }

    pub(crate) fn dismiss_selection_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.selection_epoch += 1;
        if let Some(selection) = self.selection_menu.take() {
            if let Some(focus) = selection.return_focus { focus.focus(window, cx); }
            cx.notify();
        }
    }

    fn composer_focused(&self, window: &Window, cx: &App) -> bool { self.assist.read(cx).composer.focus_handle(cx).is_focused(window) }

    pub(crate) fn selection_ai(&mut self, command: Option<&str>, window: &mut Window, cx: &mut Context<Self>) {
        let citation = self.selected_citation(window, cx).or_else(|| self.selection_menu.as_ref().map(|selection| selection.citation.clone()));
        let Some(citation) = citation else {
            if command.is_none() { self.ask_in_chat(window, cx); } else { self.flash("Highlight some text first", cx); }
            return;
        };
        self.selection_ai_citation(command, citation, window, cx);
    }

    fn selection_ai_citation(&mut self, command: Option<&str>, citation: Citation, window: &mut Window, cx: &mut Context<Self>) {
        if citation.text.trim().is_empty() { return; }
        let Some(snapshot) = self.assist_snapshot(cx) else { self.flash("Open a problem first", cx); return; };
        self.selection_menu = None; self.selection_epoch += 1;
        self.right = true; self.zen = false; self.save_layout();
        let target = self.assist.read(cx).target(&self.config);
        self.assist.update(cx, |assist, cx| {
            assist.set_problem(Some(&snapshot.slug), cx);
            assist.sync_composer(window, cx); assist.ai_tab = 2; assist.show_threads = false;
            if let Some(command) = command {
                let draft = assist.composer.read(cx).value().to_string();
                let draft_citations = std::mem::replace(&mut assist.composer_citations, vec![citation]);
                let editing = assist.editing.take();
                let instruction = if command == "Fix" { "Fix this selection and explain the change. Show the corrected snippet; do not edit files." } else { "Explain this selection in the context of this problem." };
                assist.composer.update(cx, |input, cx| input.set_value(instruction, window, cx));
                assist.start(Action::Ask, snapshot, target, window, cx);
                assist.editing = editing; assist.composer_thread = assist.selected_thread;
                assist.composer_citations = draft_citations;
                assist.composer.update(cx, |input, cx| input.set_value(draft, window, cx));
            } else {
                if assist.editing.is_some() { assist.cancel_edit(window, cx); }
                if !assist.composer_citations.iter().any(|existing| existing.label == citation.label && existing.text.trim_end() == citation.text.trim_end()) { assist.composer_citations.push(citation); }
                assist.composer.focus_handle(cx).focus(window, cx);
                if let Err(error) = assist.ensure_thread() { assist.conversation_error = Some(error.to_string()); }
                assist.composer_thread = assist.selected_thread;
            }
            assist.persist_draft(cx); assist.refresh_threads(); cx.notify();
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
        window.press("ctrl-a", cx); window.press("alt-q", cx);
        let assist = workspace.read(cx).assist.read(cx);
        ensure!(assist.composer_citations.first().is_some_and(|citation| citation.text == "answer = 42\n" && citation.preview == "1  answer = 42"), "Alt+Q did not cite selected code");
        ensure!(assist.composer.read(cx).value().is_empty(), "Quote inserted raw text instead of a citation pill");
        ensure!(assist.runs.is_empty(), "Quote sent a request");
        Ok(())
    })??;
    cx.run_until_parked(); cx.advance_clock(std::time::Duration::from_millis(500)); cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| -> anyhow::Result<()> {
        let ws = workspace.read(cx); let editor = ws.editor.clone(); let assist = ws.assist.clone();
        assist.update(cx, |assist, cx| assist.composer.update(cx, |input, cx| input.set_value("Menu draft", window, cx)));
        editor.update(cx, |editor, cx| editor.focus(window, cx));
        window.render_frame(cx); window.press("right", cx); window.render_frame(cx);
        let bounds = editor.read(cx).range_to_bounds(&(0..11)).context("Selected code has no bounds")?;
        window.drag(point(bounds.left(), bounds.center().y), point(bounds.right() + px(1.), bounds.center().y), cx);
        Ok(())
    })??;
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| -> anyhow::Result<()> {
        window.render_frame(cx);
        ensure!(window.find("selection-ai-menu").visible(), "Selection did not automatically open its menu");
        ensure!(workspace.read(cx).editor.read(cx).focus_handle(cx).is_focused(window), "Selection menu stole editor focus");
        Ok(())
    })??;
    if pixels { cx.capture_screenshot(handle)?.save(output.join("selection-menu.png"))?; }
    cx.update_window(handle, |_, window, cx| {
        window.within("selection-ai-menu").click(0usize, cx);
    })?;
    cx.run_until_parked();
    cx.update_window(handle, |_, _, cx| -> anyhow::Result<()> {
        let assist = workspace.read(cx).assist.read(cx);
        ensure!(assist.composer.read(cx).value().as_str() == "Menu draft" && assist.composer_citations.len() == 1, "Menu Quote lost its selection or replaced the draft: {:?}", assist.composer.read(cx).value());
        ensure!(assist.runs.is_empty(), "Menu Quote sent a request");
        Ok(())
    })??;
    cx.update_window(handle, |_, window, cx| {
        let key = citation_key(&workspace.read(cx).assist.read(cx).composer_citations[0]);
        window.hover(("draft-citation", key), cx);
    })?;
    cx.run_until_parked(); cx.advance_clock(std::time::Duration::from_millis(800)); cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| -> anyhow::Result<()> {
        window.render_frame(cx); ensure!(window.find("citation-preview").visible(), "Citation hover did not show source lines"); Ok(())
    })??;
    if pixels { cx.capture_screenshot(handle)?.save(output.join("citation-hover.png"))?; }
    for (key, instruction) in [("alt-e", "Explain this selection"), ("alt-f", "Fix this selection")] {
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
            ensure!(run.instructions.starts_with(instruction) && run.citations.first().is_some_and(|citation| citation.text == "answer = 42\n"), "Wrong selection prompt: {}", run.instructions);
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
        window.press("alt-q", cx);
        ensure!(assist.read(cx).composer_citations.last().is_some_and(|citation| citation.text.contains("Answer selection fixture")), "Alt+Q did not cite rendered text");
        ensure!(assist.read(cx).runs.len() == count, "Quoting rendered text sent a request");
        Ok(())
    })??;
    if pixels { cx.capture_screenshot(handle)?.save(output.join("selection-ai.png"))?; }
    cx.update_window(handle, |_, window, cx| {
        workspace.read(cx).editor.clone().update(cx, |editor, cx| editor.focus(window, cx));
        window.render_frame(cx); window.press("ctrl-a", cx);
    })?;
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| -> anyhow::Result<()> {
        window.render_frame(cx); ensure!(window.find("selection-ai-menu").visible(), "Keyboard selection menu is missing");
        window.input("replacement", cx);
        ensure!(workspace.read(cx).editor.read(cx).value().as_str() == "replacement", "Selection menu blocked replacing selected text");
        Ok(())
    })??;
    cx.run_until_parked();
    ensure!(cx.update(|cx| workspace.read(cx).selection_menu.is_none()), "Typing did not dismiss the selection menu");
    cx.update_window(handle, |_, window, cx| {
        let pane = workspace.read(cx).editor_pane.clone();
        pane.update(cx, |pane, cx| {
            let connection = cx.new(|_| crate::copilot::Connection::new(true));
            pane.connect_predictions(connection, window, cx);
            let item = serde_json::from_value(serde_json::json!({"insertText":"nt(1)\n"})).unwrap();
            *pane.predictions.as_ref().unwrap().fixture.borrow_mut() = practice::copilot::Suggestion::from_item("pri", 3, item);
        });
        window.press("ctrl-a", cx); window.input("pri", cx);
    })?;
    cx.run_until_parked(); cx.advance_clock(std::time::Duration::from_millis(300)); cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.dispatch_event(PlatformInput::ModifiersChanged(ModifiersChangedEvent { modifiers: Modifiers { alt: true, ..Default::default() }, capslock: Default::default() }), cx);
    })?;
    cx.run_until_parked(); cx.advance_clock(std::time::Duration::from_millis(300)); cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| -> anyhow::Result<()> {
        let count = workspace.read(cx).assist.read(cx).composer_citations.len();
        window.render_frame(cx); window.press("alt-l", cx);
        ensure!(workspace.read(cx).editor.read(cx).value().as_str() == "print(1)\n", "Workspace Alt+L did not accept Copilot");
        ensure!(workspace.read(cx).assist.read(cx).composer_citations.len() == count, "Alt+L also quoted text");
        Ok(())
    })??;
    cx.update_window(handle, |_, window, _| window.remove_window())?;
    cx.run_until_parked(); Ok(())
}
