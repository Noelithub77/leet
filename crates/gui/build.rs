#![forbid(unsafe_code)]

// Embeds `git describe` so `leet --version` identifies the exact development build.
fn main() {
    let describe = std::process::Command::new("git")
        .args(["describe", "--always", "--dirty", "--tags"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown".into());
    println!("cargo:rustc-env=LEET_VERSION={} ({describe})", env!("CARGO_PKG_VERSION"));
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    println!("cargo:rerun-if-changed=../../.git/index");
}
