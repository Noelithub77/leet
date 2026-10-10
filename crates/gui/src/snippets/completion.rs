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
}
impl Completions {
    pub fn new(language: Language, pane: WeakEntity<super::expansion::EditorPane>, library: Rc<Library>) -> Self {
        Self { predictions: RefCell::new(None), catalog: RefCell::new(Catalog { library, language, selection: 0..0 }), upstream: RefCell::new(None), pending: Rc::default(), published: Rc::default(), generation: Rc::default(), in_flight: Rc::default(), pane, searcher: RefCell::default(), task: RefCell::default(), cache: RefCell::default() }
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
                for hit in self.searcher.borrow_mut().rank(&cache.index, &query, 64, |_| true) {
                    let mut item = cache.items[hit.index].clone();
                    if let Some(CompletionTextEdit::Edit(edit)) = &mut item.text_edit {
                        edit.range = lsp_types::Range { start: text.offset_to_position(replacement.start), end: text.offset_to_position(replacement.end) };
                    }
                    items.push(item);
                }
            }
        }
        rank_items(&mut items, &query);
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
                    let cache = complete.then(|| CachedServerResults::new(cache_source, cache_range, cache_query, language, more.clone(), &cache_text));
                    (more, cache)
                }).await;
                if generation.get() != request { return; }
                in_flight.set(false);
                more.truncate(128);
                let mut candidates = vec![];
                prepare_items(&mut more, &text, &source, &replacement, &mut candidates);
                let mut merged = local;
                // Cached rows are replaced by the current response, keeping only local snippets.
                merged.retain(|item| item.kind == Some(CompletionItemKind::SNIPPET));
                merged.extend(more);
                rank_items(&mut merged, &query);
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
fn rank_items(items: &mut [CompletionItem], query: &str) {
    if query.is_empty() { return; }
    let query = query.to_lowercase();
    items.sort_by_cached_key(|item| {
        let label = item.filter_text.as_deref().unwrap_or(&item.label).to_lowercase();
        let snippet = item.kind == Some(CompletionItemKind::SNIPPET);
        if label == query { if snippet { 0 } else { 1 } }
        else if label.starts_with(&query) { if snippet { 2 } else { 3 } }
        else if snippet { 4 } else { 5 }
    });
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
        item.kind = Some(CompletionItemKind::SNIPPET);
        candidates.push((item.clone(), range, snippet_body));
        true
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[::core::prelude::v1::test]
    fn accepted_completion_requires_the_selected_item_and_current_document() {
        let item = CompletionItem { label: "loop".into(), ..Default::default() };
        let pending = || Pending { source: "α = loop\n".into(), candidates: vec![(item.clone(), 5..9, "for ${1:i} in range(n):\n    print($1)$0".into())] };
        assert_eq!(pending().accepted(&item, "α = loop\n").unwrap().0, 5..9);
        assert!(pending().accepted(&item, "α = loopx\n").is_none());
        assert!(pending().accepted(&CompletionItem { label: "local_value".into(), ..Default::default() }, "α = loop\n").is_none());
    }
}
