//! Release metadata is fetched once; installation reuses it and touches only the app.
use std::{path::{Path, PathBuf}, time::Duration};
use anyhow::{Context, Result, bail};
use serde::Deserialize;
use self_update::{Release, ReleaseAsset, ReleaseSource};

const LATEST: &str = "https://api.github.com/repos/Noelithub77/leet/releases/latest";

#[derive(Clone, Debug)]
pub struct Available {
    pub version: String,
    release: Release,
    asset_name: String,
    appimage: bool,
}

#[derive(Clone, Debug)]
pub struct Latest {
    pub version: String,
    pub notes: String,
    pub available: Option<Available>,
}

#[derive(Deserialize)]
struct Metadata {
    tag_name: String,
    draft: bool,
    prerelease: bool,
    assets: Vec<Asset>,
    #[serde(default)]
    body: Option<String>,
}

#[derive(Deserialize)]
struct Asset { name: String, browser_download_url: String, digest: Option<String> }

/// Development builds inherit their nearest release tag, avoiding downgrade offers.
pub fn current_version(display: &str) -> &str {
    display.split_once('(').and_then(|(_, describe)| describe.strip_prefix('v'))
        .map(|tag| tag.split(['-', ')']).next().unwrap_or(tag))
        .unwrap_or_else(|| display.split_whitespace().next().unwrap_or(display))
}

fn asset_name(os: &str, arch: &str, appimage: bool, tag: &str) -> Result<String> {
    let platform = match (os, arch) {
        ("linux", "x86_64") => "linux",
        ("linux", "aarch64") => "linux-arm",
        ("macos", "x86_64") => "mac-intel",
        ("macos", "aarch64") => "mac-arm",
        ("windows", "x86_64") => "windows",
        ("linux" | "macos" | "windows", _) => bail!("Unsupported update architecture"),
        _ => bail!("Unsupported update platform"),
    };
    let extension = match os {
        "linux" if appimage => "AppImage",
        "linux" | "macos" => "tar.gz",
        "windows" => "exe",
        _ => bail!("Unsupported update platform"),
    };
    Ok(format!("leet-{platform}-{tag}.{extension}"))
}

pub fn check(display: &str, appimage: bool) -> Result<Latest> {
    check_at(LATEST, current_version(display), std::env::consts::OS, std::env::consts::ARCH, appimage)
}

fn check_at(url: &str, current: &str, os: &str, arch: &str, appimage: bool) -> Result<Latest> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(10))).max_redirects(0).build().into();
    let metadata: Metadata = agent.get(url).header("User-Agent", "leet-update")
        .header("Accept", "application/vnd.github+json").call()?.body_mut()
        .with_config().limit(2 * 1024 * 1024).read_json()?;
    let version = metadata.tag_name.clone();
    let notes = metadata.body.clone().unwrap_or_default();
    Ok(Latest { version, notes, available: select(metadata, current, os, arch, appimage)? })
}

fn select(metadata: Metadata, current: &str, os: &str, arch: &str, appimage: bool) -> Result<Option<Available>> {
    if metadata.draft || metadata.prerelease { return Ok(None); }
    let version = metadata.tag_name.strip_prefix('v').context("Invalid release tag")?;
    if !self_update::version::bump_is_greater(current, version)? { return Ok(None); }
    let name = asset_name(os, arch, appimage, &metadata.tag_name)?;
    let asset = metadata.assets.into_iter().find(|a| a.name == name).context("Release has no build for this platform")?;
    let digest = asset.digest.context("Release is missing its checksum")?;
    let hex = digest.strip_prefix("sha256:").context("Release checksum is not SHA-256")?;
    if hex.len() != 64 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) { bail!("Invalid release checksum"); }
    let expected = format!("https://github.com/Noelithub77/leet/releases/download/{}/{}", metadata.tag_name, name);
    if asset.browser_download_url != expected { bail!("Unexpected release download URL"); }
    let release = Release::builder().version(version)
        .asset(ReleaseAsset::new(&name, asset.browser_download_url).with_digest(digest)).build()?;
    Ok(Some(Available { version: version.to_owned(), release, asset_name: name, appimage }))
}

