//! Combine local snippets and language-server results in the editor's completion menu.
use std::{cell::{Cell, RefCell}, ops::Range, rc::Rc};
use anyhow::Result;
use gpui_kit::*;
use gpui_kit::component::{input::CompletionProvider, RopeExt as _};
use lsp_types::{CompletionItem, CompletionItemKind, CompletionContext, CompletionResponse, CompletionTextEdit, Documentation, InsertTextFormat, MarkupContent, MarkupKind, TextEdit};
use practice::{language::Language, snippets::{Snippet, body}};
use ropey::Rope;
pub(super) struct Catalog {
    pub library: Vec<Snippet>,
    pub language: Language,
    pub path: Option<std::path::PathBuf>,
    pub workspace: Option<std::path::PathBuf>,
    pub selected: String,
    pub selection: Range<usize>,
}

pub(super) struct Pending {
    source: String,
    candidates: Vec<(CompletionItem, Range<usize>, body::Expansion)>,
}
impl Pending {
    pub fn accepted(self, item: &CompletionItem, document: &str) -> Option<(usize, body::Expansion)> {
        if self.source != document { return None; }
        self.candidates.into_iter().find_map(|(candidate, range, expansion)| (candidate == *item).then_some((range.start, expansion)))
    }
}

pub(super) struct Completions {
    pub predictions: RefCell<Option<Rc<crate::copilot::Predictions>>>,
    pub catalog: RefCell<Catalog>,
    pub upstream: RefCell<Option<Rc<dyn CompletionProvider>>>,
    pub pending: Rc<RefCell<Option<Pending>>>,
    generation: Rc<Cell<u64>>,
}
impl Completions {
    pub fn new(language: Language) -> Self {
        Self { predictions: RefCell::new(None), catalog: RefCell::new(Catalog { library: vec![], language, path: None, workspace: None, selected: String::new(), selection: 0..0 }), upstream: RefCell::new(None), pending: Rc::default(), generation: Rc::default() }
    }
}

impl CompletionProvider for Completions {
    fn inline_completion(&self, text: &Rope, offset: usize, trigger: lsp_types::InlineCompletionContext, window: &mut Window, cx: &mut App) -> Task<Result<lsp_types::InlineCompletionResponse>> {
        if let Some(predictions) = self.predictions.borrow().as_ref() {
            if !predictions.alt.get() { return Task::ready(Ok(lsp_types::InlineCompletionResponse::Array(vec![]))); }
            return predictions.fetch(text.to_string(), offset, cx);
        }
        self.upstream.borrow().as_ref().map_or_else(
            || Task::ready(Ok(lsp_types::InlineCompletionResponse::Array(vec![]))),
            |provider| provider.inline_completion(text, offset, trigger, window, cx),
        )
    }

    fn inline_completion_debounce(&self) -> std::time::Duration {
        if self.predictions.borrow().is_some() { return std::time::Duration::from_millis(150); }
        self.upstream.borrow().as_ref().map_or(std::time::Duration::from_millis(300), |provider| provider.inline_completion_debounce())
    }

