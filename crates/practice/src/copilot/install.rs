//! Pinned native server installation, without npm, Node, or system changes.
use std::{collections::BTreeMap, path::{Path, PathBuf}, time::Duration};
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
    let package = package()?;
    let binary = binary_at(root);
    if binary.is_file() && crate::background_process::command(&binary).arg("--version").output().is_ok_and(|output| output.status.success() && String::from_utf8_lossy(&output.stdout).contains(&package.version)) { return Ok(binary); }
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
    std::fs::create_dir_all(root.join("package"))?;
    self_update::backends::custom::Update::configure().source(Source(release)).target("server")
        .bin_name("copilot-language-server").bin_install_path(&binary)
        .bin_path_in_archive(if cfg!(windows) { "package/copilot-language-server.exe" } else { "package/copilot-language-server" })
        .current_version("0.0.0").no_confirm(true).show_output(false).timeout(Duration::from_secs(300)).retries(0)
        .verify_binary(move |path| {
            #[cfg(unix)] { use std::os::unix::fs::PermissionsExt; std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))?; }
            let output = crate::background_process::command(path).arg("--version").output()?;
            if !output.status.success() || !String::from_utf8_lossy(&output.stdout).contains(&expected) {
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
    let path = if install_server { install(&root)? } else { binary_at(&root) };
    let version = if path.is_file() {
        let output = crate::background_process::command(&path).arg("--version").output()?;
        ensure!(output.status.success(), "Copilot version check failed");
        Some(String::from_utf8(output.stdout)?.trim().to_owned())
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
