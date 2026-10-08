//! Per-problem AI conversations, structured action results, and agent lifecycle.
//! Runs execute on background threads and stream events back; conversations stay local.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures::StreamExt as _;
use gpui_kit::assets::IconName;
use gpui_kit::base::Disableable as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::shimmer::ShimmerText;
use gpui_kit::component::input::{InputEvent, InputState, TextareaState};
use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _, Theme, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use practice::agents::{self, AgentKind, Cancel, Catalog, Detected, Selection};
use practice::assist::{self, Action, Answer, Growth};
use practice::db::Db;
use practice::language::Language;
use practice::prompts::Provider;
use practice::viz::Tone;

use crate::gen_ui::player::{self, Playback};
use crate::workspace::Workspace;

gpui_kit::actions!(chat, [SendChat, NextThread, PreviousThread, OpenThread, CloseThreadList]);

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([KeyBinding::new("ctrl-enter", SendChat, Some("AgentChat")),
        KeyBinding::new("ctrl-enter", SendChat, Some("AgentChat > Input")),
        KeyBinding::new("down", NextThread, Some("ChatThreads")),
        KeyBinding::new("up", PreviousThread, Some("ChatThreads")),
        KeyBinding::new("enter", OpenThread, Some("ChatThreads")),
        KeyBinding::new("ctrl-enter", OpenThread, Some("ChatThreads")),
        KeyBinding::new("escape", CloseThreadList, Some("ChatThreads"))]);
}

#[path = "chat_view.rs"]
mod chat_view;
#[path = "conversation.rs"]
mod conversation;

const SOLVE_ATTEMPTS: usize = 5;

/// Everything a run needs about the open problem, copied so the run outlives edits.
#[derive(Clone)]
pub struct Snapshot {
    pub slug: String,
    pub title: String,
    pub difficulty: String,
    pub url: String,
    pub statement_html: String,
    pub code: String,
    pub starter: String,
    pub language: Language,
    /// (input, expected, latest outcome)
    pub cases: Vec<(String, Option<String>, Option<String>)>,
    pub focus_case: usize,
    pub solution_path: PathBuf,
    pub workspace: PathBuf,
    /// A recorded trace digest of `focus_case`, when the debugger asked for a review.
    pub trace: Option<String>,
}

impl Snapshot {
    fn context<'a>(&'a self, feedback: Option<&'a str>) -> assist::Context<'a> {
        assist::Context {
            title: &self.title, difficulty: &self.difficulty, url: &self.url, statement_html: &self.statement_html,
            code: &self.code, starter: &self.starter, language: self.language,
            cases: self.cases.iter().map(|(input, expected, outcome)| assist::CaseContext { input, expected: expected.as_deref(), outcome: outcome.clone() }).collect(),
            focus_case: Some(self.focus_case), solution_path: self.solution_path.to_str().unwrap_or_default(), feedback, trace: self.trace.as_deref(),
        }
    }

    pub fn has_attempt(&self) -> bool {
        practice::workspace::has_attempt(&self.code, &self.starter)
    }
}

/// Where a run goes: a detected agent with a model, or a web chat.
#[derive(Clone, Debug)]
pub enum Target {
    Agent(Detected, Option<Selection>),
    Web(Provider),
}

pub enum Loadable<T> {
    Loading,
    Ready(T),
    Failed(String),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Phase {
    Starting,
    Thinking,
    Writing,
    Tool(String),
    Testing,
    Confirm,
    Judging,
    Done,
    Failed(String),
    Stopped,
}

impl Phase {
    fn active(&self) -> bool { !matches!(self, Phase::Done | Phase::Failed(_) | Phase::Stopped | Phase::Confirm) }

    fn label(&self) -> String {
        match self {
            Phase::Starting => "Starting agent".into(),
            Phase::Thinking => "Thinking".into(),
            Phase::Writing => "Writing answer".into(),
            Phase::Tool(title) => title.clone(),
            Phase::Testing => "Running tests".into(),
            Phase::Confirm => "Ready to submit".into(),
            Phase::Judging => "Judging".into(),
            Phase::Done => "Done".into(),
            Phase::Failed(error) => error.clone(),
            Phase::Stopped => "Stopped".into(),
        }
    }
}

pub struct TraceReview {
    pub wrong_step: Option<usize>,
    pub explanation: String,
    pub fix_hint: String,
}

fn solve_written_phase(action: Action, attempt: usize) -> Phase {
    if action == Action::Solve && attempt == 1 { Phase::Confirm } else { Phase::Testing }
}

struct SolveState {
    attempt: usize,
    session: Option<String>,
    log: Vec<(Tone, String)>,
    accepted: bool,
}

pub struct Run {
    id: u64,
    thread_id: i64,
    message_id: i64,
    problem_title: String,
    slug: String,
    action: Action,
    agent: Option<AgentKind>,
    model: String,
    started: Instant,
    elapsed: Option<Duration>,
    artifact_mtime: Option<std::time::SystemTime>,
    phase: Phase,
    thinking: String,
    tokens: u64,
    answer: Option<Answer>,
    cancel: Cancel,
    hints_shown: usize,
    playback: Playback,
    last_frame: Instant,
    reveal: bool,
    added: Vec<usize>,
    solve: Option<SolveState>,
    snapshot: Option<Snapshot>,
    target: Option<Target>,
    generation: u64,
    collapsed: bool,
    instructions: String,
    request_prompt: String,
    history: String,
    reply: String,
}

impl Run {
    pub fn action(&self) -> Action { self.action }
    fn time(&self) -> Duration { self.elapsed.unwrap_or_else(|| self.started.elapsed()) }
    fn accepts_events(&self, generation: u64) -> bool { !self.cancel.is_cancelled() && self.generation == generation }
}

enum Msg {
    Event(agents::Event),
    Selected(Selection),
    Done(anyhow::Result<(Option<Answer>, Option<String>)>),
}

pub struct Assist {
    workspace: WeakEntity<Workspace>,
    db: Arc<Db>,
    pub agents: Vec<Detected>,
    pub detecting: bool,
    pub catalogs: HashMap<AgentKind, Loadable<Catalog>>,
    catalog_cache: Arc<agents::CatalogCache>,
    requested_catalogs: HashSet<AgentKind>,
    pub installing: Option<AgentKind>,
    runs: Vec<Run>,
    next_id: u64,
    slug: Option<String>,
    ticker: Option<Task<()>>,
    scroll: ScrollHandle,
    pub(crate) composer: Entity<TextareaState>,
    composer_thread: Option<i64>,
    threads: Vec<practice::chat::history::Thread>,
    selected_thread: Option<i64>,
    loaded_threads: HashSet<i64>,
    show_threads: bool,
    thread_scroll: UniformListScrollHandle,
    thread_search: Entity<InputState>,
    thread_query: String,
    thread_selection: usize,
    conversation_error: Option<String>,
    editing: Option<u64>,
    ai_tab: u8,
    context_key: Option<(Option<String>, bool, bool, bool)>,

}

use practice::chat::history::{SavedTurn as Saved, SavedPhase};

impl Run {
    fn saved(&self) -> Saved {
        Saved { model: self.model.clone(), agent: self.agent, answer: self.answer.clone(), instructions: self.instructions.clone(), request_prompt: self.request_prompt.clone(), action: Some(self.action), problem: self.slug.clone(), problem_title: self.problem_title.clone(),
            phase: match &self.phase { Phase::Done => SavedPhase::Done, Phase::Failed(error) => SavedPhase::Failed(error.clone()), _ => SavedPhase::Stopped } }
    }
}

impl Assist {
    pub fn new(workspace: WeakEntity<Workspace>, db: Arc<Db>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let catalog_cache = Arc::new(agents::CatalogCache::new(db.clone()));
        let catalogs = AgentKind::ALL.into_iter().filter_map(|kind| catalog_cache.cached(kind).map(|catalog| (kind, Loadable::Ready(catalog)))).collect();
        let composer = cx.new(|cx| TextareaState::new(window, cx).rows(3).placeholder("Ask a question or add instructions…"));
        cx.subscribe_in(&composer, window, |this, _, event: &InputEvent, window, cx| {
            if matches!(event, InputEvent::PressEnter { secondary: true, .. }) { this.send(window, cx); }
            if matches!(event, InputEvent::Change) { this.persist_draft(cx); }
            cx.notify();
        }).detach();
        let thread_search = cx.new(|cx| InputState::new(window, cx).placeholder("Search chats"));
        cx.subscribe_in(&thread_search, window, |this, input, event: &InputEvent, _, cx| {
            if matches!(event, InputEvent::Change) { this.thread_query = input.read(cx).value().to_string(); this.thread_selection = 0; this.refresh_threads(); cx.notify(); }
        }).detach();
        let mut this = Self { workspace, db, agents: vec![], detecting: false, catalogs, catalog_cache, requested_catalogs: HashSet::new(), installing: None,
            runs: vec![], next_id: 0, slug: None, ticker: None, scroll: ScrollHandle::new(), composer, composer_thread: None, threads: vec![], selected_thread: None, loaded_threads: HashSet::new(), show_threads: true, thread_scroll: UniformListScrollHandle::new(), thread_search, thread_query: String::new(), thread_selection: 0, conversation_error: None, editing: None, ai_tab: 0, context_key: None };
        if let Err(error) = this.db.migrate_chats() { this.conversation_error = Some(error.to_string()); }
        this.refresh_threads();
        this
    }

