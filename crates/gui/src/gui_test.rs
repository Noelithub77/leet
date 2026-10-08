//! Fixture-only GUI verification. Never runs personal workspace startup.
use std::{path::{Path, PathBuf}, sync::Arc, time::{Duration, Instant}};
use anyhow::{Context as _, Result, ensure};
use gpui_kit::{App, AppContext as _, Bounds, Entity, HeadlessAppContext, WindowBounds, WindowOptions, point, px, size};
use gpui_kit::test::TestWindowExt as _;
use practice::{language::Language, runner::{Case, Compare}};
use serde_json::json;
use crate::debug_view::{Debugger, Snapshot};

mod statement;
mod tour;

const PYTHON: &str = "import sys\n\ndef solve():\n    data = list(map(int, sys.stdin.buffer.read().split()))\n    answers = []\n    for i in range(1, len(data)):\n        n = data[i]\n        answer = n * 2\n        answers.append(answer)\n        print(answer)\n\nif __name__ == '__main__':\n    solve()\n";
const CPP: &str = "#include <bits/stdc++.h>\nusing namespace std;\n\nint main() {\n    ios::sync_with_stdio(false);\n    cin.tie(nullptr);\n    int t;\n    cin >> t;\n    vector<int> answers;\n    while (t--) {\n        int n;\n        cin >> n;\n        int answer = n * 2;\n        answers.push_back(answer);\n        cout << answer << '\\n' << flush;\n    }\n    return 0;\n}\n";

fn snapshot(slug: &str, language: Language) -> Snapshot {
    Snapshot { slug: slug.into(), language, python: "python3".into(), code: if language == Language::Python { PYTHON } else { CPP }.into(), starter: language.stdin_template().into(), meta: serde_json::Value::Null,
        cases: vec![Case { id: 0, input: "3\n2 4 6\n".into(), expected: Some("4\n8\n12\n".into()), custom: false }, Case { id: 1, input: "2\n3 5\n".into(), expected: Some("6\n10\n".into()), custom: true }], compare: Compare::Exact }
}

fn init(cx: &mut App) {
    gpui_kit::init(cx);
    crate::theme::init("Vesper", 1., "Liberation Sans", cx);
    crate::debug_view::bind_keys(cx);
}

fn status(debugger: &Entity<Debugger>, cx: &App) -> Result<Option<(usize, usize, usize, bool)>> { debugger.read(cx).fixture_status() }

fn check_state(debugger: &Entity<Debugger>, cx: &App, case: usize, index: Option<usize>, playing: bool) -> Result<()> {
    let (selected, step, len, active) = status(debugger, cx)?.context("Recording not ready")?;
    ensure!(selected == case && len > 1 && active == playing, "Unexpected debugger case/playback: {selected}, {step}/{len}, playing={active}");
    if let Some(index) = index { ensure!(step == index, "Expected step {index}, got {step}"); }
    ensure!(debugger.read(cx).fixture_verdict() == Some(true), "Fixture verdict did not pass");
    Ok(())
}

