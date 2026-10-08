//! Private, pinned tool downloads shared by native setup and the operator command.
use std::{io::{BufRead, BufReader}, path::{Path, PathBuf}, process::{Command, Stdio}, sync::mpsc, time::{Duration, Instant}};
use anyhow::{Context, Result, bail};
use serde::Deserialize;
use crate::language::Language;

const MANIFEST: &str = include_str!("../setup/tools.json");

pub fn directory() -> PathBuf { crate::config::data_dir().join("tools") }
pub fn supported(language: Language) -> bool { matches!(language, Language::Python | Language::Cpp | Language::C) }
pub fn python(root: &Path) -> PathBuf { root.join(if cfg!(windows) { "python/python.exe" } else { "python/bin/python3" }) }

#[derive(Deserialize)]
struct PythonServer { binary: PathBuf, script: PathBuf }
pub(crate) fn server(language: Language) -> Option<(PathBuf, Vec<String>, String)> {
    server_at(&directory(), language)
}
fn server_at(root: &Path, language: Language) -> Option<(PathBuf, Vec<String>, String)> {
    let root = root.canonicalize().ok()?;
    if language == Language::Python {
        let server: PythonServer = serde_json::from_slice(&std::fs::read(root.join("python-server.json")).ok()?).ok()?;
        let binary = server.binary.canonicalize().ok()?;
        let script = server.script.canonicalize().ok()?;
        if !binary.starts_with(&root) || !script.starts_with(&root) || !binary.is_file() || !script.is_file() { return None; }
        Some((binary, vec![script.to_string_lossy().into_owned(), "--stdio".into()], "basedpyright".into()))
    } else if matches!(language, Language::Cpp | Language::C) {
        let path = root.join(if cfg!(windows) { "clangd/bin/clangd.exe" } else { "clangd/bin/clangd" });
        path.is_file().then(|| (path, vec!["--background-index".into()], "clangd".into()))
    } else { None }
}

/// Resolve only default interpreter aliases; explicit custom interpreters stay authoritative.
pub(crate) fn interpreter(program: &std::ffi::OsStr) -> Option<PathBuf> {
    if program != "python" && program != "python3" { return None; }
    let binary = python(&directory());
    binary.is_file().then_some(binary)
}

fn prepare(root: &Path) -> Result<PathBuf> {
    std::fs::create_dir_all(root)?;
    std::fs::write(root.join("tools.json"), MANIFEST)?;
    std::fs::write(root.join("setup-tools.py"), include_str!("../setup/setup-tools.py"))?;
    let (name, source) = if cfg!(windows) { ("setup-tools.ps1", include_str!("../setup/setup-tools.ps1")) }
        else { ("setup-tools.sh", include_str!("../setup/setup-tools.sh")) };
    let script = root.join(name);
    std::fs::write(&script, source)?;
    Ok(script)
}

