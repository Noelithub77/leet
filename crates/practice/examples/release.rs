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
        println!("cargo run -p practice --example release -- --target TARGET --tag vX.Y.Z [--output dist]\n\nPackages target/TARGET/release/leet on its native OS. Produces a Unix archive or standalone Windows exe. No user data is accessed.");
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
    let (os, arch) = match target {
        "x86_64-unknown-linux-gnu" => ("linux", "x86_64"),
        "aarch64-unknown-linux-gnu" => ("linux", "aarch64"),
        "x86_64-apple-darwin" => ("macos", "x86_64"),
        "aarch64-apple-darwin" => ("macos", "aarch64"),
        "x86_64-pc-windows-msvc" => ("windows", "x86_64"),
        _ => bail!("Unsupported target {target}"),
    };
    if os != std::env::consts::OS { bail!("Package {target} on its native OS"); }
    let binary = PathBuf::from("target").join(target).join("release").join(if os == "windows" { "leet.exe" } else { "leet" });
    if !binary.is_file() { bail!("Build {} first", binary.display()); }
    std::fs::create_dir_all(&output)?;
    let name = format!("leet-{os}-{arch}");
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
    Ok(())
}
