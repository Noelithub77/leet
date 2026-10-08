#![forbid(unsafe_code)]
//! Native release packaging. Builds are supplied by the platform's CI runner.
use std::{path::{Path, PathBuf}, process::Command};
use anyhow::{Context, Result, bail};
use serde_json::json;

fn main() {
    if let Err(error) = package() {
        eprintln!("{}", json!({"environment":"release-artifact", "error":error.to_string()}));
        std::process::exit(1);
    }
}

fn run(program: &str, args: &[&str]) -> Result<()> {
    let status = Command::new(program).args(args).status()?;
    if !status.success() { bail!("{program} failed: {status}"); }
    Ok(())
}

fn package() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.is_empty() || args[0] == "--help" {
        println!("cargo run -p practice --example release -- --target TARGET --tag vX.Y.Z [--output dist]\n\nPackages target/TARGET/release/leet on its native OS. Produces a Unix archive or standalone Windows exe. Linux also produces an AppImage when LEET_LINUXDEPLOY, LEET_APPIMAGETOOL and LEET_APPIMAGE_RUNTIME are set. No user data is accessed.");
        return Ok(());
    }
    let mut target = None; let mut tag = None; let mut output = PathBuf::from("dist");
    let mut pairs = args.chunks_exact(2);
    for pair in &mut pairs {
        match pair[0].as_str() {
            "--target" => target = Some(pair[1].as_str()),
            "--tag" => tag = Some(pair[1].as_str()),
            "--output" => output = PathBuf::from(&pair[1]),
            _ => bail!("Unknown argument {}; use --help", pair[0]),
        }
    }
    if !pairs.remainder().is_empty() { bail!("Expected option/value pairs"); }
    let target = target.context("--target is required")?;
    let tag = tag.context("--tag is required")?;
    let version = tag.strip_prefix('v').context("Tag must be vX.Y.Z")?;
    let parts: Vec<_> = version.split('.').collect();
    if parts.len() != 3 || parts.iter().any(|part| part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit())) {
        bail!("Tag must be vX.Y.Z");
    }
    let (os, platform) = match target {
        "x86_64-unknown-linux-gnu" => ("linux", "linux"),
        "aarch64-unknown-linux-gnu" => ("linux", "linux-arm"),
        "x86_64-apple-darwin" => ("macos", "mac-intel"),
        "aarch64-apple-darwin" => ("macos", "mac-arm"),
        "x86_64-pc-windows-msvc" => ("windows", "windows"),
        _ => bail!("Unsupported target {target}"),
    };
    if os != std::env::consts::OS { bail!("Package {target} on its native OS"); }
    let binary = PathBuf::from("target").join(target).join("release").join(if os == "windows" { "leet.exe" } else { "leet" });
    if !binary.is_file() { bail!("Build {} first", binary.display()); }
    std::fs::create_dir_all(&output)?;
    let name = format!("leet-{platform}-{tag}");
    let artifact = if os == "windows" {
        let path = output.join(format!("{name}.exe"));
        std::fs::copy(&binary, &path)?;
        path
    } else {
        let staging = output.join(format!(".{name}"));
        if staging.exists() { std::fs::remove_dir_all(&staging)?; }
        std::fs::create_dir_all(&staging)?;
        if os == "macos" {
            let contents = staging.join("leet.app/Contents");
            std::fs::create_dir_all(contents.join("MacOS"))?;
            std::fs::create_dir_all(contents.join("Resources"))?;
            std::fs::copy(&binary, contents.join("MacOS/leet"))?;
            std::fs::write(contents.join("Info.plist"), format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\"><dict><key>CFBundleIdentifier</key><string>com.noel.leet</string><key>CFBundleName</key><string>leet</string><key>CFBundleExecutable</key><string>leet</string><key>CFBundlePackageType</key><string>APPL</string><key>CFBundleShortVersionString</key><string>{version}</string><key>CFBundleVersion</key><string>{version}</string><key>LSMinimumSystemVersion</key><string>13.0</string><key>NSHighResolutionCapable</key><true/></dict></plist>\n"))?;
            // Ad-hoc signing is required on Apple Silicon; it is not Developer ID notarization.
            run("codesign", &["--force", "--sign", "-", staging.join("leet.app").to_str().context("App path")?])?;
        } else {
            std::fs::copy(&binary, staging.join("leet"))?;
            std::fs::copy("crates/gui/assets/leet.svg", staging.join("leet.svg"))?;
        }
        for file in ["LICENSE", "docs/bundle.md"] {
            std::fs::copy(file, staging.join(Path::new(file).file_name().context("License name")?))?;
        }
        let path = output.join(format!("{name}.tar.gz"));
        run("tar", &["-czf", path.to_str().context("Archive path")?, "-C", staging.to_str().context("Staging path")?, "."])?;
        std::fs::remove_dir_all(staging)?;
        path
    };
    println!("{}", json!({"environment":"release-artifact", "target":target, "tag":tag, "artifact":artifact, "bytes":std::fs::metadata(&artifact)?.len(), "developer_signed":false}));
    if os == "linux" && std::env::var_os("LEET_LINUXDEPLOY").is_some() {
        appimage(&binary, &output, platform, version)?;
    }
    Ok(())
}

