//! Combine local snippets and language-server results in the editor's completion menu.
use std::{cell::{Cell, RefCell}, ops::Range, rc::Rc};
use anyhow::Result;
use gpui_kit::*;
use gpui_kit::component::{input::CompletionProvider, RopeExt as _};
use lsp_types::{CompletionItem, CompletionItemKind, CompletionContext, CompletionResponse, CompletionTextEdit, Documentation, InsertTextFormat, TextEdit};
use practice::{language::Language, snippets::search::Library};
use ropey::Rope;
pub(super) struct Catalog {
    pub library: Rc<Library>,
    pub language: Language,
    pub selection: Range<usize>,
}

pub(super) struct Pending {
    source: String,
    candidates: Vec<(CompletionItem, Range<usize>, String)>,
}
impl Pending {
    pub fn accepted(self, item: &CompletionItem, document: &str) -> Option<(Range<usize>, String)> {
        if self.source != document { return None; }
        self.candidates.into_iter().find_map(|(candidate, range, expansion)| (candidate == *item).then_some((range, expansion)))
    }
}

struct CachedServerResults {
    source: String,
    range: Range<usize>,
    query: String,
    language: Language,
    items: Vec<CompletionItem>,
    index: practice::search::Index,
}
impl CachedServerResults {
    fn new(source: String, range: Range<usize>, query: String, language: Language, items: Vec<CompletionItem>, text: &Rope) -> Self {
        let old_range = lsp_types::Range { start: text.offset_to_position(range.start), end: text.offset_to_position(range.end) };
        let items: Vec<_> = items.into_iter().filter(|item| item.additional_text_edits.as_ref().is_none_or(Vec::is_empty)
            && item.insert_text_format != Some(InsertTextFormat::SNIPPET)
            && match &item.text_edit { None => true, Some(CompletionTextEdit::Edit(edit)) => edit.range == old_range, _ => false }).collect();
        let index = practice::search::Index::new(items.iter().map(|item| format!("{} {} {}", item.label, item.filter_text.as_deref().unwrap_or(""), item.detail.as_deref().unwrap_or(""))));
        Self { source, range, query, language, items, index }
    }
    fn matches(&self, source: &str, range: &Range<usize>, query: &str, language: Language) -> bool {
        self.language == language && query.starts_with(&self.query)
            && source[..range.start] == self.source[..self.range.start]
            && source[range.end..] == self.source[self.range.end..]
    }
}