struct Cached(Release);
impl ReleaseSource for Cached {
    fn get_releases(&self) -> self_update::Result<Vec<Release>> { Ok(vec![self.0.clone()]) }
}

pub fn install(update: Available, display: &str, destination: &Path, progress: impl Fn(u64, Option<u64>) + Send + Sync + 'static) -> Result<()> {
    let expected = update.version.clone();
    let name = update.asset_name;
    let appimage = update.appimage;
    let mut builder = self_update::backends::custom::Update::configure();
    builder.source(Cached(update.release)).bin_name("leet")
        .current_version(current_version(display)).no_confirm(true).show_output(false)
        .timeout(Duration::from_secs(300)).retries(0)
        .progress_callback(progress)
        .asset_matcher(move |assets| assets.iter().find(|asset| asset.name() == name).cloned())
        .verify_binary(move |path| {
            let binary = if cfg!(target_os = "macos") && path.is_dir() { path.join("Contents/MacOS/leet") } else { path.to_path_buf() };
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut permissions = std::fs::metadata(&binary)?.permissions();
                permissions.set_mode(permissions.mode() | 0o700);
                std::fs::set_permissions(&binary, permissions)?;
            }
            #[cfg(target_os = "macos")]
            if path.is_dir() && !crate::background_process::command("codesign").args(["--verify", "--deep", "--strict"]).arg(path).status()?.success() {
                return Err(self_update::Error::verification_rejected("Downloaded app has an invalid signature"));
            }
            let mut command = crate::background_process::command(binary);
            if appimage { command.arg("--appimage-extract-and-run"); }
            let result = command.arg("--version").output().map_err(self_update::Error::from)?;
            let output = String::from_utf8_lossy(&result.stdout);
            let version = output.trim().strip_prefix("leet ").map(current_version);
            if !result.status.success() || version != Some(expected.as_str()) {
                return Err(self_update::Error::verification_rejected("Downloaded app failed its version check"));
            }
            Ok(())
        });
    #[cfg(target_os = "macos")]
    if destination.ancestors().any(|p| p.extension().is_some_and(|e| e == "app")) {
        let bundle = destination.ancestors().find(|p| p.extension().is_some_and(|e| e == "app"))
            .context("Application bundle not found")?;
        builder.bundle_path_in_archive("leet.app").bundle_install_path(bundle);
    } else {
        builder.bin_install_path(destination).bin_path_in_archive("leet.app/Contents/MacOS/leet");
    }
    #[cfg(not(target_os = "macos"))]
    builder.bin_install_path(destination).bin_path_in_archive("leet");
    if !builder.build()?.update()?.is_updated() { bail!("Release was not installed"); }
    Ok(())
}