    pub fn detect(&mut self, cx: &mut Context<Self>) {
        if self.detecting { return; }
        self.detecting = true;
        cx.spawn(async move |this, cx| {
            let found = cx.background_spawn(async move { agents::detect() }).await;
            let _ = this.update(cx, |this, cx| { this.detecting = false; this.agents = found; cx.notify(); });
        }).detach();
    }

    pub fn detected(&self, kind: AgentKind) -> Option<&Detected> { self.agents.iter().find(|a| a.kind == kind) }

    /// The agent Assist uses: the configured one when installed, else the first detected.
    pub fn target(&self, config: &practice::config::Config) -> Target {
        target_for(config, &self.agents)
    }

    pub fn catalog(&self, kind: AgentKind) -> Option<&Catalog> {
        match self.catalogs.get(&kind) { Some(Loadable::Ready(catalog)) => Some(catalog), _ => None }
    }

    pub fn load_catalog(&mut self, kind: AgentKind, cx: &mut Context<Self>) {
        if self.catalog_requested(kind) { return; }
        let Some(agent) = self.detected(kind).cloned() else { return };
        self.requested_catalogs.insert(kind);
        if self.catalog(kind).is_none() { self.catalogs.insert(kind, Loadable::Loading); }
        let cache = self.catalog_cache.clone();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = cx.background_spawn(async move { cache.fetch(&agent) }).await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(catalog) => { this.catalogs.insert(kind, Loadable::Ready(catalog)); }
                    Err(error) if this.catalog(kind).is_none() => { this.catalogs.insert(kind, Loadable::Failed(error.to_string())); }
                    Err(_) => {}
                }
                cx.notify();
            });
        }).detach();
    }

    pub fn catalog_requested(&self, kind: AgentKind) -> bool { self.requested_catalogs.contains(&kind) }

    pub fn install(&mut self, kind: AgentKind, window: &mut Window, cx: &mut Context<Self>) {
        if self.installing.is_some() { return; }
        self.installing = Some(kind);
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            let result = cx.background_spawn(async move { agents::install(kind) }).await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.installing = None;
                use gpui_kit::component::WindowExt as _;
                match result {
                    Ok(()) => window.push_notification(gpui_kit::component::notification::Notification::success(format!("{} installed", kind.label())), cx),
                    Err(error) => window.push_notification(gpui_kit::component::notification::Notification::error(format!("{} install failed: {error}", kind.label())), cx),
                }
                this.detect(cx);
            });
        }).detach();
    }

    /// Restore the open problem’s own conversation selection.
    pub fn set_problem(&mut self, slug: Option<&str>, cx: &mut Context<Self>) {
        if self.slug.as_deref() == slug { return; }
        self.slug = slug.map(str::to_owned);
        self.selected_thread = None;
        self.editing = None;
        self.refresh_threads();
        let key = self.selection_key();
        let saved = self.db.get(&key).ok().flatten().and_then(|value| value.parse::<i64>().ok());
        let selected = saved.filter(|id| self.threads.iter().any(|thread| thread.id == *id))
            .or_else(|| self.threads.first().map(|thread| thread.id));
        if let Some(id) = selected { self.load_thread(id); }
        if let Some(slug) = slug {
            match self.db.latest_chat_review(slug) { Ok(Some(message)) => self.restore_message(message), Ok(None) => {}, Err(error) => self.conversation_error = Some(error.to_string()) }
        }
        cx.notify();
    }

    fn save(&mut self, slug: &str) {
        for run in self.runs.iter().filter(|run| run.slug == slug) {
            if run.message_id == 0 { continue; }
            if let Err(error) = self.db.save_chat_message(run.message_id, &run.saved()) { self.conversation_error = Some(error.to_string()); }
        }
    }

    fn next_id(&mut self) -> u64 { self.next_id += 1; self.next_id }

    /// The latest finished review of a recorded case: (wrong step, explanation, fix hint), or `None` while running.
    pub fn review(&self, slug: &str, case: usize) -> Option<Result<TraceReview, Phase>> {
        let run = self.runs.iter().find(|run| run.slug == slug && run.snapshot.as_ref().is_some_and(|s| s.trace.is_some() && s.focus_case == case))?;
        Some(match &run.answer {
            Some(Answer::DryRun(dry)) => Ok(TraceReview { wrong_step: dry.wrong_step, explanation: dry.explanation.clone(), fix_hint: dry.fix_hint.clone() }),
            _ => Err(run.phase.clone()),
        })
    }

    pub fn running(&self) -> Option<(&Run, Duration)> {
        self.runs.iter().find(|run| run.phase.active()).map(|run| (run, run.time()))
    }

    /// Starts `action`, or opens the web chat when no agent is installed.
    pub fn start(&mut self, action: Action, snapshot: Snapshot, target: Target, window: &mut Window, cx: &mut Context<Self>) {
        self.set_problem((!snapshot.slug.is_empty()).then_some(snapshot.slug.as_str()), cx);
        self.sync_composer(window, cx);
        let instructions = self.composer.read(cx).value().trim().to_string();
        let editing = self.editing.is_some();
        let action = self.editing.and_then(|id| self.runs.iter().find(|run| run.id == id)).map_or(action, |run| run.action);
        if action == Action::Ask && instructions.is_empty() { return; }
        if instructions.chars().count() > practice::chat::INSTRUCTIONS_LIMIT {
            use gpui_kit::component::WindowExt as _;
            window.push_notification(gpui_kit::component::notification::Notification::warning("Keep the prompt under 8,000 characters"), cx);
            return;
        }
        if let Some(id) = self.editing.take() {
            let Some(run) = self.runs.iter().find(|run| run.id == id) else { return };
            match self.db.fork_chat(run.thread_id, run.message_id, false) {
                Ok(thread) => { self.selected_thread = Some(thread.id); self.refresh_threads(); self.load_thread(thread.id); self.composer_thread = Some(thread.id); }
                Err(error) => { self.conversation_error = Some(error.to_string()); cx.notify(); return; }
            }
        }
        let thread_id = match self.ensure_thread() { Ok(id) => id, Err(error) => { self.conversation_error = Some(error.to_string()); cx.notify(); return; } };
        let history = practice::chat::context(self.runs.iter().filter(|run| run.thread_id == thread_id).filter_map(|run|
            Some(practice::chat::Turn { action: run.action, instructions: format!("{}\n{}\nProblem: {}\nNative artifact: {}", run.request_prompt, run.instructions, run.problem_title, practice::chat::artifacts::path(run.message_id).display()), answer: run.answer.clone()? })));
        if let Target::Web(provider) = target {
            let prompt = practice::chat::prompt(assist::web_prompt(action, &snapshot.context(None)), &history, &instructions);
            cx.write_to_clipboard(ClipboardItem::new_string(prompt.clone()));
            let _ = open::that_detached(practice::prompts::url(provider, &prompt));
            use gpui_kit::component::WindowExt as _;
            window.push_notification(gpui_kit::component::notification::Notification::info(format!("{} opened in {} · prompt copied", action.label(), provider.label())), cx);
            let id = self.next_id();
            let mut run = Run::new(id, snapshot.slug.clone(), action, None, provider.label().to_owned(), None, None);
            run.thread_id = thread_id; run.problem_title = snapshot.title.clone(); run.instructions = instructions;
            run.phase = Phase::Done; run.elapsed = Some(Duration::ZERO);
            match self.db.append_chat(thread_id, &run.saved()) { Ok(message) => { run.message_id = message; self.runs.insert(0, run); }, Err(error) => self.conversation_error = Some(error.to_string()) }
            self.composer.update(cx, |input, cx| input.set_value("", window, cx)); self.refresh_threads(); cx.notify();
            return;
        }
        let previous: Vec<_> = self.runs.iter().filter(|run| (run.thread_id == thread_id || (action == Action::Solve && run.slug == snapshot.slug && run.solve.is_some())) && (run.phase.active() || run.phase == Phase::Confirm)).map(|run| run.id).collect();
        for id in previous { self.stop(id, cx); }
        let id = self.next_id();
        let Target::Agent(agent, selection) = &target else { return };
        let model = selection.as_ref().map(|s| s.model.clone()).unwrap_or_default();
        let mut run = Run::new(id, snapshot.slug.clone(), action, Some(agent.kind), model, Some(snapshot.clone()), Some(target.clone()));
        if action == Action::Solve { run.solve = Some(SolveState { attempt: 1, session: None, log: vec![], accepted: false }); }
        run.thread_id = thread_id; run.problem_title = snapshot.title.clone();
        run.request_prompt = if editing && action != Action::Ask { instructions.clone() } else { action.instructions().to_owned() };
        run.instructions = if editing && action != Action::Ask { String::new() } else { instructions }; run.history = history;
        match self.db.append_chat(thread_id, &run.saved()) { Ok(message) => run.message_id = message, Err(error) => { self.conversation_error = Some(error.to_string()); cx.notify(); return; } }
        if self.threads.iter().any(|thread| thread.id == thread_id && thread.title == "New chat") {
            let title = if run.instructions.is_empty() { action.label().to_owned() } else { run.instructions.lines().next().unwrap_or(action.label()).chars().take(60).collect() };
            let _ = self.db.rename_chat(thread_id, &title);
        }
        self.refresh_threads();
        self.composer.update(cx, |input, cx| input.set_value("", window, cx));
        self.runs.insert(0, run);
        self.save(&snapshot.slug);
        self.scroll.scroll_to_bottom();
        self.launch(id, None, None, window, cx);
    }

    /// Spawns the agent thread for run `id`, optionally continuing a session with feedback.
    fn launch(&mut self, id: u64, resume: Option<String>, feedback: Option<String>, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(kind) = self.runs.iter().find(|run| run.id == id).and_then(|run| run.agent) { self.load_catalog(kind, cx); }
        let Some(run) = self.runs.iter_mut().find(|run| run.id == id) else { return };
        let (Some(snapshot), Some(Target::Agent(agent, selection))) = (run.snapshot.clone(), run.target.clone()) else { return };
        run.phase = Phase::Starting;
        run.cancel = Cancel::default();
        run.generation += 1;
        let generation = run.generation;
        if let Some(elapsed) = run.elapsed.take() { run.started = Instant::now() - elapsed; }
        run.thinking.clear();
        run.tokens = 0;
        let cancel = run.cancel.clone();
        let action = run.action;
        let message_id = run.message_id;
        let request_prompt = run.request_prompt.clone();
        let instructions = run.instructions.clone();
        let history = run.history.clone();
        run.reply.clear();
        let fallback = self.catalog(agent.kind).cloned();
        let cache = self.catalog_cache.clone();
        let (tx, mut rx) = futures::channel::mpsc::unbounded::<Msg>();
        std::thread::spawn(move || {
            let selection = match selection {
                Some(selection) => selection,
                None => {
                    let catalog = match fallback { Some(catalog) => Ok(catalog), None => cache.fetch(&agent) };
                    match catalog.and_then(|catalog| default_selection(&catalog).ok_or_else(|| anyhow::anyhow!("{} reported no models", agent.kind.label()))) {
                        Ok(selection) => { let _ = tx.unbounded_send(Msg::Selected(selection.clone())); selection }
                        Err(error) => { let _ = tx.unbounded_send(Msg::Done(Err(error))); return; }
                    }
                }
            };
            let base = assist::prompt(action, &snapshot.context(feedback.as_deref())).replacen(action.instructions(), &request_prompt, 1);
            let prompt = practice::chat::prompt(base, &history, &instructions);
            let artifact = practice::chat::artifacts::path(message_id);
            let artifact_dir = practice::chat::artifacts::directory(message_id);
            if let Err(error) = std::fs::create_dir_all(&artifact_dir).and_then(|_| std::fs::write(artifact_dir.join("schema.json"), action.schema().to_string())) { let _ = tx.unbounded_send(Msg::Done(Err(error.into()))); return; }
            let mut prompt = prompt;
            prompt.push_str(&format!("\nWorkspace: {}\nEditable native response artifact: {}\nNative response JSON schema file: {}\nUse your file tools to write or revise this artifact when creating a visualization or other artifact. It uses the same response envelope as your final reply. Previous artifacts are listed in conversation history.\n", snapshot.workspace.display(), artifact.display(), artifact_dir.join("schema.json").display()));
            let request = agents::Request {
                agent, selection, prompt,
                schema: Some(action.schema()),
                cwd: snapshot.workspace.clone(),
                access: action.access(), resume,
            };
            let sender = tx.clone();
            let result = assist::execute_artifact(action, &request, &artifact, &mut |event| { let _ = sender.unbounded_send(Msg::Event(event)); }, &cancel)
                .and_then(|(answer, outcome)| {
                    let answer = if artifact.exists() { Some(practice::chat::artifacts::load(&artifact)?) } else {
                        if let Some(answer) = &answer { practice::chat::artifacts::save(&artifact, answer)?; } answer
                    };
                    Ok((answer, outcome.session))
                });
            let _ = tx.unbounded_send(Msg::Done(result));
        });
        self.ensure_ticker(cx);
        cx.spawn_in(window, async move |this, cx| {
            while let Some(msg) = rx.next().await {
                if this.update_in(cx, |this, window, cx| this.receive(id, generation, msg, window, cx)).is_err() { break; }
            }
        }).detach();
        cx.notify();
    }

    fn receive(&mut self, id: u64, generation: u64, msg: Msg, window: &mut Window, cx: &mut Context<Self>) {
        let Some(run) = self.runs.iter_mut().find(|run| run.id == id) else { return };
        if !run.accepts_events(generation) { return; }
        let save = matches!(&msg, Msg::Done(_));
        let follow = self.scroll.offset().y <= -self.scroll.max_offset().y + px(24.);
        match msg {
            Msg::Event(agents::Event::Started { session }) => {
                if let (Some(solve), Some(session)) = (run.solve.as_mut(), session) { solve.session = Some(session); }
                run.phase = Phase::Thinking;
            }
            Msg::Event(agents::Event::Thinking(text)) => { run.thinking.push_str(&text); run.phase = Phase::Thinking; }
            Msg::Event(agents::Event::Text(text)) => { if run.action == Action::Ask { run.reply.push_str(&text); } run.phase = Phase::Writing; },
            Msg::Event(agents::Event::Tool { title, done, .. }) => run.phase = if done { Phase::Thinking } else { Phase::Tool(title) },
            Msg::Event(agents::Event::Usage { input_tokens, output_tokens }) => run.tokens = input_tokens + output_tokens,
            Msg::Selected(selection) => {
                run.model = selection.model.clone();
                if let Some(Target::Agent(_, chosen)) = &mut run.target { *chosen = Some(selection.clone()); }
                let workspace = self.workspace.clone();
                window.defer(cx, move |window, cx| {
                    let _ = workspace.update(cx, |ws, cx| { ws.config.remember(selection); ws.save_config(window, cx); cx.notify(); });
                });
            }
            Msg::Done(result) => {
                let cancelled = run.cancel.is_cancelled();
                match result {
                    Err(_) if cancelled => { run.phase = Phase::Stopped; run.elapsed = Some(run.started.elapsed()); }
                    Err(error) => { run.phase = Phase::Failed(first_line(&error.to_string())); run.elapsed = Some(run.started.elapsed()); }
                    Ok((answer, session)) if run.solve.is_some() || matches!(answer, Some(Answer::Solve(_))) => {
                        if run.solve.is_none() { run.solve = Some(SolveState { attempt: 1, session: None, log: vec![], accepted: false }); }
                        if let Some(solve) = run.solve.as_mut() {
                            solve.session = session.or(solve.session.take());
                            solve.log.push((Tone::Done, format!("Attempt {} written", solve.attempt)));
                        }
                        if answer.is_some() { run.set_answer(answer); }
                        run.phase = solve_written_phase(run.action, run.solve.as_ref().map_or(1, |solve| solve.attempt));
                        let test = run.phase == Phase::Testing;
                        let (workspace, slug) = (self.workspace.clone(), run.slug.clone());
                        window.defer(cx, move |window, cx| {
                            let _ = workspace.update(cx, |ws, cx| {
                                if ws.session.as_ref().is_some_and(|s| s.slug == slug) {
                                    if ws.session.as_ref().is_some_and(|s| s.running || matches!(s.judge, Some(crate::workspace::Judge::Running { .. }))) {
                                        ws.assist.update(cx, |assist, cx| assist.fail_solve(&slug, "Current tests or judge run must finish before proceeding".into(), cx));
                                    } else { ws.reload_solution(&slug, window, cx); if test { ws.run_tests(window, cx); } }
                                }
                                else { ws.assist.update(cx, |assist, cx| assist.fail_solve(&slug, "Problem changed; solve stopped".into(), cx)); }
                            });
                        });
                    }
                    Ok((answer, _)) => {
                        run.phase = Phase::Done; run.elapsed = Some(run.started.elapsed());
                        run.set_answer(answer);
                    }
                }
            }
        }
        if let Some((slug, owner)) = self.runs.iter().find(|run| run.id == id && run.solve.is_some() && matches!(run.phase, Phase::Testing | Phase::Confirm)).map(|run| (run.slug.clone(), run.id)) {
            let previous: Vec<_> = self.runs.iter().filter(|run| run.id != owner && run.slug == slug && run.solve.is_some() && (run.phase.active() || run.phase == Phase::Confirm)).map(|run| run.id).collect();
            for other in previous { self.stop(other, cx); }
        }
        if save {
            if let Some(thread) = self.runs.iter().find(|run| run.id == id).map(|run| run.thread_id) { self.refresh_artifacts(thread); }
            if let Some(slug) = self.runs.iter().find(|run| run.id == id).map(|run| run.slug.clone()) { self.save(&slug); }
        }
        if follow { self.scroll.scroll_to_bottom(); }
        cx.notify();
    }

    /// Called by the workspace when a local or judge run of `slug` finishes.
    pub fn tests_finished(&mut self, slug: &str, passed: bool, report: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(run) = self.runs.iter_mut().find(|run| run.slug == slug && run.solve.is_some() && run.phase == Phase::Testing) else { return };
        let Some(solve) = run.solve.as_mut() else { return };
        if passed {
            solve.log.push((Tone::Done, "Tests pass".into()));
            run.phase = Phase::Confirm;
        } else {
            solve.log.push((Tone::Error, first_line(&report)));
            self.retry(slug, report, window, cx);
        }
        cx.notify();
    }

    /// Called by the workspace when a submission of `slug` is judged.
    pub fn judge_finished(&mut self, slug: &str, accepted: bool, report: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(run) = self.runs.iter_mut().find(|run| run.slug == slug && run.solve.is_some() && run.phase == Phase::Judging) else { return };
        let Some(solve) = run.solve.as_mut() else { return };
        if accepted {
            solve.accepted = true;
            solve.log.push((Tone::Done, "Accepted".into()));
            run.phase = Phase::Done; run.elapsed = Some(run.started.elapsed());
        } else {
            solve.log.push((Tone::Error, first_line(&report)));
            self.retry(slug, report, window, cx);
        }
        self.save(slug);
        cx.notify();
    }

    fn retry(&mut self, slug: &str, report: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(run) = self.runs.iter_mut().find(|run| run.slug == slug && run.solve.is_some()) else { return };
        let Some(solve) = run.solve.as_mut() else { return };
        if solve.attempt >= SOLVE_ATTEMPTS {
            run.phase = Phase::Failed(format!("Not accepted after {SOLVE_ATTEMPTS} attempts"));
            run.elapsed = Some(run.started.elapsed());
            return;
        }
        solve.attempt += 1;
        let (id, session) = (run.id, solve.session.clone());
        self.launch(id, session, Some(report), window, cx);
    }

    fn submit(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        let Some(run) = self.runs.iter_mut().find(|run| run.id == id && run.phase == Phase::Confirm) else { return };
        let (workspace, slug) = (self.workspace.clone(), run.slug.clone());
        run.phase = Phase::Judging;
        if let Some(solve) = run.solve.as_mut() { solve.log.push((Tone::Active, "Submitting".into())); }
        window.defer(cx, move |window, cx| {
            let _ = workspace.update(cx, |ws, cx| {
                if !ws.session.as_ref().is_some_and(|s| s.slug == slug) {
                    ws.assist.update(cx, |assist, cx| assist.fail_solve(&slug, "Open this problem before submitting".into(), cx));
                } else if practice::language::Source::for_problem(&slug).is_stdin() {
                    ws.judge(true, window, cx);
                    ws.assist.update(cx, |assist, cx| {
                        if let Some(run) = assist.runs.iter_mut().find(|run| run.id == id) {
                            run.phase = Phase::Done; run.elapsed = Some(run.started.elapsed());
                            if let Some(solve) = &mut run.solve { solve.log.push((Tone::Default, format!("Copied solution; submit and check the verdict in {}", practice::language::Source::for_problem(&slug).label()))); }
                        }
                        cx.notify();
                    });
                } else if !ws.client.signed_in() {
                    ws.assist.update(cx, |assist, cx| assist.fail_solve(&slug, "Sign in to LeetCode before submitting".into(), cx));
                } else { ws.judge(true, window, cx); }
            });
        });
        self.ensure_ticker(cx);
        cx.notify();
    }

    pub fn fail_solve(&mut self, slug: &str, error: String, cx: &mut Context<Self>) {
        if let Some(run) = self.runs.iter_mut().find(|run| run.slug == slug && run.solve.is_some() && (run.phase.active() || run.phase == Phase::Confirm)) {
            run.cancel.cancel(); run.phase = Phase::Failed(first_line(&error)); run.elapsed = Some(run.started.elapsed());
        }
        cx.notify();
    }

    fn stop(&mut self, id: u64, cx: &mut Context<Self>) {
        if let Some(run) = self.runs.iter_mut().find(|run| run.id == id) {
            run.cancel.cancel();
            run.phase = Phase::Stopped; run.elapsed = Some(run.started.elapsed());
            let slug = run.slug.clone();
            self.save(&slug);
        }
        cx.notify();
    }

    fn recover(&mut self, id: u64, continue_run: bool, window: &mut Window, cx: &mut Context<Self>) {
        let workspace = self.workspace.clone();
        window.defer(cx, move |window, cx| {
            let _ = workspace.update(cx, |ws, cx| {
                if ws.session.as_ref().is_some_and(|session| session.running || matches!(session.judge, Some(crate::workspace::Judge::Running { .. }))) {
                    ws.flash("Wait for the current run to finish", cx);
                    return;
                }
                // An agent edit can arrive before the file watcher refreshes the editor.
                let reload = ws.assist.read(cx).runs.iter().find(|run| run.id == id)
                    .filter(|run| run.solve.is_some() && ws.session.as_ref().is_some_and(|session| session.slug == run.slug))
                    .and_then(|run| run.snapshot.as_ref())
                    .filter(|snapshot| ws.editor.read(cx).value().as_ref() == snapshot.code)
                    .map(|snapshot| snapshot.slug.clone());
                if let Some(slug) = reload { ws.reload_solution(&slug, window, cx); }
                ws.save_now(cx);
                let Some(snapshot) = ws.assist_snapshot(cx) else { return };
                ws.assist.update(cx, |assist, cx| {
                    if continue_run {
                        let others: Vec<_> = assist.runs.iter().filter(|run| run.id != id && run.slug == snapshot.slug && run.solve.is_some()
                            && (run.phase.active() || run.phase == Phase::Confirm)).map(|run| run.id).collect();
                        for other in others { assist.stop(other, cx); }
                    }
                    let target = assist.target(&ws.config);
                    let Some(run) = assist.runs.iter_mut().find(|run| run.id == id && matches!(run.phase, Phase::Stopped | Phase::Failed(_))) else { return };
                    if run.target.is_none() { run.target = Some(target); }
                    if run.slug != snapshot.slug || run.snapshot.as_ref().is_some_and(|old| old.language != snapshot.language) { return; }
                    if run.action.needs_attempt() && !snapshot.has_attempt() { return; }
                    if continue_run && run.solve.is_some() {
                        let session = run.solve.as_ref().and_then(|solve| solve.session.clone());
                        let feedback = format!("Continue the interrupted solve from the current solution. Inspect the file before editing; preserve completed work. Previous status: {}.", run.phase.label());
                        run.snapshot = Some(snapshot);
                        if let Some(solve) = &mut run.solve {
                            if solve.attempt >= SOLVE_ATTEMPTS { solve.attempt = 1; solve.log.clear(); }
                            solve.log.push((Tone::Default, "Continuing".into()));
                        }
                        assist.launch(id, session, Some(feedback), window, cx);
                    } else {
                        run.snapshot = Some(snapshot);
                        assist.launch(id, None, None, window, cx);
                    }
                });
            });
        });
    }

    pub fn stop_solves(&mut self, cx: &mut Context<Self>) {
        let ids: Vec<_> = self.runs.iter().filter(|run| run.solve.is_some() && (run.phase.active() || run.phase == Phase::Confirm)).map(|run| run.id).collect();
        for id in ids { self.stop(id, cx); }
    }

    pub fn stop_all(&mut self, cx: &mut Context<Self>) {
        let ids: Vec<u64> = self.runs.iter().filter(|run| run.phase.active() || run.phase == Phase::Confirm).map(|run| run.id).collect();
        for id in ids { self.stop(id, cx); }
    }

    /// Redraws the stopwatch and advances playing walkthroughs.
    fn ensure_ticker(&mut self, cx: &mut Context<Self>) {
        if self.ticker.is_some() { return; }
        self.ticker = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_millis(100)).await;
                let alive = this.update(cx, |this, cx| {
                    let mut busy = false;
                    for run in &mut this.runs {
                        busy |= run.phase.active();
                        if run.playback.playing {
                            busy = true;
                            if run.last_frame.elapsed() >= Duration::from_millis(run.playback.interval_ms()) { run.playback.advance(); run.last_frame = Instant::now(); }
                        }
                    }
                    cx.notify();
                    if !busy { this.ticker = None; }
                    busy
                });
                if !matches!(alive, Ok(true)) { break; }
            }
        }));
    }

    fn control(&mut self, id: u64, control: player::Control, cx: &mut Context<Self>) {
        if let Some(run) = self.runs.iter_mut().find(|run| run.id == id) && player::apply(&mut run.playback, control) { run.last_frame = Instant::now(); }
        self.ensure_ticker(cx);
        cx.notify();
    }
}

