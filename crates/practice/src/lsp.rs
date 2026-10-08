//! Shared language-server processes and versioned documents, independent of GPUI.
use std::{collections::HashMap, ops::ControlFlow, path::{Path, PathBuf}, process::Stdio, sync::{Arc, Mutex, atomic::{AtomicU64, Ordering}}, time::Duration};
use anyhow::{Context, Result, bail};
use async_lsp::{MainLoop, ServerSocket, router::Router};
use futures::future::{Either, select};
use lsp_types::{self as lsp, notification, request};
use crate::language::Language;

fn lock<T>(value: &Mutex<T>) -> std::sync::MutexGuard<'_, T> { value.lock().unwrap_or_else(|error| error.into_inner()) }
struct Stop;

static JAVA_WORKSPACE: AtomicU64 = AtomicU64::new(0);
struct JavaWorkspace(PathBuf);
impl JavaWorkspace {
    fn new() -> Result<Self> {
        loop {
            let path = std::env::temp_dir().join(format!("leet-jdtls-{}-{}", std::process::id(), JAVA_WORKSPACE.fetch_add(1, Ordering::Relaxed)));
            match std::fs::create_dir(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error.into()),
            }
        }
    }
}
impl Drop for JavaWorkspace { fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); } }

#[derive(Clone, Debug)] pub enum Status { Starting, Ready, Failed(String) }
struct State {
    status: Status,
    diagnostics: HashMap<lsp::Url, (u64, lsp::PublishDiagnosticsParams)>,
    sequence: u64,
    completion_resolve: bool,
    subscribers: HashMap<lsp::Url, futures::channel::mpsc::Sender<()>>,
}
pub struct Server { socket: ServerSocket, state: Arc<Mutex<State>>, pub name: String }
impl Drop for Server { fn drop(&mut self) { let _ = self.socket.emit(Stop); } }
async fn deadline<T>(future: impl std::future::Future<Output = Result<T>>, seconds: u64) -> Result<T> {
    futures::pin_mut!(future);
    let timer = async_io::Timer::after(Duration::from_secs(seconds)); futures::pin_mut!(timer);
    match select(future, timer).await { Either::Left((result, _)) => result, Either::Right(_) => bail!("Language server request timed out") }
}
fn executable(name: &str) -> Option<PathBuf> {
    std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()).find_map(|dir| {
        let path = dir.join(name);
        if executable_file(&path) { return Some(path); }
        #[cfg(windows)]
        for extension in ["exe", "cmd", "bat"] {
            let path = dir.join(format!("{name}.{extension}"));
            if executable_file(&path) { return Some(path); }
        }
        None
    })
}
fn executable_file(path: &Path) -> bool {
    let Ok(metadata) = path.metadata() else { return false; };
    if !metadata.is_file() { return false; }
    #[cfg(unix)] {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o111 == 0 { return false; }
    }
    true
}
pub(crate) fn command(language: Language) -> Result<(PathBuf, Vec<String>, String)> {
    if let Some(server) = crate::tool_setup::server(language) { return Ok(server); }
    match language {
        Language::Python => {
            for name in ["basedpyright-langserver", "pyright-langserver"] {
                if let Some(path) = executable(name) { return Ok((path, vec!["--stdio".into()], name.into())); }
            }
            let script = dirs::data_dir().context("Data directory")?.join("zed/languages/basedpyright/node_modules/basedpyright/dist/pyright-langserver.js");
            if script.is_file() {
                if let Some(node) = executable("node") { return Ok((node, vec![script.to_string_lossy().into_owned(), "--stdio".into()], "basedpyright".into())); }
            }
            bail!("Install Python tools from Setup or run leet --setup-tools --language python")
        }
        Language::Cpp | Language::C => Ok((executable("clangd").context("Install clangd from Setup or run leet --setup-tools --language cpp")?, vec!["--background-index".into()], "clangd".into())),
        Language::Java => Ok((executable("jdtls").context("Install Eclipse JDT LS and put jdtls in PATH; use JDK 21 or newer")?, vec![], "jdtls".into())),
        Language::Go => Ok((executable("gopls").context("Install gopls and put it in PATH")?, vec![], "gopls".into())),
    }
}
impl Server {
    pub fn stop(&self) { let _ = self.socket.emit(Stop); }