pub fn appimage_path() -> Option<PathBuf> {
    #[cfg(target_os = "linux")]
    return std::env::var_os("APPIMAGE").map(PathBuf::from).filter(|p| p.is_absolute() && p.is_file());
    #[cfg(not(target_os = "linux"))]
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    fn metadata(digest: Option<&str>) -> Metadata {
        Metadata { tag_name: "v0.2.0".into(), body: None, draft: false, prerelease: false, assets: vec![Asset {
            name: "leet-linux-v0.2.0.tar.gz".into(),
            browser_download_url: "https://github.com/Noelithub77/leet/releases/download/v0.2.0/leet-linux-v0.2.0.tar.gz".into(),
            digest: digest.map(str::to_owned),
        }] }
    }
    #[test]
    fn versions_and_platforms_do_not_downgrade_or_cross_install() {
        assert_eq!(current_version("0.1.0 (v0.2.0-4-gabc-dirty)"), "0.2.0");
        assert_eq!(current_version("0.1.0 (abc)"), "0.1.0");
        assert!(select(metadata(None), "0.2.0", "linux", "x86_64", false).unwrap().is_none());
        assert!(select(metadata(None), "0.3.0", "linux", "x86_64", false).unwrap().is_none());
        assert!(select(metadata(None), "0.1.0", "macos", "aarch64", false).is_err());
        assert_eq!(asset_name("linux", "aarch64", true, "v0.2.0").unwrap(), "leet-linux-arm-v0.2.0.AppImage");
        assert_eq!(asset_name("macos", "aarch64", false, "v0.2.0").unwrap(), "leet-mac-arm-v0.2.0.tar.gz");
        assert_eq!(asset_name("macos", "x86_64", false, "v0.2.0").unwrap(), "leet-mac-intel-v0.2.0.tar.gz");
        assert_eq!(asset_name("windows", "x86_64", false, "v0.2.0").unwrap(), "leet-windows-v0.2.0.exe");
    }
    #[test]
    fn missing_or_invalid_checksums_and_untrusted_urls_are_rejected() {
        for digest in [None, Some("sha256:123"), Some("sha512:abcd")] {
            assert!(select(metadata(digest), "0.1.0", "linux", "x86_64", false).is_err());
        }
        let valid = format!("sha256:{}", "a".repeat(64));
        assert!(select(metadata(Some(&valid)), "0.1.0", "linux", "x86_64", false).unwrap().is_some());
        let mut bad = metadata(Some(&valid)); bad.assets[0].browser_download_url = "https://example.com/app".into();
        assert!(select(bad, "0.1.0", "linux", "x86_64", false).is_err());
    }
    #[test]
    fn launch_check_uses_one_metadata_request() {
        use std::{io::{Read, Write}, net::TcpListener};
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/latest", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0; 4096]; stream.read(&mut request).unwrap();
            let body = r#"{"tag_name":"v0.1.0","draft":false,"prerelease":false,"assets":[]}"#;
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
            drop(stream);
            listener.set_nonblocking(true).unwrap();
            assert!(listener.accept().is_err());
        });
        assert!(check_at(&url, "0.1.0", "linux", "x86_64", false).unwrap().available.is_none());
        server.join().unwrap();
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn verified_install_preserves_user_files_and_rejected_download_keeps_old_app() {
        use std::{io::{Read, Write}, net::TcpListener};
        let binary = b"#!/bin/sh\nprintf 'leet 0.2.0\\n'\n";
        let valid_digest = "sha256:ea12be7513257ca33fa5ba02e420a5d90631c23193c7b0bb159a625893733e58";
        for (version, digest, succeeds) in [("0.2.0", valid_digest, true), ("0.2.0", "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", false), ("0.2.1", valid_digest, false)] {
            let root = tempfile::tempdir().unwrap();
            let destination = root.path().join("leet");
            std::fs::write(&destination, b"old app").unwrap();
            let launcher = root.path().join("launch-leet");
            std::os::unix::fs::symlink(&destination, &launcher).unwrap();
            for file in ["config.toml", "progress.sqlite", "solution.py", "credentials"] {
                std::fs::write(root.path().join(file), b"keep me").unwrap();
            }
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let url = format!("http://{}/leet.exe", listener.local_addr().unwrap());
            let server = std::thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                stream.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
                let mut request = [0; 4096]; stream.read(&mut request).unwrap();
                write!(stream, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", binary.len()).unwrap();
                stream.write_all(binary).unwrap();
            });
            let release = Release::builder().version(version)
                .asset(ReleaseAsset::new("leet.exe", url).with_digest(digest)).build().unwrap();
            let update = Available { version: version.into(), release, asset_name: "leet.exe".into(), appimage: false };
            let result = install(update, "0.1.0", &destination, |_, _| {});
            server.join().unwrap();
            assert_eq!(result.is_ok(), succeeds, "{result:?}");
            if succeeds {
                let next_launch = crate::background_process::command(&launcher).arg("--version").output().unwrap();
                assert!(next_launch.status.success());
                assert_eq!(String::from_utf8(next_launch.stdout).unwrap().trim(), "leet 0.2.0");
            }
            assert_eq!(std::fs::read(&destination).unwrap(), if succeeds { binary.as_slice() } else { b"old app" });
            for file in ["config.toml", "progress.sqlite", "solution.py", "credentials"] {
                assert_eq!(std::fs::read(root.path().join(file)).unwrap(), b"keep me");
            }
        }
    }
}
