//! Pinned native server installation, without npm, Node, or system changes.
use std::{collections::BTreeMap, io::Read as _, path::{Path, PathBuf}, process::Stdio, sync::{Arc, Mutex}, time::{Duration, Instant}};
use anyhow::{Context, Result, ensure};
use serde::Deserialize;

const MANIFEST: &str = include_str!("../../setup/copilot.json");
#[derive(Clone, Deserialize)]
struct Package { version: String, url: String, digest: String }
pub fn directory() -> PathBuf { crate::tool_setup::directory().join("copilot") }
pub fn binary_at(root: &Path) -> PathBuf { root.join(if cfg!(windows) { "package/copilot-language-server.exe" } else { "package/copilot-language-server" }) }
pub fn binary() -> Option<PathBuf> { let path = binary_at(&directory()); path.is_file().then_some(path) }
fn package() -> Result<Package> {
    let os = match std::env::consts::OS { "macos" => "darwin", "windows" => "win32", os => os };
    let arch = match std::env::consts::ARCH { "x86_64" => "x64", "aarch64" => "arm64", arch => arch };
    let packages: BTreeMap<String, Package> = serde_json::from_str(MANIFEST)?;
    packages.get(&format!("{os}-{arch}")).cloned().context("Copilot is unavailable on this platform")
}
pub fn install(root: &Path) -> Result<PathBuf> {
    install_with_progress(root, |_| {})
}

fn version(path: &Path) -> Result<String> {
    let mut child = crate::background_process::command(path).arg("--version")
        .stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn()?;
    let stdout = child.stdout.take().context("Copilot version output")?;
    let reader = std::thread::spawn(move || {
        let mut output = String::new();
        stdout.take(16 * 1024).read_to_string(&mut output).map(|_| output)
    });
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait()? { break status; }
        if started.elapsed() >= Duration::from_secs(5) {
            let _ = child.kill(); let _ = child.wait(); let _ = reader.join();
            anyhow::bail!("Copilot server version check timed out; retry setup");
        }
        std::thread::sleep(Duration::from_millis(25));
    };
    let output = reader.join().map_err(|_| anyhow::anyhow!("Copilot version reader stopped"))??;
    ensure!(status.success(), "Copilot server version check failed");
    Ok(output.trim().to_owned())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Progress {
    Checking,
    Downloading { received: u64, total: Option<u64> },
    Extracting,
    Verifying,
}
impl Progress {
    pub fn label(&self) -> String {
        match self {
            Self::Checking => "Checking Copilot server".into(),
            Self::Downloading { received, total } => {
                let received_mib = *received as f64 / 1_048_576.;
                match total.filter(|total| *total > 0) {
                    Some(total) => format!("Downloading Copilot · {:.0}% · {received_mib:.1} / {:.1} MiB", (*received as f64 / total as f64 * 100.).min(100.), total as f64 / 1_048_576.),
                    None => format!("Downloading Copilot · {received_mib:.1} MiB"),
                }
            }
            Self::Extracting => "Unpacking verified Copilot download".into(),
            Self::Verifying => "Verifying Copilot server".into(),
        }
    }
}

pub fn install_with_progress(root: &Path, progress: impl Fn(Progress) + Send + Sync + 'static) -> Result<PathBuf> {
    let progress = Arc::new(progress);
    progress(Progress::Checking);
    let package = package()?;
    let binary = binary_at(root);
    if binary.is_file() && version(&binary).is_ok_and(|version| version.contains(&package.version)) { return Ok(binary); }
    std::fs::create_dir_all(root)?;
    let lock = root.join(".install-lock");
    std::fs::create_dir(&lock).context("Copilot installation already running; retry when it finishes")?;
    struct Guard(PathBuf);
    impl Drop for Guard { fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); } }
    let _guard = Guard(lock.clone());
    struct Source(self_update::Release);
    impl self_update::ReleaseSource for Source {
        fn get_releases(&self) -> self_update::Result<Vec<self_update::Release>> { Ok(vec![self.0.clone()]) }
    }
    let name = "server.tar.gz";
    let release = self_update::Release::builder().version(&package.version)
        .asset(self_update::ReleaseAsset::new(name, package.url).with_digest(package.digest)).build()?;
    let expected = package.version;
    let downloading = progress.clone();
    let extracting = progress.clone();
    let last_progress = Mutex::new(Instant::now());
    progress(Progress::Downloading { received: 0, total: None });
    std::fs::create_dir_all(root.join("package"))?;
    self_update::backends::custom::Update::configure().source(Source(release)).target("server")
        .bin_name("copilot-language-server").bin_install_path(&binary)
        .bin_path_in_archive(if cfg!(windows) { "package/copilot-language-server.exe" } else { "package/copilot-language-server" })
        .current_version("0.0.0").no_confirm(true).show_output(false).timeout(Duration::from_secs(300)).retries(0)
        .progress_callback(move |received, total| {
            let mut last = last_progress.lock().unwrap_or_else(|error| error.into_inner());
            if last.elapsed() >= Duration::from_millis(100) || total == Some(received) {
                *last = Instant::now(); downloading(Progress::Downloading { received, total });
            }
        })
        .verify_archive(move |_| { extracting(Progress::Extracting); Ok(()) })
        .verify_binary(move |path| {
            progress(Progress::Verifying);
            #[cfg(unix)] { use std::os::unix::fs::PermissionsExt; std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))?; }
            if !version(path).is_ok_and(|version| version.contains(&expected)) {
                return Err(self_update::Error::verification_rejected("Downloaded Copilot server failed its version check"));
            }
            Ok(())
        }).build()?.update()?;
    Ok(binary)
}
pub fn cli(args: &[String]) -> Result<()> {
    if args.iter().any(|arg| matches!(arg.as_str(), "--help" | "-h")) {
        println!("./ops copilot [--install | --probe] [--directory PATH] [--json]\nInstalls or probes the pinned native completion server. Does not sign in or read solutions.");
        return Ok(());
    }
    let mut root = directory(); let mut install_server = false; let mut probe = false; let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--install" if !install_server && !probe => install_server = true,
            "--probe" if !probe && !install_server => probe = true,
            "--directory" => { i += 1; root = PathBuf::from(args.get(i).context("--directory requires a path")?); ensure!(root.is_absolute(), "Copilot directory must be absolute"); }
            "--json" => {},
            _ => anyhow::bail!("Use ./ops copilot --help"),
        }
        i += 1;
    }
    let path = if install_server { install_with_progress(&root, |progress| eprintln!("{}", progress.label()))? } else { binary_at(&root) };
    let version = if path.is_file() {
        Some(version(&path)?)
    } else { None };
    let status = if probe {
        ensure!(path.is_file(), "Install Copilot with ./ops copilot --install");
        // A private empty workspace; only initialization, no prediction or authentication.
        let temporary = root.join(format!("probe-{}", std::process::id())); std::fs::create_dir(&temporary)?;
        let result = (|| -> Result<_> {
            let client = super::Client::start(&path, &temporary)?;
            async_io::block_on(client.ready())?;
            let status = format!("{:?}", client.status()); client.stop(); Ok(status)
        })();
        let _ = std::fs::remove_dir(&temporary); Some(result?)
    } else { None };
    println!("{}", serde_json::json!({"command":"copilot","environment":"local-private-tools","directory":root,"binary":path,"installed":path.is_file(),"version":version,"status":status,"authentication_started":false}));
    Ok(())
}