    pub fn supports_completion_resolve(&self) -> bool { lock(&self.state).completion_resolve }

    pub fn start(language: Language, root: &Path) -> Result<Arc<Self>> {
        let (binary, mut args, name) = command(language)?;
        let java_workspace = if language == Language::Java { Some(JavaWorkspace::new()?) } else { None };
        if let Some(workspace) = &java_workspace { args.extend(["-data".into(), workspace.0.to_string_lossy().into_owned()]); }
        let root = root.to_path_buf();
        let uri = lsp::Url::from_directory_path(&root).map_err(|_| anyhow::anyhow!("Invalid workspace directory"))?;
        let state = Arc::new(Mutex::new(State { status: Status::Starting, diagnostics: HashMap::new(), sequence: 0, completion_resolve: false, subscribers: HashMap::new() }));
        let diagnostic_state = state.clone();
        let (main_loop, socket) = MainLoop::new_client(move |_| {
            let mut router = Router::new(diagnostic_state);
            router.notification::<notification::PublishDiagnostics>(|state, diagnostics| {
                let mut state = lock(state); state.sequence += 1; let sequence = state.sequence;
                let uri = diagnostics.uri.clone();
                state.diagnostics.insert(uri.clone(), (sequence, diagnostics));
                if let Some(subscriber) = state.subscribers.get_mut(&uri) { let _ = subscriber.try_send(()); }
                ControlFlow::Continue(())
            }).notification::<notification::LogMessage>(|_, _| ControlFlow::Continue(()))
                .notification::<notification::ShowMessage>(|_, _| ControlFlow::Continue(()))
                .notification::<notification::Progress>(|_, _| ControlFlow::Continue(()))
                .request::<request::WorkDoneProgressCreate, _>(|_, _| async { Ok(()) })
                .request::<request::WorkspaceConfiguration, _>(move |_, params| async move {
                    Ok(params.items.iter().map(|item| if language == Language::Python { crate::python::configuration(item.section.as_deref().unwrap_or_default()) } else { serde_json::Value::Null }).collect())
                })
                .request::<request::RegisterCapability, _>(|_, _| async { Ok(()) })
                .request::<request::UnregisterCapability, _>(|_, _| async { Ok(()) })
                .event(|_, _: Stop| ControlFlow::Break(Ok(())));
            router
        });
        let server = Arc::new(Self { socket: socket.clone(), state: state.clone(), name });
        std::thread::Builder::new().name(format!("leet-lsp-{}", language.id())).spawn(move || {
            let _java_workspace = java_workspace;
            let outcome: Result<()> = async_io::block_on(async {
                let mut process = async_process::Command::new(binary);
                #[cfg(windows)] {
                    use async_process::windows::CommandExt as _;
                    process.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
                }
                let mut child = process.args(args).current_dir(&root)
                    .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).kill_on_drop(true).spawn()?;
                let input = child.stdout.take().context("Server stdout")?;
                let output = child.stdin.take().context("Server stdin")?;
                let initialize = async {
                    let capabilities = serde_json::from_value(serde_json::json!({
                        "textDocument": {"completion": {"completionItem": {"snippetSupport": false, "documentationFormat": ["markdown", "plaintext"], "resolveSupport": {"properties": ["documentation", "detail"]}}}, "hover": {"contentFormat": ["markdown", "plaintext"]}, "publishDiagnostics": {"versionSupport": true}, "definition": {"linkSupport": true}, "codeAction": {"codeActionLiteralSupport": {"codeActionKind": {"valueSet": ["quickfix", "refactor", "source", "source.organizeImports"]}}}},
                        "workspace": {"workspaceFolders": true, "configuration": true}
                    }))?;
                    let params = lsp::InitializeParams { process_id: Some(std::process::id()), capabilities,
                        workspace_folders: Some(vec![lsp::WorkspaceFolder { uri, name: "Solutions".into() }]), ..Default::default() };
                    let initialized = deadline(async { Ok(socket.request::<request::Initialize>(params).await?) }, 20).await;
                    match initialized {
                        Ok(result) => {
                            socket.notify::<notification::Initialized>(lsp::InitializedParams {})?;
                            let mut state = lock(&state);
                            state.completion_resolve = result.capabilities.completion_provider.and_then(|provider| provider.resolve_provider).unwrap_or(false);
                            state.status = Status::Ready;
                        }
                        Err(error) => { lock(&state).status = Status::Failed(error.to_string()); let _ = socket.emit(Stop); }
                    }
                    anyhow::Ok(())
                };
                let (transport, init) = futures::join!(main_loop.run_buffered(input, output), initialize);
                init?; transport?; Ok(())
            });
            let mut state = lock(&state);
            if let Err(error) = outcome { state.status = Status::Failed(error.to_string()); }
            else if matches!(state.status, Status::Ready) { state.status = Status::Failed("Language server stopped".into()); }
            for subscriber in state.subscribers.values_mut() { let _ = subscriber.try_send(()); }
        })?;
        Ok(server)
    }
    pub fn status(&self) -> Status { lock(&self.state).status.clone() }
    pub async fn ready(&self) -> Result<()> {
        for _ in 0..210 {
            match self.status() { Status::Ready => return Ok(()), Status::Failed(error) => bail!("{error}"), Status::Starting => {} }
            async_io::Timer::after(Duration::from_millis(100)).await;
        }
        bail!("Language server initialization timed out")
    }
    pub async fn request<R: request::Request>(&self, params: R::Params) -> Result<R::Result> {
        self.ready().await?; deadline(async { Ok(self.socket.request::<R>(params).await?) }, 10).await
    }
    pub async fn open(self: &Arc<Self>, path: &Path, language: Language, text: String) -> Result<Arc<Document>> {
        self.ready().await?;
        let uri = lsp::Url::from_file_path(path).map_err(|_| anyhow::anyhow!("Invalid document path"))?;
        self.socket.notify::<notification::DidOpenTextDocument>(lsp::DidOpenTextDocumentParams { text_document: lsp::TextDocumentItem { uri: uri.clone(), language_id: language.id().into(), version: 1, text: crate::python::language_document(language, &text) } })?;
        Ok(Arc::new(Document { server: self.clone(), uri, language, content: Mutex::new((1, text)) }))
    }
}
pub struct Document { pub server: Arc<Server>, pub uri: lsp::Url, language: Language, content: Mutex<(i32, String)> }
impl Document {
    pub fn sync(&self, text: String) -> Result<()> {
        let mut content = lock(&self.content);
        if content.1 == text { return Ok(()); }
        let version = content.0 + 1;
        self.server.socket.notify::<notification::DidChangeTextDocument>(lsp::DidChangeTextDocumentParams { text_document: lsp::VersionedTextDocumentIdentifier { uri: self.uri.clone(), version }, content_changes: vec![lsp::TextDocumentContentChangeEvent { range: None, range_length: None, text: crate::python::language_document(self.language, &text) }] })?;
        *content = (version, text); Ok(())
    }
    pub fn position(&self, text: &str, offset: usize) -> lsp::TextDocumentPositionParams {
        let offset = offset.min(text.len());
        let prefix = text.get(..offset).unwrap_or("");
        let line = prefix.bytes().filter(|byte| *byte == b'\n').count() as u32;
        let character = prefix.rsplit('\n').next().unwrap_or("").encode_utf16().count() as u32;
        lsp::TextDocumentPositionParams { text_document: lsp::TextDocumentIdentifier { uri: self.uri.clone() }, position: lsp::Position { line, character } }
    }
    pub fn diagnostics(&self) -> Option<(u64, Vec<lsp::Diagnostic>)> {
        let state = lock(&self.server.state);
        let (sequence, params) = state.diagnostics.get(&self.uri)?;
        if params.version.is_some_and(|version| version != lock(&self.content).0) { return None; }
        let user_lines = lock(&self.content).1.split('\n').count() as u32;
        Some((*sequence, params.diagnostics.iter().filter_map(|diagnostic| {
            let mut diagnostic = diagnostic.clone();
            if self.language == Language::Python && diagnostic.code.is_none()
                && diagnostic.range.start.line >= user_lines && diagnostic.message.contains("Expected indented block") {
                let end = lsp::Position::new(user_lines.saturating_sub(1), 0);
                diagnostic.range = lsp::Range::new(end, end);
            }
            crate::python::syntax_diagnostic(self.language, &diagnostic, user_lines).then_some(diagnostic)
        }).collect()))
    }
    pub fn subscribe(&self) -> futures::channel::mpsc::Receiver<()> {
        let (mut sender, receiver) = futures::channel::mpsc::channel(1);
        let mut state = lock(&self.server.state);
        let _ = sender.try_send(());
        state.subscribers.insert(self.uri.clone(), sender);
        receiver
    }
    pub fn version(&self) -> i32 { lock(&self.content).0 }
    pub fn saved(&self) { let _ = self.server.socket.notify::<notification::DidSaveTextDocument>(lsp::DidSaveTextDocumentParams { text_document: lsp::TextDocumentIdentifier { uri: self.uri.clone() }, text: None }); }
}
impl Drop for Document { fn drop(&mut self) { let _ = self.server.socket.notify::<notification::DidCloseTextDocument>(lsp::DidCloseTextDocumentParams { text_document: lsp::TextDocumentIdentifier { uri: self.uri.clone() } }); let mut state = lock(&self.server.state); state.diagnostics.remove(&self.uri); state.subscribers.remove(&self.uri); } }