#[cfg(test)]
mod recovery_tests {
    #[test]
    fn quick_solve_checks_only_after_the_first_rejection() {
        assert_eq!(super::solve_written_phase(practice::assist::Action::Solve, 1), super::Phase::Confirm);
        assert_eq!(super::solve_written_phase(practice::assist::Action::Solve, 2), super::Phase::Testing);
        assert_eq!(super::solve_written_phase(practice::assist::Action::Ask, 1), super::Phase::Testing);
    }
    use super::Run;
    use practice::assist::Action;
    use practice::agents::Cancel;

    #[test]
    fn legacy_answers_and_interrupted_chat_requests_restore() {
        let legacy: super::Saved = serde_json::from_value(serde_json::json!({
            "model": "old-model", "agent": null, "answer": {"action": "hints", "answer": {"hints": []}}
        })).unwrap();
        assert!(legacy.instructions.is_empty());
        assert!(matches!(legacy.phase, super::SavedPhase::Done));
        assert!(matches!(legacy.answer, Some(practice::assist::Answer::Hints(_))));
        let mut run = Run::new(1, "two-sum".into(), Action::Ask, None, String::new(), None, None);
        run.instructions = "Why check before inserting?".into();
        let saved = run.saved();
        let restored: super::Saved = serde_json::from_str(&serde_json::to_string(&saved).unwrap()).unwrap();
        assert_eq!(restored.instructions, run.instructions);
        assert_eq!(restored.action, Some(Action::Ask));
        assert!(matches!(restored.phase, super::SavedPhase::Stopped));
        assert!(restored.answer.is_none());
    }

