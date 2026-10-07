use std::{path::{Path, PathBuf}, process::Command, time::Duration};
use super::{AgentKind, Cancel, Detected, transport::Process};
fn search(binary: &str, directories: &[PathBuf], windows: bool) -> Option<PathBuf> {
    for directory in directories { for suffix in if windows { &[".exe", ".cmd", ""][..] } else { &[""][..] } { let path = directory.join(format!("{binary}{suffix}")); if executable(&path) { return Some(path); } } } None
}
fn executable(path: &Path) -> bool {
    let Ok(metadata) = path.metadata() else { return false }; if !metadata.is_file() { return false; }
    #[cfg(unix)] { use std::os::unix::fs::PermissionsExt; metadata.permissions().mode() & 0o111 != 0 }
    #[cfg(not(unix))] { true }
}
pub fn detect() -> Vec<Detected> {
    let mut directories: Vec<_> = std::env::var_os("PATH").map(|path|std::env::split_paths(&path).collect()).unwrap_or_default();
    if let Some(home) = dirs::home_dir() { for path in [".local/bin", ".opencode/bin", ".npm-global/bin", ".bun/bin", ".local/share/mise/shims"] { directories.push(home.join(path)); } }
    directories.extend([PathBuf::from("/opt/homebrew/bin"),PathBuf::from("/usr/local/bin")]);
    if let Some(appdata) = std::env::var_os("APPDATA") { directories.push(PathBuf::from(appdata).join("npm")); }
    if let Some(local)=std::env::var_os("LOCALAPPDATA") { directories.push(PathBuf::from(local).join("agy/bin")); }
    let jobs: Vec<_> = AgentKind::ALL.into_iter().filter_map(|kind|search(kind.binary(), &directories, cfg!(windows)).map(|path|(kind,path))).map(|(kind,path)|std::thread::spawn(move || {
        let version = (|| -> anyhow::Result<String> { let mut process = Process::spawn(Command::new(&path).arg("--version"),Duration::from_secs(3))?; process.close_input(); let mut lines=Vec::new(); while let Some(line)=process.line(&Cancel::default())? { lines.push(line); } Ok(lines.join("\n").trim().to_owned()) })().ok().filter(|s|!s.is_empty());
        Detected { kind, path, version }
    })).collect(); jobs.into_iter().filter_map(|job|job.join().ok()).collect()
}
#[cfg(test)] mod tests { use super::*; #[test] fn agents_search_paths() { let dir=std::env::temp_dir().join(format!("leet-detect-{}",std::process::id())); std::fs::create_dir_all(&dir).unwrap(); let file=dir.join("agent.cmd"); std::fs::write(&file,"test").unwrap(); #[cfg(unix)] { use std::os::unix::fs::PermissionsExt; std::fs::set_permissions(&file,std::fs::Permissions::from_mode(0o700)).unwrap(); } assert_eq!(search("agent",std::slice::from_ref(&dir),true),Some(file)); assert_eq!(search("missing",std::slice::from_ref(&dir),false),None); std::fs::remove_dir_all(dir).unwrap(); } }
