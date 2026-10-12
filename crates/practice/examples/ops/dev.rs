//! Build before replacing this checkout's Linux development instance.
use std::{fs::{self, File}, path::{Path, PathBuf}, process::{Child, Command, Stdio}, time::{Duration, Instant}};
use std::os::unix::{fs::MetadataExt as _, process::ExitStatusExt as _};
use anyhow::{Context, Result, bail, ensure};
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
struct Metadata { workspace_root: PathBuf }
struct Instance { pid: u32, start: String }

pub fn run(args: &[String]) -> Result<()> {
    ensure!(args.iter().all(|arg| matches!(arg.as_str(), "--json" | "--help" | "-h")), "Use ./ops dev [--json]");
    if args.iter().any(|arg| matches!(arg.as_str(), "--help" | "-h")) {
        println!("./ops dev [--json]\nLinux development app. Build with two jobs and gui-test, then replace matching dev instances from this checkout. Build failures leave the old app running. Installed apps, Companion, and fixtures are excluded. Re-run after edits; Rust does not hot-reload.");
        return Ok(());
    }
    let metadata = Command::new("cargo").args(["metadata", "--no-deps", "--format-version", "1"]).output()?;
    ensure!(metadata.status.success(), "Cargo metadata failed");
    let root = serde_json::from_slice::<Metadata>(&metadata.stdout)?.workspace_root;
    fs::create_dir_all(root.join("target"))?;
    let lock = File::options().create(true).read(true).write(true).truncate(false).open(root.join("target/dev.lock"))?;
    lock.lock().context("Lock development restart")?;
    let allowed = fs::read_to_string("/proc/self/status")?;
    let list = allowed.lines().find_map(|line| line.strip_prefix("Cpus_allowed_list:\t")).context("Read allowed CPUs")?;
    let mut cpus = Vec::new();
    for entry in list.split(',') {
        let (start, end) = entry.split_once('-').unwrap_or((entry, entry));
        for cpu in start.parse::<u32>()?..=end.parse::<u32>()? { cpus.push(cpu.to_string()); if cpus.len() == 2 { break; } }
        if cpus.len() == 2 { break; }
    }
    ensure!(!cpus.is_empty(), "No allowed build CPU");
    let mut build = Command::new("nice");
    build.args(["-n", "19", "ionice", "-c", "3", "taskset", "-c", &cpus.join(","),
        "cargo", "build", "--jobs", "2", "-p", "gui", "--bin", "leet", "--features", "gui-test", "--message-format=json-render-diagnostics"]);
    let json_output = args.iter().any(|arg| arg == "--json");
    let mut child = build_and_start(&mut build, &root, json_output)?;
    drop(lock);
    let status = child.wait()?;
    ensure!(status.success() || status.signal() == Some(15), "Development app exited with {status}");
    if json_output { println!("{}", json!({"command":"dev","environment":"local","event":"exited","pid":child.id(),"code":status.code(),"signal":status.signal()})); }
    Ok(())
}

fn build_and_start(build: &mut Command, root: &Path, json_output: bool) -> Result<Child> {
    let output = build.current_dir(root).stderr(Stdio::inherit()).output()?;
    ensure!(output.status.success(), "Development build failed; existing app was kept running");
    let binary = output.stdout.split(|byte| *byte == b'\n').filter_map(|line| serde_json::from_slice::<serde_json::Value>(line).ok())
        .filter(|message| message["reason"] == "compiler-artifact" && message["target"]["name"] == "leet")
        .filter_map(|message| message["executable"].as_str().map(PathBuf::from)).last().context("Cargo did not report the leet binary")?;
    let binary = binary.canonicalize()?;
    let stopped = stop_existing(root, &binary)?;
    let child = Command::new(&binary).current_dir(root).stdout(File::options().write(true).open("/dev/stderr")?).spawn()?;
    if json_output { println!("{}", json!({"command":"dev","environment":"local","event":"started","binary":binary,"pid":child.id(),"stopped_pids":stopped})); }
    else { eprintln!("Dev started: PID {} (replaced {} matching instance(s))", child.id(), stopped.len()); }
    Ok(child)
}

