//! Solution history: the workspace is a git repository and each Accepted submission is a commit.

use std::path::Path;

use anyhow::{Context, Result};
use git2::{DiffOptions, IndexAddOption, Oid, Repository, Signature};

#[derive(Clone, Debug)]
pub struct Commit {
    pub id: String,
    pub summary: String,
    /// Seconds since the Unix epoch.
    pub time: i64,
}

pub fn ensure_repo(workspace: &Path) -> Result<Repository> {
    std::fs::create_dir_all(workspace)?;
    if let Ok(repo) = Repository::open(workspace) {
        return Ok(repo);
    }
    let repo = Repository::init(workspace)?;
    let ignore = workspace.join(".gitignore");
    if !ignore.exists() {
        std::fs::write(ignore, "__pycache__/\n*.pyc\n.leet/\n.vg/\n")?;
    }
    Ok(repo)
}

fn signature(repo: &Repository) -> Result<Signature<'static>> {
    Ok(repo.signature().or_else(|_| Signature::now("leet", "leet@localhost"))?)
}

/// Persist the draft in the index before a historical version replaces the worktree.
pub fn stage(workspace: &Path, file: &Path, code: &str) -> Result<()> {
    anyhow::ensure!(file.components().all(|part| matches!(part, std::path::Component::Normal(_))), "Invalid solution path");
    let repo = ensure_repo(workspace)?;
    std::fs::write(workspace.join(file), code)?;
    let mut index = repo.index()?; index.add_path(file)?; index.write()?; Ok(())
}

pub fn staged_code(workspace: &Path, file: &Path) -> Result<Option<String>> {
    let repo = ensure_repo(workspace)?; let index = repo.index()?;
    let Some(entry) = index.get_path(file, 0) else { return Ok(None); };
    let head = repo.head().ok().and_then(|head| head.peel_to_tree().ok()).and_then(|tree| tree.get_path(file).ok()).map(|entry| entry.id());
    if head == Some(entry.id) { return Ok(None); }
    Ok(Some(String::from_utf8(repo.find_blob(entry.id)?.content().to_vec())?))
}

/// Commits `files` (workspace-relative) if they changed. Returns the new commit id.
pub fn commit(workspace: &Path, files: &[&Path], message: &str) -> Result<Option<String>> {
    let repo = ensure_repo(workspace)?;
    let mut index = repo.index()?;
    index.add_all(files.iter().chain([&Path::new(".gitignore")]), IndexAddOption::DEFAULT, None)?;
    index.write()?;
    let parent = repo.head().ok().and_then(|h| h.peel_to_commit().ok());
    let mut committed = git2::Index::new()?;
    if let Some(parent) = &parent { committed.read_tree(&parent.tree()?)?; }
    for file in files.iter().copied().chain([Path::new(".gitignore")]) {
        anyhow::ensure!(file.components().all(|part| matches!(part, std::path::Component::Normal(_))), "Invalid solution path");
        if let Some(entry) = index.get_path(file, 0) { committed.add(&entry)?; }
    }
    let tree_id = committed.write_tree_to(&repo)?;
    if parent.as_ref().is_some_and(|p| p.tree_id() == tree_id) { return Ok(None); }
    let tree = repo.find_tree(tree_id)?;
    let sig = signature(&repo)?;
    let parents: Vec<_> = parent.iter().collect();
    let id = repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &parents)?;
    Ok(Some(id.to_string()))
}

/// Commits that changed `file`, newest first.
pub fn history(workspace: &Path, file: &Path) -> Result<Vec<Commit>> {
    let Ok(repo) = Repository::open(workspace) else { return Ok(vec![]) };
    let Ok(head) = repo.head() else { return Ok(vec![]) };
    let mut walk = repo.revwalk()?;
    walk.push(head.target().context("detached head")?)?;
    let mut out = vec![];
    for oid in walk {
        let commit = repo.find_commit(oid?)?;
        let blob = |c: &git2::Commit| c.tree().ok()?.get_path(file).ok().map(|e| e.id());
        let current = blob(&commit);
        let previous = commit.parent(0).ok().and_then(|p| blob(&p));
        if current.is_some() && current != previous {
            out.push(Commit {
                id: commit.id().to_string(),
                summary: commit.summary().unwrap_or_default().to_owned(),
                time: commit.time().seconds(),
            });
        }
    }
    Ok(out)
}

