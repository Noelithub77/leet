//! The Assist panel: an icon grid of AI actions, live run status, and a result card per run.
//! Runs execute on their own threads and stream events back; cards are kept per problem.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures::StreamExt as _;
use gpui_kit::assets::IconName;
use gpui_kit::base::Disableable as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::shimmer::ShimmerText;
use gpui_kit::component::tooltip::Tooltip;
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
        let code = self.code.trim();
        !code.is_empty() && code != self.starter.trim()
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

struct SolveState {
    attempt: usize,
    session: Option<String>,
    log: Vec<(Tone, String)>,
    accepted: bool,
}

pub struct Run {
    id: u64,
    slug: String,
    action: Action,
    agent: Option<AgentKind>,
    model: String,
    started: Instant,
    elapsed: Option<Duration>,
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
}

impl Run {
    pub fn action(&self) -> Action { self.action }
    fn time(&self) -> Duration { self.elapsed.unwrap_or_else(|| self.started.elapsed()) }
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
    pub installing: Option<AgentKind>,
    runs: Vec<Run>,
    next_id: u64,
    slug: Option<String>,
    ticker: Option<Task<()>>,
    scroll: ScrollHandle,
}

/// Saved answers per problem, newest first.
#[derive(serde::Serialize, serde::Deserialize)]
struct Saved { model: String, agent: Option<AgentKind>, answer: Answer }