pub(super) struct Completions {
    pub predictions: RefCell<Option<Rc<crate::copilot::Predictions>>>,
    pub catalog: RefCell<Catalog>,
    pub upstream: RefCell<Option<Rc<dyn CompletionProvider>>>,
    pub pending: Rc<RefCell<Option<Pending>>>,
    pub published: Rc<RefCell<Option<(String, usize, Vec<CompletionItem>)>>>,
    generation: Rc<Cell<u64>>,
    in_flight: Rc<Cell<bool>>,
    pub pane: WeakEntity<super::expansion::EditorPane>,
    searcher: RefCell<practice::search::Searcher>,
    task: RefCell<Option<Task<()>>>,
    cache: RefCell<Option<CachedServerResults>>,
    symbols: RefCell<super::symbols::Symbols>,
}
impl Completions {
    pub fn new(language: Language, pane: WeakEntity<super::expansion::EditorPane>, library: Rc<Library>) -> Self {
        Self { predictions: RefCell::new(None), catalog: RefCell::new(Catalog { library, language, selection: 0..0 }), upstream: RefCell::new(None), pending: Rc::default(), published: Rc::default(), generation: Rc::default(), in_flight: Rc::default(), pane, searcher: RefCell::new(practice::search::Searcher::autocomplete()), task: RefCell::default(), cache: RefCell::default(), symbols: RefCell::new(super::symbols::Symbols::new(language)) }
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
        let query = prefix.to_owned();
        let mut items = vec![];
        let declarations = if selection.is_none() { self.symbols.borrow_mut().completions(language, text, &source, start) } else { vec![] };
        let defined: Vec<_> = declarations.iter().map(|item| item.label.clone()).collect();
        let index = practice::search::Index::new(declarations.iter().map(|item| &item.label));
        for hit in self.searcher.borrow_mut().rank(&index, &query, 64, |_| true) { items.push(declarations[hit.index].clone()); }
        for row in catalog.library.rank(&mut self.searcher.borrow_mut(), prefix, language, 64) {
            let (snippet, prefix_key) = catalog.library.row(row);
            items.push(CompletionItem {
                label: prefix_key.into(), kind: Some(CompletionItemKind::SNIPPET), detail: Some(snippet.name.clone()),
                filter_text: Some(prefix_key.into()), sort_text: Some(format!("0-{row:06}")),
                insert_text: Some(snippet.body.clone()), insert_text_format: Some(InsertTextFormat::SNIPPET),
                documentation: Some(Documentation::String(catalog.library.description(row).into())), ..Default::default()
            });
        }
        if selection.is_none() {
            if let Some(cache) = self.cache.borrow().as_ref().filter(|cache| cache.matches(&source, &replacement, &query, language)) {
                for hit in self.searcher.borrow_mut().rank(&cache.index, &query, cache.items.len(), |_| true) {
                    let mut item = cache.items[hit.index].clone();
                    if items.iter().any(|local| local.label == item.label && local.kind != Some(CompletionItemKind::SNIPPET)) {
                        items.retain(|local| local.label != item.label || local.kind == Some(CompletionItemKind::SNIPPET));
                    }
                    if let Some(CompletionTextEdit::Edit(edit)) = &mut item.text_edit {
                        edit.range = lsp_types::Range { start: text.offset_to_position(replacement.start), end: text.offset_to_position(replacement.end) };
                    }
                    items.push(item);
                }
            }
        }
        rank_items(&mut items, &query, &defined);
        items.truncate(128);
        let generation = self.generation.clone(); let request = generation.get().wrapping_add(1); generation.set(request);
        self.task.borrow_mut().take();
        self.in_flight.set(false);
        self.published.borrow_mut().take();
        let library = catalog.library.clone();
        drop(catalog);
        let mut candidates = vec![];
        prepare_items(&mut items, text, &source, &replacement, &mut candidates);
        *self.pending.borrow_mut() = Some(Pending { source: source.clone(), candidates });
        if let Some(upstream) = self.upstream.borrow().as_ref() {
            let task = upstream.completions(text, offset, trigger, window, cx);
            let text = text.clone(); let pane = self.pane.clone(); let pending = self.pending.clone(); let published = self.published.clone();
            let local = items.clone();
            let in_flight = self.in_flight.clone(); in_flight.set(true);
            *self.task.borrow_mut() = Some(window.spawn(cx, async move |cx| {
                let response = task.await;
                if generation.get() != request { return; }
                let Ok(response) = response else { in_flight.set(false); return; };
                let (more, complete) = match response { CompletionResponse::Array(items) => (items, true), CompletionResponse::List(list) => (list.items, !list.is_incomplete) };
                let complete = complete && selection.is_none();
                let cache_source = source.clone(); let cache_range = replacement.clone(); let cache_query = query.clone(); let cache_text = text.clone();
                let (mut more, cache) = cx.background_spawn(async move {
                    let mut more = more;
                    more.retain(|item| useful_import(item, language));
                    let cache = complete.then(|| CachedServerResults::new(cache_source, cache_range, cache_query, language, more.clone(), &cache_text));
                    (more, cache)
                }).await;
                if generation.get() != request { return; }
                in_flight.set(false);
                rank_items(&mut more, &query, &defined);
                more.truncate(128);
                let mut candidates = vec![];
                prepare_items(&mut more, &text, &source, &replacement, &mut candidates);
                let mut merged = local;
                // Prefer semantic LSP edits over the immediate declaration fallback.
                merged.retain(|item| item.kind == Some(CompletionItemKind::SNIPPET)
                    || (item.detail.as_deref() == Some("Defined in this file") && !more.iter().any(|server| server.label == item.label && !auto_import(server))));
                merged.extend(more);
                rank_items(&mut merged, &query, &defined);
                let _ = pane.update_in(cx, |pane, window, cx| {
                    let editor = pane.state.read(cx);
                    if editor.value().as_str() != source || editor.cursor() != offset || pane.language != language
                        || !Rc::ptr_eq(&pane.library, &library) || !editor.focus_handle(cx).is_focused(window)
                        || selection.as_ref().is_some_and(|range| editor.selected_range() != *range) { return; }
                    *pane.completions.cache.borrow_mut() = cache;
                    if let Some(pending) = pending.borrow_mut().as_mut() { pending.candidates.extend(candidates); }
                    *published.borrow_mut() = Some((source.clone(), offset, merged.clone()));
                    pane.merge_completions(start, &query, merged, cx);
                });
            }));
        }
        Task::ready(Ok(CompletionResponse::Array(items)))
    }

