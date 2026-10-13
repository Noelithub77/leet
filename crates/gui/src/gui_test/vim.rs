//! Vim input through the production source pane, isolated from user storage.
use super::*;
use crate::workspace::{Center, Session, Workspace};
use gpui_kit::base::input::{VimMode, VimShared};
use gpui_kit::component::input::{EditorState, TabSize};
use gpui_kit::{ClipboardItem, Window};
use practice::{config::Config, db::Db, language::Source};
use std::{cell::RefCell, rc::Rc};

fn settle(cx: &HeadlessAppContext) {
    cx.run_until_parked();
    cx.advance_clock(Duration::from_millis(200));
    cx.run_until_parked();
}

fn keys(window: &mut Window, keys: &[&str], cx: &mut App) {
    for key in keys {
        window.press(key, cx);
        window.render_frame(cx);
    }
}

fn mode(editor: &Entity<EditorState>, expected: VimMode, cx: &App) -> Result<()> {
    ensure!(
        editor.read(cx).vim_mode() == Some(expected),
        "Expected {expected:?}, got {:?}",
        editor.read(cx).vim_mode()
    );
    Ok(())
}

fn text(editor: &Entity<EditorState>, expected: &str, cx: &App) -> Result<()> {
    ensure!(
        editor.read(cx).value().as_str() == expected,
        "Unexpected Vim document: expected {expected:?}, got {:?}",
        editor.read(cx).value().as_str()
    );
    Ok(())
}

fn reset(editor: &Entity<EditorState>, source: &str, window: &mut Window, cx: &mut App) {
    // Fixture setup only. All operations under assertion are dispatched input.
    editor.update(cx, |editor, cx| {
        editor.set_vim_enabled(false, cx);
        editor.set_readonly(false, cx);
        editor.set_value(source, window, cx);
        editor.set_selected_range(0..0, cx);
        editor.set_vim_enabled(true, cx);
        editor.focus(window, cx);
    });
    window.render_frame(cx);
}