impl Assist {
    pub fn new(workspace: WeakEntity<Workspace>, db: Arc<Db>, cx: &mut Context<Self>) -> Self {
        let mut this = Self { workspace, db, agents: vec![], detecting: false, catalogs: HashMap::new(), installing: None,
            runs: vec![], next_id: 0, slug: None, ticker: None, scroll: ScrollHandle::new() };
        this.detect(cx);
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
        if matches!(self.catalogs.get(&kind), Some(Loadable::Loading | Loadable::Ready(_))) { return; }
        let Some(agent) = self.detected(kind).cloned() else { return };
        self.catalogs.insert(kind, Loadable::Loading);
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = cx.background_spawn(async move { agents::catalog(&agent) }).await;
            let _ = this.update(cx, |this, cx| {
                this.catalogs.insert(kind, match result { Ok(catalog) => Loadable::Ready(catalog), Err(error) => Loadable::Failed(error.to_string()) });
                cx.notify();
            });
        }).detach();
    }

    pub fn reload_catalog(&mut self, kind: AgentKind, cx: &mut Context<Self>) {
        self.catalogs.remove(&kind);
        self.load_catalog(kind, cx);
    }

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

    /// Shows the cards of `slug`, restoring saved answers the first time.
    pub fn set_problem(&mut self, slug: Option<&str>, cx: &mut Context<Self>) {
        if self.slug.as_deref() == slug { return; }
        self.slug = slug.map(str::to_owned);
        if let Some(slug) = slug && !self.runs.iter().any(|run| run.slug == slug) {
                let saved: Vec<Saved> = self.db.get(&format!("assist:{slug}")).ok().flatten().and_then(|text| serde_json::from_str(&text).ok()).unwrap_or_default();
                for saved in saved.into_iter().rev() {
                    let id = self.next_id(); let action = action_of(&saved.answer);
                    let mut run = Run::new(id, slug.into(), action, saved.agent, saved.model, None, None);
                    run.elapsed = Some(Duration::ZERO); run.phase = Phase::Done; run.set_answer(Some(saved.answer));
                    self.runs.insert(0, run);
                }
        }
        cx.notify();
    }

    fn save(&self, slug: &str) {
        let saved: Vec<Saved> = self.runs.iter().filter(|run| run.slug == slug && run.action != Action::Solve)
            .filter_map(|run| Some(Saved { model: run.model.clone(), agent: run.agent, answer: run.answer.clone()? })).take(12).collect();
        if let Ok(text) = serde_json::to_string(&saved) { let _ = self.db.set(&format!("assist:{slug}"), &text); }
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
        if let Target::Web(provider) = target {
            let prompt = assist::web_prompt(action, &snapshot.context(None));
            cx.write_to_clipboard(ClipboardItem::new_string(prompt.clone()));
            let _ = open::that_detached(practice::prompts::url(provider, &prompt));
            use gpui_kit::component::WindowExt as _;
            window.push_notification(gpui_kit::component::notification::Notification::info(format!("{} opened in {} · prompt copied", action.label(), provider.label())), cx);
            return;
        }
        let previous: Vec<_> = self.runs.iter().filter(|run| run.slug == snapshot.slug && run.action == action && (run.phase.active() || run.phase == Phase::Confirm)).map(|run| run.id).collect();
        for id in previous { self.stop(id, cx); }
        let id = self.next_id();
        let Target::Agent(agent, selection) = &target else { return };
        let model = selection.as_ref().map(|s| s.model.clone()).unwrap_or_default();
        let mut run = Run::new(id, snapshot.slug.clone(), action, Some(agent.kind), model, Some(snapshot.clone()), Some(target.clone()));
        if action == Action::Solve { run.solve = Some(SolveState { attempt: 1, session: None, log: vec![], accepted: false }); }
        self.runs.insert(0, run);
        self.scroll.set_offset(point(px(0.), px(0.)));
        self.launch(id, None, None, window, cx);
    }

    /// Spawns the agent thread for run `id`, optionally continuing a session with feedback.
    fn launch(&mut self, id: u64, resume: Option<String>, feedback: Option<String>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(run) = self.runs.iter_mut().find(|run| run.id == id) else { return };
        let (Some(snapshot), Some(Target::Agent(agent, selection))) = (run.snapshot.clone(), run.target.clone()) else { return };
        run.phase = Phase::Starting;
        run.cancel = Cancel::default();
        let cancel = run.cancel.clone();
        let action = run.action;
        let fallback = self.catalog(agent.kind).cloned();
        let (tx, mut rx) = futures::channel::mpsc::unbounded::<Msg>();
        std::thread::spawn(move || {
            let selection = match selection {
                Some(selection) => selection,
                None => {
                    let catalog = match fallback { Some(catalog) => Ok(catalog), None => agents::catalog(&agent) };
                    match catalog.and_then(|catalog| default_selection(&catalog).ok_or_else(|| anyhow::anyhow!("{} reported no models", agent.kind.label()))) {
                        Ok(selection) => { let _ = tx.unbounded_send(Msg::Selected(selection.clone())); selection }
                        Err(error) => { let _ = tx.unbounded_send(Msg::Done(Err(error))); return; }
                    }
                }
            };
            let prompt = assist::prompt(action, &snapshot.context(feedback.as_deref()));
            let request = agents::Request {
                agent, selection, prompt,
                schema: (action != Action::Solve).then(|| action.schema()),
                cwd: if action == Action::Solve { snapshot.workspace.clone() } else { std::env::temp_dir() },
                access: action.access(), resume,
            };
            let sender = tx.clone();
            let result = assist::execute(action, &request, &mut |event| { let _ = sender.unbounded_send(Msg::Event(event)); }, &cancel)
                .map(|(answer, outcome)| (answer, outcome.session));
            let _ = tx.unbounded_send(Msg::Done(result));
        });
        self.ensure_ticker(cx);
        cx.spawn_in(window, async move |this, cx| {
            while let Some(msg) = rx.next().await {
                if this.update_in(cx, |this, window, cx| this.receive(id, msg, window, cx)).is_err() { break; }
            }
        }).detach();
        cx.notify();
    }

    fn receive(&mut self, id: u64, msg: Msg, window: &mut Window, cx: &mut Context<Self>) {
        let Some(run) = self.runs.iter_mut().find(|run| run.id == id) else { return };
        if run.cancel.is_cancelled() { return; }
        match msg {
            Msg::Event(agents::Event::Started { session }) => {
                if let (Some(solve), Some(session)) = (run.solve.as_mut(), session) { solve.session = Some(session); }
                run.phase = Phase::Thinking;
            }
            Msg::Event(agents::Event::Thinking(text)) => { run.thinking.push_str(&text); run.phase = Phase::Thinking; }
            Msg::Event(agents::Event::Text(_)) => run.phase = Phase::Writing,
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
                    Ok((answer, session)) if run.action == Action::Solve => {
                        if let Some(solve) = run.solve.as_mut() {
                            solve.session = session.or(solve.session.take());
                            solve.log.push((Tone::Done, format!("Attempt {} written", solve.attempt)));
                        }
                        if answer.is_some() { run.set_answer(answer); }
                        run.phase = Phase::Testing;
                        let (workspace, slug) = (self.workspace.clone(), run.slug.clone());
                        window.defer(cx, move |window, cx| {
                            let _ = workspace.update(cx, |ws, cx| {
                                if ws.session.as_ref().is_some_and(|s| s.slug == slug) { ws.reload_solution(&slug, window, cx); ws.run_tests(window, cx); }
                                else { ws.assist.update(cx, |assist, cx| assist.fail_solve(&slug, "Problem changed; solve stopped".into(), cx)); }
                            });
                        });
                    }
                    Ok((answer, _)) => {
                        run.phase = Phase::Done; run.elapsed = Some(run.started.elapsed());
                        run.set_answer(answer);
                        let slug = run.slug.clone();
                        self.save(&slug);
                    }
                }
            }
        }
        cx.notify();
    }

    /// Called by the workspace when a local or judge run of `slug` finishes.
    pub fn tests_finished(&mut self, slug: &str, passed: bool, report: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(run) = self.runs.iter_mut().find(|run| run.slug == slug && run.action == Action::Solve && run.phase == Phase::Testing) else { return };
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
        let Some(run) = self.runs.iter_mut().find(|run| run.slug == slug && run.action == Action::Solve && run.phase == Phase::Judging) else { return };
        let Some(solve) = run.solve.as_mut() else { return };
        if accepted {
            solve.accepted = true;
            solve.log.push((Tone::Done, "Accepted".into()));
            run.phase = Phase::Done; run.elapsed = Some(run.started.elapsed());
        } else {
            solve.log.push((Tone::Error, first_line(&report)));
            self.retry(slug, report, window, cx);
        }
        cx.notify();
    }

    fn retry(&mut self, slug: &str, report: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(run) = self.runs.iter_mut().find(|run| run.slug == slug && run.action == Action::Solve && run.solve.is_some()) else { return };
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
                } else if slug.starts_with("cf:") {
                    ws.judge(true, window, cx);
                    ws.assist.update(cx, |assist, cx| {
                        if let Some(run) = assist.runs.iter_mut().find(|run| run.id == id) {
                            run.phase = Phase::Done; run.elapsed = Some(run.started.elapsed());
                            if let Some(solve) = &mut run.solve { solve.log.push((Tone::Default, "Copied solution; submit and check the verdict in Codeforces".into())); }
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
        if let Some(run) = self.runs.iter_mut().find(|run| run.slug == slug && run.action == Action::Solve && run.phase.active()) {
            run.cancel.cancel(); run.phase = Phase::Failed(first_line(&error)); run.elapsed = Some(run.started.elapsed());
        }
        cx.notify();
    }

    fn stop(&mut self, id: u64, cx: &mut Context<Self>) {
        if let Some(run) = self.runs.iter_mut().find(|run| run.id == id) {
            run.cancel.cancel();
            run.phase = Phase::Stopped; run.elapsed = Some(run.started.elapsed());
        }
        cx.notify();
    }

    pub fn stop_solves(&mut self, cx: &mut Context<Self>) {
        let ids: Vec<_> = self.runs.iter().filter(|run| run.action == Action::Solve && (run.phase.active() || run.phase == Phase::Confirm)).map(|run| run.id).collect();
        for id in ids { self.stop(id, cx); }
    }

    pub fn stop_all(&mut self, cx: &mut Context<Self>) {
        let ids: Vec<u64> = self.runs.iter().filter(|run| run.phase.active() || run.phase == Phase::Confirm).map(|run| run.id).collect();
        for id in ids { self.stop(id, cx); }
    }

    fn dismiss(&mut self, id: u64, cx: &mut Context<Self>) {
        let slug = self.runs.iter().find(|run| run.id == id).map(|run| run.slug.clone());
        if let Some(run) = self.runs.iter().find(|run| run.id == id) { run.cancel.cancel(); }
        self.runs.retain(|run| run.id != id);
        if let Some(slug) = slug { self.save(&slug); }
        cx.notify();
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

fn default_selection(catalog: &Catalog) -> Option<Selection> {
    let model = catalog.default_model.as_ref().and_then(|id| catalog.models.iter().find(|m| &m.id == id)).or(catalog.models.first())?;
    let effort = model.default_effort.clone().or_else(|| model.efforts.first().map(|e| e.id.clone()));
    Some(Selection { agent: catalog.agent, model: model.id.clone(), effort, fast: false })
}

fn first_line(text: &str) -> String {
    let line = text.lines().find(|l| !l.trim().is_empty()).unwrap_or(text).trim();
    if line.chars().count() > 160 { format!("{}…", line.chars().take(160).collect::<String>()) } else { line.to_owned() }
}

fn action_of(answer: &Answer) -> Action {
    match answer {
        Answer::Hints(_) => Action::Hints, Answer::Tests(_) => Action::Tests, Answer::Bugs(_) => Action::Bugs,
        Answer::Analyze(_) => Action::Analyze, Answer::Stuck(_) => Action::Stuck, Answer::Explain(_) => Action::Explain,
        Answer::Visualize(_) => Action::Visualize, Answer::Optimize(_) => Action::Optimize, Answer::Pattern(_) => Action::Pattern,
        Answer::DryRun(_) => Action::DryRun, Answer::Solve(_) => Action::Solve,
    }
}

impl Run {
    fn new(id: u64, slug: String, action: Action, agent: Option<AgentKind>, model: String, snapshot: Option<Snapshot>, target: Option<Target>) -> Self {
        Self { id, slug, action, agent, model, started: Instant::now(), elapsed: None, phase: Phase::Starting, thinking: String::new(), tokens: 0,
            answer: None, cancel: Cancel::default(), hints_shown: 1, playback: Playback::new(0), last_frame: Instant::now(), reveal: false,
            added: vec![], solve: None, snapshot, target }
    }

    fn set_answer(&mut self, answer: Option<Answer>) {
        let frames = match &answer {
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
        Action::Visualize | Action::DryRun => theme.primary,
    }
}

pub fn command(action: Action) -> Box<dyn gpui_kit::Action> {
    use crate::actions::*;
    match action {
        Action::Hints => Box::new(AssistHints), Action::Stuck => Box::new(AssistStuck), Action::Bugs => Box::new(AssistBugs),
        Action::Tests => Box::new(AssistTests), Action::Analyze => Box::new(AssistAnalyze), Action::Optimize => Box::new(AssistOptimize),
        Action::Visualize => Box::new(AssistVisualize), Action::DryRun => Box::new(AssistDryRun), Action::Pattern => Box::new(AssistPattern),
        Action::Explain => Box::new(AssistExplain), Action::Solve => Box::new(AssistSolve),
    }
}

fn clock(duration: Duration) -> String {
    let secs = duration.as_secs_f32();
    if secs < 60. { format!("{secs:.1}s") } else { format!("{}:{:02}", secs as u64 / 60, secs as u64 % 60) }
}

impl Render for Assist {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let workspace = self.workspace.upgrade();
        let current = workspace.as_ref().and_then(|ws| ws.read(cx).session.as_ref().map(|s| s.slug.clone()));
        if current != self.slug { self.set_problem(current.as_deref(), cx); }
        let (has_problem, has_attempt) = workspace.as_ref().map_or((false, false), |ws| {
            let ws = ws.read(cx);
            (ws.session.as_ref().is_some_and(|s| s.question.is_some()), ws.session.as_ref().is_some_and(|s| s.question.as_ref().and_then(|q| q.starter(s.language)).is_some_and(|starter| ws.editor.read(cx).value().trim() != starter.trim() && !ws.editor.read(cx).value().trim().is_empty())))
        });
        let web = workspace.as_ref().map(|ws| matches!(self.target(&ws.read(cx).config), Target::Web(_))).unwrap_or(true);
        let running: Vec<Action> = self.runs.iter().filter(|run| Some(&run.slug) == self.slug.as_ref() && run.phase.active()).map(|run| run.action).collect();
        let grid = h_flex().flex_wrap().gap_1().px_2().children(Action::ALL.into_iter().enumerate().map(|(index, action)| {
            let color = accent(action, &theme);
            let disabled = !has_problem || (action.needs_attempt() && !has_attempt);
            let busy = running.contains(&action);
            let tip = if !has_problem { "Open a problem first".to_string() } else if disabled { format!("{} · write some code first", action.summary()) } else if web && action == Action::Solve { "Solve needs a local agent".into() } else { action.summary().to_string() };
            let command = command(action);
            v_flex().id(("assist-action", index)).w(px(98.)).py_2().gap_1p5().items_center().rounded_lg()
                .when(!disabled, |el| el.cursor_pointer().hover(|s| s.bg(theme.list_hover)))
                .when(disabled || (web && action == Action::Solve), |el| el.opacity(0.4))
                .child(div().relative().size(px(40.)).rounded_xl().flex().items_center().justify_center()
                    .bg(color.opacity(if busy { 0.26 } else { 0.13 })).border_1().border_color(color.opacity(if busy { 0.8 } else { 0.22 }))
                    .child(Icon::new(icon(action)).size_5().text_color(color))
                    .when(busy, |el| el.child(div().absolute().inset(px(-3.)).rounded(px(15.)).border_1().border_color(color.opacity(0.5))
                        .with_animation(SharedString::from(format!("tile-ring-{index}")), Animation::new(Duration::from_millis(1200)).repeat(),
                            |el, t| el.opacity(1. - t).inset(px(-3. - 4. * t))))))
                .child(div().text_xs().text_color(if disabled { theme.muted_foreground } else { theme.foreground }).child(action.label()))
                .tooltip(move |window, cx| Tooltip::new(tip.clone()).action(command.as_ref(), Some(crate::actions::WORKSPACE)).build(window, cx))
                .when(!disabled && !(web && action == Action::Solve), |el| el.on_click(cx.listener(move |this, _, window, cx| {
                    let workspace = this.workspace.clone();
                    window.defer(cx, move |window, cx| { let _ = workspace.update(cx, |ws, cx| ws.run_assist(action, window, cx)); });
                })))
        }));
        let slug = self.slug.clone();
        let cards: Vec<AnyElement> = self.runs.iter().filter(|run| Some(&run.slug) == slug.as_ref()).map(|run| self.card(run, window, cx)).collect();
        v_flex().size_full().min_h_0()
            .child(grid)
            .child(div().mx_3().my_2().h(px(1.)).bg(theme.border))
            .child(v_flex().id("assist-cards").flex_1().min_h_0().overflow_y_scroll().track_scroll(&self.scroll).px_3().pb_4().gap_3()
                .when(cards.is_empty(), |el| el.child(div().pt_6().text_center().text_xs().text_color(theme.muted_foreground)
                    .child(if !has_problem { "Open a problem to use Assist" } else if web { "Actions open your web chat" } else { "Pick an action above" })))
                .children(cards))
    }
}

impl Assist {
    fn card(&self, run: &Run, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let color = accent(run.action, &theme);
        let id = run.id;
        let active = run.phase.active();
        let header = h_flex().gap_2().items_center()
            .child(div().size(px(24.)).rounded_lg().flex().items_center().justify_center().bg(color.opacity(0.14)).child(Icon::new(icon(run.action)).size_3p5().text_color(color)))
            .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child(run.action.label()))
            .when_some(run.agent, |el, agent| el.child(crate::brand::agent_icon(agent).xsmall()))
            .child(div().min_w_0().flex_1().truncate().text_xs().text_color(theme.muted_foreground).child(run.model.clone()))
            .when(run.elapsed != Some(Duration::ZERO), |el| el.child(h_flex().gap_1().text_xs().text_color(if active { color } else { theme.muted_foreground })
                .font_family(theme.mono_font_family.clone()).child(Icon::new(IconName::Timer).size_3()).child(clock(run.time()))))
            .child(if active || run.phase == Phase::Confirm {
                Button::new(("assist-stop", id)).ghost().xsmall().icon(IconName::CircleStop).tooltip("Stop").accessibility_label("Stop")
                    .on_click(cx.listener(move |this, _, _, cx| this.stop(id, cx))).into_any_element()
            } else {
                Button::new(("assist-dismiss", id)).ghost().xsmall().icon(IconName::X).tooltip("Dismiss").accessibility_label("Dismiss")
                    .on_click(cx.listener(move |this, _, _, cx| this.dismiss(id, cx))).into_any_element()
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
        let body = run.answer.as_ref().map(|answer| self.body(run, answer, window, cx));
        let solve = run.solve.as_ref().map(|solve| self.solve_body(run, solve, cx));
        v_flex().id(("assist-card", id)).p_3().gap_2p5().rounded_lg().bg(theme.background.opacity(0.55)).border_1()
            .border_color(if active { color.opacity(0.45) } else { theme.border })
            .child(header)
            .children(status)
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
        let prose = |text: &str| div().text_sm().text_color(theme.foreground).child(text.to_owned());
        let muted = |text: &str| div().text_xs().text_color(theme.muted_foreground).child(text.to_owned());
        let mono = |text: &str| div().p_2().rounded_md().bg(theme.muted).font_family(theme.mono_font_family.clone()).text_xs().whitespace_normal().child(text.to_owned());
        let label = |text: &'static str| div().text_xs().font_weight(FontWeight::MEDIUM).text_color(theme.muted_foreground).child(text);
        match answer {
            Answer::Hints(hints) => {
                let shown = run.hints_shown.min(hints.hints.len());
                v_flex().gap_2()
                    .children(hints.hints.iter().take(shown).enumerate().map(|(i, hint)| {
                        h_flex().gap_2().items_start()
                            .child(div().mt(px(2.)).size(px(18.)).flex_shrink_0().rounded_full().flex().items_center().justify_center().bg(theme.warning.opacity(0.18))
                                .text_size(px(10.)).text_color(theme.warning).child((i + 1).to_string()))
                            .child(v_flex().gap_0p5().min_w_0().flex_1().child(div().text_sm().font_weight(FontWeight::MEDIUM).child(hint.title.clone())).child(muted(&hint.body)))
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
                            .child(div().flex_1().min_w_0().text_sm().truncate().child(case.title.clone()))
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
                        .child(h_flex().gap_2().items_center().child(line_badge(bug.line, format!("bug-{id}-{i}").into())).child(div().text_sm().font_weight(FontWeight::MEDIUM).child(bug.title.clone())))
                        .child(muted(&bug.detail))
                        .child(h_flex().gap_1p5().items_start().text_xs().child(Icon::new(IconName::Sparkles).size_3().text_color(theme.success)).child(div().text_color(theme.foreground).child(bug.fix.clone())))
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
                    .child(div().text_xs().font_family(theme.mono_font_family.clone()).text_color(theme.warning).child(cost.cost.clone()))
                    .child(div().flex_1().min_w_0().text_xs().text_color(theme.muted_foreground).child(cost.note.clone()))))
                .into_any_element(),
            Answer::Stuck(stuck) => v_flex().gap_2()
                .child(h_flex().gap_2().items_center().child(tag(&stuck.concept, theme.info)).when_some(stuck.line, |el, line| el.child(line_badge(line, format!("stuck-{id}").into()))))
                .child(prose(&stuck.diagnosis))
                .child(v_flex().gap_1().p_2().rounded_md().bg(theme.info.opacity(0.08)).child(label("Next step")).child(prose(&stuck.next_step)))
                .child(h_flex().gap_1p5().items_start().child(Icon::new(IconName::Brain).size_3p5().text_color(theme.info)).child(div().text_sm().italic().text_color(theme.foreground).child(stuck.question.clone())))
                .into_any_element(),
            Answer::Explain(explain) => v_flex().gap_2()
                .child(label("Intuition")).child(prose(&explain.intuition))
                .child(h_flex().gap_2().child(tag(&explain.pattern, theme.info)))
                .children(explain.approaches.iter().map(|a| v_flex().gap_0p5()
                    .child(h_flex().gap_2().child(div().text_sm().font_weight(FontWeight::MEDIUM).child(a.name.clone())).child(div().text_xs().font_family(theme.mono_font_family.clone()).text_color(theme.warning).child(format!("{} · {}", a.time, a.space))))
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
                            .child(v_flex().min_w_0().flex_1().child(div().text_sm().child(similar.title.clone())).child(div().text_xs().text_color(theme.muted_foreground).truncate().child(similar.why.clone())))
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
                .child(h_flex().gap_1p5().items_start().text_xs().child(Icon::new(IconName::Sparkles).size_3().text_color(theme.success)).child(div().text_color(theme.foreground).child(dry.fix_hint.clone())))
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
                .child(div().p_2().rounded_md().bg(theme.muted).font_family(theme.mono_font_family.clone()).text_xs().child(text.to_owned()))
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
            .when(!scene.title.is_empty(), |el| el.child(div().text_xs().text_color(theme.muted_foreground).child(scene.title.clone())))
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

fn tag(text: &str, color: Hsla) -> Div {
    div().px_2().h(px(20.)).flex().items_center().rounded_full().bg(color.opacity(0.14)).border_1().border_color(color.opacity(0.2))
        .text_size(px(11.)).text_color(color).font_weight(FontWeight::MEDIUM).child(text.to_owned())
}

fn badge(title: &'static str, time: &str, space: &str, color: Hsla, theme: &Theme) -> Div {
    v_flex().flex_1().p_2().gap_0p5().rounded_lg().bg(color.opacity(0.08)).border_1().border_color(color.opacity(0.25))
        .child(div().text_xs().text_color(theme.muted_foreground).child(title))
        .child(div().text_lg().font_weight(FontWeight::SEMIBOLD).font_family(theme.mono_font_family.clone()).text_color(color).child(time.to_owned()))
        .when(!space.is_empty(), |el| el.child(div().text_xs().text_color(theme.muted_foreground).child(format!("space {space}"))))
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
    /// Runs an Assist action on the open problem. Dry run uses the native debugger when it can.
    pub fn run_assist(&mut self, action: Action, window: &mut Window, cx: &mut Context<Self>) {
        if action == Action::DryRun && self.session.as_ref().is_some_and(|s| practice::debugger::supported(s.language) && !s.slug.starts_with("cf:")) {
            self.set_debug(true, window, cx);
            return;
        }
        if action == Action::Solve && self.session.as_ref().is_some_and(|s| s.running || matches!(s.judge, Some(crate::workspace::Judge::Running { .. }))) { self.flash("Wait for the current test or submission", cx); return; }
        self.save_now(cx);
        let Some(snapshot) = self.assist_snapshot(cx) else { self.flash("Open a problem first", cx); return };
        if action.needs_attempt() && !snapshot.has_attempt() { self.flash("Write some code first", cx); return; }
        let target = self.assist.read(cx).target(&self.config);
        if action == Action::Solve && matches!(target, Target::Web(_)) {
            use gpui_kit::component::WindowExt as _;
            window.push_notification(gpui_kit::component::notification::Notification::warning("Solve needs a local agent. Install OpenCode or Antigravity from the AI menu."), cx);
            return;
        }
        if !matches!(target, Target::Web(_)) {
            self.right = true; self.assist_open = true; self.history_mode = false; self.save_layout();
        }
        self.flash(format!("AI: {}", action.label()), cx);
        self.assist.update(cx, |assist, cx| assist.start(action, snapshot, target, window, cx));
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
