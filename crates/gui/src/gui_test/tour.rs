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
        window.press("enter", cx);
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
    for step in 0..6 {
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| window.render_frame(cx))?;
        if video {
            let frames = output.join("tour-frames");
            std::fs::create_dir_all(&frames)?;
            for frame in 0..30 {
                let started = Instant::now();
                cx.update_window(handle, |_, window, cx| window.render_frame(cx))?;
                cx.capture_screenshot(handle)?.save(frames.join(format!("frame-{:04}.png", step * 30 + frame)))?;
                if let Some(delay) = Duration::from_secs_f64(1. / 60.).checked_sub(started.elapsed()) { std::thread::sleep(delay); }
            }
        } else if pixels {
            // GPUI's unsynchronized transitions use wall-clock time.
            std::thread::sleep(Duration::from_millis(320));
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
            if step == 1 {
                window.click("tour-try", cx);
            }
            if step == 3 {
                window.press("alt-d", cx);
                ensure!(!workspace.read(cx).right, "Tour shortcut did not toggle AI");
            }
            Ok(())
        })??;
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| -> Result<()> {
            if step == 1 { ensure!(workspace.read(cx).center == Center::Editor, "Editor action did not reach editor"); }
            if matches!(step, 1 | 3) {
                window.render_frame(cx);
                ensure!(window.find("skip-tour").bounds().top() < px(100.), "Guide did not move aside for practice");
            }
            window.click("tour-next", cx);
            Ok(())
        })??;
    }
    cx.update_window(handle, |_, window, cx| -> Result<()> {
        ensure!(workspace.read(cx).tour.is_none(), "Done did not finish the tour");
        ensure!(workspace.read(cx).left && workspace.read(cx).home.sidebar, "Tour did not restore Explorer");
        window.remove_window();
        Ok(())
    })??;
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
    Ok(json!({"fixture":"guided-tour","passed":true,"checks":["Explorer and NeetCode 150 defaults","automatic start","problem required","clickable search and editor","AI keyboard shortcut","guide moves aside for practice","Skip persistence across workspaces","manual replay","Escape after shortcut reload","six steps and Done","layout restoration","dark/light, minimum size and zoom bounds"],"pixels":pixels,"video":video}))
}