pub(super) fn native(
    cx: &mut HeadlessAppContext,
    output: &Path,
    pixels: bool,
) -> Result<serde_json::Value> {
    cx.update(crate::snippets::expansion::bind_keys);
    let shared = Rc::new(RefCell::new(VimShared::default()));
    let (handle, pane) = cx.update(|cx| {
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: point(px(0.), px(0.)),
                    size: size(px(1280.), px(800.)),
                })),
                show: false,
                focus: false,
                ..Default::default()
            },
            cx,
            |window, cx| {
                cx.new(|cx| {
                    let state = cx.new(|cx| {
                        EditorState::new(window, cx)
                            .language("python")
                            .line_number(true)
                            .tab_size(TabSize {
                                tab_size: 4,
                                hard_tabs: false,
                            })
                    });
                    let snippet = practice::snippets::Snippet {
                        name: "Vim fixture loop".into(),
                        prefixes: vec!["loop".into()],
                        body: "for ${1:item} in ${2:items}:\n    print($1)$0".into(),
                        description: "Fixture-only linked placeholders".into(),
                        scope: Some(Language::Python),
                        template: false,
                        origin: practice::snippets::Origin::User,
                    };
                    let pane = crate::snippets::expansion::EditorPane::new(
                        state.clone(),
                        Language::Python,
                        Rc::new(practice::snippets::search::Library::new(vec![snippet])),
                        Default::default(),
                        window,
                        cx,
                    );
                    state.update(cx, |editor, cx| {
                        editor.set_vim_shared(shared.clone());
                        editor.set_vim_enabled(true, cx);
                        editor.focus(window, cx);
                    });
                    pane
                })
            },
        )
    })?;
    let handle = handle.into();
    let editor = cx.update(|cx| pane.read(cx).state.clone());
    settle(cx);

    cx.update_window(handle, |_, window, cx| {
        reset(&editor, "", window, cx);
        window.press("i", cx);
        window.input("lo", cx);
    })?;
    settle(cx);
    cx.update_window(handle, |_, window, cx| -> Result<()> {
        window.render_frame(cx);
        ensure!(
            window.find("source-completions").visible(),
            "Insert assistance did not show completion"
        );
        window.press("escape", cx);
        mode(&editor, VimMode::Normal, cx)?;
        window.render_frame(cx);
        ensure!(
            window
                .try_find("source-completions")
                .is_none_or(|menu| !menu.visible()),
            "One Escape did not dismiss completion"
        );
        reset(&editor, "", window, cx);
        window.press("i", cx);
        window.input("loop", cx);
        Ok(())
    })??;
    settle(cx);
    cx.update_window(handle, |_, window, cx| -> Result<()> {
        window.render_frame(cx);
        window.press("tab", cx);
        ensure!(
            editor.read(cx).selected_value().as_str() == "item",
            "Vim Insert Tab did not expand snippets"
        );
        window.input("node", cx);
        Ok(())
    })??;
    settle(cx);
    cx.update_window(handle, |_, window, cx| -> Result<()> {
        text(&editor, "for node in items:\n    print(node)", cx)?;
        window.press("escape", cx);
        mode(&editor, VimMode::Normal, cx)?;
        window.press("tab", cx);
        ensure!(
            editor.read(cx).selected_value().as_str() != "items",
            "Escape left snippet navigation active"
        );
        Ok(())
    })??;
    cx.update_window(handle, |_, window, cx| -> Result<()> {
        window.activate_window();
        reset(&editor, "alpha beta\ngamma delta\n", window, cx);
        mode(&editor, VimMode::Normal, cx)?;
        // Native text/IME delivery must not bypass Normal mode interception.
        editor.update(cx, |editor, cx| {
            gpui_kit::EntityInputHandler::replace_text_in_range(
                editor,
                None,
                "accidental text",
                window,
                cx,
            );
            gpui_kit::EntityInputHandler::replace_and_mark_text_in_range(
                editor,
                None,
                "IME preedit",
                None,
                window,
                cx,
            );
        });
        text(&editor, "alpha beta\ngamma delta\n", cx)?;
        keys(window, &["tab", "enter", "backspace", "delete"], cx);
        text(&editor, "alpha beta\ngamma delta\n", cx)?;
        mode(&editor, VimMode::Normal, cx)?;
        window.press("ctrl-v", cx);
        mode(&editor, VimMode::VisualBlock, cx)?;
        window.press("ctrl-v", cx);
        mode(&editor, VimMode::Normal, cx)?;
        ensure!(
            editor.read(cx).cursor() == 0,
            "Visual Block exit moved past its inclusive endpoint"
        );
        keys(window, &["v", "l", "ctrl-v"], cx);
        mode(&editor, VimMode::VisualBlock, cx)?;
        keys(window, &["escape", "shift-v", "ctrl-v"], cx);
        mode(&editor, VimMode::VisualBlock, cx)?;
        window.press("escape", cx);
        ensure!(
            editor.read(cx).cursor() == 1,
            "Visual mode switching lost the endpoint"
        );
        keys(window, &["0", "i"], cx);
        mode(&editor, VimMode::Insert, cx)?;
        cx.write_to_clipboard(ClipboardItem::new_string("one\ntwo".into()));
        window.press("ctrl-v", cx);
        text(&editor, "one\ntwoalpha beta\ngamma delta\n", cx)?;
        mode(&editor, VimMode::Insert, cx)?;
        window.press("escape", cx);
        mode(&editor, VimMode::Normal, cx)?;
        window.press("u", cx);
        text(&editor, "alpha beta\ngamma delta\n", cx)?;
        window.press("ctrl-r", cx);
        text(&editor, "one\ntwoalpha beta\ngamma delta\n", cx)?;
        Ok(())
    })??;
    settle(cx);

    cx.update_window(handle, |_, window, cx| -> Result<()> {
        reset(&editor, "left right", window, cx);
        window.press("i", cx);
        editor.update(cx, |editor, cx| editor.set_selected_range(0..4, cx));
        cx.write_to_clipboard(ClipboardItem::new_string("new\nvalue".into()));
        window.press("ctrl-v", cx);
        text(&editor, "new\nvalue right", cx)?;
        mode(&editor, VimMode::Insert, cx)?;
        window.press("escape", cx);
        window.press("u", cx);
        text(&editor, "left right", cx)?;
        window.press("ctrl-r", cx);
        text(&editor, "new\nvalue right", cx)?;
        reset(&editor, "unchanged", window, cx);
        window.press("i", cx);
        cx.write_to_clipboard(ClipboardItem::new_string(String::new()));
        window.press("ctrl-v", cx);
        text(&editor, "unchanged", cx)?;
        mode(&editor, VimMode::Insert, cx)?;
        window.press("ctrl-[", cx);
        mode(&editor, VimMode::Normal, cx)?;
        reset(&editor, "start", window, cx);
        keys(window, &["i"], cx);
        cx.write_to_clipboard(ClipboardItem::new_string("clip".into()));
        window.press("ctrl-v", cx);
        cx.write_to_clipboard(ClipboardItem::new_string("different clipboard".into()));
        keys(window, &["escape", "$", "."], cx);
        text(&editor, "clipstarclipt", cx)?;
        Ok(())
    })??;
    settle(cx);

    cx.update_window(handle, |_, window, cx| -> Result<()> {
        let source = (1..=120)
            .map(|row| format!("row_{row}\n"))
            .collect::<String>();
        reset(&editor, &source, window, cx);
        let (_, height) = editor
            .read(cx)
            .cursor_layout()
            .context("Missing Vim cursor layout")?;
        let rows = (editor.read(cx).input_bounds().size.height / height)
            .floor()
            .max(1.) as usize;
        window.press("ctrl-d", cx);
        let row = editor.read(cx).value().as_str()[..editor.read(cx).cursor()]
            .bytes()
            .filter(|byte| *byte == b'\n')
            .count();
        ensure!(
            row == (rows / 2).max(1),
            "Ctrl+D did not move half a viewport"
        );
        window.press("ctrl-u", cx);
        ensure!(
            editor.read(cx).cursor() == 0,
            "Ctrl+U did not return to the first row"
        );
        window.press("ctrl-f", cx);
        let row = editor.read(cx).value().as_str()[..editor.read(cx).cursor()]
            .bytes()
            .filter(|byte| *byte == b'\n')
            .count();
        ensure!(
            row == rows.saturating_sub(2).max(1),
            "Ctrl+F did not move a viewport"
        );
        window.press("ctrl-b", cx);
        ensure!(
            editor.read(cx).cursor() == 0,
            "Ctrl+B did not return to the first row"
        );
        keys(window, &[":", "6", "0", "enter"], cx);
        let cursor = editor.read(cx).cursor();
        for (key, fraction) in [("t", 0.), ("z", 0.5), ("b", 1.)] {
            keys(window, &["z", key], cx);
            ensure!(
                editor.read(cx).cursor() == cursor,
                "z{key} moved the logical cursor"
            );
            let (caret, height) = editor
                .read(cx)
                .cursor_window_layout()
                .context("Missing scrolled cursor")?;
            let bounds = editor.read(cx).input_bounds();
            let expected = bounds.top() + (bounds.size.height - height) * fraction;
            ensure!(
                (caret.top() - expected).abs() < px(2.),
                "z{key} placed the cursor at {:?}, expected {:?}",
                caret.top(),
                expected
            );
        }
        keys(
            window,
            &[
                ":", "n", "o", "t", "-", "s", "u", "p", "p", "o", "r", "t", "e", "d", "enter",
            ],
            cx,
        );
        ensure!(
            editor
                .read(cx)
                .vim_message()
                .is_some_and(|message| message.contains("Unsupported Vim command")),
            "Unsupported ex command was not visible"
        );
        Ok(())
    })??;
    settle(cx);

    let cases: &[(&str, &str, &[&str], &str)] = &[
        (
            "operator-count",
            "one two three four five six seven",
            &["2", "d", "3", "w"],
            "seven",
        ),
        (
            "inner-word",
            "one two three",
            &["w", "d", "i", "w"],
            "one  three",
        ),
        (
            "around-word",
            "one two three",
            &["w", "d", "a", "w"],
            "one three",
        ),
        (
            "inner-parens",
            "a(foo bar)b",
            &["f", "f", "d", "i", "("],
            "a()b",
        ),
        ("visual-forward", "abcdef", &["v", "2", "l", "d"], "def"),
        (
            "visual-reverse",
            "abcdef",
            &["3", "l", "v", "2", "h", "d"],
            "aef",
        ),
        (
            "visual-endpoint",
            "abcdef",
            &["v", "2", "l", "o", "l", "d"],
            "adef",
        ),
        (
            "visual-line",
            "one\ntwo\nthree\n",
            &["shift-v", "j", "d"],
            "three\n",
        ),
        (
            "line-delete-final",
            "one\ntwo",
            &["shift-g", "d", "d"],
            "one",
        ),
        ("join", "one\n  two\n", &["shift-j"], "one two\n"),
        (
            "block-delete",
            "abcd\nefgh\nijkl\n",
            &["l", "ctrl-v", "j", "l", "d"],
            "ad\neh\nijkl\n",
        ),
        (
            "block-replace",
            "abcd\nefgh\nijkl\n",
            &["l", "ctrl-v", "j", "l", "r", "x"],
            "axxd\nexxh\nijkl\n",
        ),
        (
            "block-insert",
            "abc\ndef\nghi",
            &["ctrl-v", "j", "shift-i", "x", "escape"],
            "xabc\nxdef\nghi",
        ),
        (
            "block-append",
            "abc\ndef\nghi",
            &["ctrl-v", "j", "shift-a", "x", "escape"],
            "axbc\ndxef\nghi",
        ),
        (
            "block-change",
            "abc\ndef\nghi",
            &["ctrl-v", "j", "l", "c", "x", "escape"],
            "xc\nxf\nghi",
        ),
        (
            "block-short-empty",
            "abcd\nx\n\nwxyz",
            &["l", "ctrl-v", "3", "j", "l", "d"],
            "ad\nx\n\nwz",
        ),
        (
            "block-tab",
            "\tab\n1234ab",
            &["ctrl-v", "j", "3", "l", "d"],
            "\n",
        ),
        (
            "block-wide",
            "界ab\n1234",
            &["ctrl-v", "j", "l", "d"],
            "ab\n34",
        ),
        (
            "block-put",
            "abcd\nefgh\nijkl\nmnop",
            &["ctrl-v", "j", "l", "y", "g", "g", "2", "j", "0", "shift-p"],
            "abcd\nefgh\nabijkl\nefmnop",
        ),
        (
            "block-newline",
            "abc\ndef\nghi",
            &["ctrl-v", "j", "shift-i", "x", "enter", "y", "escape"],
            "x\nyabc\ndef\nghi",
        ),
        ("indent", "abc\ndef\nghi", &[">", ">"], "    abc\ndef\nghi"),
    ];
    for (name, source, commands, expected) in cases {
        cx.update_window(handle, |_, window, cx| -> Result<()> {
            reset(&editor, source, window, cx);
            keys(window, commands, cx);
            text(&editor, expected, cx).with_context(|| format!("Vim input case {name}"))?;
            mode(&editor, VimMode::Normal, cx)?;
            window.press("u", cx);
            text(&editor, source, cx).with_context(|| format!("One logical undo for {name}"))?;
            window.press("ctrl-r", cx);
            text(&editor, expected, cx).with_context(|| format!("Redo {name}"))?;
            Ok(())
        })??;
        settle(cx);
    }

    cx.update_window(handle, |_, window, cx| -> Result<()> {
        reset(&editor, "abc\ndef\nghi", window, cx);
        keys(
            window,
            &["ctrl-v", "j", "shift-i", "x", "escape", "shift-g", "0", "."],
            cx,
        );
        text(&editor, "xabc\nxdef\nxghi", cx)?;
        window.press("u", cx);
        text(&editor, "xabc\nxdef\nghi", cx)?;
        window.press("u", cx);
        text(&editor, "abc\ndef\nghi", cx)?;
        reset(&editor, "alpha beta alpha", window, cx);
        keys(window, &["/", "a", "l", "p", "h", "a", "enter"], cx);
        ensure!(
            editor.read(cx).cursor() == 11,
            "Forward literal search did not find next match"
        );
        window.press("n", cx);
        ensure!(editor.read(cx).cursor() == 0, "Search did not wrap");
        window.press("shift-n", cx);
        ensure!(editor.read(cx).cursor() == 11, "N did not reverse search");
        reset(&editor, "red blue redwood red", window, cx);
        window.press("*", cx);
        ensure!(
            editor.read(cx).cursor() == 17,
            "Whole-word search matched a substring"
        );
        reset(&editor, "abcdef", window, cx);
        keys(window, &["v", "2", "l", "escape", "g", "v"], cx);
        mode(&editor, VimMode::Visual, cx)?;
        ensure!(
            editor.read(cx).selected_value().as_str() == "abc",
            "gv lost the inclusive selection"
        );
        window.press("escape", cx);
        window.press("2", cx);
        ensure!(
            editor.read(cx).vim_pending().contains('2'),
            "Pending count is hidden"
        );
        window.blur(cx);
        Ok(())
    })??;
    settle(cx);
    cx.update_window(handle, |_, window, cx| -> Result<()> {
        editor.update(cx, |editor, cx| editor.focus(window, cx));
        ensure!(
            editor.read(cx).vim_pending().is_empty(),
            "Focus loss preserved an incomplete count"
        );
        Ok(())
    })??;

    for (name, commands) in [
        ("vim-normal.png", vec![]),
        ("vim-visual.png", vec!["v", "3", "l"]),
        ("vim-block.png", vec!["ctrl-v", "2", "j", "2", "l"]),
        ("vim-command.png", vec![":", "w"]),
    ] {
        cx.update_window(handle, |_, window, cx| {
            reset(
                &editor,
                "def solve():\n\t界 = 1\n    return 界\n",
                window,
                cx,
            );
            keys(window, &commands, cx);
            window.render_frame(cx);
        })?;
        settle(cx);
        if pixels {
            capture(cx, handle, &output.join(name))?;
        }
    }
    cx.update_window(handle, |_, window, _| window.remove_window())?;
    settle(cx);
    workspace_input(cx, output, pixels)?;
    Ok(json!({"fixture":"vim","passed":true,"pixels":pixels,
        "checks":["Normal native text guard", "Ctrl+V in five modes", "Insert multiline and empty clipboard", "Insert selection replacement", "paste undo and redo", "paste actual-text dot repeat", "native completion and snippet assistance", "one Escape dismisses completion and snippet navigation", "operator counts and text objects", "inclusive bidirectional visual and endpoint swapping", "visual line and gv", "rectangular delete replace I A c put and repeat", "short empty tab wide and final lines", "logical undo and redo", "literal wrap and whole-word search", "focus clears pending count", "read-only navigation yank search and mutation guard"],
        "input_cases":cases.iter().map(|(name, _, _, _)| name).collect::<Vec<_>>(),
        "workspace_checks":["Settings live toggle and persistence", "palette toggle", "new parked closed reopened solution editors", "shared register search repeat", "snippet source and read-only preview", "non-code input isolation", "shortcut precedence", "successful save and failed save retention"],
        "screenshots":["vim-normal.png","vim-visual.png","vim-block.png","vim-command.png","vim-settings.png","vim-workspace.png","vim-snippet.png"]}))
}