pub fn run() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(2).collect();
    let mut statement_view = false;
    let mut output = None; let mut video = false; let mut pixels = true; let mut wayland = false; let mut explore = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--output" if output.is_none() => { i += 1; output = Some(PathBuf::from(args.get(i).context("--output requires a path")?)); }
            "--statement" if !statement_view => statement_view = true,
            "--video" if !video => video = true,
            "--wayland" if !wayland => wayland = true,
            "--explore" if !explore => { explore = true; wayland = true; }
            "--interaction-only" if pixels => pixels = false,
            option => anyhow::bail!("Unknown GUI-test option: {option}"),
        }
        i += 1;
    }
    ensure!(!video || pixels, "Video requires pixel rendering");
    let output = output.context("--output is required")?;
    std::fs::create_dir_all(&output)?;
    if wayland { ensure!(!video && pixels, "Wayland smoke uses screenshots"); return run_wayland(output, explore, statement_view); }
    let mut cx = HeadlessAppContext::with_platform(gpui_kit::platform::current_platform(true).text_system(), Arc::new(crate::assets::Assets), move || {
        if pixels { gpui_kit::platform::current_headless_renderer() } else { Ok(None) }
    });
    cx.allow_parking();
    cx.update(init);
    let mut reports = vec![statement::native(&mut cx, &output, pixels)?];
    reports.push(tour::native(&mut cx, &output, pixels, video)?);
    for slug in ["cf:1:A", "cc:GUIFIXTURE"] {
        for language in [Language::Python, Language::Cpp] {
            let name = format!("{}-{}", if slug.starts_with("cf:") { "codeforces" } else { "codechef" }, language.id());
            let fixture = snapshot(slug, language);
            let (handle, debugger) = cx.update(|cx| {
                gpui_kit::open_window(WindowOptions { window_bounds: Some(WindowBounds::Windowed(Bounds { origin: point(px(0.), px(0.)), size: size(px(1280.), px(800.)) })), show: false, focus: false, ..Default::default() }, cx,
                    |window, cx| {
                        let debugger = cx.new(Debugger::isolated);
                        debugger.update(cx, |view, cx| { view.load(fixture, 0, cx); view.focus(window, cx); });
                        debugger
                    })
            })?;
            let handle = handle.into();
            let started = Instant::now();
            loop {
                cx.run_until_parked();
                if cx.update(|cx| status(&debugger, cx))?.is_some() { break; }
                ensure!(started.elapsed() < Duration::from_secs(90), "Fixture recording timed out");
                std::thread::sleep(Duration::from_millis(10));
            }
            cx.update_window(handle, |_, window, cx| -> Result<()> {
                window.render_frame(cx);
                check_state(&debugger, cx, 0, Some(0), false)?;
                ensure!(window.find(("debug-case", 0usize)).visible(), "Case chip not visible");
                window.click("debugger-next", cx);
                check_state(&debugger, cx, 0, Some(1), false)?;
                window.press("left", cx);
                check_state(&debugger, cx, 0, Some(0), false)
            })??;
            if pixels { capture(&mut cx, handle, &output.join(format!("{name}-initial.png")))?; }
            cx.update_window(handle, |_, window, cx| -> Result<()> {
                window.click(("debug-case", 1usize), cx);
                check_state(&debugger, cx, 1, Some(0), false)?;
                window.press("up", cx);
                check_state(&debugger, cx, 0, Some(0), false)?;
                window.press("down", cx);
                check_state(&debugger, cx, 1, Some(0), false)?;
                let track = window.find("debugger-seek").bounds();
                window.drag(point(track.left() + px(1.), track.center().y), point(track.right() - px(1.), track.center().y), cx);
                let (_, index, len, _) = status(&debugger, cx)?.context("No trace after seek")?;
                ensure!(index == len - 1, "Seek did not reach last step");
                window.press("home", cx);
                check_state(&debugger, cx, 1, Some(0), false)?;
                window.click("debugger-play", cx);
                check_state(&debugger, cx, 1, Some(0), true)?;
                window.press("space", cx);
                check_state(&debugger, cx, 1, Some(0), false)?;
                window.press("end", cx);
                let (_, _, len, _) = status(&debugger, cx)?.context("No final trace")?;
                check_state(&debugger, cx, 1, Some(len - 1), false)
            })??;
            if pixels { capture(&mut cx, handle, &output.join(format!("{name}-final.png")))?; }
            if video && reports.len() == 2 { motion_video(&mut cx, handle, &debugger, &output)?; }
            reports.push(json!({"fixture":name,"passed":true,"checks":["real recording","case clicks","step button","keyboard steps","case shortcuts","seek drag","play/pause","final verdict"],"pixels":pixels}));
            cx.update_window(handle, |_, window, _| window.remove_window())?;
            cx.run_until_parked();
        }
    }
    let report = json!({"command":"gui:test","environment":"isolated-fixtures","backend":"gpui-headless","passed":true,"pixels":pixels,"video":video,"output":output,"fixtures":reports});
    std::fs::write(output.join("report.json"), serde_json::to_vec_pretty(&report)?)?;
    println!("{report}");
    Ok(())
}

fn capture(cx: &mut HeadlessAppContext, handle: gpui_kit::AnyWindowHandle, path: &Path) -> Result<()> {
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))?;
    let image = cx.capture_screenshot(handle)?;
    // GPUI's test platform renders logical window bounds at its default 2x scale.
    ensure!(image.width() == 2560 && image.height() == 1600, "Unexpected screenshot dimensions: {}x{}", image.width(), image.height());
    ensure!(image.pixels().any(|p| p.0[0] != image.get_pixel(0, 0).0[0]), "Screenshot is blank");
    image.save(path)?;
    Ok(())
}

fn motion_video(cx: &mut HeadlessAppContext, handle: gpui_kit::AnyWindowHandle, debugger: &Entity<Debugger>, output: &Path) -> Result<()> {
    let frames = output.join("frames"); std::fs::create_dir_all(&frames)?;
    cx.update_window(handle, |_, window, cx| { window.click(("debug-case", 0usize), cx); window.press("home", cx); })?;
    for frame in 0..240 {
        let started = Instant::now();
        if frame % 12 == 0 {
            cx.update_window(handle, |_, window, cx| window.press("right", cx))?;
        }
        cx.advance_clock(Duration::from_secs_f64(1. / 60.));
        capture(cx, handle, &frames.join(format!("frame-{frame:04}.png")))?;
        if let Some(delay) = Duration::from_secs_f64(1. / 60.).checked_sub(started.elapsed()) { std::thread::sleep(delay); }
    }
    cx.update(|cx| {
        let (_, index, len, _) = status(debugger, cx)?.context("Video trace disappeared")?;
        check_state(debugger, cx, 0, Some(index), false)?;
        ensure!(index > 1 && index < len, "Video did not advance through the trace");
        Ok::<_, anyhow::Error>(())
    })?;
    let result = std::process::Command::new("ffmpeg").args(["-nostdin", "-v", "error", "-framerate", "60", "-i"]).arg(frames.join("frame-%04d.png"))
        .args(["-vf", "scale=1280:800", "-c:v", "libx264", "-crf", "18", "-pix_fmt", "yuv420p", "-y"]).arg(output.join("motion.mp4")).status().context("Start FFmpeg")?;
    ensure!(result.success(), "FFmpeg video encoding failed");
    Ok(())
}