    fn completions(&self, text: &Rope, offset: usize, trigger: CompletionContext, window: &mut Window, cx: &mut App) -> Task<Result<CompletionResponse>> {
        let catalog = self.catalog.borrow();
        let language = catalog.language;
        let source = text.to_string();
        if offset > source.len() || !source.is_char_boundary(offset) { return Task::ready(Ok(CompletionResponse::Array(vec![]))); }
        let mut start = source[..offset].char_indices().rev().find(|(_, ch)| !ch.is_alphanumeric() && *ch != '_').map_or(0, |(i, ch)| i + ch.len_utf8());
        for prefix_key in catalog.library.iter().filter(|s| s.applies_to(language)).flat_map(|s| &s.prefixes)
            .filter(|prefix| prefix.chars().next().is_some_and(|ch| !ch.is_alphanumeric() && ch != '_')) {
            for end in prefix_key.char_indices().map(|(i, ch)| i + ch.len_utf8()) {
                if source[..offset].ends_with(&prefix_key[..end]) { start = start.min(offset - end); }
            }
        }
        let selection = (trigger.trigger_kind == lsp_types::CompletionTriggerKind::INVOKED && !catalog.selection.is_empty()
            && catalog.selection.end <= source.len() && source.is_char_boundary(catalog.selection.start) && source.is_char_boundary(catalog.selection.end))
            .then(|| catalog.selection.clone());
        let replacement = selection.clone().unwrap_or(start..offset);
        if let Some(selection) = &selection { start = selection.start; }
        let prefix = if selection.is_some() { "" } else { &source[start..offset] };
        let line_start = source[..start].rfind('\n').map_or(0, |i| i + 1);
        let line = source[line_start..].split('\n').next().unwrap_or_default();
        let context = body::Context {
            path: catalog.path.clone(), workspace: catalog.workspace.clone(), language: Some(language),
            selected: catalog.selected.clone(),
            clipboard: cx.read_from_clipboard().and_then(|c| c.text()).unwrap_or_default(),
            line: line.into(), line_index: source[..line_start].bytes().filter(|b| *b == b'\n').count(),
            word: prefix.into(), indent: line.chars().take_while(|c| c.is_whitespace()).collect(), unit: "    ".into(), ..Default::default()
        };
        let mut items = vec![];
        for snippet in catalog.library.iter().filter(|s| s.applies_to(language)) {
            for prefix_key in &snippet.prefixes {
                if !prefix_key.to_lowercase().starts_with(&prefix.to_lowercase()) { continue; }
                items.push(CompletionItem {
                    label: prefix_key.clone(), kind: Some(CompletionItemKind::SNIPPET), detail: Some(snippet.name.clone()),
                    filter_text: Some(prefix_key.clone()), sort_text: Some(format!("0-{prefix_key}")),
                    insert_text: Some(snippet.body.clone()), insert_text_format: Some(InsertTextFormat::SNIPPET), ..Default::default()
                });
                let mut description = snippet.description.clone();
                if snippet.origin == practice::snippets::Origin::Builtin {
                    if let Some(guide) = practice::snippets::guide::guide(prefix_key) {
                        description.push_str(&format!("\nInput · {}\nOutput · {}\nExample · {}", guide.input, guide.output, guide.example));
                    }
                }
                items.last_mut().unwrap().documentation = Some(Documentation::String(description));
            }
        }
        let upstream = self.upstream.borrow().as_ref().map(|p| p.completions(text, offset, trigger, window, cx));
        let text = text.clone(); let pending = self.pending.clone();
        let generation = self.generation.clone(); let request = generation.get().wrapping_add(1); generation.set(request);
        cx.spawn(async move |_| {
            if let Some(task) = upstream {
                if let Ok(response) = task.await {
                    let more = match response { CompletionResponse::Array(items) => items, CompletionResponse::List(list) => list.items };
                    items.extend(more);
                }
            }
            if generation.get() != request { return Ok(CompletionResponse::Array(vec![])); }
            let mut candidates = vec![];
            items.retain_mut(|item| {
                if item.insert_text_format != Some(InsertTextFormat::SNIPPET) {
                    if item.text_edit.is_none() {
                        if let Some(insert_text) = &item.insert_text {
                            item.text_edit = Some(CompletionTextEdit::Edit(TextEdit { range: lsp_types::Range {
                                start: text.offset_to_position(replacement.start), end: text.offset_to_position(replacement.end)
                            }, new_text: insert_text.clone() }));
                        }
                    }
                    return true;
                }
                let (range, snippet_body) = match &item.text_edit {
                    Some(CompletionTextEdit::Edit(edit)) => (text.position_to_offset(&edit.range.start)..text.position_to_offset(&edit.range.end), edit.new_text.clone()),
                    Some(CompletionTextEdit::InsertAndReplace(edit)) => (text.position_to_offset(&edit.replace.start)..text.position_to_offset(&edit.replace.end), edit.new_text.clone()),
                    None => (replacement.clone(), item.insert_text.clone().unwrap_or_else(|| item.label.clone())),
                };
                if range.start > range.end || range.end > source.len() || !source.is_char_boundary(range.start) || !source.is_char_boundary(range.end) { return false; }
                let expansion = body::expand(&snippet_body, &context);
                if expansion.truncated { return false; }
                item.kind = Some(CompletionItemKind::SNIPPET);
                let description = item.documentation.as_ref().map(|doc| match doc {
                    Documentation::String(text) => text.as_str(), Documentation::MarkupContent(markup) => markup.value.as_str()
                }).unwrap_or("");
                item.documentation = Some(Documentation::MarkupContent(MarkupContent { kind: MarkupKind::Markdown,
                    value: format!("{description}\n\n```{}\n{}\n```", language.id(), expansion.text) }));
                item.text_edit = Some(CompletionTextEdit::Edit(TextEdit { range: lsp_types::Range {
                    start: text.offset_to_position(range.start), end: text.offset_to_position(range.end)
                }, new_text: expansion.text.clone() }));
                item.insert_text_format = Some(InsertTextFormat::PLAIN_TEXT);
                candidates.push((item.clone(), range, expansion)); true
            });
            *pending.borrow_mut() = Some(Pending { source, candidates });
            Ok(CompletionResponse::Array(items))
        })
    }

    fn is_completion_trigger(&self, offset: usize, new_text: &str, cx: &mut App) -> bool {
        if self.predictions.borrow().as_ref().is_some_and(|predictions| predictions.alt.get()) { return false; }
        new_text.chars().last().is_some_and(|ch| ch.is_alphanumeric() || ch == '_'
            || self.catalog.borrow().library.iter().flat_map(|s| &s.prefixes).any(|prefix| prefix.starts_with(ch)))
            || self.upstream.borrow().as_ref().is_some_and(|p| p.is_completion_trigger(offset, new_text, cx))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[::core::prelude::v1::test]
    fn accepted_completion_requires_the_selected_item_and_current_document() {
        let item = CompletionItem { label: "loop".into(), ..Default::default() };
        let pending = || Pending { source: "α = loop\n".into(), candidates: vec![(item.clone(), 5..9, body::preview("for ${1:i} in range(n):\n    print($1)$0"))] };
        assert_eq!(pending().accepted(&item, "α = loop\n").unwrap().0, 5);
        assert!(pending().accepted(&item, "α = loopx\n").is_none());
        assert!(pending().accepted(&CompletionItem { label: "local_value".into(), ..Default::default() }, "α = loop\n").is_none());
    }
}