#[cfg(test)] mod tests {
    use super::*;
    #[test]
    #[ignore = "requires basedpyright or pyright"]
    fn explicit_stop_terminates_a_server_retained_by_a_document() {
        async_io::block_on(async {
            let root = tempfile::tempdir().unwrap();
            let path = root.path().join("main.py");
            std::fs::write(&path, "print(1)\n").unwrap();
            let server = Server::start(Language::Python, root.path()).unwrap();
            let document = server.open(&path, Language::Python, "print(1)\n".into()).await.unwrap();
            server.stop();
            for _ in 0..100 {
                if matches!(server.status(), Status::Failed(_)) { break; }
                async_io::Timer::after(Duration::from_millis(20)).await;
            }
            assert!(matches!(server.status(), Status::Failed(_)));
            assert!(document.server.ready().await.is_err());
        });
    }

    #[test]
    #[ignore = "requires basedpyright or pyright"]
    fn live_python_contest_prelude_and_syntax_only_diagnostics() {
        async_io::block_on(async {
            let root = tempfile::tempdir().unwrap(); let path = root.path().join("main.py");
            let text = "class Solution:\n    def f(self, values: List[int]):\n        counts = defaultdict(int)\n        return counts\n";
            std::fs::write(&path, text).unwrap();
            let server = Server::start(Language::Python, root.path()).unwrap();
            let doc = server.open(&path, Language::Python, text.into()).await.unwrap();
            async_io::Timer::after(Duration::from_secs(2)).await;
            let hover = server.request::<request::HoverRequest>(lsp::HoverParams { text_document_position_params: doc.position(text, text.find("defaultdict").unwrap() + 1), work_done_progress_params: Default::default() }).await.unwrap();
            assert!(hover.is_some(), "preloaded defaultdict has docs");
            assert!(doc.diagnostics().is_some_and(|(_, diagnostics)| diagnostics.is_empty()), "no unknown-type noise");
            doc.sync("class Solution:\n    def f(self):\n".into()).unwrap();
            for _ in 0..30 {
                async_io::Timer::after(Duration::from_millis(100)).await;
                if doc.diagnostics().is_some_and(|(_, diagnostics)| diagnostics.iter().any(|diagnostic| diagnostic.message.contains("Expected indented block"))) { return; }
            }
            panic!("missing method body must retain its syntax diagnostic");
        });
    }
    #[test]
    #[ignore = "requires installed language servers"]
    fn live_completion_hover_definition_and_diagnostics() {
        check_language_features(&[Language::Python, Language::Cpp, Language::Go, Language::C]);
    }
    #[test]
    #[ignore = "requires private Python and clangd tools; isolate XDG_DATA_HOME for verification"]
    fn live_private_python_and_cpp_servers() {
        for language in [Language::Python, Language::Cpp] {
            assert!(crate::tool_setup::server(language).is_some(), "Private {} server is required", language.label());
        }
        check_language_features(&[Language::Python, Language::Cpp]);
    }
    fn check_language_features(languages: &[Language]) {
        async_io::block_on(async {
            let root = tempfile::tempdir().unwrap();
            std::fs::write(root.path().join("go.mod"), "module vgtest\n\ngo 1.23\n").unwrap();
            for (language, text, needle) in [
                (Language::Python, "import math\nmath.sqrt(4)\n", "sqrt"),
                (Language::Cpp, "#include <cmath>\nint main(){return std::sqrt(4);}\n", "sqrt"),
                (Language::Go, "package main\nimport \"fmt\"\nfunc main(){fmt.Println(4)}\n", "Println"),
                (Language::C, "#include <stdio.h>\nint main(){printf(\"ok\");}\n", "printf"),
            ] {
                if !languages.contains(&language) { continue; }
                let path = root.path().join(format!("main.{}", language.extension()));
                std::fs::write(&path, text).unwrap();
                let server = Server::start(language, root.path()).unwrap();
                let doc = server.open(&path, language, text.into()).await.unwrap();
                async_io::Timer::after(Duration::from_secs(2)).await;
                let offset = text.find(needle).unwrap();
                let hover = server.request::<request::HoverRequest>(lsp::HoverParams { text_document_position_params: doc.position(text, offset + 1), work_done_progress_params: Default::default() }).await.unwrap();
                assert!(hover.is_some(), "{} hover", language.label());
                let definition = server.request::<request::GotoDefinition>(lsp::GotoDefinitionParams { text_document_position_params: doc.position(text, offset + 1), work_done_progress_params: Default::default(), partial_result_params: Default::default() }).await.unwrap();
                assert!(definition.is_some(), "{} definition", language.label());
                let completion = server.request::<request::Completion>(lsp::CompletionParams { text_document_position: doc.position(text, offset + 2), context: None, work_done_progress_params: Default::default(), partial_result_params: Default::default() }).await.unwrap();
                assert!(completion.is_some(), "{} completion", language.label());
                use futures::StreamExt as _;
                let mut updates = doc.subscribe(); let _ = updates.next().await;
                doc.sync(format!("{text}\n{{{{invalid syntax")).unwrap();
                let mut diagnostics = None;
                for _ in 0..10 { deadline(async { updates.next().await.context("Diagnostic stream closed") }, 5).await.unwrap(); diagnostics = doc.diagnostics(); if diagnostics.as_ref().is_some_and(|(_, values)| !values.is_empty()) { break; } }
                assert!(diagnostics.is_some_and(|(_, values)| !values.is_empty()), "{} diagnostics", language.label());
                println!("{}: completion, hover, definition, versioned diagnostics passed", language.label());
            }
        });
    }
}