    #[test]
    fn resumed_runs_ignore_events_from_the_stopped_process() {
        let mut run = Run::new(1, "two-sum".into(), Action::Solve, None, String::new(), None, None);
        run.generation = 1;
        assert!(run.accepts_events(1));
        run.cancel.cancel();
        assert!(!run.accepts_events(1));
        run.cancel = Cancel::default();
        run.generation = 2;
        assert!(!run.accepts_events(1));
        assert!(run.accepts_events(2));
    }
}

fn default_selection(catalog: &Catalog) -> Option<Selection> {
    let model = catalog.default_model.as_ref().and_then(|id| catalog.models.iter().find(|m| &m.id == id)).or(catalog.models.first())?;
    let effort = model.default_effort.clone().or_else(|| model.efforts.first().map(|e| e.id.clone()));
    Some(Selection { agent: catalog.agent, model: model.id.clone(), effort, fast: false })
}

fn first_line(text: &str) -> String {
    let line = text.lines().find(|l| !l.trim().is_empty()).unwrap_or(text).trim();
    if line.chars().count() > 160 { format!("{}…", line.chars().take(160).collect::<String>()) } else { line.to_owned() }
}

impl Run {
    fn new(id: u64, slug: String, action: Action, agent: Option<AgentKind>, model: String, snapshot: Option<Snapshot>, target: Option<Target>) -> Self {
        Self { id, thread_id: 0, message_id: 0, problem_title: String::new(), slug, action, agent, model, started: Instant::now(), elapsed: None, artifact_mtime: None, phase: Phase::Starting, thinking: String::new(), tokens: 0,
            answer: None, cancel: Cancel::default(), hints_shown: 1, playback: Playback::new(0), last_frame: Instant::now(), reveal: false,
            added: vec![], solve: None, snapshot, target, generation: 0, collapsed: false, instructions: String::new(), request_prompt: action.instructions().to_owned(), history: String::new(), reply: String::new() }
    }