pub fn install(language: Language, root: &Path, progress: impl Fn(String)) -> Result<()> {
    if !supported(language) { bail!("Automatic setup is available for Python, C and C++ editor tools"); }
    if cfg!(all(target_os = "linux", not(target_arch = "x86_64"))) && language != Language::Python { bail!("The pinned upstream clangd archive supports Linux x64 only"); }
    let root = if root.is_absolute() { root.to_path_buf() } else { std::env::current_dir()?.join(root) };
    // Keep normal Windows paths for PowerShell; canonicalize adds a verbatim prefix.
    let root = std::path::absolute(root)?;
    if root.join(".install-lock").exists() { bail!("Tool installation is already running"); }
    let script = prepare(&root)?;
    if server_at(&root, language).is_some() {
        progress("Tools already installed.".into());
        return Ok(());
    }
    let mut command = if cfg!(windows) {
        let shell = std::env::var_os("SystemRoot").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("C:/Windows"))
            .join("System32/WindowsPowerShell/v1.0/powershell.exe");
        let mut command = Command::new(shell);
        command.args(["-NoLogo", "-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-File"]).arg(script).arg("-Directory").arg(&root).arg("-Language").arg(language.id());
        command
    } else {
        let mut command = Command::new("/bin/sh");
        command.arg(script).arg(&root).arg(language.id());
        command
    };
    crate::background_process::hide_console(&mut command);
    let install_id = format!("{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_nanos());
    command.env("LEET_TOOL_INSTALL_ID", &install_id);
    #[cfg(unix)] {
        use std::os::unix::process::CommandExt as _;
        command.process_group(0);
    }
    let mut child = command.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().context("Start tool installer")?;
    let (tx, rx) = mpsc::channel();
    let mut readers = Vec::new();
    for stream in [child.stdout.take().map(|stream| Box::new(stream) as Box<dyn std::io::Read + Send>), child.stderr.take().map(|stream| Box::new(stream) as Box<dyn std::io::Read + Send>)] {
        let sender = tx.clone();
        if let Some(stream) = stream { readers.push(std::thread::spawn(move || {
            for line in BufReader::new(stream).lines().map_while(Result::ok) { if sender.send(line).is_err() { break; } }
        })); }
    }
    drop(tx);
    let started = Instant::now();
    let mut log = String::new();
    let status = loop {
        while let Ok(line) = rx.try_recv() {
            if log.len() < 16 * 1024 { log.push_str(&line.chars().take(1024).collect::<String>()); log.push('\n'); }
            progress(line);
        }
        if let Some(status) = child.try_wait()? { break status; }
        if started.elapsed() > Duration::from_secs(1200) {
            #[cfg(unix)] { let _ = Command::new("kill").args(["-TERM", "--", &format!("-{}", child.id())]).status(); }
            #[cfg(windows)] {
                let mut stop = Command::new("taskkill.exe");
                crate::background_process::hide_console(&mut stop);
                let _ = stop.args(["/PID", &child.id().to_string(), "/T", "/F"]).output();
            }
            let _ = child.kill(); let _ = child.wait();
            if std::fs::read_to_string(root.join(".install-lock/owner")).is_ok_and(|owner| owner.trim() == install_id) {
                let _ = std::fs::remove_dir_all(root.join(".install-lock"));
            }
            bail!("Tool setup timed out; downloaded files remain in {}", root.display());
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    for reader in readers { let _ = reader.join(); }
    for line in rx { if log.len() < 16 * 1024 { log.push_str(&line.chars().take(1024).collect::<String>()); log.push('\n'); } progress(line); }
    if !status.success() { bail!("Tool setup failed: {}", log.trim()); }
    let runtime = python(&root);
    if !runtime.is_file() || server_at(&root, language).is_none() { bail!("Tool setup finished without its runtime or language server"); }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn private_server_paths_cannot_escape_the_tools_directory() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().join("tools");
        std::fs::create_dir(&root).unwrap();
        let binary = root.join("node");
        let script = root.join("server.js");
        std::fs::write(&binary, "fixture").unwrap();
        std::fs::write(&script, "fixture").unwrap();
        let manifest = root.join("python-server.json");
        std::fs::write(&manifest, serde_json::json!({"binary":binary,"script":script}).to_string()).unwrap();
        assert!(server_at(&root, Language::Python).is_some());
        let outside = temporary.path().join("outside.js");
        std::fs::write(&outside, "fixture").unwrap();
        std::fs::write(&manifest, serde_json::json!({"binary":binary,"script":root.join("../outside.js")}).to_string()).unwrap();
        assert!(server_at(&root, Language::Python).is_none());
        assert!(interpreter(std::ffi::OsStr::new("/custom/python")).is_none());
    }

    #[test]
    fn pinned_bootstrap_scripts_match_the_download_manifest() {
        let manifest: serde_json::Value = serde_json::from_str(MANIFEST).unwrap();
        let scripts = format!("{}\n{}", include_str!("../setup/setup-tools.sh"), include_str!("../setup/setup-tools.ps1"));
        for asset in manifest["python"].as_object().unwrap().values() {
            assert!(scripts.contains(asset["url"].as_str().unwrap()));
            assert!(scripts.contains(asset["sha256"].as_str().unwrap()));
        }
    }
}

pub fn cli(args: &[String]) -> Result<()> {
    if args.iter().any(|arg| matches!(arg.as_str(), "--help" | "-h")) {
        println!("leet --setup-tools --language python|cpp|c [--directory PATH] [--dry-run] [--json]\nDownloads pinned tools into a private directory. No package manager, system PATH change, accounts or solution access.");
        return Ok(());
    }
    let mut language = None; let mut root = directory(); let mut dry_run = false; let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--language" if language.is_none() => { index += 1; language = Some(match args.get(index).map(String::as_str) {
                Some("python") => Language::Python, Some("cpp") => Language::Cpp, Some("c") => Language::C, _ => bail!("Use --language python|cpp|c"),
            }); }
            "--directory" => { index += 1; root = PathBuf::from(args.get(index).context("--directory requires a path")?); }
            "--dry-run" => dry_run = true,
            "--json" => {}
            _ => bail!("Unexpected setup option; use --help"),
        }
        index += 1;
    }
    let language = language.context("--language is required")?;
    if !dry_run { install(language, &root, |line| eprintln!("{line}"))?; }
    println!("{}", serde_json::json!({"command":"toolchain:setup","environment":"private-user-tools","language":language.id(),"directory":root,"dry_run":dry_run,"manifest":serde_json::from_str::<serde_json::Value>(MANIFEST)?}));
    Ok(())
}
