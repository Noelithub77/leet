//! Competitive-programming globals shared by execution and language documents.
use crate::language::Language;
pub const PRELUDE: &str = include_str!("python_prelude.py");
const NODES: &str = "\nclass ListNode:\n    def __init__(self, val: int = 0, next: Optional['ListNode'] = None):\n        self.val = val\n        self.next = next\n\nclass TreeNode:\n    def __init__(self, val: int = 0, left: Optional['TreeNode'] = None, right: Optional['TreeNode'] = None):\n        self.val = val\n        self.left = left\n        self.right = right\n";
pub fn language_document(language: Language, text: &str) -> String {
    if language == Language::Python { format!("{text}\n\n{PRELUDE}{NODES}") } else { text.into() }
}
pub fn syntax_diagnostic(language: Language, diagnostic: &lsp_types::Diagnostic, user_lines: u32) -> bool {
    if diagnostic.range.start.line >= user_lines { return false; }
    if language != Language::Python { return diagnostic.severity != Some(lsp_types::DiagnosticSeverity::WARNING); }
    diagnostic.code.is_none() && matches!(diagnostic.severity, None | Some(lsp_types::DiagnosticSeverity::ERROR))
}
pub fn configuration(section: &str) -> serde_json::Value {
    let analysis = serde_json::json!({"typeCheckingMode":"off", "diagnosticMode":"openFilesOnly", "autoSearchPaths":true, "useLibraryCodeForTypes":true, "autoImportCompletions":true});
    match section { "python" | "pyright" | "basedpyright" => serde_json::json!({"analysis":analysis}),
        "python.analysis" | "pyright.analysis" | "basedpyright.analysis" => analysis, _ => serde_json::Value::Null }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn judge_globals_leave_user_coordinates_intact_and_filter_type_noise() {
        let text = "class Solution:\n    def f(self, values: List[int]):\n        return Counter(values)\n";
        let document = language_document(Language::Python, text);
        assert!(document.starts_with(text)); assert!(document.contains("from collections import Counter"));
        assert_eq!(language_document(Language::Go, text), text);
        let mut diagnostic = lsp_types::Diagnostic { severity: Some(lsp_types::DiagnosticSeverity::ERROR), ..Default::default() };
        assert!(syntax_diagnostic(Language::Python, &diagnostic, 4));
        diagnostic.code = Some(lsp_types::NumberOrString::String("reportUnknownVariableType".into()));
        assert!(!syntax_diagnostic(Language::Python, &diagnostic, 4));
        diagnostic.code = None; diagnostic.range.start.line = 5;
        assert!(!syntax_diagnostic(Language::Python, &diagnostic, 4));
    }
}
