//! Private, pinned tool downloads shared by native setup and the operator command.
use std::{io::{BufRead, BufReader}, path::{Path, PathBuf}, process::{Command, Stdio}, sync::mpsc, time::{Duration, Instant}};
use anyhow::{Context, Result, bail};
use serde::Deserialize;
use crate::language::Language;

const MANIFEST: &str = include_str!("../setup/tools.json");
const TOOLCHAINS: &str = include_str!("../setup/toolchains.json");

pub fn directory() -> PathBuf { crate::config::data_dir().join("tools") }
pub fn supported(_language: Language) -> bool { true }
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
    } else if language == Language::Go {
        let binary = root.join(format!("go-tools/bin/gopls{}", std::env::consts::EXE_SUFFIX));
        binary.is_file().then(|| (binary, vec![], "gopls".into()))
    } else if language == Language::Java {
        let binary = java_home(&root).join(format!("bin/java{}", std::env::consts::EXE_SUFFIX));
        let plugins = root.join("jdtls/server/plugins");
        let launcher = std::fs::read_dir(plugins).ok()?.filter_map(Result::ok).map(|entry| entry.path())
            .find(|path| path.file_name().is_some_and(|name| name.to_string_lossy().starts_with("org.eclipse.equinox.launcher_")))?;
        let config = match (std::env::consts::OS, std::env::consts::ARCH) {
            ("macos", "aarch64") => "config_mac_arm", ("linux", "aarch64") => "config_linux_arm",
            ("windows", _) => "config_win", ("macos", _) => "config_mac", _ => "config_linux",
        };
        let config = root.join("jdtls/server").join(config);
        if !binary.is_file() || !config.is_dir() { return None; }
        Some((binary, vec!["-Declipse.application=org.eclipse.jdt.ls.core.id1".into(), "-Dosgi.bundles.defaultStartLevel=4".into(),
            "-Declipse.product=org.eclipse.jdt.ls.core.product".into(), "-Xmx512m".into(), "--add-modules=ALL-SYSTEM".into(),
            "--add-opens".into(), "java.base/java.util=ALL-UNNAMED".into(), "--add-opens".into(), "java.base/java.lang=ALL-UNNAMED".into(),
            "-jar".into(), launcher.to_string_lossy().into_owned(), "-configuration".into(), config.to_string_lossy().into_owned()], "jdtls".into()))
    } else { None }
}

pub(crate) fn java_home(root: &Path) -> PathBuf {
    root.join(if cfg!(target_os = "macos") { "jdk/Contents/Home" } else { "jdk" })
}

pub(crate) fn java_debug_bundle() -> Option<PathBuf> {
    std::fs::read_dir(directory().join("java-debug/server")).ok()?.filter_map(Result::ok).map(|entry| entry.path())
        .find(|path| path.extension().is_some_and(|extension| extension == "jar") && path.file_name().is_some_and(|name| name.to_string_lossy().starts_with("com.microsoft.java.debug.plugin-")))
}

pub fn ready(language: Language) -> bool { ready_at(&directory(), language) }
fn ready_at(root: &Path, language: Language) -> bool {
    let shared = if language == Language::C { Language::Cpp } else { language };
    if !std::fs::read_to_string(root.join(format!("{}-ready.json", shared.id()))).is_ok_and(|manifest| manifest == TOOLCHAINS) { return false; }
    let suffix = std::env::consts::EXE_SUFFIX;
    let paths = match language {
        Language::Python => vec![python(root), root.join("python-server.json")],
        Language::Cpp | Language::C => vec![root.join(format!("zig/zig{suffix}")), root.join(format!("codelldb/adapter/codelldb{suffix}")), root.join("compiler-flags.json")],
        Language::Go => vec![root.join(format!("go/bin/go{suffix}")), root.join(format!("delve/dlv{suffix}")), root.join(format!("go-tools/bin/gopls{suffix}"))],
        Language::Java => vec![java_home(root).join(format!("bin/java{suffix}")), java_home(root).join(format!("bin/javac{suffix}")), root.join("java-debug/.leet-asset")],
    };
    paths.iter().all(|path| path.is_file()) && server_at(root, language).is_some()
}

pub(crate) fn managed_command(program: &std::ffi::OsStr) -> Option<(PathBuf, Vec<String>)> {
    if let Some(binary) = interpreter(program) { return Some((binary, vec![])); }
    let root = directory();
    let suffix = std::env::consts::EXE_SUFFIX;
    let (binary, args) = match program.to_str()? {
        "g++" | "clang++" => (root.join(format!("zig/zig{suffix}")), vec!["c++".into(), "-I".into(), root.join("include").to_string_lossy().into_owned()]),
        "gcc" | "clang" => (root.join(format!("zig/zig{suffix}")), vec!["cc".into()]),
        "go" => (root.join(format!("go/bin/go{suffix}")), vec![]),
        "java" | "javac" => (java_home(&root).join("bin").join(format!("{}{suffix}", program.to_str()?)), vec![]),
        _ => return None,
    };
    if !binary.is_file() { return None; }
    let mut args = args;
    if matches!(program.to_str(), Some("g++" | "clang++" | "gcc" | "clang")) {
        let system = if cfg!(windows) { "windows-gnu" } else if cfg!(target_os = "macos") { "macos" } else { "linux-gnu" };
        args.extend(["-target".into(), format!("{}-{system}", std::env::consts::ARCH)]);
    }
    Some((binary, args))
}

