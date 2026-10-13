//! Prepare a version commit and start one GitHub release workflow.
use std::{fs, path::Path, process::{Command, Stdio}};
use anyhow::{Context, Result, bail, ensure};
use serde_json::json;

fn capture(root: &Path, program: &str, args: &[&str]) -> Result<String> {
    let output = Command::new(program).args(args).current_dir(root).stdin(Stdio::null()).output()
        .with_context(|| format!("Run {program}; Git and authenticated GitHub CLI are required"))?;
    ensure!(output.status.success(), "{program} failed: {}", String::from_utf8_lossy(&output.stderr).trim());
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}

fn version(value: &str) -> Result<([u64; 3], String)> {
    let value = value.strip_prefix('v').unwrap_or(value);
    let parts = value.split('.').collect::<Vec<_>>();
    ensure!(parts.len() == 3, "Use a stable version such as 0.4.1");
    let mut parsed = [0; 3];
    for (index, part) in parts.iter().enumerate() {
        ensure!(!part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()) && (part.len() == 1 || !part.starts_with('0')), "Use a stable version such as 0.4.1");
        parsed[index] = part.parse()?;
    }
    Ok((parsed, value.to_owned()))
}

fn updated_files(root: &Path, next: &str) -> Result<(String, String)> {
    let manifest = fs::read_to_string(root.join("Cargo.toml"))?;
    let parsed: toml::Value = toml::from_str(&manifest)?;
    let current = parsed.get("workspace").and_then(|value| value.get("package")).and_then(|value| value.get("version")).and_then(toml::Value::as_str).context("Workspace version is missing")?;
    ensure!(version(next)?.0 > version(current)?.0, "Version {next} must be newer than {current}");
    let needle = format!("version = \"{current}\"");
    let section = manifest.find("[workspace.package]").context("Workspace package section is missing")?;
    let offset = manifest[section..].find(&needle).context("Workspace version must use the existing quoted format")? + section;
    let mut updated_manifest = manifest.clone();
    updated_manifest.replace_range(offset..offset + needle.len(), &format!("version = \"{next}\""));
    let mut lock = fs::read_to_string(root.join("Cargo.lock"))?;
    for package in ["gui", "practice"] {
        let needle = format!("name = \"{package}\"\nversion = \"{current}\"");
        ensure!(lock.matches(&needle).count() == 1, "Lockfile version for {package} does not match the workspace");
        lock = lock.replacen(&needle, &format!("name = \"{package}\"\nversion = \"{next}\""), 1);
    }
    Ok((updated_manifest, lock))
}

fn local_preflight(root: &Path, next: &str) -> Result<(String, String)> {
    ensure!(capture(root, "git", &["branch", "--show-current"])? == "main", "Publish from main");
    ensure!(capture(root, "git", &["status", "--porcelain"])?.is_empty(), "Commit pending changes before publishing; no unrelated work will be staged");
    let (manifest, lock) = updated_files(root, next)?;
    let notes = fs::read_to_string(root.join("docs/changelog.md"))?;
    ensure!(notes.contains("<details open>\n<summary>Feat</summary>") && notes.contains("<details>\n<summary>Fix</summary>"), "Prepare categorized release notes in docs/changelog.md");
    let tag = format!("v{next}");
    ensure!(capture(root, "git", &["tag", "--list", &tag])?.is_empty(), "Tag {tag} already exists locally; do not overwrite released history");
    Ok((manifest, lock))
}