fn run_wayland(output: PathBuf, explore: bool, statement_view: bool) -> Result<()> {
    if explore {
        let name = std::env::var("OMABOX_BOX").context("Exploration requires an OmaBox session")?;
        ensure!(!name.is_empty() && std::env::var("OMABOX").as_deref() == Ok("1") && std::env::var("OMABOX_NAME").as_deref() == Ok(name.as_str()), "Exploration requires omabox run in a named box");
    } else {
        ensure!(std::env::var("LEET_GUI_TEST_PRIVATE_DISPLAY").as_deref() == Ok("1"), "Wayland fixtures require the private-display operator");
    }
    let runtime = PathBuf::from(std::env::var_os("XDG_RUNTIME_DIR").context("Missing private runtime")?);
    ensure!(runtime.is_dir() && (explore || runtime == output.join("session/runtime")), "Runtime is not the fixture's private directory");
    let socket = std::env::var("WAYLAND_DISPLAY").context("Missing private Wayland socket")?;
    ensure!(!socket.contains('/') && runtime.join(&socket).exists(), "Private compositor socket is missing");
    if statement_view { ensure!(explore, "Statement exploration requires OmaBox"); return statement::explore(output); }
    let result = Arc::new(std::sync::Mutex::new(None));
    let outcome = result.clone();
    gpui_kit::application().with_assets(crate::assets::Assets).run(move |cx| {
        init(cx);
        let opened = gpui_kit::open_window(WindowOptions { app_id: Some("leet-gui-fixture".into()), window_bounds: Some(WindowBounds::Windowed(Bounds { origin: point(px(0.), px(0.)), size: size(px(1280.), px(800.)) })), ..Default::default() }, cx, |window, cx| {
            let debugger = cx.new(Debugger::isolated);
            debugger.update(cx, |view, cx| { view.load(snapshot("cc:GUIFIXTURE", Language::Python), 0, cx); view.focus(window, cx); });
            debugger
        });
        let (handle, debugger) = match opened { Ok(value) => value, Err(error) => { *outcome.lock().unwrap() = Some(Err(error)); cx.quit(); return; } };
        let handle: gpui_kit::AnyWindowHandle = handle.into();
        cx.spawn(async move |cx| {
            let checked: Result<()> = async {
                let started = Instant::now();
                loop {
                    cx.background_executor().timer(Duration::from_millis(20)).await;
                    if cx.update(|cx| status(&debugger, cx))?.is_some() { break; }
                    ensure!(started.elapsed() < Duration::from_secs(90), "Wayland fixture recording timed out");
                }
                cx.update_window(handle, |_, window, cx| -> Result<()> {
                    window.render_frame(cx); check_state(&debugger, cx, 0, Some(0), false)
                })??;
                cx.background_executor().timer(Duration::from_millis(400)).await;
                wayland_capture(&output.join("wayland-initial.png"))?;
                if explore {
                    std::fs::write(output.join("ready.json"), serde_json::to_vec_pretty(&json!({"backend":"omabox","ready":true,"lifetime_seconds":90}))?)?;
                    // Leave the production view available for external compositor input.
                    cx.background_executor().timer(Duration::from_secs(90)).await;
                    return Ok(());
                }
                cx.update_window(handle, |_, window, cx| -> Result<()> {
                    window.click(("debug-case", 1usize), cx);
                    window.press("end", cx);
                    let (_, _, len, _) = status(&debugger, cx)?.context("No Wayland trace")?;
                    check_state(&debugger, cx, 1, Some(len - 1), false)
                })??;
                cx.background_executor().timer(Duration::from_millis(400)).await;
                wayland_capture(&output.join("wayland-final.png"))?;
                let report = json!({"command":"gui:test","environment":"isolated-fixtures","backend":"cage-wayland","passed":true,"pixels":true,"video":false,"checks":["native Wayland window","real recording","case click","keyboard seek","final verdict"],"output":output});
                std::fs::write(output.join("report.json"), serde_json::to_vec_pretty(&report)?)?;
                println!("{report}");
                Ok(())
            }.await;
            *outcome.lock().unwrap() = Some(checked);
            let _ = cx.update(|cx| cx.quit());
        }).detach();
    });
    result.lock().unwrap().take().context("Wayland fixture exited before verification")?
}

fn wayland_capture(path: &Path) -> Result<()> {
    let result = std::process::Command::new("grim").arg(path).output().context("Capture private Wayland display")?;
    ensure!(result.status.success(), "Private screenshot failed: {}", String::from_utf8_lossy(&result.stderr));
    ensure!(std::fs::metadata(path)?.len() > 512, "Private screenshot is empty");
    Ok(())
}