pub(crate) fn environment(command: &mut Command) {
    let root = directory();
    command.env("ZIG_GLOBAL_CACHE_DIR", root.join("zig-cache"));
    if root.join("go").is_dir() {
        command.env("GOROOT", root.join("go")).env("GOPATH", root.join("go-work")).env("GOCACHE", root.join("go-cache"))
            .env("GOPLSCACHE", root.join("gopls-cache")).env("CGO_ENABLED", "0").env("GOTOOLCHAIN", "local").env("GOENV", "off").env("GOMAXPROCS", "2").env("GOFLAGS", "-p=2");
    }
    if java_home(&root).is_dir() { command.env("JAVA_HOME", java_home(&root)); }
    let mut paths: Vec<_> = [root.join("go/bin"), java_home(&root).join("bin")].into_iter().filter(|path| path.is_dir()).collect();
    paths.extend(std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()));
    if let Ok(path) = std::env::join_paths(paths) { command.env("PATH", path); }
}

pub(crate) fn cpp_flags(language: Language) -> Vec<String> {
    let root = directory();
    std::fs::read(root.join("compiler-flags.json")).ok()
        .and_then(|bytes| serde_json::from_slice::<std::collections::HashMap<String, Vec<String>>>(&bytes).ok())
        .and_then(|mut flags| flags.remove(language.id())).unwrap_or_default()
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
    std::fs::write(root.join("toolchains.json"), TOOLCHAINS)?;
    std::fs::write(root.join("setup-toolchains.py"), include_str!("../setup/setup-toolchains.py"))?;
    let headers = root.join("include/bits");
    std::fs::create_dir_all(&headers)?;
    std::fs::write(headers.join("stdc++.h"), include_str!("../setup/stdc++.h"))?;
    std::fs::write(root.join("setup-tools.py"), include_str!("../setup/setup-tools.py"))?;
    let (name, source) = if cfg!(windows) { ("setup-tools.ps1", include_str!("../setup/setup-tools.ps1")) }
        else { ("setup-tools.sh", include_str!("../setup/setup-tools.sh")) };
    let script = root.join(name);
    std::fs::write(&script, source)?;
    Ok(script)
}

fn log_line(log: &mut String, line: &str) {
    log.extend(line.chars().take(1024)); log.push('\n');
    let mut start = log.len().saturating_sub(16 * 1024);
    while !log.is_char_boundary(start) { start += 1; }
    log.drain(..start);
}

pub fn install(language: Language, root: &Path, progress: impl Fn(String)) -> Result<()> {
    let root = if root.is_absolute() { root.to_path_buf() } else { std::env::current_dir()?.join(root) };
    // Keep normal Windows paths for PowerShell; canonicalize adds a verbatim prefix.
    let root = std::path::absolute(root)?;
    if root.join(".install-lock").exists() { bail!("Tool installation is already running"); }
    let script = prepare(&root)?;
    if ready_at(&root, language) {
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
            log_line(&mut log, &line);
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
    for line in rx { log_line(&mut log, &line); progress(line); }
    if !status.success() { bail!("Tool setup failed: {}", log.trim()); }
    let runtime = python(&root);
    if !runtime.is_file() || server_at(&root, language).is_none() { bail!("Tool setup finished without its runtime or language server"); }
    let shared = if language == Language::C { Language::Cpp } else { language };
    std::fs::write(root.join(format!("{}-ready.json", shared.id())), TOOLCHAINS)?;
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

    #[test]
    fn portable_catalog_covers_every_distributed_target_and_language() {
        let catalog: serde_json::Value = serde_json::from_str(TOOLCHAINS).unwrap();
        for platform in ["linux-x86_64", "linux-aarch64", "macos-x86_64", "macos-aarch64", "windows-x86_64"] {
            for name in ["zig", "go", "codelldb", "delve", "jdk"] {
                let asset = &catalog["platforms"][platform][name];
                assert!(asset["url"].as_str().unwrap().starts_with("https://"));
                let hash = asset["sha256"].as_str().unwrap();
                assert_eq!(hash.len(), 64); assert!(hash.bytes().all(|byte| byte.is_ascii_hexdigit()));
                assert!(!asset["executable"].as_str().unwrap().starts_with('/'));
            }
        }
        let root = tempfile::tempdir().unwrap();
        for language in Language::ALL {
            assert!(!ready_at(root.path(), language), "An empty directory was treated as a ready toolchain");
        }
    }

    #[cfg(unix)]
    #[test]
    fn private_archive_integrity_and_retry_regressions() {
        let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/toolchain_install.py");
        let output = Command::new("python3").arg(script).output().expect("Python 3 is required for installer regressions");
        assert!(output.status.success(), "{}\n{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
    }
}

pub fn cli(args: &[String]) -> Result<()> {
    if args.iter().any(|arg| matches!(arg.as_str(), "--help" | "-h")) {
        println!("leet --setup-tools --language python|cpp|c|go|java [--directory PATH] [--dry-run] [--json]\nDownloads pinned runtimes, compilers, editor tools and debuggers into a private directory. No system package manager, PATH change, accounts or solution access.");
        return Ok(());
    }
    let mut language = None; let mut root = directory(); let mut dry_run = false; let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--language" if language.is_none() => { index += 1; language = Some(match args.get(index).map(String::as_str) {
                Some("python") => Language::Python, Some("cpp") => Language::Cpp, Some("c") => Language::C, Some("go") => Language::Go, Some("java") => Language::Java, _ => bail!("Use --language python|cpp|c|go|java"),
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
    println!("{}", serde_json::json!({"command":"toolchain:setup","environment":"private-user-tools","language":language.id(),"directory":root,"dry_run":dry_run,"manifest":serde_json::from_str::<serde_json::Value>(MANIFEST)?,"toolchains":serde_json::from_str::<serde_json::Value>(TOOLCHAINS)?}));
    Ok(())
}