    fn set_answer(&mut self, answer: Option<Answer>) {
        let frames = match &answer {
            Some(Answer::Review(review)) => review.dry_run.scene.frames.len(),
            Some(Answer::Visualize(scene)) => scene.frames.len(),
            Some(Answer::Explain(explain)) => explain.walkthrough.frames.len(),
            Some(Answer::DryRun(dry)) => dry.scene.frames.len(),
            _ => 0,
        };
        self.playback = Playback::new(frames);
        if let Some(Answer::DryRun(dry)) = &answer && let Some(wrong) = dry.wrong_frame { self.playback.seek(wrong); }
        self.answer = answer;
    }
}

// ---- presentation --------------------------------------------------------------------

pub fn icon(action: Action) -> IconName {
    match action {
        Action::Hints => IconName::Lightbulb,
        Action::Stuck => IconName::LifeBuoy,
        Action::Bugs => IconName::Bug,
        Action::Tests => IconName::FlaskConical,
        Action::Analyze => IconName::Gauge,
        Action::Optimize => IconName::Rocket,
        Action::Visualize => IconName::Sparkles,
        Action::DryRun => IconName::Footprints,
        Action::Pattern => IconName::Puzzle,
        Action::Explain => IconName::BookOpen,
        Action::Solve => IconName::WandSparkles,
        Action::Ask => IconName::MessageCircle,
        Action::Review => IconName::Code,
    }
}

/// Each action keeps one pastel accent everywhere it appears.
pub fn accent(action: Action, theme: &Theme) -> Hsla {
    match action {
        Action::Hints | Action::Optimize => theme.warning,
        Action::Stuck | Action::Explain => theme.info,
        Action::Bugs => theme.danger,
        Action::Tests | Action::Solve => theme.success,
        Action::Analyze | Action::Pattern => rgb(0xc3b1ff).into(),
        Action::Visualize | Action::DryRun | Action::Ask | Action::Review => theme.primary,
    }
}

fn clock(duration: Duration) -> String {
    let secs = duration.as_secs_f32();
    if secs < 60. { format!("{secs:.1}s") } else { format!("{}:{:02}", secs as u64 / 60, secs as u64 % 60) }
}

