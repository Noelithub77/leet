//! Adapt maintained LSP clients to the editor's existing providers and overlays.
use std::{collections::{HashMap, HashSet}, rc::Rc, sync::{Arc, Weak}};
use anyhow::Result;
use futures::StreamExt as _;
use gpui_kit::*;
use gpui_kit::component::input::{CodeActionProvider, CompletionProvider, DefinitionProvider, EditorState, HoverProvider};
use lsp_wire::{self as wire, request};
use ropey::Rope;
use practice::{language::Language, lsp::{Document, Server, Status}};
use crate::workspace::{Center, Workspace};

#[derive(Default)]
pub struct Intelligence {
    active: Option<Language>,
    pending: HashSet<EntityId>,
    servers: HashMap<Language, Arc<Server>>,
    documents: HashMap<EntityId, Weak<Document>>,
    errors: HashMap<Language, String>,
    generations: HashMap<EntityId, u64>,
}
impl Intelligence {
    pub fn label(&self, language: Language) -> String {
        if self.errors.contains_key(&language) { return format!("{} · LSP unavailable", language.label()); }
        match self.servers.get(&language).map(|server| server.status()) {
            Some(Status::Ready) => format!("{} · IntelliSense", language.label()),
            Some(Status::Starting) => format!("{} · starting LSP", language.label()),
            Some(Status::Failed(_)) => format!("{} · LSP unavailable", language.label()),
            None => language.label().into(),
        }
    }
    pub fn detail(&self, language: Language) -> String {
        if let Some(error) = self.errors.get(&language) { return error.clone(); }
        self.servers.get(&language).map_or_else(|| "Language server".into(), |server| match server.status() {
            Status::Failed(error) => error,
            _ => format!("{} · Ctrl+Space completion · Ctrl+F12 definition", server.name),
        })
    }
}
fn convert<T: serde::de::DeserializeOwned>(value: impl serde::Serialize) -> Result<T> { Ok(serde_json::from_value(serde_json::to_value(value)?)?) }
struct Provider(Arc<Document>);
impl CompletionProvider for Provider {
    fn completions(&self, text: &Rope, offset: usize, _trigger: lsp_types::CompletionContext, _: &mut Window, cx: &mut App) -> Task<Result<lsp_types::CompletionResponse>> {
        let doc = self.0.clone(); let text = text.to_string();
        cx.background_spawn(async move {
            doc.sync(text.clone())?;
            let params = wire::CompletionParams { text_document_position: doc.position(&text, offset), context: Some(wire::CompletionContext { trigger_kind: wire::CompletionTriggerKind::INVOKED, trigger_character: None }), work_done_progress_params: Default::default(), partial_result_params: Default::default() };
            let response = doc.server.request::<request::Completion>(params).await?;
            let mut items = match response.unwrap_or(wire::CompletionResponse::Array(vec![])) {
                wire::CompletionResponse::Array(items) => items,
                wire::CompletionResponse::List(list) => list.items,
            };
            let start = text[..offset].char_indices().rev().find(|(_, ch)| !ch.is_alphanumeric() && *ch != '_').map_or(0, |(index, ch)| index + ch.len_utf8());
            let prefix = text[start..offset].to_lowercase();
            items.sort_by(|left, right| (!left.label.to_lowercase().starts_with(&prefix), left.sort_text.as_deref().unwrap_or(&left.label)).cmp(&(!right.label.to_lowercase().starts_with(&prefix), right.sort_text.as_deref().unwrap_or(&right.label))));
            let resolves = items.iter().enumerate().take(32).filter(|(_, item)| doc.server.supports_completion_resolve() && item.documentation.is_none()).map(|(index, item)| {
                let server = doc.server.clone(); let item = item.clone();
                async move { (index, server.request::<request::ResolveCompletionItem>(item).await) }
            }).collect::<Vec<_>>();
            let resolved = futures::stream::iter(resolves).buffer_unordered(6).collect::<Vec<_>>().await;
            for (index, result) in resolved { if let Ok(item) = result { items[index] = item; } }
            convert(wire::CompletionResponse::Array(items))
        })
    }
    fn is_completion_trigger(&self, _: usize, new_text: &str, _: &mut App) -> bool {
        new_text.chars().last().is_some_and(|ch| ch.is_alphanumeric() || ch == '_' || ch == '.')
    }
}
impl HoverProvider for Provider {
    fn hover(&self, text: &Rope, offset: usize, _: &mut Window, cx: &mut App) -> Task<Result<Option<lsp_types::Hover>>> {
        let doc = self.0.clone(); let text = text.to_string();
        cx.background_spawn(async move {
            doc.sync(text.clone())?;
            convert(doc.server.request::<request::HoverRequest>(wire::HoverParams { text_document_position_params: doc.position(&text, offset), work_done_progress_params: Default::default() }).await?)
        })
    }
}
impl DefinitionProvider for Provider {
    fn definitions(&self, text: &Rope, offset: usize, _: &mut Window, cx: &mut App) -> Task<Result<Vec<lsp_types::LocationLink>>> {
        let doc = self.0.clone(); let text = text.to_string();
        cx.background_spawn(async move {
            doc.sync(text.clone())?;
            let response = doc.server.request::<request::GotoDefinition>(wire::GotoDefinitionParams { text_document_position_params: doc.position(&text, offset), work_done_progress_params: Default::default(), partial_result_params: Default::default() }).await?;
            let links: Vec<wire::LocationLink> = match response {
                Some(wire::GotoDefinitionResponse::Link(links)) => links,
                Some(wire::GotoDefinitionResponse::Scalar(location)) => vec![location_link(location)],
                Some(wire::GotoDefinitionResponse::Array(locations)) => locations.into_iter().map(location_link).collect(),
                None => vec![],
            };
            convert(links)
        })
    }
}
impl CodeActionProvider for Provider {
    fn id(&self) -> SharedString { "language-server".into() }
    fn code_actions(&self, editor: Entity<EditorState>, range: std::ops::Range<usize>, _: &mut Window, cx: &mut App) -> Task<Result<Vec<lsp_types::CodeAction>>> {
        let text = editor.read(cx).value().to_string(); let doc = self.0.clone();
        cx.background_spawn(async move {
            doc.sync(text.clone())?;
            let range = wire::Range { start: doc.position(&text, range.start).position, end: doc.position(&text, range.end).position };
            let params = wire::CodeActionParams { text_document: wire::TextDocumentIdentifier { uri: doc.uri.clone() }, range,
                context: wire::CodeActionContext { diagnostics: doc.diagnostics().map_or_else(Vec::new, |(_, values)| values), only: None, trigger_kind: Some(wire::CodeActionTriggerKind::INVOKED) },
                work_done_progress_params: Default::default(), partial_result_params: Default::default() };
            let actions = doc.server.request::<request::CodeActionRequest>(params).await?.unwrap_or_default();
            let actions: Vec<wire::CodeAction> = actions.into_iter().filter_map(|action| match action { wire::CodeActionOrCommand::CodeAction(action) => Some(action), wire::CodeActionOrCommand::Command(command) => Some(wire::CodeAction { title: command.title.clone(), command: Some(command), ..Default::default() }) }).collect();
            convert(actions)
        })
    }
    fn perform_code_action(&self, editor: Entity<EditorState>, action: lsp_types::CodeAction, _: bool, window: &mut Window, cx: &mut App) -> Task<Result<()>> {
        let uri = self.0.uri.as_str();
        let mut edits = Vec::new();
        if let Some(edit) = &action.edit {
            if let Some(changes) = &edit.document_changes {
                match changes {
                    lsp_types::DocumentChanges::Edits(documents) => {
                        for document in documents {
                            if document.text_document.uri.as_str() != uri { return Task::ready(Err(anyhow::anyhow!("This action changes another file; use the external editor"))); }
                            if document.text_document.version.is_some_and(|version| version != self.0.version()) { return Task::ready(Err(anyhow::anyhow!("Document changed; request the code action again"))); }
                            edits.extend(document.edits.iter().map(|edit| match edit { lsp_types::OneOf::Left(edit) => edit.clone(), lsp_types::OneOf::Right(edit) => edit.text_edit.clone() }));
                        }
                    }
                    lsp_types::DocumentChanges::Operations(_) => return Task::ready(Err(anyhow::anyhow!("Workspace file operations require the external editor"))),
                }
            }
            if let Some(changes) = &edit.changes {
                if changes.keys().any(|target| target.as_str() != uri) { return Task::ready(Err(anyhow::anyhow!("This action changes another file; use the external editor"))); }
                edits.extend(changes.values().flatten().cloned());
            }
        }
        edits.sort_by(|left, right| right.range.start.cmp(&left.range.start));
        if !edits.is_empty() { editor.update(cx, |editor, cx| editor.apply_lsp_edits(&edits, window, cx)); }
        let doc = self.0.clone();
        if let Some(command) = action.command {
            return cx.background_spawn(async move { doc.server.request::<request::ExecuteCommand>(wire::ExecuteCommandParams { command: command.command, arguments: command.arguments.unwrap_or_default(), work_done_progress_params: Default::default() }).await?; Ok(()) });
        }
        Task::ready(Ok(()))
    }
}

