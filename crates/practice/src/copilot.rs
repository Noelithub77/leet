//! GitHub's completion protocol; the server owns authentication and credentials.
use std::{ops::{ControlFlow, Range}, path::Path, process::Stdio, sync::{Arc, Mutex}, time::Duration};
use anyhow::{Context, Result, bail, ensure};
use async_lsp::{MainLoop, ServerSocket, router::Router};
use futures::future::{Either, select};
use lsp_types::{self as lsp, notification, request};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use crate::language::Language;

pub mod install;
#[cfg(test)] mod tests;

fn lock<T>(value: &Mutex<T>) -> std::sync::MutexGuard<'_, T> { value.lock().unwrap_or_else(|error| error.into_inner()) }
struct Stop;
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Status { #[default] Starting, Ready, Unavailable(String) }
#[derive(Default)]
struct State { initialized: bool, failure: Option<String>, status: Status, message: Option<String>, quota: Option<Quota> }
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Quota {
    pub copilot_plan: Option<String>,
    pub completions: Option<QuotaSnapshot>,
    pub chat: Option<QuotaSnapshot>,
    #[serde(rename = "premium_interactions")] pub premium: Option<QuotaSnapshot>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct QuotaSnapshot { pub unlimited: Option<bool>, pub percent_remaining: Option<f64>, pub reset_date: Option<String> }
impl Quota {
    pub fn detail(&self) -> String {
        let mut lines = Vec::new();
        if let Some(plan) = &self.copilot_plan { lines.push(format!("Plan · {plan}")); }
        let mut reported = false;
        for (label, snapshot) in [("Completions", &self.completions), ("Chat", &self.chat), ("Premium", &self.premium)] {
            let Some(snapshot) = snapshot else { continue; };
            if snapshot.unlimited == Some(true) { lines.push(format!("{label} · unlimited")); reported = true; }
            else if let Some(remaining) = snapshot.percent_remaining.filter(|value| value.is_finite() && (0.0..=100.0).contains(value)) {
                reported = true;
                lines.push(format!("{label} · {:.1}% used · {remaining:.1}% remaining", 100.0 - remaining));
                if let Some(reset) = &snapshot.reset_date { lines.push(format!("Resets · {reset}")); }
            }
        }
        if !reported { lines.push("Usage not reported by Copilot".into()); }
        lines.join("\n")
    }
}
struct Document { uri: lsp::Url, language: Language, version: i32, text: String }
pub struct Client { socket: ServerSocket, state: Arc<Mutex<State>>, document: Mutex<Option<Document>> }
impl Drop for Client { fn drop(&mut self) { let _ = self.socket.emit(Stop); } }

#[derive(Clone, Deserialize, Serialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct SignIn { pub user_code: Option<String>, pub command: Option<lsp::Command> }
#[derive(Clone, Debug, Deserialize, Serialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub insert_text: String,
    pub range: Option<lsp::Range>,
    #[serde(default, deserialize_with = "completion_command")]
    pub command: Option<lsp::Command>,
    #[serde(flatten)] extra: std::collections::BTreeMap<String, Value>,
}
fn completion_command<'de, D: serde::Deserializer<'de>>(deserializer: D) -> std::result::Result<Option<lsp::Command>, D::Error> {
    let Some(mut value) = Option::<Value>::deserialize(deserializer)? else { return Ok(None); };
    if let Some(object) = value.as_object_mut() { object.entry("title").or_insert_with(|| json!("")); }
    serde_json::from_value(value).map(Some).map_err(serde::de::Error::custom)
}
#[derive(Deserialize, Serialize)] struct Items { items: Vec<Item> }
#[derive(Deserialize, Serialize)] struct ServerStatus { kind: String, #[serde(default)] message: String }
enum SignInRequest {}
#[derive(Deserialize, Serialize)]
struct SignInParams {}
impl request::Request for SignInRequest { type Params = SignInParams; type Result = SignIn; const METHOD: &'static str = "signIn"; }
enum InlineRequest {}
impl request::Request for InlineRequest { type Params = Value; type Result = Items; const METHOD: &'static str = "textDocument/inlineCompletion"; }
enum StatusChanged {}
impl notification::Notification for StatusChanged { type Params = ServerStatus; const METHOD: &'static str = "didChangeStatus"; }
enum FocusDocument {}
impl notification::Notification for FocusDocument { type Params = Value; const METHOD: &'static str = "textDocument/didFocus"; }
enum Shown {}
impl notification::Notification for Shown { type Params = Value; const METHOD: &'static str = "textDocument/didShowCompletion"; }
enum QuotaChanged {}
impl notification::Notification for QuotaChanged { type Params = Quota; const METHOD: &'static str = "copilot/quotaChange"; }
enum QuotaWarning {}
impl notification::Notification for QuotaWarning { type Params = Value; const METHOD: &'static str = "copilot/quotaWarning"; }

async fn deadline<T>(future: impl std::future::Future<Output = Result<T>>, seconds: u64) -> Result<T> {
    futures::pin_mut!(future);
    let timer = async_io::Timer::after(Duration::from_secs(seconds)); futures::pin_mut!(timer);
    match select(future, timer).await { Either::Left((result, _)) => result, Either::Right(_) => bail!("Copilot request timed out") }
}
impl Client {
    pub fn start(binary: &Path, root: &Path) -> Result<Arc<Self>> { Self::start_with(binary, vec!["--stdio".into()], root) }
    fn start_with(binary: &Path, args: Vec<String>, root: &Path) -> Result<Arc<Self>> {
        let root = root.to_path_buf(); let binary = binary.to_path_buf();
        let uri = lsp::Url::from_directory_path(&root).map_err(|_| anyhow::anyhow!("Invalid Copilot workspace"))?;
        let state = Arc::new(Mutex::new(State::default())); let router_state = state.clone();
        let (main_loop, socket) = MainLoop::new_client(move |_| {
            let mut router = Router::new(router_state);
            router.notification::<StatusChanged>(|state, status| {
                lock(state).status = if status.kind == "Normal" { Status::Ready } else { Status::Unavailable(status.message) };
                ControlFlow::Continue(())
            }).notification::<QuotaChanged>(|state, quota| { lock(state).quota = Some(quota); ControlFlow::Continue(()) })
                .notification::<QuotaWarning>(|state, params| { lock(state).message = params.get("message").and_then(Value::as_str).map(str::to_owned); ControlFlow::Continue(()) })
                .notification::<notification::LogMessage>(|_, _| ControlFlow::Continue(()))
                .notification::<notification::ShowMessage>(|state, message| { lock(state).message = Some(message.message); ControlFlow::Continue(()) })
                .request::<request::ShowMessageRequest, _>(|state, params| {
                    lock(state).message = Some(params.message);
                    async { Ok(None) }
                })
                .request::<request::ShowDocument, _>(|_, params| async move {
                    // Authentication opens only GitHub's verification page.
                    let success = matches!(params.uri.host_str(), Some("github.com")) && params.uri.scheme() == "https" && params.uri.path().starts_with("/login/")
                        && open::that(params.uri.as_str()).is_ok();
                    Ok(lsp::ShowDocumentResult { success })
                })
                .request::<request::WorkspaceConfiguration, _>(|_, params| async move { Ok(params.items.iter().map(|_| Value::Null).collect()) })
                .request::<request::WorkDoneProgressCreate, _>(|_, _| async { Ok(()) })
                .request::<request::RegisterCapability, _>(|_, _| async { Ok(()) })
                .request::<request::UnregisterCapability, _>(|_, _| async { Ok(()) })
                .unhandled_notification(|_, _| ControlFlow::Continue(()))
                .event(|_, _: Stop| ControlFlow::Break(Ok(())));
            router
        });
        let client = Arc::new(Self { socket: socket.clone(), state: state.clone(), document: Mutex::new(None) });
        std::thread::Builder::new().name("leet-copilot".into()).spawn(move || {
            let outcome: Result<()> = async_io::block_on(async {
                let mut process = async_process::Command::new(binary);
                #[cfg(windows)] { use async_process::windows::CommandExt as _; process.creation_flags(0x0800_0000); }
                let mut child = process.args(args).current_dir(root).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).kill_on_drop(true).spawn()?;
                let input = child.stdout.take().context("Copilot stdout")?; let output = child.stdin.take().context("Copilot stdin")?;
                let initialize = async {
                    let params = lsp::InitializeParams { process_id: Some(std::process::id()),
                        workspace_folders: Some(vec![lsp::WorkspaceFolder { uri, name: "Solutions".into() }]),
                        initialization_options: Some(json!({"editorInfo":{"name":"Leet","version":env!("CARGO_PKG_VERSION")},"editorPluginInfo":{"name":"Leet Copilot","version":env!("CARGO_PKG_VERSION")}})),
                        capabilities: lsp::ClientCapabilities {
                            workspace: Some(lsp::WorkspaceClientCapabilities { workspace_folders: Some(true), ..Default::default() }),
                            window: Some(lsp::WindowClientCapabilities { show_document: Some(lsp::ShowDocumentClientCapabilities { support: true }), ..Default::default() }),
                            ..Default::default()
                        }, ..Default::default() };
                    deadline(async { Ok(socket.request::<request::Initialize>(params).await?) }, 30).await?;
                    socket.notify::<notification::Initialized>(lsp::InitializedParams {})?;
                    socket.notify::<notification::DidChangeConfiguration>(lsp::DidChangeConfigurationParams { settings: json!({"telemetry":{"telemetryLevel":"off"}}) })?;
                    lock(&state).initialized = true; anyhow::Ok(())
                };
                let (transport, initialization) = futures::join!(main_loop.run_buffered(input, output), async {
                    let result = initialize.await;
                    if let Err(error) = &result { lock(&state).failure = Some(error.to_string()); let _ = socket.emit(Stop); }
                    result
                });
                initialization?; transport?; Ok(())
            });
            lock(&state).failure = Some(outcome.err().map_or_else(|| "Copilot stopped".into(), |error| error.to_string()));
        })?;
        Ok(client)
    }
    pub fn stop(&self) { let _ = self.socket.emit(Stop); }
    pub fn status(&self) -> Status { let state = lock(&self.state); state.failure.as_ref().map_or_else(|| state.status.clone(), |message| Status::Unavailable(message.clone())) }
    pub fn message(&self) -> Option<String> { lock(&self.state).message.clone() }
    pub fn quota(&self) -> Option<Quota> { lock(&self.state).quota.clone() }
    async fn ready(&self) -> Result<()> {
        for _ in 0..320 {
            { let state = lock(&self.state); if let Some(error) = &state.failure { bail!("{error}"); } if state.initialized { return Ok(()); } }
            async_io::Timer::after(Duration::from_millis(100)).await;
        }
        bail!("Copilot initialization timed out")
    }
    pub async fn sign_in(&self) -> Result<SignIn> { self.ready().await?; deadline(async { Ok(self.socket.request::<SignInRequest>(SignInParams {}).await?) }, 30).await }
    pub async fn finish_sign_in(&self, command: lsp::Command) -> Result<()> {
        ensure!(command.command == "github.copilot.finishDeviceFlow", "Unexpected Copilot sign-in command");
        self.execute(command, 300).await
    }
    async fn execute(&self, command: lsp::Command, seconds: u64) -> Result<()> {
        deadline(async { self.socket.request::<request::ExecuteCommand>(lsp::ExecuteCommandParams { command: command.command, arguments: command.arguments.unwrap_or_default(), work_done_progress_params: Default::default() }).await?; Ok(()) }, seconds).await
    }
    fn sync(&self, path: &Path, language: Language, text: &str, offset: usize) -> Result<Value> {
        ensure!(offset <= text.len() && text.is_char_boundary(offset), "Invalid completion cursor");
        let uri = lsp::Url::from_file_path(path).map_err(|_| anyhow::anyhow!("Invalid Copilot document"))?;
        let mut active = lock(&self.document);
        if active.as_ref().is_some_and(|doc| doc.uri != uri || doc.language != language) {
            if let Some(doc) = active.take() { self.socket.notify::<notification::DidCloseTextDocument>(lsp::DidCloseTextDocumentParams { text_document: lsp::TextDocumentIdentifier { uri: doc.uri } })?; }
        }
        if active.is_none() {
            self.socket.notify::<notification::DidOpenTextDocument>(lsp::DidOpenTextDocumentParams { text_document: lsp::TextDocumentItem { uri: uri.clone(), language_id: language.id().into(), version: 1, text: text.into() } })?;
            self.socket.notify::<FocusDocument>(json!({"textDocument":{"uri":uri}}))?;
            *active = Some(Document { uri: uri.clone(), language, version: 1, text: text.into() });
        }
        let doc = active.as_mut().context("Copilot document")?;
        if doc.text != text {
            doc.version += 1;
            self.socket.notify::<notification::DidChangeTextDocument>(lsp::DidChangeTextDocumentParams {
                text_document: lsp::VersionedTextDocumentIdentifier { uri: uri.clone(), version: doc.version },
                content_changes: vec![lsp::TextDocumentContentChangeEvent { range: Some(lsp::Range::new(lsp::Position::new(0, 0), position(&doc.text, doc.text.len()))), range_length: None, text: text.into() }] })?;
            doc.text = text.into();
        }
        Ok(json!({"textDocument":{"uri":uri,"version":doc.version},"position":position(text,offset),"context":{"triggerKind":2},"formattingOptions":{"tabSize":4,"insertSpaces":true}}))
    }
    pub async fn predict(&self, path: &Path, language: Language, text: &str, offset: usize) -> Result<Option<Suggestion>> {
        self.ready().await?;
        let params = self.sync(path, language, text, offset)?;
        let response = deadline(async { Ok(self.socket.request::<InlineRequest>(params).await?) }, 15).await?;
        Ok(response.items.into_iter().find_map(|item| Suggestion::from_item(text, offset, item)))
    }
    pub fn shown(&self, suggestion: &Suggestion) { let _ = self.socket.notify::<Shown>(json!({"item":suggestion.item})); }
    pub async fn accepted(&self, suggestion: Suggestion) -> Result<()> { if let Some(command) = suggestion.item.command { self.execute(command, 10).await?; } Ok(()) }
}

pub fn position(text: &str, offset: usize) -> lsp::Position {
    let prefix = text.get(..offset).unwrap_or_default();
    lsp::Position::new(prefix.bytes().filter(|byte| *byte == b'\n').count() as u32, prefix.rsplit('\n').next().unwrap_or_default().encode_utf16().count() as u32)
}
fn byte_offset(text: &str, position: lsp::Position) -> Option<usize> {
    let mut base = 0;
    for (line, value) in text.split('\n').enumerate() {
        if line == position.line as usize {
            let mut units = 0;
            for (offset, ch) in value.char_indices() { if units == position.character { return Some(base + offset); } units += ch.len_utf16() as u32; }
            return (units == position.character).then_some(base + value.len());
        }
        base += value.len() + 1;
    }
    None
}
#[derive(Clone, Debug)]
pub struct Suggestion { source: String, offset: usize, text: String, item: Item }
impl Suggestion {
    pub fn from_item(source: &str, offset: usize, item: Item) -> Option<Self> {
        if offset > source.len() || !source.is_char_boundary(offset) { return None; }
        // The native renderer inserts at the caret; keep only exact insertion predictions.
        let normalized = item.insert_text.replace("\r\n", "\n");
        let range = match item.range { Some(range) => byte_offset(source, range.start)?..byte_offset(source, range.end)?, None => offset..offset };
        if range.start > offset || range.end < offset || range.end > source.len() { return None; }
        let before = source.get(range.start..offset)?; let after = source.get(offset..range.end)?;
        let inserted = normalized.strip_prefix(before)?.strip_suffix(after)?;
        if inserted.is_empty() { return None; }
        Some(Self { source: source.into(), offset, text: inserted.into(), item })
    }
    pub fn matches(&self, text: &str, offset: usize) -> bool { self.source == text && self.offset == offset }
    pub fn text(&self) -> &str { &self.text }
    pub fn range(&self) -> Range<usize> { self.offset..self.offset }
}