impl Assist {
    fn card(&self, run: &Run, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let color = accent(run.action, &theme);
        let id = run.id;
        let active = run.phase.active();
        let request_prompt = run.request_prompt.clone();
        let header = h_flex().gap_2().items_center()
            .child(Button::new(("assist-collapse", id)).ghost().xsmall().icon(if run.collapsed { IconName::ChevronRight } else { IconName::ChevronDown })
                .tooltip(if run.collapsed { "Expand result" } else { "Collapse result" }).accessibility_label(if run.collapsed { "Expand result" } else { "Collapse result" })
                .on_click(cx.listener(move |this, _, _, cx| {
                    if let Some(run) = this.runs.iter_mut().find(|run| run.id == id) { run.collapsed = !run.collapsed; if run.collapsed { run.playback.playing = false; } }
                    cx.notify();
                })))
            .child(div().size(px(24.)).rounded_lg().flex().items_center().justify_center().bg(color.opacity(0.14)).child(Icon::new(icon(run.action)).size_3p5().text_color(color)))
            .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child("Assistant"))
            .when_some(run.agent, |el, agent| el.child(crate::brand::agent_icon(agent).xsmall()))
            .child(div().min_w_0().flex_1().truncate().text_xs().text_color(theme.muted_foreground).child(run.model.clone()))
            .when(run.elapsed != Some(Duration::ZERO), |el| el.child(h_flex().gap_1().text_xs().text_color(if active { color } else { theme.muted_foreground })
                .font_family(theme.mono_font_family.clone()).child(Icon::new(IconName::Timer).size_3()).child(clock(run.time()))))
            .child(if active || run.phase == Phase::Confirm {
                Button::new(("assist-stop", id)).ghost().xsmall().icon(IconName::CircleStop).tooltip("Stop").accessibility_label("Stop")
                    .on_click(cx.listener(move |this, _, _, cx| this.stop(id, cx))).into_any_element()
            } else {
                let weak = cx.entity().downgrade();
                use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
                Button::new(("chat-message-options", id)).ghost().xsmall().icon(IconName::Ellipsis).tooltip("Message actions")
                    .dropdown_menu(move |menu, _, _| {
                        let fork = weak.clone(); let edit = weak.clone(); let reload = weak.clone();
                        menu.item(PopupMenuItem::new("Fork from here").icon(IconName::GitBranch).on_click(move |_, window, cx| { let _ = fork.update(cx, |this, cx| this.fork_message(id, window, cx)); }))
                            .item(PopupMenuItem::new("Edit request").icon(IconName::Pencil).on_click(move |_, window, cx| { let _ = edit.update(cx, |this, cx| this.edit_message(id, window, cx)); }))
                            .separator()
                            .item(PopupMenuItem::new("Reload artifact").icon(IconName::RefreshCw).on_click(move |_, window, cx| { let _ = reload.update(cx, |this, cx| this.reload_artifact(id, window, cx)); }))
                    }).into_any_element()
            });
        let status = (active || matches!(run.phase, Phase::Failed(_) | Phase::Stopped)).then(|| {
            let label = run.phase.label();
            let thinking: String = run.thinking.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or_default().chars().take(140).collect();
            v_flex().gap_1()
                .child(h_flex().gap_2().items_center().text_xs()
                    .child(match &run.phase {
                        Phase::Failed(_) => Icon::new(IconName::CircleX).size_3p5().text_color(theme.danger).into_any_element(),
                        Phase::Stopped => Icon::new(IconName::CircleStop).size_3p5().text_color(theme.muted_foreground).into_any_element(),
                        _ => gpui_kit::component::spinner::Spinner::new().xsmall().color(color).into_any_element(),
                    })
                    .child(if active { ShimmerText::new(format!("{label}…")).id(("assist-phase", id)).into_any_element() }
                        else { div().text_color(if matches!(run.phase, Phase::Failed(_)) { theme.danger } else { theme.muted_foreground }).child(label).into_any_element() })
                    .when(run.tokens > 0, |el| el.child(div().text_color(theme.muted_foreground).child(format!("· {} tok", run.tokens)))))
                .when(active && !thinking.is_empty(), |el| el.child(div().pl_6().text_xs().italic().text_color(theme.muted_foreground).truncate().child(thinking)))
        });
        let body = run.answer.as_ref().filter(|_| !run.collapsed).map(|answer| self.body(run, answer, window, cx));
        let solve = run.solve.as_ref().filter(|_| !run.collapsed || run.phase == Phase::Confirm).map(|solve| self.solve_body(run, solve, cx));
        v_flex().id(("assist-card", id)).p_3().gap_2p5().rounded_lg().bg(theme.background.opacity(0.55)).border_1()
            .border_color(if active { color.opacity(0.45) } else { theme.border })
            .child(v_flex().gap_1().pb_2().border_b_1().border_color(theme.border)
                .child(div().text_xs().text_color(theme.muted_foreground).child("You"))
                .when(run.action != Action::Ask, |el| el.child(h_flex().id(("chat-action-prompt", id)).gap_2().items_center().child(Icon::new(icon(run.action)).small().text_color(color)).child(div().text_sm().child(run.action.label()))
                    .tooltip(move |window, cx| gpui_kit::component::tooltip::Tooltip::new(request_prompt.clone()).build(window, cx))))
                .when(!run.instructions.is_empty(), |el| el.child(crate::rich_text::markdown(SharedString::from(format!("chat-request-{id}")), run.instructions.clone()).selectable(true))))
            .child(header)
            .children(status)
            .when(run.answer.is_none() && !run.reply.is_empty() && !run.reply.trim_start().starts_with('{'), |el| el.child(
                crate::rich_text::markdown(SharedString::from(format!("chat-stream-{id}")), run.reply.clone()).selectable(true)))
            .when(matches!(run.phase, Phase::Stopped | Phase::Failed(_)), |el| el.child(h_flex().gap_2()
                .when(run.solve.is_some(), |el| el.child(Button::new(("assist-continue", id)).primary().small().icon(IconName::Play).label("Continue")
                    .tooltip("Continue using the current solution and agent session when available")
                    .on_click(cx.listener(move |this, _, window, cx| this.recover(id, true, window, cx)))))
                .child(Button::new(("assist-retry", id)).ghost().small().icon(IconName::RefreshCw).label("Retry")
                    .tooltip("Start a new run using the current code")
                    .on_click(cx.listener(move |this, _, window, cx| this.recover(id, false, window, cx))))))
            .children(solve)
            .children(body)
            .with_animation(("assist-card-in", id), Animation::new(Duration::from_millis(260)).with_easing(gpui_kit::base::animation::ease_out_cubic),
                |el, t| el.opacity(t).mt(px(8. * (1. - t))))
            .into_any_element()
    }

    fn solve_body(&self, run: &Run, solve: &SolveState, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let id = run.id;
        v_flex().gap_1p5()
            .child(h_flex().gap_1().children((1..=SOLVE_ATTEMPTS).map(|n| {
                let color = if n < solve.attempt { theme.danger.opacity(0.7) } else if n == solve.attempt { if solve.accepted { theme.success } else { theme.primary } } else { theme.muted };
                div().h(px(4.)).flex_1().rounded_full().bg(color)
            })))
            .children(solve.log.iter().map(|(tone, text)| {
                let (_, _, fg) = crate::gen_ui::style::tone_colors(*tone, &theme);
                h_flex().gap_2().text_xs().child(div().size(px(6.)).rounded_full().bg(fg)).child(div().text_color(theme.foreground).child(text.clone()))
            }))
            .when(run.phase == Phase::Confirm, |el| el.child(h_flex().gap_2().pt_1()
                .child(Button::new(("assist-submit", id)).primary().small().icon(IconName::Send).label("Submit")
                    .tooltip("Submit this solution to the judge").on_click(cx.listener(move |this, _, window, cx| this.submit(id, window, cx))))
                .child(Button::new(("assist-keep", id)).ghost().small().label("Not now")
                    .on_click(cx.listener(move |this, _, _, cx| this.stop(id, cx))))))
            .when(solve.accepted, |el| el.child(h_flex().gap_2().items_center().text_sm().text_color(theme.success)
                .child(Icon::new(IconName::CircleCheck).size_4()).child("Accepted")
                .with_animation(("solve-accepted", id), Animation::new(Duration::from_millis(500)).with_easing(gpui_kit::base::animation::ease_out_cubic), |el, t| el.opacity(t))))
            .into_any_element()
    }

    fn body(&self, run: &Run, answer: &Answer, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let id = run.id;
        let workspace = self.workspace.clone();
        let jump = move |line: u32| {
            let workspace = workspace.clone();
            move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
                let workspace = workspace.clone();
                window.defer(cx, move |window, cx| { let _ = workspace.update(cx, |ws, cx| ws.jump_to_line(line, window, cx)); });
            }
        };
        let line_badge = |line: u32, key: SharedString| {
            div().id(key).flex_shrink_0().px_1p5().h(px(18.)).flex().items_center().rounded_md().bg(theme.secondary).cursor_pointer()
                .hover(|s| s.bg(theme.list_hover)).text_size(px(10.)).font_family(theme.mono_font_family.clone()).text_color(theme.info)
                .child(format!("L{line}")).on_click(jump(line))
        };
        let sequence = std::cell::Cell::new(0usize);
        let rich = |text: &str| {
            let index = sequence.get(); sequence.set(index + 1);
            crate::rich_text::markdown(SharedString::from(format!("answer-{id}-{:?}-{index}", std::mem::discriminant(answer))), text)
        };
        let prose = |text: &str| div().min_w_0().text_sm().text_color(theme.foreground).child(rich(text));
        let muted = |text: &str| div().min_w_0().text_xs().text_color(theme.muted_foreground).child(rich(text));
        let tag = |text: &str, color: Hsla| div().px_2().min_h(px(20.)).flex().items_center().rounded_full()
            .bg(color.opacity(0.14)).border_1().border_color(color.opacity(0.2))
            .text_size(px(11.)).text_color(color).font_weight(FontWeight::MEDIUM).child(rich(text));
        let mono = |text: &str| div().p_2().rounded_md().bg(theme.muted).font_family(theme.mono_font_family.clone()).text_xs().whitespace_normal().child(text.to_owned());
        let label = |text: &'static str| div().text_xs().font_weight(FontWeight::MEDIUM).text_color(theme.muted_foreground).child(text);
        match answer {
            Answer::Review(review) => v_flex().gap_4()
                .child(self.body(run, &Answer::Bugs(review.correctness.clone()), window, cx))
                .child(self.body(run, &Answer::Analyze(review.complexity.clone()), window, cx))
                .child(self.body(run, &Answer::Optimize(review.improvements.clone()), window, cx))
                .child(self.body(run, &Answer::DryRun(review.dry_run.clone()), window, cx)).into_any_element(),
            Answer::Chat(text) => crate::rich_text::markdown(SharedString::from(format!("chat-answer-{id}")), text.clone()).selectable(true).into_any_element(),
            Answer::Hints(hints) => {
                let shown = run.hints_shown.min(hints.hints.len());
                v_flex().gap_2()
                    .children(hints.hints.iter().take(shown).enumerate().map(|(i, hint)| {
                        h_flex().gap_2().items_start()
                            .child(div().mt(px(2.)).size(px(18.)).flex_shrink_0().rounded_full().flex().items_center().justify_center().bg(theme.warning.opacity(0.18))
                                .text_size(px(10.)).text_color(theme.warning).child((i + 1).to_string()))
                            .child(v_flex().gap_0p5().min_w_0().flex_1().child(div().text_sm().font_weight(FontWeight::MEDIUM).child(rich(&hint.title))).child(muted(&hint.body)))
                            .with_animation(SharedString::from(format!("hint-{id}-{i}")), Animation::new(Duration::from_millis(300)).with_easing(gpui_kit::base::animation::ease_out_cubic), |el, t| el.opacity(t))
                    }))
                    .when(shown < hints.hints.len(), |el| el.child(h_flex().justify_between().items_center()
                        .child(h_flex().gap_1().children((0..hints.hints.len()).map(|i| div().size(px(6.)).rounded_full().bg(if i < shown { theme.warning } else { theme.muted }))))
                        .child(Button::new(("hint-next", id)).ghost().xsmall().icon(IconName::Lightbulb).label("Next hint")
                            .on_click(cx.listener(move |this, _, _, cx| { if let Some(run) = this.runs.iter_mut().find(|r| r.id == id) { run.hints_shown += 1; } cx.notify(); })))))
                    .into_any_element()
            }
            Answer::Tests(tests) => v_flex().gap_2()
                .children(tests.cases.iter().enumerate().map(|(i, case)| {
                    let added = run.added.contains(&i);
                    let (input, expected) = (case.input.clone(), case.expected.clone());
                    v_flex().gap_1()
                        .child(h_flex().gap_2().items_center()
                            .child(tag(&format!("{:?}", case.kind).to_lowercase(), theme.success))
                            .child(div().flex_1().min_w_0().text_sm().truncate().child(rich(&case.title)))
                            .child(Button::new(("test-add", id * 100 + i as u64)).ghost().xsmall().icon(if added { IconName::Check } else { IconName::Plus })
                                .tooltip(if added { "Added to test cases" } else { "Add to test cases" }).disabled(added)
                                .on_click(cx.listener(move |this, _, window, cx| this.add_case(id, i, input.clone(), expected.clone(), window, cx)))))
                        .child(mono(&format!("{}\n→ {}", case.input.trim(), case.expected.trim())))
                        .child(muted(&case.why))
                }))
                .into_any_element(),
            Answer::Bugs(bugs) => v_flex().gap_2()
                .child(prose(&bugs.summary))
                .children(bugs.bugs.iter().enumerate().map(|(i, bug)| {
                    let color = match bug.severity { assist::Severity::Bug => theme.danger, assist::Severity::Risk => theme.warning, assist::Severity::Style => theme.info };
                    v_flex().gap_1().pl_2().border_l_2().border_color(color.opacity(0.7))
                        .child(h_flex().gap_2().items_center().child(line_badge(bug.line, format!("bug-{id}-{i}").into())).child(div().text_sm().font_weight(FontWeight::MEDIUM).child(rich(&bug.title))))
                        .child(muted(&bug.detail))
                        .child(h_flex().gap_1p5().items_start().text_xs().child(Icon::new(IconName::Sparkles).size_3().text_color(theme.success)).child(div().text_color(theme.foreground).child(rich(&bug.fix))))
                }))
                .when_some(bugs.failing_input.clone(), |el, input| {
                    let added = run.added.contains(&usize::MAX);
                    el.child(h_flex().gap_2().items_center().child(label("Breaks on")).child(div().flex_1().min_w_0().child(mono(&input)))
                        .child(Button::new(("bug-add", id)).ghost().xsmall().icon(if added { IconName::Check } else { IconName::Plus }).tooltip("Add as test case").disabled(added)
                            .on_click(cx.listener(move |this, _, window, cx| this.add_case(id, usize::MAX, input.clone(), String::new(), window, cx)))))
                })
                .into_any_element(),
            Answer::Analyze(analysis) => v_flex().gap_2()
                .child(h_flex().gap_2()
                    .child(badge("Yours", &analysis.time, &analysis.space, if analysis.time_growth > analysis.best_growth { theme.warning } else { theme.success }, &theme))
                    .child(badge("Best", &analysis.best_time, &analysis.best_space, theme.success, &theme)))
                .child(growth_chart(analysis.time_growth, analysis.best_growth, &theme))
                .child(prose(&analysis.summary))
                .children(analysis.lines.iter().enumerate().map(|(i, cost)| h_flex().gap_2().items_center()
                    .child(line_badge(cost.line, format!("cost-{id}-{i}").into()))
                    .child(div().text_xs().font_family(theme.mono_font_family.clone()).text_color(theme.warning).child(rich(&cost.cost)))
                    .child(div().flex_1().min_w_0().text_xs().text_color(theme.muted_foreground).child(rich(&cost.note)))))
                .into_any_element(),
            Answer::Stuck(stuck) => v_flex().gap_2()
                .child(h_flex().gap_2().items_center().child(tag(&stuck.concept, theme.info)).when_some(stuck.line, |el, line| el.child(line_badge(line, format!("stuck-{id}").into()))))
                .child(prose(&stuck.diagnosis))
                .child(v_flex().gap_1().p_2().rounded_md().bg(theme.info.opacity(0.08)).child(label("Next step")).child(prose(&stuck.next_step)))
                .child(h_flex().gap_1p5().items_start().child(Icon::new(IconName::Brain).size_3p5().text_color(theme.info)).child(div().text_sm().italic().text_color(theme.foreground).child(rich(&stuck.question))))
                .into_any_element(),
            Answer::Explain(explain) => v_flex().gap_2()
                .child(label("Intuition")).child(prose(&explain.intuition))
                .child(h_flex().gap_2().child(tag(&explain.pattern, theme.info)))
                .children(explain.approaches.iter().map(|a| v_flex().gap_0p5()
                    .child(h_flex().gap_2().child(div().text_sm().font_weight(FontWeight::MEDIUM).child(rich(&a.name))).child(div().text_xs().font_family(theme.mono_font_family.clone()).text_color(theme.warning).child(rich(&format!("{} · {}", a.time, a.space)))))
                    .child(muted(&a.idea))))
                .when(!explain.walkthrough.frames.is_empty(), |el| el.child(label("Walkthrough")).child(self.scene(run, &explain.walkthrough, None, window, cx)))
                .when(!explain.pitfalls.is_empty(), |el| el.child(label("Pitfalls")).children(explain.pitfalls.iter().map(|p| muted(&format!("• {p}")))))
                .child(self.reveal(run, "Show solution", &explain.code, cx))
                .into_any_element(),
            Answer::Visualize(scene) => self.scene(run, scene, None, window, cx),
            Answer::Optimize(opt) => v_flex().gap_2()
                .child(h_flex().gap_2()
                    .child(badge("Yours", &opt.current, "", if opt.already_optimal { theme.success } else { theme.warning }, &theme))
                    .child(badge("Best", &opt.best, "", theme.success, &theme)))
                .when(!opt.already_optimal, |el| el.child(growth_chart(opt.current_growth, opt.best_growth, &theme)))
                .when_some(opt.bottleneck_line, |el, line| el.child(h_flex().gap_2().items_center().child(line_badge(line, format!("opt-{id}").into())).child(muted(&opt.bottleneck))))
                .child(v_flex().gap_1().p_2().rounded_md().bg(theme.warning.opacity(0.08)).child(label("Nudge")).child(prose(&opt.nudge)))
                .child(self.reveal(run, "Reveal the idea", &opt.idea, cx))
                .into_any_element(),
            Answer::Pattern(pattern) => {
                let workspace = self.workspace.clone();
                v_flex().gap_2()
                    .child(h_flex().gap_2().child(tag(&pattern.pattern, rgb(0xc3b1ff).into())))
                    .child(prose(&pattern.why))
                    .children(pattern.signals.iter().map(|s| muted(&format!("• {s}"))))
                    .child(self.reveal(run, "Show template", &pattern.template, cx))
                    .child(label("Similar"))
                    .children(pattern.similar.iter().enumerate().map(|(i, similar)| {
                        let (workspace, slug) = (workspace.clone(), similar.slug.clone());
                        h_flex().id(("similar", id * 100 + i as u64)).gap_2().items_center().px_2().py_1().rounded_md().cursor_pointer().hover(|s| s.bg(theme.list_hover))
                            .child(Icon::new(IconName::ArrowUpRight).size_3p5().text_color(theme.info))
                            .child(v_flex().min_w_0().flex_1().child(div().text_sm().child(rich(&similar.title))).child(div().text_xs().text_color(theme.muted_foreground).truncate().child(rich(&similar.why))))
                            .on_click(move |_, window, cx| {
                                let (workspace, slug) = (workspace.clone(), slug.clone());
                                window.defer(cx, move |window, cx| { let _ = workspace.update(cx, |ws, cx| ws.open_problem(slug, window, cx)); });
                            })
                    }))
                    .into_any_element()
            }
            Answer::DryRun(dry) => v_flex().gap_2()
                .child(self.scene(run, &dry.scene, dry.wrong_frame, window, cx))
                .child(v_flex().gap_1().p_2().rounded_md().bg(theme.danger.opacity(0.08)).child(label("Where it goes wrong")).child(prose(&dry.explanation)))
                .child(h_flex().gap_1p5().items_start().text_xs().child(Icon::new(IconName::Sparkles).size_3().text_color(theme.success)).child(div().text_color(theme.foreground).child(rich(&dry.fix_hint))))
                .into_any_element(),
            Answer::Solve(report) => v_flex().gap_1()
                .child(h_flex().gap_2().child(tag(&report.time, theme.warning)).child(tag(&report.space, theme.info)))
                .child(prose(&report.approach)).child(muted(&report.summary))
                .into_any_element(),
        }
    }

    fn reveal(&self, run: &Run, label: &'static str, text: &str, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let id = run.id;
        if run.reveal {
            let copy = text.to_owned();
            v_flex().gap_1()
                .child(div().min_w_0().p_2().rounded_md().bg(theme.muted).text_xs().child(crate::rich_text::markdown(
                    SharedString::from(format!("reveal-{id}")),
                    if label == "Reveal the idea" { text.to_owned() } else { format!("```\n{text}\n```") })))
                .child(h_flex().justify_end().child(Button::new(("reveal-copy", id)).ghost().xsmall().icon(IconName::Copy).label("Copy")
                    .on_click(move |_, _, cx| cx.write_to_clipboard(ClipboardItem::new_string(copy.clone())))))
                .into_any_element()
        } else {
            Button::new(("reveal", id)).ghost().xsmall().icon(IconName::Eye).label(label)
                .on_click(cx.listener(move |this, _, _, cx| { if let Some(run) = this.runs.iter_mut().find(|r| r.id == id) { run.reveal = true; } cx.notify(); }))
                .into_any_element()
        }
    }

    fn scene(&self, run: &Run, scene: &practice::viz::Scene, wrong: Option<usize>, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let id = run.id;
        let index = run.playback.index.min(scene.frames.len().saturating_sub(1));
        let markers: Vec<player::Marker> = wrong.map(|w| player::Marker { index: w, color: theme.danger }).into_iter().collect();
        let entity = cx.entity().downgrade();
        let controls = player::controls(&format!("scene-{id}"), &run.playback, &markers, move |control, _, cx| {
            let _ = entity.update(cx, |this, cx| this.control(id, control, cx));
        }, cx);
        let frame = scene.frames.get(index);
        v_flex().gap_2()
            .when(!scene.title.is_empty(), |el| el.child(div().text_xs().text_color(theme.muted_foreground).child(crate::rich_text::markdown(SharedString::from(format!("scene-title-{id}")), &scene.title))))
            .child(div().p_3().rounded_lg().bg(theme.sidebar).border_1().border_color(if wrong == Some(index) { theme.danger.opacity(0.6) } else { theme.border })
                .children(frame.map(|frame| crate::gen_ui::frame(&format!("scene-{id}"), frame, window, cx))))
            .when_some(frame.and_then(|f| f.line), |el, line| el.child(h_flex().gap_1().text_xs().text_color(theme.muted_foreground).child("line").child(line.to_string())))
            .child(controls)
            .into_any_element()
    }

    fn add_case(&mut self, id: u64, index: usize, input: String, expected: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(run) = self.runs.iter().find(|r| r.id == id) else { return };
        if run.added.contains(&index) { return; }
        let (workspace, slug, assist) = (self.workspace.clone(), run.slug.clone(), cx.entity().downgrade());
        window.defer(cx, move |_, cx| {
            let added = workspace.update(cx, |ws, cx| {
                if !ws.session.as_ref().is_some_and(|s| s.slug == slug) { return false; }
                let added = ws.save_test_case(None, input, expected, cx);
                if added { ws.flash("Test case added", cx); }
                added
            }).unwrap_or(false);
            if added {
                let _ = assist.update(cx, |this, cx| {
                    if let Some(run) = this.runs.iter_mut().find(|r| r.id == id) { run.added.push(index); }
                    cx.notify();
                });
            }
        });
    }

}

