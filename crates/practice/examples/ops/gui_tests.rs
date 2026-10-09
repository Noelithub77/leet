//! Bounded GUI checks on fixture-only data and private displays.
use std::{fs, path::PathBuf, process::{Child, Command, Stdio}, time::{Duration, Instant, SystemTime, UNIX_EPOCH}};
use anyhow::{Context, Result, bail, ensure};
use serde_json::json;

struct Session(PathBuf);
impl Drop for Session { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); } }
struct Process(Child);
impl Drop for Process {
    fn drop(&mut self) {
        if !matches!(self.0.try_wait(), Ok(Some(_))) {
            let group = format!("-{}", self.0.id());
            let _ = Command::new("kill").args(["-TERM", "--", &group]).stdout(Stdio::null()).stderr(Stdio::null()).status();
            let _ = self.0.kill(); let _ = self.0.wait();
        }
    }
}

pub fn run(args: &[String]) -> Result<()> {
    if args.iter().any(|a| matches!(a.as_str(), "--help" | "-h")) {
        println!("./ops gui:test [--backend native|cage] [--interaction-only] [--video] [--output /absolute/new-directory] [--json]\nLocal fixture-only GUI checks. Native: real GPUI input and offscreen PNGs; --video captures 60 fps motion with FFmpeg. Cage: private headless Wayland window smoke test (requires cage and grim). No user desktop input, accounts, services, or saved data. Fails rather than falling back to your active display. Results and logs remain in the output directory; temporary session data is removed."); return Ok(());
    }
    let mut backend = "native"; let mut video = false; let mut interaction_only = false; let mut output = None; let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--backend" => { i += 1; backend = args.get(i).context("--backend needs native or cage")?; ensure!(matches!(backend, "native" | "cage"), "Backend must be native or cage"); }
            "--output" if output.is_none() => { i += 1; output = Some(PathBuf::from(args.get(i).context("--output needs a path")?)); }
            "--video" if !video => video = true,
            "--interaction-only" if !interaction_only => interaction_only = true,
            "--json" => {},
            option => bail!("Unknown GUI-test option: {option}"),
        }
        i += 1;
    }
    ensure!(!(video && interaction_only), "Video requires rendering");
    ensure!(backend != "cage" || (!video && !interaction_only), "Cage mode is a Wayland screenshot smoke check; use native for interaction-only or video");
    for language in [practice::language::Language::Python, practice::language::Language::Cpp] {
        practice::debugger::requirement(language, "python3").map_err(anyhow::Error::msg)?;
    }
    if video { ensure!(Command::new("ffmpeg").arg("-version").output().is_ok_and(|o| o.status.success()), "Video needs installed FFmpeg"); }
    if backend == "cage" {
        for tool in ["cage", "grim"] { ensure!(Command::new(tool).arg(if tool == "cage" { "-v" } else { "-h" }).output().is_ok_and(|o| o.status.success()), "Cage checks need installed {tool}"); }
    }
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
    let output = output.unwrap_or_else(|| std::env::temp_dir().join(format!("leet-gui-{}-{stamp}", std::process::id())));
    ensure!(output.is_absolute() && !output.exists(), "Output must be a new absolute directory");
    fs::create_dir_all(&output)?;
    let session = Session(output.join("session"));
    for dir in ["runtime", "config", "data", "cache", "solutions"] { fs::create_dir_all(session.0.join(dir))?; }
    use std::os::unix::{fs::PermissionsExt as _, process::CommandExt as _};
    fs::set_permissions(session.0.join("runtime"), fs::Permissions::from_mode(0o700))?;
    let build_log = output.join("build.log");
    let build = Command::new("cargo").args(["build", "-p", "gui", "--features", "gui-test"]).stdout(fs::File::create(&build_log)?).stderr(fs::OpenOptions::new().append(true).open(&build_log)?).status()?;
    ensure!(build.success(), "GUI fixture build failed; see {}", build_log.display());
    let target = std::env::var_os("CARGO_TARGET_DIR").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("target"));
    let binary = std::env::current_dir()?.join(target).join("debug/leet");
    ensure!(binary.is_file(), "Missing GUI fixture binary: {}", binary.display());
    let mut command = if backend == "cage" {
        let mut command = Command::new("cage"); command.arg("--").arg(&binary); command
    } else { Command::new(&binary) };
    command.arg("--gui-test").arg("--output").arg(&output);
    if backend == "cage" { command.arg("--wayland").env("WLR_BACKENDS", "headless").env("WLR_LIBINPUT_NO_DEVICES", "1").env("WLR_RENDERER", "pixman").env("LEET_GUI_TEST_PRIVATE_DISPLAY", "1"); }
    if interaction_only { command.arg("--interaction-only"); }
    if video { command.arg("--video"); }
    for key in ["DISPLAY", "WAYLAND_DISPLAY", "HYPRLAND_INSTANCE_SIGNATURE", "SWAYSOCK", "DBUS_SESSION_BUS_ADDRESS", "XAUTHORITY"] { command.env_remove(key); }
    command.env("XDG_RUNTIME_DIR", session.0.join("runtime")).env("XDG_CONFIG_HOME", session.0.join("config")).env("XDG_DATA_HOME", session.0.join("data")).env("XDG_CACHE_HOME", session.0.join("cache"));
    command.stdin(Stdio::null()).stdout(fs::File::create(output.join("runner.log"))?).stderr(fs::File::create(output.join("errors.log"))?).process_group(0);
    let mut child = Process(command.spawn().context("Start isolated GUI runner")?);
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.0.try_wait()? { break status; }
        ensure!(started.elapsed() < Duration::from_secs(180), "GUI runner exceeded 180 seconds; see {}", output.display());
        std::thread::sleep(Duration::from_millis(20));
    };
    ensure!(status.success(), "Isolated GUI test failed ({status}); see {}", output.display());
    let report: serde_json::Value = serde_json::from_slice(&fs::read(output.join("report.json"))?)?;
    ensure!(report["passed"] == true, "GUI report did not confirm success");
    println!("{}", json!({"command":"gui:test","environment":"isolated-fixtures","backend":backend,"passed":true,"output":output,"duration_seconds":started.elapsed().as_secs_f64(),"report":report}));
    Ok(())
}
