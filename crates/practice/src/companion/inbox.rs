//! Durable browser-import handoff between the background receiver and the GUI.
use std::{path::{Path, PathBuf}, time::{Duration, SystemTime, UNIX_EPOCH}};
use anyhow::{Context, Result, bail};
use super::Import;
fn root() -> PathBuf { crate::config::data_dir().join("companion") }
fn now() -> u64 { SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() }
pub struct Presence { path: PathBuf }
impl Presence {
    pub fn claim(role: &str) -> Result<Self> { let presence = Self { path: root().join(format!("{role}.json")) }; presence.refresh()?; Ok(presence) }
    pub fn refresh(&self) -> Result<()> { std::fs::create_dir_all(root())?; write_atomic(&self.path, &serde_json::json!({"pid":std::process::id(),"time":now()})) }
}
impl Drop for Presence { fn drop(&mut self) { if std::fs::read(&self.path).ok().and_then(|raw| serde_json::from_slice::<serde_json::Value>(&raw).ok()).is_some_and(|value| value["pid"].as_u64() == Some(u64::from(std::process::id()))) { let _ = std::fs::remove_file(&self.path); } } }
pub fn alive(role: &str) -> bool {
    let value = std::fs::read(root().join(format!("{role}.json"))).ok().and_then(|raw| serde_json::from_slice::<serde_json::Value>(&raw).ok());
    value.is_some_and(|value| {
        let fresh = value["time"].as_u64().is_some_and(|time| now().saturating_sub(time) < 5);
        #[cfg(target_os = "linux")]
        let running = value["pid"].as_u64().is_some_and(|pid| Path::new("/proc").join(pid.to_string()).exists());
        #[cfg(not(target_os = "linux"))]
        let running = true;
        fresh && running
    })
}
pub fn window_running() -> bool {
    if alive("window") { return true; }
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::ffi::OsStrExt;
        let Ok(processes) = std::fs::read_dir("/proc") else { return alive("window"); };
        for process in processes.flatten() {
            let Ok(pid) = process.file_name().to_string_lossy().parse::<u32>() else { continue; };
            if pid == std::process::id() { continue; }
            let Ok(raw) = std::fs::read(process.path().join("cmdline")) else { continue; };
            let args: Vec<_> = raw.split(|byte| *byte == 0).filter(|arg| !arg.is_empty()).collect();
            let Some(executable) = args.first() else { continue; };
            let executable = std::fs::read_link(process.path().join("exe")).unwrap_or_else(|_| PathBuf::from(std::ffi::OsStr::from_bytes(executable)));
            let name = executable.file_name().and_then(|name| name.to_str()).unwrap_or_default();
            if !(name == "leet" || name.starts_with("leet-")) || args.iter().skip(1).any(|arg| matches!(*arg, b"--companion-service" | b"--version" | b"-V" | b"--help" | b"-h" | b"--1337")) { continue; }
            let Ok(environment) = std::fs::read(process.path().join("environ")) else { continue; };
            let same_paths = ["XDG_CONFIG_HOME", "XDG_DATA_HOME"].into_iter().all(|key| {
                let prefix = format!("{key}=");
                let actual = environment.split(|byte| *byte == 0).find_map(|entry| entry.strip_prefix(prefix.as_bytes()));
                let actual = xdg_root(key, actual.map(std::ffi::OsStr::from_bytes));
                let expected = std::env::var_os(key);
                actual == xdg_root(key, expected.as_deref())
            });
            if same_paths { return true; }
        }
        false
    }
    #[cfg(not(target_os = "linux"))]
    { alive("window") }
}
fn xdg_root(key: &str, value: Option<&std::ffi::OsStr>) -> PathBuf {
    value.map(PathBuf::from).unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join(if key == "XDG_CONFIG_HOME" { ".config" } else { ".local/share" }))
}
pub fn claim_receiver() -> Result<Option<std::fs::File>> {
    std::fs::create_dir_all(root())?;
    claim_receiver_at(&root().join("receiver.lock"))
}
fn claim_receiver_at(path: &Path) -> Result<Option<std::fs::File>> {
    let file = std::fs::File::options().read(true).write(true).create(true).truncate(false).open(path)?;
    match file.try_lock() {
        Ok(()) => Ok(Some(file)),
        Err(std::fs::TryLockError::WouldBlock) => Ok(None),
        Err(error) => Err(error.into()),
    }
}
fn write_atomic(path: &Path, value: &impl serde::Serialize) -> Result<()> {
    let temporary = path.with_extension(format!("{}.tmp", std::process::id()));
    std::fs::write(&temporary, serde_json::to_vec(value)?)?; std::fs::rename(temporary, path)?; Ok(())
}
pub fn enqueue(import: &Import) -> Result<PathBuf> {
    enqueue_at(&root().join("inbox"), import)
}
fn enqueue_at(directory: &Path, import: &Import) -> Result<PathBuf> {
    import.slug()?;
    std::fs::create_dir_all(directory)?;
    if pending_at(directory)?.len() >= 128 { bail!("CPH inbox is full; open Leet to process pending imports"); }
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let path = directory.join(format!("{stamp:030}-{}.json", std::process::id())); write_atomic(&path, import)?; Ok(path)
}
pub fn pending() -> Result<Vec<PathBuf>> {
    pending_at(&root().join("inbox"))
}
fn pending_at(directory: &Path) -> Result<Vec<PathBuf>> {
    if !directory.exists() { return Ok(vec![]); }
    let mut files = vec![];
    for entry in std::fs::read_dir(directory)? { let entry = entry?; if entry.file_type()?.is_file() && entry.path().extension().is_some_and(|ext| ext == "json") { files.push(entry.path()); } }
    files.sort(); Ok(files)
}
pub fn read(path: &Path) -> Result<Import> {
    if std::fs::metadata(path)?.len() > 1 << 20 { bail!("CPH payload exceeds 1 MiB"); }
    let import: Import = serde_json::from_slice(&std::fs::read(path)?).context("Read pending CPH import")?; import.slug()?; Ok(import)
}
pub fn finish(path: &Path, success: bool) -> Result<()> { if success { std::fs::remove_file(path)?; } else { std::fs::rename(path, path.with_extension("failed"))?; } Ok(()) }
pub fn publish_ports(ports: &[u16]) -> Result<()> { std::fs::create_dir_all(root())?; write_atomic(&root().join("ports.json"), &ports) }
pub fn listening_ports() -> Vec<u16> {
    if !alive("receiver") { return vec![]; }
    std::fs::read(root().join("ports.json")).ok().and_then(|raw| serde_json::from_slice(&raw).ok()).unwrap_or_default()
}
pub fn warning(message: &str) -> Result<()> { std::fs::create_dir_all(root())?; write_atomic(&root().join("warning.json"), &message) }
pub fn take_warning() -> Option<String> { let path = root().join("warning.json"); let message = serde_json::from_slice(&std::fs::read(&path).ok()?).ok(); let _ = std::fs::remove_file(path); message }
pub const POLL: Duration = Duration::from_millis(300);

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_default_paths_match_unset_paths_and_test_profiles_stay_separate() {
        for key in ["XDG_CONFIG_HOME", "XDG_DATA_HOME"] {
            let default = xdg_root(key, None);
            assert_eq!(xdg_root(key, Some(default.as_os_str())), default);
            assert_ne!(xdg_root(key, Some(std::ffi::OsStr::new("/tmp/leet-isolated-profile"))), default);
        }
    }
    #[test]
    fn only_one_receiver_owns_the_profile_and_exit_releases_it() {
        let dir = tempfile::tempdir().unwrap(); let path = dir.path().join("receiver.lock");
        let first = claim_receiver_at(&path).unwrap().unwrap();
        assert!(claim_receiver_at(&path).unwrap().is_none());
        drop(first); assert!(claim_receiver_at(&path).unwrap().is_some());
    }
    #[test]
    fn pending_import_survives_restart_and_only_finishes_after_processing() {
        let dir = tempfile::tempdir().unwrap();
        let import: Import = serde_json::from_value(serde_json::json!({"name":"Number Mirror","group":"CodeChef","url":"https://www.codechef.com/problems/START01","tests":[{"input":"123","output":"123"}]})).unwrap();
        let path = enqueue_at(dir.path(), &import).unwrap();
        assert_eq!(pending_at(dir.path()).unwrap(), [path.clone()]);
        assert!(read(&path).unwrap() == import);
        finish(&path, true).unwrap(); assert!(pending_at(dir.path()).unwrap().is_empty());
        let path = enqueue_at(dir.path(), &import).unwrap(); finish(&path, false).unwrap();
        assert!(path.with_extension("failed").exists()); assert!(pending_at(dir.path()).unwrap().is_empty());
    }
}