fn badge(title: &'static str, time: &str, space: &str, color: Hsla, theme: &Theme) -> Div {
    v_flex().flex_1().p_2().gap_0p5().rounded_lg().bg(color.opacity(0.08)).border_1().border_color(color.opacity(0.25))
        .child(div().text_xs().text_color(theme.muted_foreground).child(title))
        .child(div().text_lg().font_weight(FontWeight::SEMIBOLD).font_family(theme.mono_font_family.clone()).text_color(color).child(crate::rich_text::markdown(SharedString::from(format!("badge-{title}-time")), time)))
        .when(!space.is_empty(), |el| el.child(div().text_xs().text_color(theme.muted_foreground).child(crate::rich_text::markdown(SharedString::from(format!("badge-{title}-space")), format!("space {space}")))))
}

/// Your growth curve against the best one, on a log scale so both stay visible.
fn growth_chart(yours: Growth, best: Growth, theme: &Theme) -> AnyElement {
    let (yours_color, best_color, grid) = (theme.warning, theme.success, theme.border);
    let curves = [(yours, yours_color), (best, best_color)];
    v_flex().gap_1()
        .child(div().h(px(76.)).w_full().rounded_md().bg(theme.sidebar).child(canvas(|_, _, _| (), move |bounds, _, window, _| {
            let (w, h) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
            let o = bounds.origin;
            let mut axis = PathBuilder::stroke(px(1.));
            axis.move_to(point(o.x + px(8.), o.y + px(h - 8.)));
            axis.line_to(point(o.x + px(w - 6.), o.y + px(h - 8.)));
            if let Ok(path) = axis.build() { window.paint_path(path, grid); }
            let max = Growth::Cubic.max(yours).max(best).cost(48.).ln().max(1.);
            for (growth, color) in curves {
                let mut path = PathBuilder::stroke(px(2.));
                for step in 0..=40 {
                    let n = 1. + step as f64 * 47. / 40.;
                    let y = (growth.cost(n).max(1.).ln() / max).min(1.) as f32;
                    let p = point(o.x + px(8. + (w - 16.) * step as f32 / 40.), o.y + px(h - 8. - (h - 16.) * y));
                    if step == 0 { path.move_to(p) } else { path.line_to(p) }
                }
                if let Ok(path) = path.build() { window.paint_path(path, color); }
            }
        }).size_full()))
        .child(h_flex().gap_3().text_xs().text_color(theme.muted_foreground)
            .child(h_flex().gap_1().items_center().child(div().size(px(8.)).rounded_full().bg(yours_color)).child(format!("yours {}", yours.notation())))
            .child(h_flex().gap_1().items_center().child(div().size(px(8.)).rounded_full().bg(best_color)).child(format!("best {}", best.notation()))))
        .into_any_element()
}