pub fn file_at(workspace: &Path, commit: &str, file: &Path) -> Result<String> {
    let repo = Repository::open(workspace)?;
    let commit = repo.find_commit(Oid::from_str(commit)?)?;
    let entry = commit.tree()?.get_path(file)?;
    let blob = repo.find_blob(entry.id())?;
    Ok(String::from_utf8_lossy(blob.content()).into_owned())
}

/// Unified diff of `file` between `commit` and its parent.
pub fn diff(workspace: &Path, commit: &str, file: &Path) -> Result<String> {
    let repo = Repository::open(workspace)?;
    let commit = repo.find_commit(Oid::from_str(commit)?)?;
    let old = commit.parent(0).ok().map(|p| p.tree()).transpose()?;
    let mut opts = DiffOptions::new();
    opts.pathspec(file);
    let diff = repo.diff_tree_to_tree(old.as_ref(), Some(&commit.tree()?), Some(&mut opts))?;
    let mut out = String::new();
    diff.print(git2::DiffFormat::Patch, |_, _, line| {
        if matches!(line.origin(), '+' | '-' | ' ') {
            out.push(line.origin());
        }
        out.push_str(&String::from_utf8_lossy(line.content()));
        true
    })?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restoring_a_version_preserves_the_staged_draft_and_other_question_staging() {
        let dir = tempfile::tempdir().unwrap(); let workspace = dir.path();
        let file = Path::new("two-sum.py"); std::fs::write(workspace.join(file), "original").unwrap();
        let original = commit(workspace, &[file], "Original").unwrap().unwrap();
        stage(workspace, Path::new("other.py"), "other draft").unwrap();
        stage(workspace, file, "current draft").unwrap();
        std::fs::write(workspace.join(file), file_at(workspace, &original, file).unwrap()).unwrap();
        assert_eq!(staged_code(workspace, file).unwrap().as_deref(), Some("current draft"));
        std::fs::write(workspace.join(file), "accepted version").unwrap();
        let accepted = commit(workspace, &[file], "Accepted").unwrap().unwrap();
        let repo = Repository::open(workspace).unwrap();
        assert!(repo.find_commit(Oid::from_str(&accepted).unwrap()).unwrap().tree().unwrap().get_path(Path::new("other.py")).is_err());
        assert_eq!(staged_code(workspace, Path::new("other.py")).unwrap().as_deref(), Some("other draft"));
    }

    #[test]
    fn commits_only_changes_and_tracks_file_history() {
        let dir = tempfile::tempdir().unwrap();
        let ws = dir.path();
        let file = Path::new("two-sum.py");
        std::fs::write(ws.join(file), "v1\n").unwrap();
        let first = commit(ws, &[file], "two-sum: Accepted").unwrap().unwrap();
        assert!(commit(ws, &[file], "same").unwrap().is_none());
        std::fs::write(ws.join("other.py"), "x").unwrap();
        commit(ws, &[Path::new("other.py")], "other").unwrap();
        std::fs::write(ws.join(file), "v2\n").unwrap();
        commit(ws, &[file], "two-sum: Accepted again").unwrap();

        let log = history(ws, file).unwrap();
        assert_eq!(log.len(), 2);
        assert_eq!(log[1].id, first);
        assert_eq!(file_at(ws, &first, file).unwrap(), "v1\n");
        let patch = diff(ws, &log[0].id, file).unwrap();
        assert!(patch.contains("-v1") && patch.contains("+v2"), "{patch}");
    }
}
