//! Where Solutions live: `<workspace>/leetcode/<id>-<slug>.py`.

use std::path::{Path, PathBuf};

use anyhow::Result;

/// Whether the buffer contains an attempt beyond the supplied language template.
pub fn has_attempt(code: &str, starter: &str) -> bool {
    let code = code.trim();
    !code.is_empty() && code != starter.trim()
}

/// Workspace-relative path of a problem's Solution.
pub fn solution_rel(frontend_id: u32, slug: &str, language: crate::language::Language) -> PathBuf {
    if let Ok((contest, index)) = crate::codeforces::problem_id(slug) {
        return Path::new("codeforces").join(format!("{contest}{index}")).join(format!("main.{}", language.extension()));
    }
    if let Ok(code) = crate::codechef::problem_code(slug) {
        return Path::new("codechef").join(code).join(format!("main.{}", language.extension()));
    }
    Path::new("leetcode").join(format!("{frontend_id:04}-{slug}.{}", language.extension()))
}

/// Creates the Solution from the starter code unless it already exists.
pub fn ensure_solution(workspace: &Path, rel: &Path, starter: &str) -> Result<PathBuf> {
    let path = workspace.join(rel);
    if !path.exists() {
        std::fs::create_dir_all(path.parent().unwrap_or(workspace))?;
        std::fs::write(&path, format!("{}\n", starter.trim_end()))?;
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::language::Language;
    #[test]
    fn templates_are_not_attempts_in_every_language() {
        for language in Language::ALL {
            let starter = language.stdin_template();
            assert!(!has_attempt(starter, starter), "{}", language.label());
            assert!(!has_attempt(&format!("\n{starter}\n"), starter));
            assert!(!has_attempt(" \n\t", starter));
            // Even malformed edits must reach the runner's normal diagnostics.
            assert!(has_attempt(&format!("{starter}broken code"), starter));
        }
        let starter = "class Solution:\n    def twoSum(self, nums, target):";
        assert!(!has_attempt(starter, starter));
        assert!(has_attempt(&format!("{starter}\n    return []"), starter));
    }

    #[test]
    fn language_solutions_have_separate_paths_and_keep_edits() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(solution_rel(0, "cc:START01", Language::Python), Path::new("codechef/START01/main.py"));
        let python = solution_rel(1, "two-sum", Language::Python);
        assert_eq!(python.to_str().unwrap(), "leetcode/0001-two-sum.py");
        let path = ensure_solution(dir.path(), &python, "starter").unwrap();
        std::fs::write(&path, "my solution").unwrap();
        ensure_solution(dir.path(), &python, "replacement").unwrap();
        assert_eq!(std::fs::read_to_string(path).unwrap(), "my solution");
        for language in [Language::Cpp, Language::Go, Language::C, Language::Java] {
            let rel = solution_rel(1, "two-sum", language);
            assert_ne!(rel, python);
            assert_eq!(rel.extension().unwrap(), language.extension());
        }
    }
}