impl Workspace {
    /// Sends a prompt shortcut with the current problem context.
    pub fn run_assist(&mut self, action: Action, window: &mut Window, cx: &mut Context<Self>) {
        let slug = self.session.as_ref().map(|session| session.slug.clone());
        self.assist.update(cx, |assist, cx| {
            assist.set_problem(slug.as_deref(), cx);
            assist.sync_composer(window, cx);
        });
        if action == Action::Solve && self.session.as_ref().is_some_and(|s| s.running || matches!(s.judge, Some(crate::workspace::Judge::Running { .. }))) { self.flash("Wait for the current test or submission", cx); return; }
        self.save_now(cx);
        let Some(snapshot) = self.assist_snapshot(cx) else { self.flash("Open a problem first", cx); return; };
        if action.needs_attempt() && !snapshot.has_attempt() { self.flash("Write some code first", cx); return; }
        let target = self.assist.read(cx).target(&self.config);
        if action == Action::Solve && matches!(target, Target::Web(_)) {
            use gpui_kit::component::WindowExt as _;
            window.push_notification(gpui_kit::component::notification::Notification::warning("Solve needs a local agent. Install OpenCode or Antigravity from the AI menu."), cx);
            return;
        }
        if !matches!(target, Target::Web(_)) {
            self.right = true; self.save_layout();
        }
        self.flash(format!("AI: {}", action.label()), cx);
        self.assist.update(cx, |assist, cx| { assist.ai_tab = if action == Action::Review { 1 } else { 2 }; assist.start(action, snapshot, target, window, cx); });
        cx.notify();
    }

    fn assist_snapshot(&self, cx: &App) -> Option<Snapshot> {
        let session = self.session.as_ref()?;
        let question = session.question.as_ref()?;
        let cases = session.cases.iter().zip(&session.results).map(|(case, result)| {
            let outcome = result.as_ref().map(|r| if r.error.is_empty() { r.output.clone() } else { r.error.lines().last().unwrap_or("error").to_owned() });
            (case.input.clone(), case.expected.clone(), outcome)
        }).collect();
        Some(Snapshot {
            slug: session.slug.clone(), title: question.title.clone(), difficulty: question.difficulty.clone(),
            url: crate::roadmap::problem_url(session.source, &question.slug), statement_html: question.content.clone(),
            code: self.editor.read(cx).value().to_string(), starter: question.starter(session.language).unwrap_or_default().to_owned(),
            language: session.language, cases, focus_case: session.selected_case,
            solution_path: session.path.clone(), workspace: self.config.workspace.clone(), trace: None,
        })
    }

    /// Asks the agent where a recorded case first goes wrong.
    pub fn review_trace(&mut self, case: usize, digest: String, window: &mut Window, cx: &mut Context<Self>) {
        self.save_now(cx);
        let Some(mut snapshot) = self.assist_snapshot(cx) else { return };
        let target = self.assist.read(cx).target(&self.config);
        if matches!(target, Target::Web(_)) { self.flash("Explaining a trace needs a local agent", cx); return; }
        snapshot.focus_case = case;
        snapshot.trace = Some(digest);
        self.flash("AI: reviewing the trace", cx);
        self.assist.update(cx, |assist, cx| assist.start(Action::DryRun, snapshot, target, window, cx));
    }

    /// Loads the solution file into the editor after an agent edited it.
    pub fn reload_solution(&mut self, slug: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(session) = self.session.as_mut().filter(|s| s.slug == slug) else { return };
        let Ok(text) = std::fs::read_to_string(&session.path) else { return };
        session.disk_mtime = std::fs::metadata(&session.path).and_then(|m| m.modified()).ok();
        if self.editor.read(cx).value().as_ref() != text { self.editor.update(cx, |editor, cx| editor.set_value(text, window, cx)); }
    }

    pub fn jump_to_line(&mut self, line: u32, window: &mut Window, cx: &mut Context<Self>) {
        self.debug_mode = false;
        self.focus_area = crate::workspace::Focus::Editor;
        self.editor.update(cx, |editor, cx| editor.set_cursor_position(gpui_kit::component::input::Position::new(line.saturating_sub(1), 0), window, cx));
        cx.notify();
    }
}

fn target_for(config: &practice::config::Config, detected: &[Detected]) -> Target {
    let agent = if config.ai_web { None } else {
        config.agent.and_then(|kind| detected.iter().find(|agent| agent.kind == kind))
            .or_else(|| config.agent.is_none().then(|| detected.first()).flatten())
    };
    match agent {
        Some(agent) => Target::Agent(agent.clone(), config.selection(agent.kind).cloned()),
        None => Target::Web(config.web_chat),
    }
}

impl Drop for Assist {
    fn drop(&mut self) { for run in &self.runs { run.cancel.cancel(); } }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[::core::prelude::v1::test]
    fn web_choice_survives_detected_agents() {
        let agents = vec![Detected { kind: AgentKind::Codex, path: "codex".into(), version: None }];
        let mut config = practice::config::Config::default();
        assert!(matches!(target_for(&config, &agents), Target::Agent(_, _)));
        config.ai_web = true;
        assert!(matches!(target_for(&config, &agents), Target::Web(_)));
        config.ai_web = false;
        config.agent = Some(AgentKind::Claude);
        assert!(matches!(target_for(&config, &agents), Target::Web(_)));
    }
}
