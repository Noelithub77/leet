//! Production walkthrough with isolated storage and a bundled public question.
use super::*;
use crate::workspace::{Center, Session, Workspace};
use practice::{config::Config, db::Db, language::Source, roadmap::List};

pub(super) fn native(cx: &mut HeadlessAppContext, output: &Path, pixels: bool, video: bool) -> Result<serde_json::Value> {
    let db = Arc::new(Db::open(&output.join("session/tour.sqlite"))?);
    let config = Config { onboarding_completed: true, workspace: output.join("session/solutions"), ..Config::default() };
    cx.update(|cx| {
        let bindings = cx.key_bindings().borrow().bindings().cloned().collect();
        cx.set_global(crate::actions::ComponentBindings(bindings));
        crate::actions::reload_keys(&config, cx);
    });
    let (handle, workspace) = cx.update(|cx| gpui_kit::open_window(WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds { origin: point(px(0.), px(0.)), size: size(px(1280.), px(800.)) })),
        show: false, focus: false, ..Default::default()
    }, cx, |window, cx| cx.new(|cx| {
        let mut workspace = Workspace::from_storage(config.clone(), db.clone(), [None, None], window, cx);
        workspace.start_default_tour(window, cx);
        workspace
    })))?;
    let handle = handle.into();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| -> Result<()> {
        window.render_frame(cx);
        let view = workspace.read(cx);
        ensure!(view.left && view.home.sidebar, "Explorer is hidden by default");
        ensure!(view.config.source == Source::NeetCode && view.config.roadmap_list == List::NeetCode150, "Wrong default practice list");
        ensure!(view.tour.is_some(), "Existing users did not receive the tour");
        ensure!(window.find("tour-try").visible(), "Tour card is hidden");
        window.press("right", cx);
        ensure!(workspace.read(cx).center == Center::Home, "Tour advanced without a problem");
        window.click("tour-try", cx);
        Ok(())
    })??;
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| -> Result<()> {
        ensure!(workspace.read(cx).omni.open, "Tour action did not open problem search");
        window.press("escape", cx);
        window.click("skip-tour", cx);
        ensure!(workspace.read(cx).tour.is_none(), "Skip did not dismiss the tour");
        workspace.update(cx, |view, cx| view.start_default_tour(window, cx));
        ensure!(workspace.read(cx).tour.is_none(), "Dismissed tour restarted automatically");
        for shortcut in ["ctrl-p", "ctrl-shift-p"] {
            window.press(shortcut, cx);
            ensure!(workspace.read(cx).omni.open && workspace.read(cx).omni.scope == crate::omnibar::Scope::All, "{shortcut} did not open full search");
            window.render_frame(cx);
            window.press("escape", cx);
        }
        window.click("guided-tour", cx);
        ensure!(workspace.read(cx).tour.is_some(), "Manual replay did not start");
        window.press("escape", cx);
        ensure!(workspace.read(cx).tour.is_none(), "Escape stopped working after shortcut reload");
        // Fixture data replaces loading a problem, so no accounts, LSPs, or network jobs run.
        let question = db.question("two-sum")?.context("Missing bundled question")?;
        let path = output.join("session/tour.py");
        std::fs::write(&path, &question.python)?;
        workspace.update(cx, |view, cx| {
            view.editor.update(cx, |editor, cx| editor.set_value(question.python.clone(), window, cx));
            view.statement.update(cx, |statement, cx| {
                statement.slug = question.slug.clone().into();
                statement.title = question.title.clone().into();
                statement.blocks = practice::description::parse(&question.content);
                cx.notify();
            });
            view.session = Some(Session {
                slug: question.slug.clone(), title: question.title.clone(), frontend_id: 1, question: Some(question),
                language: Language::Python, source: Source::NeetCode, rel: "tour.py".into(), path,
                disk_mtime: None, cases: vec![], results: vec![], compile_error: None, selected_case: 0,
                running: false, run_id: 0, judge: None, hints_shown: 0, history: vec![], selected_commit: 0,
                remote_versions: vec![], history_status: None, history_loading: false, history_loaded: false,
                version_sequence: 0, history_sequence: 0,
            });
            view.start_tour(window, cx);
        });
        Ok(())
    })??;
    for step in 0..crate::tour::STEP_COUNT {
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            assert!(gpui_kit::base::active_focus_trap(window, cx).is_some(), "Tour did not contain keyboard focus");
        })?;
        if video {
            let frames = output.join("tour-frames");
            std::fs::create_dir_all(&frames)?;
            for frame in 0..30 {
                let started = Instant::now();
                cx.advance_clock(Duration::from_secs_f64(1. / 60.));
                cx.run_until_parked();
                cx.update_window(handle, |_, window, cx| window.render_frame(cx))?;
                cx.capture_screenshot(handle)?.save(frames.join(format!("frame-{:04}.png", step * 30 + frame)))?;
                if let Some(delay) = Duration::from_secs_f64(1. / 60.).checked_sub(started.elapsed()) { std::thread::sleep(delay); }
            }
        } else if pixels {
            // GPUI's unsynchronized transitions use wall-clock time.
            std::thread::sleep(Duration::from_millis(320));
            cx.advance_clock(Duration::from_millis(320));
        }
        cx.run_until_parked();
        if pixels {
            cx.update_window(handle, |_, window, cx| window.render_frame(cx))?;
            cx.capture_screenshot(handle)?.save(output.join(format!("tour-step-{step}.png")))?;
        }
        cx.update_window(handle, |_, window, cx| -> Result<()> {
            let top = window.find("skip-tour").bounds();
            let bottom = window.find("tour-next").bounds();
            ensure!(top.top() >= px(0.) && bottom.right() <= window.viewport_size().width && bottom.bottom() <= window.viewport_size().height, "Tour card escaped the window");
            let before = workspace.read(cx).editor.read(cx).value().to_string();
            if step == 2 {
                workspace.update(cx, |view, cx| {
                    view.config.keybindings.insert("ToggleDescription".into(), "ctrl-x".into());
                    crate::actions::reload_keys(&view.config, cx);
                    view.focus_editor(window, cx);
                });
                window.render_frame(cx);
            }
            if step == 1 {
                // Simulate focus being stolen by the editor before tour input arrives.
                workspace.update(cx, |view, cx| view.focus_editor(window, cx));
                window.render_frame(cx);
                window.press("x", cx);
                ensure!(workspace.read(cx).editor.read(cx).value().as_ref() == before, "Tour typing reached the editor");
                window.press("left", cx);
                ensure!(workspace.read(cx).tour.as_ref().map(crate::tour::Tour::index) == Some(0), "Editor swallowed tour Prev");
                window.render_frame(cx);
                window.press("right", cx);
                ensure!(workspace.read(cx).tour.as_ref().map(crate::tour::Tour::index) == Some(1), "Editor swallowed tour Next");
            }
            Ok(())
        })??;
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            match step {
                1 => window.press("alt-s", cx),
                3 => window.click("tour-try", cx),
                2 => window.press("ctrl-x", cx),
                5 => window.press("alt-d", cx),
                6 => window.press("ctrl-`", cx),
                7 => window.press("space", cx),
                _ => window.click("tour-next", cx),
            }
        })?;
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| -> Result<()> {
            if step + 1 < crate::tour::STEP_COUNT {
                ensure!(workspace.read(cx).tour.as_ref().map(crate::tour::Tour::index) == Some(step + 1), "Tour action did not advance from step {step}");
                window.render_frame(cx);
            }
            Ok(())
        })??;
    }
    cx.update_window(handle, |_, _, cx| -> Result<()> {
        ensure!(workspace.read(cx).tour.is_none(), "Done did not finish the tour");
        ensure!(workspace.read(cx).left && workspace.read(cx).home.sidebar, "Tour did not restore Explorer");
        Ok(())
    })??;
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| -> Result<()> {
        window.render_frame(cx);
        workspace.update(cx, |view, cx| {
            let hints = vec!["Keep track of visited values.".to_string(), "Look up the missing value.".to_string()];
            view.session.as_mut().unwrap().question.as_mut().unwrap().hints = hints.clone();
            view.statement.update(cx, |statement, cx| { statement.hints = hints.into_iter().map(Into::into).collect(); statement.hints_shown = 0; statement.reference_open = false; cx.notify(); });
            view.description = true;
            view.center = Center::Editor;
            view.zen = false;
            view.config.keybindings.insert("RevealHint".into(), "ctrl-alt-shift-h".into());
            crate::actions::reload_keys(&view.config, cx);
            cx.notify();
        });
        Ok(())
    })??;
    cx.advance_clock(Duration::from_millis(400));
    cx.run_until_parked();
    std::thread::sleep(Duration::from_millis(400));
    for (button, expected) in [("cycle-hint", 1), ("hide-hints", 0), ("cycle-hint", 1), ("cycle-hint", 2), ("cycle-hint", 0)] {
        cx.update_window(handle, |_, window, cx| window.click(button, cx))?;
        cx.run_until_parked();
        cx.update(|cx| -> Result<()> {
            ensure!(workspace.read(cx).statement.read(cx).hints_shown == expected, "Compact {button} did not show {expected} hints");
            Ok(())
        })?;
    }
    cx.update_window(handle, |_, window, cx| -> Result<()> {
        window.press("ctrl-alt-shift-h", cx);
        ensure!(workspace.read(cx).statement.read(cx).hints_shown == 1, "Hint shortcut stopped working");
        window.hover("cycle-hint", cx);
        Ok(())
    })??;
    cx.advance_clock(Duration::from_millis(700));
    cx.run_until_parked();
    if pixels {
        std::thread::sleep(Duration::from_millis(700));
        cx.update_window(handle, |_, window, cx| window.render_frame(cx))?;
        cx.capture_screenshot(handle)?.save(output.join("hint-tooltip.png"))?;
    }
    cx.update_window(handle, |_, window, cx| -> Result<()> {
        window.press("ctrl-p", cx);
        ensure!(workspace.read(cx).omni.open, "Full search did not open after finishing the tour");
        Ok(())
    })??;
    cx.run_until_parked();
    if pixels {
        cx.update_window(handle, |_, window, cx| window.render_frame(cx))?;
        cx.advance_clock(Duration::from_millis(320));
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(320));
        cx.update_window(handle, |_, window, cx| window.render_frame(cx))?;
        cx.capture_screenshot(handle)?.save(output.join("search-arrows.png"))?;
    }
    cx.update_window(handle, |_, window, _| window.remove_window())?;
    ensure!(db.get("guided-tour-v1")?.as_deref() == Some("dismissed"), "Tour dismissal was not persisted");
    if video {
        let result = std::process::Command::new("ffmpeg").args(["-nostdin", "-v", "error", "-framerate", "60", "-i"]).arg(output.join("tour-frames/frame-%04d.png"))
            .args(["-vf", "scale=1280:800", "-c:v", "libx264", "-crf", "18", "-pix_fmt", "yuv420p", "-y"]).arg(output.join("tour-motion.mp4")).status().context("Start tour FFmpeg")?;
        ensure!(result.success(), "Tour video encoding failed");
    }
    cx.run_until_parked();
    for (name, width, height, theme, zoom) in [("compact", 720., 480., "Vesper", 1.), ("light", 1280., 800., "Solarized Light", 1.), ("zoom", 720., 600., "Vesper", 1.4)] {
        cx.update(|cx| { crate::theme::apply(theme, cx); crate::theme::set_zoom(zoom, cx); });
        let (handle, workspace) = cx.update(|cx| gpui_kit::open_window(WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds { origin: point(px(0.), px(0.)), size: size(px(width), px(height)) })),
            show: false, focus: false, ..Default::default()
        }, cx, |window, cx| cx.new(|cx| {
            let mut view = Workspace::from_storage(config.clone(), db.clone(), [None, None], window, cx);
            view.start_default_tour(window, cx);
            assert!(view.tour.is_none(), "Dismissal did not survive a new workspace");
            view.start_tour(window, cx);
            view
        })))?;
        let handle = handle.into();
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| window.render_frame(cx))?;
        if pixels { std::thread::sleep(Duration::from_millis(320)); }
        cx.update_window(handle, |_, window, cx| -> Result<()> {
            window.render_frame(cx);
            let top = window.find("skip-tour").bounds();
            let bottom = window.find("tour-next").bounds();
            ensure!(top.top() >= px(0.) && bottom.bottom() <= window.viewport_size().height && bottom.right() <= window.viewport_size().width, "Tour controls escaped {name} window");
            Ok(())
        })??;
        if pixels { cx.capture_screenshot(handle)?.save(output.join(format!("tour-{name}.png")))?; }
        cx.update_window(handle, |_, window, cx| {
            window.press("escape", cx);
            assert!(workspace.read(cx).tour.is_none());
            window.remove_window();
        })?;
        cx.run_until_parked();
    }
    cx.update(|cx| { crate::theme::apply("Vesper", cx); crate::theme::set_zoom(1., cx); });
    Ok(json!({"fixture":"guided-tour","passed":true,"checks":["Explorer and NeetCode 150 defaults","automatic start","problem required","clickable search and editor","Explorer action and shortcut","description and AI keyboard shortcuts","automatic action and shortcut advancement","tour input priority over editor","custom shortcut overrides editor Cut","Left/Right tour navigation","Ctrl+P and Ctrl+Shift+P full search","Skip persistence across workspaces","manual replay","Escape after shortcut reload","eight steps and Done","layout restoration","dark/light, minimum size and zoom bounds"],"pixels":pixels,"video":video}))
}