fn fixture_session(db: &Db, path: PathBuf, title: &str) -> Result<Session> {
    let question = db
        .question("two-sum")?
        .context("Missing bundled Vim question")?;
    Ok(Session {
        slug: format!("fixture-vim-{title}"),
        title: title.into(),
        frontend_id: 1,
        question: Some(question),
        language: Language::Python,
        source: Source::NeetCode,
        rel: format!("{title}.py").into(),
        path,
        disk_mtime: None,
        cases: vec![],
        results: vec![],
        compile_error: None,
        selected_case: 0,
        running: false,
        run_id: 0,
        judge: None,
        hints_shown: 0,
        history: vec![],
        selected_commit: 0,
        remote_versions: vec![],
        history_status: None,
        history_loading: false,
        history_loaded: false,
        version_sequence: 0,
        history_sequence: 0,
    })
}

fn workspace_input(cx: &mut HeadlessAppContext, output: &Path, pixels: bool) -> Result<()> {
    ensure!(
        Config::path().starts_with(output.join("session/config")),
        "Vim settings fixture requires the operator's isolated config directory"
    );
    let db = Arc::new(Db::open(&output.join("session/vim.sqlite"))?);
    let config = Config {
        onboarding_completed: true,
        copilot_enabled: false,
        workspace: output.join("session/vim-solutions"),
        ..Config::default()
    };
    ensure!(!config.vim_mode, "Vim must be disabled by default");
    std::fs::create_dir_all(&config.workspace)?;
    cx.update(|cx| {
        let bindings = cx.key_bindings().borrow().bindings().cloned().collect();
        cx.set_global(crate::actions::ComponentBindings(bindings));
        crate::actions::reload_keys(&config, cx);
    });
    let (handle, workspace) = cx.update(|cx| {
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: point(px(0.), px(0.)),
                    size: size(px(1280.), px(800.)),
                })),
                show: false,
                focus: false,
                ..Default::default()
            },
            cx,
            |window, cx| {
                cx.new(|cx| Workspace::from_storage(config, db.clone(), [None, None], window, cx))
            },
        )
    })?;
    let handle = handle.into();
    settle(cx);
    cx.update_window(handle, |_, window, cx| -> Result<()> {
        window.activate_window();
        ensure!(
            workspace.read(cx).editor.read(cx).vim_mode().is_none(),
            "Default solution uses Vim"
        );
        workspace.update(cx, |workspace, cx| {
            workspace.open_settings(Some(crate::settings::Setting::Vim), window, cx);
        });
        Ok(())
    })??;
    settle(cx);
    cx.update_window(handle, |_, window, cx| -> Result<()> {
        window.render_frame(cx);
        window.scroll(
            "settings-scroll",
            gpui_kit::ScrollDelta::Pixels(point(px(0.), px(-700.))),
            cx,
        );
        Ok(())
    })??;
    settle(cx);
    cx.update_window(handle, |_, window, cx| -> Result<()> {
        window.render_frame(cx);
        window.click(("setting", 6usize), cx);
        ensure!(
            workspace.read(cx).config.vim_mode,
            "Settings toggle did not enable Vim"
        );
        mode(&workspace.read(cx).editor, VimMode::Normal, cx)?;
        ensure!(
            Config::load()?.vim_mode,
            "Settings toggle was not persisted"
        );
        window.render_frame(cx);
        Ok(())
    })??;
    if pixels {
        capture(cx, handle, &output.join("vim-settings.png"))?;
    }

    let path = output.join("session/vim-solutions/first.py");
    let first_session = fixture_session(&db, path.clone(), "first")?;
    std::fs::write(&path, "alpha beta alpha")?;
    cx.update_window(handle, |_, window, cx| {
        workspace.update(cx, |workspace, cx| {
            workspace.session = Some(first_session);
            workspace.tabs = vec![None];
            workspace.active_tab = Some(0);
            workspace.center = Center::Editor;
            workspace.left = false;
            workspace.description = false;
            workspace.bottom = false;
            workspace.editor.update(cx, |editor, cx| {
                editor.set_value("alpha beta alpha", window, cx);
                editor.set_selected_range(0..0, cx);
            });
            workspace.focus_editor(window, cx);
        });
        window.render_frame(cx);
    })?;
    settle(cx);
    let first = cx.update(|cx| workspace.read(cx).editor.clone());
    cx.update_window(handle, |_, window, cx| -> Result<()> {
        keys(
            window,
            &["y", "w", "/", "a", "l", "p", "h", "a", "enter"],
            cx,
        );
        ensure!(
            first.read(cx).cursor() == 11,
            "Workspace search routing failed"
        );
        keys(window, &["x"], cx);
        // Ctrl+V owns the input before the application/native Paste binding.
        cx.write_to_clipboard(ClipboardItem::new_string("must not paste".into()));
        window.press("ctrl-v", cx);
        mode(&first, VimMode::VisualBlock, cx)?;
        text(&first, "alpha beta lpha", cx)?;
        window.press("escape", cx);
        window.render_frame(cx);
        Ok(())
    })??;
    if pixels {
        capture(cx, handle, &output.join("vim-workspace.png"))?;
    }

    let second_path = output.join("session/vim-solutions/second.py");
    std::fs::write(&second_path, "alpha beta alpha")?;
    let second_session = fixture_session(&db, second_path.clone(), "second")?;
    cx.update_window(handle, |_, window, cx| {
        workspace.update(cx, |workspace, cx| {
            workspace.park_tab();
            workspace.new_editor(window, cx);
            workspace.session = Some(second_session);
            workspace.tabs.push(None);
            workspace.active_tab = Some(1);
            workspace.editor.update(cx, |editor, cx| {
                editor.set_value("alpha beta alpha", window, cx);
                editor.set_selected_range(0..0, cx);
            });
            workspace.focus_editor(window, cx);
        });
        window.render_frame(cx);
    })?;
    settle(cx);
    let second = cx.update(|cx| workspace.read(cx).editor.clone());
    cx.update_window(handle, |_, window, cx| -> Result<()> {
        mode(&second, VimMode::Normal, cx)?;
        window.press("n", cx);
        ensure!(
            second.read(cx).cursor() == 11,
            "Search was not shared with the new tab"
        );
        keys(window, &["/", "up", "enter"], cx);
        ensure!(
            second.read(cx).cursor() == 0,
            "Literal search history was not shared with the new tab"
        );
        keys(window, &["0", "."], cx);
        text(&second, "lpha beta alpha", cx)?;
        window.press("u", cx);
        text(&second, "alpha beta alpha", cx)?;
        // x replaced the unnamed register with 'a' in the first editor.
        window.press("shift-p", cx);
        text(&second, "aalpha beta alpha", cx)?;
        keys(window, &[":", "w", "enter"], cx);
        Ok(())
    })??;
    settle(cx);
    cx.update_window(handle, |_, window, cx| -> Result<()> {
        ensure!(
            std::fs::read_to_string(&second_path)? == "aalpha beta alpha",
            ":w did not use the production save flow"
        );
        keys(window, &[":", "w", "q", "enter"], cx);
        Ok(())
    })??;
    settle(cx);
    cx.update_window(handle, |_, window, cx| -> Result<()> {
        ensure!(
            workspace.read(cx).active_tab == Some(0),
            ":wq did not close the saved tab"
        );
        workspace.update(cx, |workspace, cx| workspace.toggle_vim(window, cx));
        ensure!(
            first.read(cx).vim_mode().is_none() && second.read(cx).vim_mode().is_none(),
            "Toggle missed active or recently closed editors"
        );
        workspace.update(cx, |workspace, cx| workspace.reopen_problem(window, cx));
        ensure!(
            workspace.read(cx).editor == second && second.read(cx).vim_mode().is_none(),
            "Reopen lost retained editor or toggle"
        );
        workspace.update(cx, |workspace, cx| workspace.toggle_vim(window, cx));
        mode(&first, VimMode::Normal, cx)?;
        mode(&second, VimMode::Normal, cx)?;
        Ok(())
    })??;
    settle(cx);
    cx.update_window(handle, |_, window, cx| -> Result<()> {
        window.press("u", cx);
        text(&second, "alpha beta alpha", cx)?;
        window.press("ctrl-r", cx);
        text(&second, "aalpha beta alpha", cx)?;
        let blocked = output.join("session/save-blocker");
        std::fs::write(&blocked, "a file is not a directory")?;
        workspace.update(cx, |workspace, _| {
            workspace.session.as_mut().unwrap().path = blocked.join("solution.py")
        });
        keys(window, &[":", "w", "q", "enter"], cx);
        Ok(())
    })??;
    settle(cx);
    cx.update_window(handle, |_, window, cx| -> Result<()> {
        ensure!(
            workspace.read(cx).active_tab == Some(1) && workspace.read(cx).session.is_some(),
            "Failed :wq discarded the solution tab"
        );
        text(&second, "aalpha beta alpha", cx)?;
        workspace.update(cx, |workspace, _| {
            workspace.session.as_mut().unwrap().path = second_path.clone()
        });
        workspace.update(cx, |workspace, cx| {
            workspace.open_snippet_editor(window, cx)
        });
        window.render_frame(cx);
        Ok(())
    })??;
    settle(cx);
    let snippet = cx.update(|cx| workspace.read(cx).snippet_editor.clone().unwrap());
    let source = cx.update(|cx| snippet.read(cx).fixture_source());
    let preview = cx.update(|cx| snippet.read(cx).fixture_preview());
    cx.update_window(handle, |_, window, cx| -> Result<()> {
        mode(&source, VimMode::Normal, cx)?;
        mode(&preview, VimMode::Normal, cx)?;
        window.click("new-snippet", cx);
        window.input("Normal text input", cx);
        mode(&source, VimMode::Normal, cx)?;
        source.update(cx, |source, cx| {
            source.set_value("abc\ndef\n", window, cx);
            source.set_selected_range(0..0, cx);
            source.focus(window, cx);
        });
        window.render_frame(cx);
        // Hit-test the production snippet body rather than a test-only input.
        window.click("snippet-body-drop", cx);
        Ok(())
    })??;
    settle(cx);
    cx.update_window(handle, |_, window, cx| -> Result<()> {
        keys(window, &["g", "g", "0", "ctrl-v", "j", "l"], cx);
        mode(&source, VimMode::VisualBlock, cx)?;
        window.render_frame(cx);
        ensure!(
            window.find("snippet-vim-status").visible(),
            "Snippet mode label missing"
        );
        Ok(())
    })??;
    if pixels {
        capture(cx, handle, &output.join("vim-snippet.png"))?;
    }
    cx.update_window(handle, |_, window, cx| -> Result<()> {
        keys(window, &["d"], cx);
        text(&source, "c\nf\n", cx)?;
        window.press("u", cx);
        text(&source, "abc\ndef\n", cx)?;
        preview.update(cx, |preview, cx| {
            preview.set_value("readonly one readonly", window, cx);
            preview.set_selected_range(0..0, cx);
            preview.focus(window, cx);
        });
        window.render_frame(cx);
        ensure!(
            preview.read(cx).presentation().is_readonly(),
            "Production preview lost its read-only facade"
        );
        keys(window, &["shift-g", "v", "$"], cx);
        preview.update(cx, |preview, cx| preview.set_value("x", window, cx));
        window.press("y", cx);
        text(&preview, "x", cx)?;
        preview.update(cx, |preview, cx| {
            preview.set_value("readonly one readonly", window, cx);
            preview.set_selected_range(0..0, cx);
        });
        keys(window, &["w", "v", "l", "y"], cx);
        mode(&preview, VimMode::Normal, cx)?;
        keys(window, &["d", "d", "i"], cx);
        window.input("denied", cx);
        text(&preview, "readonly one readonly", cx)?;
        keys(
            window,
            &["/", "r", "e", "a", "d", "o", "n", "l", "y", "enter"],
            cx,
        );
        ensure!(
            preview.read(cx).cursor() == 13,
            "Read-only navigation/search was blocked"
        );
        window.press("escape", cx);
        // Toggle through the production palette, with source parked behind it.
        window.press("ctrl-p", cx);
        window.render_frame(cx);
        window.input("Toggle Vim keybindings", cx);
        Ok(())
    })??;
    settle(cx);
    cx.update_window(handle, |_, window, cx| -> Result<()> {
        window.render_frame(cx);
        let command = crate::actions::COMMANDS
            .iter()
            .position(|command| command.id == "ToggleVim")
            .context("Missing Vim palette command")?;
        let position = workspace
            .read(cx)
            .omni
            .results()
            .position(|(_, target)| *target == crate::omnibar::Target::Command(command))
            .context("Missing Vim palette result")?;
        for _ in 0..position {
            window.press("down", cx);
        }
        window.press("enter", cx);
        Ok(())
    })??;
    settle(cx);
    cx.update_window(handle, |_, window, cx| -> Result<()> {
        ensure!(
            !workspace.read(cx).config.vim_mode
                && source.read(cx).vim_mode().is_none()
                && preview.read(cx).vim_mode().is_none(),
            "Palette toggle missed snippet editors"
        );
        ensure!(
            !Config::load()?.vim_mode,
            "Palette preference was not persisted"
        );
        window.remove_window();
        Ok(())
    })??;
    settle(cx);
    Ok(())
}