pub fn run(args: &[String]) -> Result<()> {
    if args.iter().any(|arg| matches!(arg.as_str(), "--help" | "-h")) {
        println!("./ops release:publish --version X.Y.Z [--dry-run] [--json]\n\nPublic GitHub release from a clean main checkout. Requires Git, authenticated gh, and prepared docs/changelog.md. Updates workspace and lockfile versions, commits only those files, atomically pushes main and a new tag, then starts one Release workflow on main. CI publishes after all platform builds and tests pass. --dry-run performs read-only preflight; it creates no commit, tag, push, or workflow.");
        return Ok(());
    }
    let mut requested = None; let mut dry_run = false; let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--version" if requested.is_none() => { index += 1; requested = Some(args.get(index).context("--version requires X.Y.Z")?.as_str()); }
            "--dry-run" => dry_run = true,
            "--json" => {},
            _ => bail!("Use ./ops release:publish --version X.Y.Z [--dry-run] [--json]"),
        }
        index += 1;
    }
    let (_, next) = version(requested.context("--version is required")?)?;
    let root = std::env::current_dir()?;
    let (manifest, lock) = local_preflight(&root, &next)?;
    let tag = format!("v{next}");
    ensure!(capture(&root, "git", &["ls-remote", "--tags", "origin", &format!("refs/tags/{tag}")])?.is_empty(), "Tag {tag} already exists on origin");
    let remote = capture(&root, "git", &["remote", "get-url", "origin"])?;
    let repository = capture(&root, "gh", &["repo", "view", &remote, "--json", "nameWithOwner", "--jq", ".nameWithOwner"])?;
    let remote_main = capture(&root, "git", &["ls-remote", "origin", "refs/heads/main"])?;
    let remote_sha = remote_main.split_whitespace().next().context("Origin main is missing")?;
    capture(&root, "git", &["merge-base", "--is-ancestor", remote_sha, "HEAD"]).context("Local main must contain origin main; fetch and integrate remote changes first")?;
    if dry_run {
        println!("{}", json!({"command":"release:publish","environment":"github","repository":repository,"remote":remote,"tag":tag,"dry_run":true,"workflow":"release.yml","operations":["update versions","commit version files","atomic push main and tag","dispatch one release workflow"]}));
        return Ok(());
    }
    fs::write(root.join("Cargo.toml"), manifest)?; fs::write(root.join("Cargo.lock"), lock)?;
    capture(&root, "git", &["add", "--", "Cargo.toml", "Cargo.lock"])?;
    capture(&root, "git", &["commit", "--only", "-m", &format!("Release {tag}"), "-m", "Set the workspace and lockfile versions for the prepared release. Native builds, tests, packaging, and publication run once in GitHub Actions.", "--", "Cargo.toml", "Cargo.lock"])?;
    let revision = capture(&root, "git", &["rev-parse", "HEAD"])?;
    capture(&root, "git", &["tag", "-a", &tag, "-m", &format!("leet {tag}")])?;
    capture(&root, "git", &["push", "--atomic", "origin", "HEAD:refs/heads/main", &format!("refs/tags/{tag}")]).context("Version commit and tag remain local; retry the atomic push after resolving the error")?;
    capture(&root, "gh", &["workflow", "run", "release.yml", "--repo", &repository, "--ref", "main", "-f", &format!("tag={tag}"), "-f", "publish=true"]).with_context(|| format!("Tag was pushed but workflow dispatch failed. Retry: gh workflow run release.yml --repo {repository} --ref main -f tag={tag} -f publish=true"))?;
    println!("{}", json!({"command":"release:publish","environment":"github","repository":repository,"tag":tag,"revision":revision,"status":"queued","workflow":"release.yml","workflow_url":format!("https://github.com/{repository}/actions/workflows/release.yml"),"release_url":format!("https://github.com/{repository}/releases/tag/{tag}")}));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preflight_rejects_pending_work_other_branches_and_existing_tags() {
        let dir = tempfile::tempdir().unwrap(); let root = dir.path();
        capture(root, "git", &["init", "--initial-branch=main"]).unwrap();
        capture(root, "git", &["config", "user.name", "Release fixture"]).unwrap();
        capture(root, "git", &["config", "user.email", "fixture@example.invalid"]).unwrap();
        fs::create_dir(root.join("docs")).unwrap();
        fs::write(root.join("docs/changelog.md"), "<details open>\n<summary>Feat</summary>\n</details>\n<details>\n<summary>Fix</summary>\n</details>").unwrap();
        fs::write(root.join("Cargo.toml"), "[workspace.package]\nversion = \"0.4.0\"\n").unwrap();
        fs::write(root.join("Cargo.lock"), "[[package]]\nname = \"gui\"\nversion = \"0.4.0\"\n[[package]]\nname = \"practice\"\nversion = \"0.4.0\"\n").unwrap();
        capture(root, "git", &["add", "."]).unwrap(); capture(root, "git", &["commit", "-m", "Fixture"]).unwrap();
        assert!(local_preflight(root, "0.4.1").is_ok());
        assert!(capture(root, "git", &["status", "--porcelain"]).unwrap().is_empty());
        fs::write(root.join("concurrent.txt"), "preserve this work").unwrap();
        assert!(local_preflight(root, "0.4.1").unwrap_err().to_string().contains("pending changes"));
        fs::remove_file(root.join("concurrent.txt")).unwrap();
        capture(root, "git", &["checkout", "-b", "feature"]).unwrap();
        assert!(local_preflight(root, "0.4.1").unwrap_err().to_string().contains("main"));
        capture(root, "git", &["checkout", "main"]).unwrap(); capture(root, "git", &["tag", "v0.4.1"]).unwrap();
        assert!(local_preflight(root, "0.4.1").unwrap_err().to_string().contains("already exists"));
    }
    #[test]
    fn stable_versions_reject_shell_text_and_noncanonical_tags() {
        assert_eq!(version("v0.4.1").unwrap().0, [0, 4, 1]);
        for invalid in ["0.4", "0.4.1;echo secret", "0.4.1-beta", "00.4.1", "-1.2.3", "1.2.3.4", "1.2."] { assert!(version(invalid).is_err(), "{invalid}"); }
    }
    #[test]
    fn version_bump_preserves_dependencies_and_rejects_reuse() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("Cargo.toml"), "[workspace.package]\nversion = \"0.4.0\"\n[workspace.dependencies]\nother = \"0.4.0\"\n").unwrap();
        fs::write(dir.path().join("Cargo.lock"), "[[package]]\nname = \"gui\"\nversion = \"0.4.0\"\n[[package]]\nname = \"practice\"\nversion = \"0.4.0\"\n[[package]]\nname = \"other\"\nversion = \"0.4.0\"\n").unwrap();
        let (manifest, lock) = updated_files(dir.path(), "0.4.1").unwrap();
        assert!(manifest.contains("version = \"0.4.1\""));
        assert!(manifest.contains("other = \"0.4.0\""));
        assert!(lock.contains("name = \"other\"\nversion = \"0.4.0\""));
        assert_eq!(lock.matches("version = \"0.4.1\"").count(), 2);
        assert!(updated_files(dir.path(), "0.4.0").is_err());
        assert!(updated_files(dir.path(), "0.3.9").is_err());
    }
}
