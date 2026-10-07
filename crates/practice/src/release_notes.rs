//! The small, classified changelog shared by GitHub releases and the desktop.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Changelog {
    pub feat: Vec<String>,
    pub fix: Vec<String>,
}

impl Changelog {
    pub fn parse(notes: &str) -> Self {
        let mut result = Self::default();
        let mut section = None;
        for line in notes.lines() {
            let text = line.trim();
            match text {
                "<summary>Feat</summary>" | "## Feat" => { section = Some(true); continue; }
                "<summary>Fix</summary>" | "## Fix" => { section = Some(false); continue; }
                "</details>" => { section = None; continue; }
                _ if text.starts_with('#') => { section = None; continue; }
                _ => {}
            }
            let Some(feat) = section else { continue; };
            let list = if feat { &mut result.feat } else { &mut result.fix };
            if let Some(item) = text.strip_prefix("- ").filter(|item| !item.trim().is_empty()) {
                list.push(item.trim().to_owned());
            } else if line.starts_with("  ") && !text.is_empty() {
                if let Some(item) = list.last_mut() { item.push(' '); item.push_str(text); }
            }
        }
        result
    }

    pub fn is_empty(&self) -> bool { self.feat.is_empty() && self.fix.is_empty() }
}

#[cfg(test)]
mod tests {
    use super::Changelog;

    #[test]
    fn published_details_classify_bullets_without_download_instructions() {
        let notes = "<details open>\n<summary>Feat</summary>\n\n- Switch languages.\n  Keep each saved solution.\n</details>\n<details>\n<summary>Fix</summary>\n\n- Restart local builds.\n</details>\n## Downloads\n- Install command.\n";
        let parsed = Changelog::parse(notes);
        assert_eq!(parsed.feat, ["Switch languages. Keep each saved solution."]);
        assert_eq!(parsed.fix, ["Restart local builds."]);
        assert!(Changelog::parse("Native downloads and installation instructions.").is_empty());
        assert_eq!(Changelog::parse("## Feat\n- New feature.\n## Fix\n- Fixed bug."), Changelog { feat: vec!["New feature.".into()], fix: vec!["Fixed bug.".into()] });
    }

    #[test]
    fn current_changelog_has_required_release_categories() {
        let notes = include_str!("../../../docs/changelog.md");
        assert!(notes.starts_with("<details open>\n<summary>Feat</summary>"));
        assert!(notes.contains("<details>\n<summary>Fix</summary>"));
        assert_eq!(notes.matches("<details open>").count(), 1);
        let parsed = Changelog::parse(notes);
        assert!(!parsed.feat.is_empty());
        assert!(!parsed.fix.is_empty());
    }
}
