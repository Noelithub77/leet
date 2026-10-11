//! Immediate document declarations while the language server is catching up.
use gpui_kit::component::highlighter::SyntaxHighlighter;
use lsp_types::{CompletionItem, CompletionItemKind};
use practice::language::Language;
use ropey::Rope;

pub(super) struct Symbols {
    language: Language,
    parser: SyntaxHighlighter,
}
impl Symbols {
    pub fn new(language: Language) -> Self {
        Self { language, parser: SyntaxHighlighter::new(language.id()) }
    }

    pub fn completions(&mut self, language: Language, text: &Rope, source: &str, start: usize) -> Vec<CompletionItem> {
        if self.language != language { *self = Self::new(language); }
        // Member access belongs to the LSP; bare document names are not members.
        let before = source[..start].trim_end();
        if before.ends_with('.') || before.ends_with("->") || before.ends_with("::") { return vec![]; }
        if !self.parser.update(None, text, Some(std::time::Duration::from_millis(3))) { return vec![]; }
        let Some(tree) = self.parser.tree() else { return vec![]; };
        let mut location = tree.root_node().descendant_for_byte_range(start, start);
        while let Some(node) = location {
            if matches!(node.kind(), "comment" | "string" | "string_literal" | "raw_string_literal" | "interpreted_string_literal") { return vec![]; }
            location = node.parent();
        }
        let mut cursor = tree.walk();
        let mut items = Vec::new();
        loop {
            let node = cursor.node();
            if matches!(node.kind(), "identifier" | "field_identifier") && node.start_byte() < start {
                let mut child = node;
                let mut parent = node.parent();
                let mut kind = None;
                while let Some(owner) = parent {
                    let field = |name| (0..owner.child_count() as u32).any(|ix| owner.field_name_for_child(ix) == Some(name) && owner.child(ix) == Some(child));
                    match owner.kind() {
                        "function_definition" | "function_declaration" | "method_declaration" | "method_definition" => {
                            if field("name") { kind = Some(CompletionItemKind::FUNCTION); }
                            break;
                        }
                        "class_definition" | "class_declaration" | "struct_specifier" => {
                            if field("name") { kind = Some(CompletionItemKind::CLASS); }
                            break;
                        }
                        "function_declarator" => {
                            if field("declarator") { kind = Some(CompletionItemKind::FUNCTION); }
                            break;
                        }
                        "assignment" | "augmented_assignment" | "short_var_declaration" | "for_statement" | "range_clause" => {
                            if field("left") { kind = Some(CompletionItemKind::VARIABLE); }
                            break;
                        }
                        "declaration" | "variable_declarator" | "init_declarator" | "parameter_declaration" | "formal_parameter" | "var_spec" | "const_spec" | "enhanced_for_statement" | "for_range_loop" => {
                            if field("name") || field("declarator") { kind = Some(CompletionItemKind::VARIABLE); }
                            break;
                        }
                        "parameters" | "lambda_parameters" => { kind = Some(CompletionItemKind::VARIABLE); break; }
                        "default_parameter" | "typed_parameter" | "typed_default_parameter" => {
                            if field("name") || (owner.kind() == "typed_parameter" && child == owner.named_child(0).unwrap_or(owner)) {
                                kind = Some(CompletionItemKind::VARIABLE);
                            }
                            break;
                        }
                        // Only binding patterns propagate; exclude type annotations,
                        // default expressions, attributes and subscript assignments.
                        "pattern_list" | "tuple_pattern" | "list_pattern" | "list_splat_pattern" | "dictionary_splat_pattern" | "identifier_list" | "expression_list" | "pointer_declarator" | "reference_declarator" | "array_declarator" | "structured_binding_declarator" => {}
                        _ => break,
                    }
                    child = owner;
                    parent = owner.parent();
                }
                if let Some(kind) = kind {
                    let mut owner = node.parent();
                    let mut visible = true;
                    while let Some(scope) = owner {
                        if kind == CompletionItemKind::VARIABLE && matches!(scope.kind(), "class_definition" | "class_declaration" | "struct_specifier")
                            && !(scope.start_byte()..scope.end_byte()).contains(&start) { visible = false; }
                        if language != Language::Python && kind == CompletionItemKind::VARIABLE
                            && matches!(scope.kind(), "block" | "compound_statement" | "for_statement" | "enhanced_for_statement" | "for_range_loop")
                            && !(scope.start_byte()..scope.end_byte()).contains(&start) { visible = false; }
                        if matches!(scope.kind(), "function_definition" | "function_declaration" | "method_declaration" | "method_definition" | "lambda") {
                            // Function names remain useful as word completions outside
                            // their body; parameters and locals stay inside their scope.
                            if kind == CompletionItemKind::VARIABLE && !(scope.start_byte()..scope.end_byte()).contains(&start) { visible = false; }
                            break;
                        }
                        owner = scope.parent();
                    }
                    if visible {
                        if let Some(label) = source.get(node.byte_range()) {
                            if !items.iter().any(|item: &CompletionItem| item.label == label) {
                                items.push(CompletionItem { label: label.into(), kind: Some(kind), detail: Some("Defined in this file".into()), ..Default::default() });
                            }
                        }
                    }
                }
            }
            if cursor.goto_first_child() { continue; }
            while !cursor.goto_next_sibling() {
                if !cursor.goto_parent() { return items; }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn names(language: Language, source: &str) -> Vec<String> {
        let start = source.rfind("TARGET").unwrap();
        Symbols::new(language).completions(language, &Rope::from_str(source), source, start).into_iter().map(|item| item.label).collect()
    }
    #[test]
    fn python_bindings_exclude_strings_comments_types_and_other_functions() {
        let source = "def other(secret):\n    hidden = 1\n\ndef trap(height: list[int], limit=100):\n    left, right = 0, len(height)\n    for index, value in enumerate(height):\n        water = value\n    obj.attribute = 1\n    # fake = 1\n    text = 'def pretend(): pass'\n    TARGET\n";
        let found = names(Language::Python, source);
        for name in ["trap", "height", "limit", "left", "right", "index", "value", "water", "text"] { assert!(found.contains(&name.into()), "Missing {name}: {found:?}"); }
        for name in ["secret", "hidden", "list", "int", "attribute", "fake", "pretend", "enumerate"] { assert!(!found.contains(&name.into()), "False declaration {name}: {found:?}"); }
    }
    #[test]
    fn solution_method_is_available_outside_class_without_leaking_locals() {
        let found = names(Language::Python, "class Solution:\n    def trap(self, height):\n        water = 0\n        return water\n\nTARGET\n");
        assert!(found.contains(&"trap".into()));
        assert!(!found.contains(&"height".into()));
        assert!(!found.contains(&"water".into()));
    }
    #[test]
    fn declarations_work_in_other_practice_languages() {
        for (language, source) in [
            (Language::Cpp, "int trap(int height) { int water = 0; TARGET; }"),
            (Language::C, "int trap(int height) { int water = 0; TARGET; }"),
            (Language::Java, "class Solution { int trap(int height) { int water = 0; TARGET; } }"),
            (Language::Go, "package main\nfunc trap(height int) int { water := 0; TARGET; return water }"),
        ] {
            let found = names(language, source);
            for name in ["trap", "height", "water"] { assert!(found.contains(&name.into()), "{language:?} missing {name}: {found:?}"); }
        }
    }
    #[test]
    fn member_access_does_not_offer_unrelated_bare_names() {
        assert!(names(Language::Python, "water = 0\nobj.TARGET").is_empty());
        assert!(names(Language::Python, "water = 0\n# TARGET").is_empty());
        assert!(names(Language::Python, "water = 0\ntext = 'TARGET'").is_empty());
        assert!(names(Language::Cpp, "int water = 0; ptr->TARGET").is_empty());
    }
    #[test]
    fn block_locals_do_not_leak_and_loop_bindings_are_discovered() {
        let found = names(Language::Cpp, "int trap(int height) { { int hidden = 0; } int water = 0; TARGET; }");
        assert!(found.contains(&"water".into()));
        assert!(!found.contains(&"hidden".into()));
        for (language, source) in [
            (Language::Go, "package main\nfunc trap(height []int) { for index, value := range height { TARGET } }"),
            (Language::Java, "class Solution { void trap(int[] height) { for (int value : height) { TARGET; } } }"),
            (Language::Cpp, "int trap() { for (auto [index, value] : values) { TARGET; } }"),
        ] {
            let found = names(language, source);
            assert!(found.contains(&"value".into()), "{language:?} missing range variable: {found:?}");
        }
    }
}