fn location_link(location: wire::Location) -> wire::LocationLink { wire::LocationLink { origin_selection_range: None, target_uri: location.uri, target_range: location.range, target_selection_range: location.range } }

impl Workspace {
    pub(crate) fn activate_language(&mut self, language: Language, cx: &mut Context<Self>) {
        if self.intelligence.active == Some(language) { return; }
        self.suspend_language_servers(cx);
        self.intelligence.active = Some(language);
    }

    pub(crate) fn suspend_language_servers(&mut self, cx: &mut Context<Self>) {
        for (_, server) in self.intelligence.servers.drain() { server.stop(); }
        self.intelligence.documents.clear();
        self.intelligence.pending.clear();
        self.intelligence.errors.clear();
        for generation in self.intelligence.generations.values_mut() { *generation = generation.wrapping_add(1); }
        clear_editor_adapters(&self.editor, cx);
        self.clear_tab_language_adapters(cx);
        self.intelligence.active = None;
    }
    pub fn restart_language_server(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(session) = self.session.as_ref() else { return; };
        let language = session.language;
        self.intelligence.active = None;
        self.activate_language(language, cx);
        self.attach_language_server(window, cx);
    }
    pub fn language_document_saved(&self) {
        if let Some(doc) = self.intelligence.documents.get(&self.editor.entity_id()).and_then(Weak::upgrade) { doc.saved(); }
    }
    pub fn sync_language_document(&self, editor: &Entity<EditorState>, cx: &App) {
        if let Some(doc) = self.intelligence.documents.get(&editor.entity_id()).and_then(Weak::upgrade) { let _ = doc.sync(editor.read(cx).value().to_string()); }
    }
    pub fn attach_language_server(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.center != Center::Editor { return; }
        let Some(session) = self.session.as_ref().filter(|session| session.question.is_some()) else { return; };
        let language = session.language; let path = session.path.clone();
        self.activate_language(language, cx);
        if self.intelligence.documents.get(&self.editor.entity_id()).and_then(Weak::upgrade).is_some() { self.sync_language_document(&self.editor, cx); return; }
        if self.intelligence.pending.contains(&self.editor.entity_id()) { return; }
        let server = if let Some(server) = self.intelligence.servers.get(&language).filter(|server| !matches!(server.status(), Status::Failed(_))) { server.clone() }
            else {
                match Server::start(language, &self.config.workspace) {
                    Ok(server) => { self.intelligence.servers.insert(language, server.clone()); self.intelligence.errors.remove(&language); server },
                    Err(error) => { self.intelligence.errors.insert(language, error.to_string()); cx.notify(); return; },
                }
            };
        let generation = self.intelligence.generations.entry(self.editor.entity_id()).or_default(); *generation += 1; let generation = *generation;
        let editor_id = self.editor.entity_id();
        self.intelligence.pending.insert(editor_id);
        let editor = self.editor.downgrade(); let text = self.editor.read(cx).value().to_string();
        cx.spawn_in(window, async move |this, cx| {
            let result = cx.background_spawn(async move { server.open(&path, language, text).await }).await;
            let document = match result {
                Ok(document) => document,
                Err(error) => { let _ = this.update(cx, |ws, cx| { if ws.intelligence.generations.get(&editor_id) == Some(&generation) { ws.intelligence.pending.remove(&editor_id); } if ws.intelligence.active == Some(language) && ws.editor.entity_id() == editor_id && ws.intelligence.generations.get(&editor_id) == Some(&generation) { ws.intelligence.errors.insert(language, error.to_string()); cx.notify(); } }); return; },
            };
            let weak_doc = Arc::downgrade(&document);
            let mut updates = document.subscribe();
            let mut last = 0;
            let attached = this.update_in(cx, |ws, _, cx| {
                if ws.intelligence.generations.get(&editor_id) == Some(&generation) { ws.intelligence.pending.remove(&editor_id); }
                if ws.intelligence.active != Some(language) || ws.editor.entity_id() != editor_id || ws.intelligence.generations.get(&editor_id) != Some(&generation) { return false; }
                let Some(editor) = editor.upgrade() else { return false; };
                ws.intelligence.documents.retain(|_, doc| doc.strong_count() > 0);
                ws.intelligence.documents.insert(editor.entity_id(), Arc::downgrade(&document));
                let _ = document.sync(editor.read(cx).value().to_string());
                let provider = Rc::new(Provider(document.clone()));
                let document_uri = document.uri.to_string(); let workspace = cx.weak_entity();
                editor.update(cx, |editor, cx| {
                    editor.lsp_mut().completion_menu.max_width = px(360.);
                    editor.lsp_mut().completion_provider = Some(provider.clone());
                    editor.lsp_mut().hover_provider = Some(provider.clone());
                    editor.lsp_mut().definition_provider = Some(provider.clone());
                    editor.lsp_mut().code_action_providers = vec![provider];
                    editor.lsp_mut().show_document = Some(Rc::new(move |params, window, cx| {
                        if params.uri.as_str() == document_uri { return false; }
                        let Ok(url) = url::Url::parse(params.uri.as_str()) else { return true; };
                        let Ok(path) = url.to_file_path() else { return true; };
                        let _ = workspace.update(cx, |ws, cx| {
                            let known = ws.tabs.iter().position(|tab| tab.as_ref().is_some_and(|tab| tab.session.path == path));
                            if let Some(index) = known { ws.select_tab(index, window, cx); if let Some(range) = params.selection { ws.editor.update(cx, |editor, cx| editor.set_cursor_position(range.start, window, cx)); } }
                            else { let _ = std::process::Command::new(&ws.config.external_editor).arg(&path).spawn(); }
                        }); true
                    }));
                    editor.refresh(cx);
                }); cx.notify(); true
            }).unwrap_or(false);
            drop(document);
            if !attached { return; }
            while updates.next().await.is_some() {
                let Some(document) = weak_doc.upgrade() else { break; };
                let diagnostics = document.diagnostics(); drop(document);
                let Some((sequence, diagnostics)) = diagnostics else {
                    if this.update(cx, |_, cx| cx.notify()).is_err() { break; }
                    continue;
                };
                if sequence == last { continue; } last = sequence;
                let converted: Result<Vec<lsp_types::Diagnostic>> = convert(diagnostics);
                let Ok(diagnostics) = converted else { continue; };
                if this.update(cx, |ws, cx| {
                    if ws.intelligence.active != Some(language) || ws.editor.entity_id() != editor_id || ws.intelligence.generations.get(&editor_id) != Some(&generation) { return false; }
                    let Some(editor) = editor.upgrade() else { return false; };
                    editor.update(cx, |editor, cx| {
                        let text = editor.text().clone();
                        if let Some(set) = editor.diagnostics_mut() { set.reset(&text); set.extend(diagnostics); }
                        editor.refresh(cx);
                    }); true
                }).unwrap_or(false) == false { break; }
            }
        }).detach(); cx.notify();
    }
}
gpui_kit::actions!(intelligence, [Complete]);

pub(crate) fn clear_editor_adapters(editor: &Entity<EditorState>, cx: &mut App) {
    editor.update(cx, |editor, cx| {
        editor.lsp_mut().completion_provider = None;
        editor.lsp_mut().hover_provider = None;
        editor.lsp_mut().definition_provider = None;
        editor.lsp_mut().code_action_providers.clear();
        editor.lsp_mut().show_document = None;
        let text = editor.text().clone();
        if let Some(diagnostics) = editor.diagnostics_mut() { diagnostics.reset(&text); }
        editor.refresh(cx);
    });
}