    fn is_completion_trigger(&self, offset: usize, new_text: &str, cx: &mut App) -> bool {
        if self.predictions.borrow().as_ref().is_some_and(|predictions| predictions.alt.get()) { return false; }
        (new_text.is_empty() && self.pending.borrow().is_some()) || new_text.chars().last().is_some_and(|ch| ch.is_alphanumeric() || ch == '_'
            || self.catalog.borrow().library.iter().flat_map(|s| &s.prefixes).any(|prefix| prefix.starts_with(ch)))
            || self.upstream.borrow().as_ref().is_some_and(|p| p.is_completion_trigger(offset, new_text, cx))
    }
}

impl Completions {
    pub fn set_upstream(&self, provider: Option<Rc<dyn CompletionProvider>>) {
        self.generation.set(self.generation.get().wrapping_add(1));
        self.in_flight.set(false);
        self.task.borrow_mut().take();
        self.cache.borrow_mut().take();
        self.published.borrow_mut().take();
        *self.upstream.borrow_mut() = provider;
    }
    pub fn cache_documentation(&self, original: &CompletionItem, resolved: &CompletionItem) {
        if let Some(cache) = &mut *self.cache.borrow_mut() {
            if let Some(item) = cache.items.iter_mut().find(|item| item.label == original.label && item.kind == original.kind && item.data == original.data) {
                item.documentation.clone_from(&resolved.documentation);
            }
        }
    }
    pub fn is_pending(&self) -> bool { self.in_flight.get() }
    pub fn cancel(&self) {
        self.generation.set(self.generation.get().wrapping_add(1));
        self.in_flight.set(false);
        self.task.borrow_mut().take();
        self.pending.borrow_mut().take();
        self.published.borrow_mut().take();
    }
}
fn rank_items(items: &mut [CompletionItem], query: &str, defined: &[String]) {
    let query = query.to_lowercase();
    items.sort_by_cached_key(|item| {
        let label = item.filter_text.as_deref().unwrap_or(&item.label).to_lowercase();
        let category = if auto_import(item) { 3 } else if item.kind == Some(CompletionItemKind::SNIPPET) { 2 }
            else if defined.contains(&item.label) { 0 } else { 1 };
        let relevance = if label == query { 0 } else if label.starts_with(&query) { 1 } else { 2 };
        (category, relevance)
    });
}

fn auto_import(item: &CompletionItem) -> bool {
    item.data.as_ref().and_then(|data| data.get("autoImportText")).and_then(serde_json::Value::as_str).is_some_and(|text| !text.is_empty())
        || item.detail.as_deref().is_some_and(|detail| detail.eq_ignore_ascii_case("auto-import"))
        || item.additional_text_edits.as_ref().is_some_and(|edits| edits.iter().any(|edit| {
            let text = edit.new_text.trim_start();
            text.starts_with("import ") || text.starts_with("from ") || text.starts_with("#include")
        }))
}