fn start_time(pid: u32) -> Option<String> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    stat.rsplit_once(") ")?.1.split_whitespace().nth(19).map(str::to_owned)
}
fn matching(pid: u32, root: &Path, binary: &Path) -> Option<Instance> {
    let process = PathBuf::from(format!("/proc/{pid}"));
    if fs::metadata(&process).ok()?.uid() != fs::metadata("/proc/self").ok()?.uid() { return None; }
    if fs::read_link(process.join("cwd")).ok()? != root { return None; }
    let executable = fs::read_link(process.join("exe")).ok()?;
    let executable = executable.to_str()?.strip_suffix(" (deleted)").unwrap_or(executable.to_str()?);
    if Path::new(executable) != binary { return None; }
    let args = fs::read(process.join("cmdline")).ok()?;
    if args.split(|byte| *byte == 0).skip(1).any(|arg| matches!(arg,
        b"--gui-test" | b"--companion-service" | b"--setup-tools" | b"snippets" | b"--version" | b"-V")) { return None; }
    Some(Instance { pid, start: start_time(pid)? })
}
fn stop_existing(root: &Path, binary: &Path) -> Result<Vec<u32>> {
    let mut stopped = Vec::new();
    for entry in fs::read_dir("/proc")? {
        let entry = entry?;
        let Some(pid) = entry.file_name().to_str().and_then(|name| name.parse::<u32>().ok()) else { continue; };
        let Some(instance) = matching(pid, root, binary) else { continue; };
        if start_time(instance.pid).as_ref() != Some(&instance.start) { continue; }
        let status = Command::new("kill").args(["-TERM", "--", &pid.to_string()]).status()?;
        if !status.success() && matching(pid, root, binary).is_some() { bail!("Could not stop dev PID {pid}"); }
        let deadline = Instant::now() + Duration::from_secs(3);
        while matching(pid, root, binary).is_some_and(|current| current.start == instance.start) {
            ensure!(Instant::now() < deadline, "Dev PID {pid} did not exit after SIGTERM; replacement was not launched");
            std::thread::sleep(Duration::from_millis(25));
        }
        stopped.push(pid);
    }
    Ok(stopped)
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Process(Child);
    impl Drop for Process { fn drop(&mut self) { let _ = self.0.kill(); let _ = self.0.wait(); } }
    fn spawn(binary: &Path, root: &Path) -> Process { Process(Command::new(binary).arg("30").current_dir(root).spawn().unwrap()) }
    #[test]
    fn replacement_only_stops_matching_checkout_dev_even_after_rebuild() {
        let temp = tempfile::tempdir().unwrap(); let root = temp.path();
        let binary = root.join("leet"); fs::copy("/bin/sleep", &binary).unwrap();
        let mut dev = spawn(&binary, root);
        let installed = root.join("installed"); fs::copy("/bin/sleep", &installed).unwrap();
        let mut release = spawn(&installed, root);
        let foreign_root = root.join("foreign"); fs::create_dir(&foreign_root).unwrap();
        let mut foreign = spawn(&binary, &foreign_root);
        fs::remove_file(&binary).unwrap(); fs::copy("/bin/sleep", &binary).unwrap();
        assert_eq!(stop_existing(root, &binary).unwrap(), vec![dev.0.id()]);
        dev.0.wait().unwrap();
        assert!(release.0.try_wait().unwrap().is_none());
        assert!(foreign.0.try_wait().unwrap().is_none());
    }
    #[test]
    fn failed_build_keeps_existing_dev_running() {
        let temp = tempfile::tempdir().unwrap(); let binary = temp.path().join("leet");
        fs::copy("/bin/sleep", &binary).unwrap(); let mut dev = spawn(&binary, temp.path());
        assert!(build_and_start(&mut Command::new("/bin/false"), temp.path(), false).is_err());
        assert!(dev.0.try_wait().unwrap().is_none());
    }
}