fn appimage(binary: &Path, output: &Path, platform: &str, version: &str) -> Result<()> {
    let tool = |name| std::env::var_os(name).map(PathBuf::from).with_context(|| format!("{name} is required for AppImage packaging"));
    let linuxdeploy = tool("LEET_LINUXDEPLOY")?;
    let appimagetool = tool("LEET_APPIMAGETOOL")?;
    let runtime = tool("LEET_APPIMAGE_RUNTIME")?;
    let appdir = output.join(format!(".{platform}.AppDir"));
    if appdir.exists() { std::fs::remove_dir_all(&appdir)?; }
    std::fs::create_dir_all(&appdir)?;
    let desktop = output.join(".leet.desktop");
    std::fs::write(&desktop, "[Desktop Entry]\nType=Application\nName=leet\nExec=leet\nIcon=leet\nCategories=Development;IDE;\nTerminal=false\nStartupWMClass=leet\n")?;
    let status = Command::new(linuxdeploy).arg("--appimage-extract-and-run")
        .arg("--appdir").arg(&appdir).arg("--executable").arg(binary)
        .arg("--desktop-file").arg(&desktop).arg("--icon-file").arg("crates/gui/assets/leet.svg").status()?;
    if !status.success() { bail!("AppImage dependency bundling failed: {status}"); }
    let licenses = appdir.join("usr/share/doc/leet");
    std::fs::create_dir_all(&licenses)?;
    std::fs::copy("LICENSE", licenses.join("LICENSE"))?;
    std::fs::copy("docs/bundle.md", licenses.join("bundle.md"))?;
    // Preserve distro copyright notices for libraries bundled by linuxdeploy.
    for entry in std::fs::read_dir(appdir.join("usr/lib"))? {
        let path = entry?.path();
        let Some(file) = path.file_name().and_then(|name| name.to_str()) else { continue; };
        let result = Command::new("dpkg-query").args(["-S", &format!("*/{file}")]).output()?;
        if !result.status.success() { bail!("Cannot identify bundled library {file}"); }
        for line in String::from_utf8_lossy(&result.stdout).lines() {
            if let Some((package, _)) = line.split_once(": /") {
                let package = package.split(':').next().context("Library package name")?;
                let notice = PathBuf::from("/usr/share/doc").join(package).join("copyright");
                if notice.is_file() { std::fs::copy(notice, licenses.join(format!("{package}.copyright")))?; }
            }
        }
    }
    let artifact = output.join(format!("leet-{platform}-v{version}.AppImage"));
    let status = Command::new(appimagetool).arg("--appimage-extract-and-run")
        .arg("--no-appstream").arg("--runtime-file").arg(runtime)
        .arg(&appdir).arg(&artifact).env("ARCH", if platform == "linux-arm" { "aarch64" } else { "x86_64" }).env("VERSION", version).status()?;
    if !status.success() { bail!("AppImage creation failed: {status}"); }
    let probe = Command::new(&artifact).args(["--appimage-extract-and-run", "--version"]).output()?;
    if !probe.status.success() || !String::from_utf8_lossy(&probe.stdout).starts_with("leet ") {
        bail!("Packaged AppImage failed its executable check");
    }
    std::fs::remove_dir_all(appdir)?;
    std::fs::remove_file(desktop)?;
    println!("{}", json!({"environment":"release-artifact", "artifact":artifact, "bytes":std::fs::metadata(&artifact)?.len()}));
    Ok(())
}