fn useful_import(item: &CompletionItem, language: Language) -> bool {
    if !auto_import(item) || language != Language::Python { return true; }
    let import = item.data.as_ref().and_then(|data| data.get("autoImportText")).and_then(serde_json::Value::as_str);
    let module = item.label_details.as_ref().and_then(|detail| detail.description.as_deref())
        .or_else(|| import.and_then(|text| text.split_whitespace().skip_while(|word| !matches!(*word, "from" | "import")).nth(1)));
    module.is_some_and(|module| matches!(module.split('.').next().unwrap_or_default(),
        "collections" | "bisect" | "functools" | "heapq" | "itertools" | "math" | "operator" | "random" | "re" | "string" | "sys" | "typing" | "array" | "fractions" | "decimal" | "statistics" | "io"))
}
fn prepare_items(items: &mut Vec<CompletionItem>, text: &Rope, source: &str, replacement: &Range<usize>, candidates: &mut Vec<(CompletionItem, Range<usize>, String)>) {
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
        if item.kind.is_none() { item.kind = Some(CompletionItemKind::SNIPPET); }
        candidates.push((item.clone(), range, snippet_body));
        true
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    fn import(label: &str, module: &str) -> CompletionItem {
        CompletionItem { label: label.into(), detail: Some("Auto-import".into()), data: Some(serde_json::json!({"autoImportText":format!("from {module} import {label}")})), ..Default::default() }
    }
    #[::core::prelude::v1::test]
    fn defined_symbols_then_language_then_snippets_then_auto_imports() {
        let mut items = vec![import("water", "heapq"), CompletionItem { label: "water".into(), kind: Some(CompletionItemKind::SNIPPET), ..Default::default() },
            CompletionItem { label: "while".into(), kind: Some(CompletionItemKind::KEYWORD), ..Default::default() },
            CompletionItem { label: "water".into(), kind: Some(CompletionItemKind::VARIABLE), ..Default::default() }];
        rank_items(&mut items, "wa", &["water".into()]);
        assert_eq!(items[0].kind, Some(CompletionItemKind::VARIABLE));
        assert_eq!(items[1].kind, Some(CompletionItemKind::KEYWORD));
        assert_eq!(items[2].kind, Some(CompletionItemKind::SNIPPET));
        assert!(auto_import(&items[3]));
    }
    #[::core::prelude::v1::test]
    fn filtering_and_ranking_happen_before_the_result_cap() {
        let mut items: Vec<_> = (0..200).map(|i| import(&format!("TextWrapper{i}"), "textwrap")).collect();
        items.push(import("heappush", "heapq"));
        items.push(CompletionItem { label: "trap".into(), kind: Some(CompletionItemKind::METHOD), ..Default::default() });
        items.retain(|item| useful_import(item, Language::Python));
        rank_items(&mut items, "trap", &["trap".into()]);
        items.truncate(128);
        assert_eq!(items[0].label, "trap");
        assert_eq!(items.len(), 2);
        assert_eq!(items[1].label, "heappush");
    }
    #[::core::prelude::v1::test]
    fn cp_imports_accept_metadata_and_leave_explicit_imports_usable() {
        for module in ["collections", "collections.abc", "heapq", "math", "bisect", "functools", "typing"] { assert!(useful_import(&import("symbol", module), Language::Python)); }
        for module in ["asyncio", "turtle", "tkinter", "textwrap", "numpy", "traceback"] { assert!(!useful_import(&import("symbol", module), Language::Python)); }
        assert!(useful_import(&CompletionItem { label: "TextWrapper".into(), ..Default::default() }, Language::Python));
        let mut item = import("deque", "bad_source");
        item.label_details = Some(lsp_types::CompletionItemLabelDetails { description: Some("collections".into()), ..Default::default() });
        assert!(useful_import(&item, Language::Python));
        assert!(auto_import(&CompletionItem { data: Some(serde_json::json!({"autoImportText":"from heapq import heappush"})), ..Default::default() }));
    }
    #[::core::prelude::v1::test]
    fn function_placeholders_preserve_symbol_kind_and_expansion() {
        let source = "tra";
        let mut items = vec![CompletionItem { label: "trap".into(), kind: Some(CompletionItemKind::FUNCTION), insert_text_format: Some(InsertTextFormat::SNIPPET), insert_text: Some("trap(${1:height})$0".into()), ..Default::default() }];
        let mut candidates = vec![];
        prepare_items(&mut items, &Rope::from_str(source), source, &(0..3), &mut candidates);
        assert_eq!(items[0].kind, Some(CompletionItemKind::FUNCTION));
        assert_eq!(Pending { source: source.into(), candidates }.accepted(&items[0], source).unwrap().1, "trap(${1:height})$0");
    }
    #[::core::prelude::v1::test]
    fn accepted_completion_requires_the_selected_item_and_current_document() {
        let item = CompletionItem { label: "loop".into(), ..Default::default() };
        let pending = || Pending { source: "α = loop\n".into(), candidates: vec![(item.clone(), 5..9, "for ${1:i} in range(n):\n    print($1)$0".into())] };
        assert_eq!(pending().accepted(&item, "α = loop\n").unwrap().0, 5..9);
        assert!(pending().accepted(&item, "α = loopx\n").is_none());
        assert!(pending().accepted(&CompletionItem { label: "local_value".into(), ..Default::default() }, "α = loop\n").is_none());
    }
}
