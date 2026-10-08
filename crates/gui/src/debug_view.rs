//! Debug mode: every test case recorded up front by the native tracer, then scrubbed like
//! a video. Code with a heat gutter on the left, live state on the right, seek bar below.

use std::ops::Range;
use std::time::{Duration, Instant};

use gpui_kit::assets::IconName;
use gpui_kit::base::{Spring, spring};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::highlighter::SyntaxHighlighter;
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Icon, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use practice::debugger::{self, Limits, Step, StepKind, Trace};
use practice::language::Language;
use practice::runner::{Case, Compare};
use practice::viz::Structure;

use crate::assist::{Assist, TraceReview};
use crate::gen_ui::player::{self, Playback};
use crate::workspace::Workspace;

const LINE_H: f32 = 22.;

gpui_kit::actions!(debugger, [StepBack, StepForward, JumpBack, JumpForward, PlayPause, FirstStep, LastStep, PrevCase, NextCase]);

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("left", StepBack, Some("Debugger")),
        KeyBinding::new("right", StepForward, Some("Debugger")),
        KeyBinding::new("shift-left", JumpBack, Some("Debugger")),
        KeyBinding::new("shift-right", JumpForward, Some("Debugger")),
        KeyBinding::new("space", PlayPause, Some("Debugger")),
        KeyBinding::new("home", FirstStep, Some("Debugger")),
        KeyBinding::new("end", LastStep, Some("Debugger")),
        KeyBinding::new("up", PrevCase, Some("Debugger")),
        KeyBinding::new("down", NextCase, Some("Debugger")),
    ]);
}

/// What recording needs, copied from the open problem.
#[derive(Clone)]
pub struct Snapshot {
    pub slug: String,
    pub language: Language,
    pub python: String,
    pub code: String,
    pub starter: String,
    pub meta: serde_json::Value,
    pub cases: Vec<Case>,
    pub compare: Compare,
}

impl Snapshot {
    fn has_attempt(&self) -> bool {
        practice::workspace::has_attempt(&self.code, &self.starter)
    }
}

enum Recording {
    Waiting,
    Running,
    Ready(Box<Trace>),
    Failed(String),
}

struct SourceLine {
    text: SharedString,
    range: Range<usize>,
}

// Parse the whole snapshot so multiline tokens keep their context across rows.
fn source_lines(source: &str) -> Vec<SourceLine> {
    let mut offset = 0;
    source.split_inclusive('\n').map(|row| {
        let text = row.strip_suffix('\n').unwrap_or(row);
        let text = text.strip_suffix('\r').unwrap_or(text);
        let line = SourceLine { text: text.to_owned().into(), range: offset..offset + text.len() };
        offset += row.len();
        line
    }).collect()
}

fn line_highlights(highlighter: &SyntaxHighlighter, range: &Range<usize>, theme: &gpui_kit::component::highlighter::HighlightTheme) -> Vec<(Range<usize>, HighlightStyle)> {
    highlighter.styles(range, theme).into_iter()
        .map(|(token, style)| (token.start - range.start..token.end - range.start, style))
        .collect()
}

struct Explanation {
    workspace: WeakEntity<Workspace>,
    assist: Entity<Assist>,
    _observe: Subscription,
}

pub struct Debugger {
    explanation: Option<Explanation>,
    focus: FocusHandle,
    snapshot: Option<Snapshot>,
    lines: Vec<SourceLine>,
    highlighter: SyntaxHighlighter,
    traces: Vec<Recording>,
    selected: usize,
    playback: Playback,
    last_frame: Instant,
    ticker: Option<Task<()>>,
    /// Bumped per recording so stale results are dropped.
    generation: u64,
    unsupported: Option<String>,
    editor_ready: bool,
    installing: bool,
    install_error: Option<String>,
    preview: bool,
    code_scroll: ScrollHandle,
}

impl Debugger {
    pub fn new(workspace: WeakEntity<Workspace>, assist: Entity<Assist>, cx: &mut Context<Self>) -> Self {
        let observe = cx.observe(&assist, |_, _, cx| cx.notify());
        Self::build(Some(Explanation { workspace, assist, _observe: observe }), cx)
    }

    #[cfg(feature = "gui-test")]
    pub(crate) fn isolated(cx: &mut Context<Self>) -> Self { Self::build(None, cx) }

    fn build(explanation: Option<Explanation>, cx: &mut Context<Self>) -> Self {
        Self { explanation, focus: cx.focus_handle(), snapshot: None, lines: vec![], highlighter: SyntaxHighlighter::new("python"), traces: vec![], selected: 0, playback: Playback::new(0),
            last_frame: Instant::now(), ticker: None, generation: 0, unsupported: None, editor_ready: true, installing: false, install_error: None, preview: false, code_scroll: ScrollHandle::new() }
    }

    pub fn focus(&self, window: &mut Window, cx: &mut App) { self.focus.focus(window, cx); }

    /// Records every case unless the same code and cases were already recorded.
    pub fn load(&mut self, snapshot: Snapshot, selected: usize, cx: &mut Context<Self>) {
        self.load_mode(snapshot, selected, false, cx);
    }

    pub fn preview(&mut self, snapshot: Snapshot, selected: usize, cx: &mut Context<Self>) {
        self.load_mode(snapshot, selected, true, cx);
    }

    fn load_mode(&mut self, snapshot: Snapshot, selected: usize, preview: bool, cx: &mut Context<Self>) {
        let same = self.snapshot.as_ref().is_some_and(|old| old.slug == snapshot.slug && old.code == snapshot.code && old.starter == snapshot.starter && old.language == snapshot.language
            && old.cases.iter().map(|c| (&c.input, &c.expected)).eq(snapshot.cases.iter().map(|c| (&c.input, &c.expected))));
        if same && self.unsupported.is_none() && (preview || !self.preview) && !self.traces.iter().any(|t| matches!(t, Recording::Failed(_))) { self.select(selected.min(self.traces.len().saturating_sub(1)), cx); return; }
        self.generation += 1;
        self.preview = preview;
        let source = snapshot.code.replace('\t', "    ");
        self.lines = source_lines(&source);
        self.highlighter = SyntaxHighlighter::new(snapshot.language.id());
        self.highlighter.update(None, &ropey::Rope::from_str(&source), None);
        self.unsupported = debugger::requirement(snapshot.language, &snapshot.python).err();
        self.editor_ready = practice::toolchain::editor_ready(snapshot.language);
        self.traces = snapshot.cases.iter().map(|_| Recording::Waiting).collect();
        self.selected = selected.min(snapshot.cases.len().saturating_sub(1));
        self.playback = Playback::new(0);
        self.snapshot = Some(snapshot.clone());
        if preview || !snapshot.has_attempt() || self.unsupported.is_some() { cx.notify(); return; }
        let generation = self.generation;
        // Record the selected case first so it is ready soonest.
        let mut order: Vec<usize> = (0..snapshot.cases.len()).collect();
        order.sort_by_key(|&i| i != self.selected);
        for &index in &order { self.traces[index] = Recording::Running; }
        let (tx, mut rx) = futures::channel::mpsc::unbounded::<(usize, anyhow::Result<Trace>)>();
        std::thread::spawn(move || {
            for index in order {
                let case = &snapshot.cases[index];
                let trace = if practice::language::Source::for_problem(&snapshot.slug).is_stdin() {
                    debugger::record_stdin_code(snapshot.language, &snapshot.python, &snapshot.code, case, Limits::default())
                } else {
                    debugger::record_code(snapshot.language, &snapshot.python, &snapshot.code, &snapshot.meta, case, Limits::default())
                };
                if tx.unbounded_send((index, trace)).is_err() { break; }
            }
        });
        cx.spawn(async move |this, cx| {
            use futures::StreamExt as _;
            while let Some((index, trace)) = rx.next().await {
                let alive = this.update(cx, |this, cx| {
                    if this.generation != generation { return false; }
                    this.traces[index] = match trace { Ok(trace) => Recording::Ready(Box::new(trace)), Err(error) => Recording::Failed(error.to_string()) };
                    if index == this.selected { this.reset_playback(); }
                    cx.notify();
                    true
                });
                if !matches!(alive, Ok(true)) { break; }
            }
        }).detach();
        cx.notify();
    }

    fn install_tools(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(language) = self.snapshot.as_ref().map(|snapshot| snapshot.language) else { return };
        if self.installing || !practice::tool_setup::supported(language) { return; }
        self.installing = true;
        self.install_error = None;
        cx.spawn_in(window, async move |this, cx| {
            let result = cx.background_spawn(async move {
                practice::tool_setup::install(language, &practice::tool_setup::directory(), |_| {})
            }).await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.installing = false;
                match result {
                    Ok(()) => {
                        if let Some(explanation) = &this.explanation {
                            let _ = explanation.workspace.update(cx, |workspace, cx| {
                                workspace.suspend_language_servers(cx);
                                if workspace.center == crate::workspace::Center::Editor { workspace.attach_language_server(window, cx); }
                            });
                        }
                        if let Some(snapshot) = this.snapshot.take() { this.load_mode(snapshot, this.selected, this.preview, cx); }
                    }
                    Err(error) => this.install_error = Some(error.to_string()),
                }
                cx.notify();
            });
        }).detach();
        cx.notify();
    }

    #[cfg(feature = "gui-test")]
    pub(crate) fn fixture_status(&self) -> anyhow::Result<Option<(usize, usize, usize, bool)>> {
        for recording in &self.traces {
            if let Recording::Failed(error) = recording { anyhow::bail!("{error}"); }
            if let Recording::Ready(trace) = recording {
                if let Some(error) = &trace.error { anyhow::bail!("{error}"); }
            }
        }
        if self.traces.iter().any(|trace| !matches!(trace, Recording::Ready(_))) { return Ok(None); }
        Ok(Some((self.selected, self.playback.index, self.playback.len, self.playback.playing)))
    }

    #[cfg(feature = "gui-test")]
    pub(crate) fn fixture_verdict(&self) -> Option<bool> { self.verdict(self.selected) }

    fn trace(&self) -> Option<&Trace> {
        match self.traces.get(self.selected) { Some(Recording::Ready(trace)) => Some(trace), _ => None }
    }

    fn reset_playback(&mut self) {
        let len = self.trace().map_or(0, |t| t.steps.len());
        self.playback = Playback::new(len);
    }

    fn select(&mut self, index: usize, cx: &mut Context<Self>) {
        if index >= self.traces.len() { return; }
        self.selected = index;
        self.reset_playback();
        cx.notify();
    }

    fn control(&mut self, control: player::Control, cx: &mut Context<Self>) {
        if player::apply(&mut self.playback, control) { self.last_frame = Instant::now(); self.ensure_ticker(cx); }
        if let Some(step) = self.trace().and_then(|t| t.steps.get(self.playback.index)) {
            self.code_scroll.set_offset(point(px(0.), -px(step.line.saturating_sub(4) as f32 * LINE_H)));
        }
        cx.notify();
    }

    fn ensure_ticker(&mut self, cx: &mut Context<Self>) {
        if self.ticker.is_some() { return; }
        self.ticker = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_millis(30)).await;
                let alive = this.update(cx, |this, cx| {
                    if this.last_frame.elapsed() >= Duration::from_millis(this.playback.interval_ms()) {
                        this.last_frame = Instant::now();
                        this.playback.advance();
                        if let Some(step) = this.trace().and_then(|t| t.steps.get(this.playback.index)) {
                            this.code_scroll.set_offset(point(px(0.), -px(step.line.saturating_sub(4) as f32 * LINE_H)));
                        }
                        cx.notify();
                    }
                    if !this.playback.playing { this.ticker = None; }
                    this.playback.playing
                });
                if !matches!(alive, Ok(true)) { break; }
            }
        }));
    }

    /// Step indices of the next or previous call/return/exception, for quick jumps.
    fn jump(&mut self, forward: bool, cx: &mut Context<Self>) {
        let Some(trace) = self.trace() else { return };
        let index = self.playback.index;
        let interesting = |s: &Step| s.kind != StepKind::Line;
        let target = if forward { trace.steps.iter().enumerate().skip(index + 1).find(|(_, s)| interesting(s)).map(|(i, _)| i).unwrap_or(trace.steps.len().saturating_sub(1)) }
            else { trace.steps.iter().enumerate().take(index).rev().find(|(_, s)| interesting(s)).map(|(i, _)| i).unwrap_or(0) };
        self.control(player::Control::Seek(target), cx);
    }

    fn verdict(&self, index: usize) -> Option<bool> {
        let (Some(Recording::Ready(trace)), Some(snapshot)) = (self.traces.get(index), &self.snapshot) else { return None };
        let expected = snapshot.cases.get(index)?.expected.as_deref()?;
        if trace.error.is_some() { return Some(false); }
        let actual = trace.output.as_deref()?;
        Some(if practice::language::Source::for_problem(&snapshot.slug).is_stdin() {
            actual.split_whitespace().eq(expected.split_whitespace())
        } else { practice::runner::outputs_match(actual, expected, snapshot.compare) })
    }
}

impl Focusable for Debugger {
    fn focus_handle(&self, _: &App) -> FocusHandle { self.focus.clone() }
}

impl Render for Debugger {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let template = self.snapshot.as_ref().is_some_and(|snapshot| !snapshot.has_attempt());
        let chips = h_flex().gap_1p5().children(self.traces.iter().enumerate().map(|(index, recording)| {
            let active = index == self.selected;
            let (icon, color) = match (recording, self.verdict(index)) {
                _ if template || self.preview => (Some(IconName::CircleDot), theme.muted_foreground),
                (Recording::Running | Recording::Waiting, _) => (None, theme.muted_foreground),
                (Recording::Failed(_), _) => (Some(IconName::TriangleAlert), theme.danger),
                (_, Some(true)) => (Some(IconName::CircleCheck), theme.success),
                (_, Some(false)) => (Some(IconName::CircleX), theme.danger),
                (_, None) => (Some(IconName::CircleDot), theme.info),
            };
            h_flex().id(("debug-case", index)).test_support().h_7().px_2p5().gap_1p5().rounded_md().text_xs().cursor_pointer()
                .bg(if active { theme.secondary_active } else { theme.secondary })
                .when(active, |el| el.border_1().border_color(color.opacity(0.6)))
                .child(match icon { Some(icon) => Icon::new(icon).size_3p5().text_color(color).into_any_element(), None => Spinner::new().xsmall().into_any_element() })
                .child(format!("Case {}", index + 1))
                .tooltip(|window, cx| gpui_kit::component::tooltip::Tooltip::new("Switch cases · ↑ / ↓").build(window, cx))
                .on_click(cx.listener(move |this, _, _, cx| this.select(index, cx)))
        }));
        let trace = self.trace();
        let review = self.explanation.as_ref().and_then(|explanation| self.snapshot.as_ref().and_then(|s| explanation.assist.read(cx).review(&s.slug, self.selected)));
        let wrong = match &review { Some(Ok(review)) => review.wrong_step, _ => None };
        let step = trace.and_then(|t| t.steps.get(self.playback.index));
        let previous = trace.and_then(|t| self.playback.index.checked_sub(1).and_then(|i| t.steps.get(i)));
        let header = h_flex().h_10().px_3().gap_3().items_center().border_b_1().border_color(theme.border)
            .child(Icon::new(IconName::BugPlay).size_4().text_color(theme.primary))
            .child(chips)
            .child(div().flex_1())
            .when(self.snapshot.as_ref().is_some_and(|snapshot| practice::tool_setup::supported(snapshot.language))
                && (!self.editor_ready || self.snapshot.as_ref().is_some_and(|snapshot| snapshot.language == Language::Python && self.unsupported.is_some())), |el| {
                el.child(Button::new("debug-install-tools").ghost().xsmall().label(if self.installing { "Installing…" } else { "Install tools" })
                    .disabled(self.installing)
                    .tooltip("Install private Python / basedpyright or clangd editor tools. C++ debugging also requires a compiler and GDB with Python support.")
                    .on_click(cx.listener(|this, _, window, cx| this.install_tools(window, cx))))
            })
            .when_some(trace, |el, t| el.child(div().text_xs().text_color(theme.muted_foreground)
                .child(format!("{} steps{}", t.steps.len(), if t.truncated { " · first part" } else { "" }))))
            .when(self.explanation.is_some() && trace.is_some() && !matches!(review, Some(Err(_))), |el| el.child(Button::new("debug-explain").ghost().xsmall().icon(IconName::Sparkles).label("Explain")
                .tooltip("Ask AI where this case first goes wrong, using the real trace")
                .on_click(cx.listener(|this, _, window, cx| {
                    let Some(digest) = this.trace().map(|trace| practice::assist::trace_digest(trace, 400)) else { return };
                    let Some(explanation) = &this.explanation else { return };
                    let (workspace, case) = (explanation.workspace.clone(), this.selected);
                    window.defer(cx, move |window, cx| { let _ = workspace.update(cx, |ws, cx| ws.review_trace(case, digest, window, cx)); });
                }))))
            .when(matches!(review, Some(Err(_))), |el| el.child(h_flex().gap_1p5().text_xs().text_color(theme.muted_foreground)
                .child(Spinner::new().xsmall()).child("Reviewing")))
            .child(Button::new("debug-rerecord").ghost().xsmall().icon(IconName::RefreshCw).tooltip("Record again").accessibility_label("Record again")
                .on_click(cx.listener(|this, _, _, cx| { if let Some(snapshot) = this.snapshot.take() { let selected = this.selected; this.load(snapshot, selected, cx); } })));

        let state: AnyElement = if template || self.preview {
            v_flex().flex_1().min_w_0().h_full().p_4().gap_4()
                .child(h_flex().gap_2().items_center().text_sm()
                    .child(Icon::new(IconName::Code).size_4().text_color(theme.muted_foreground))
                    .child(if template { "Just a template" } else { "Ready to debug" }))
                .children([("Call stack", "No calls yet"), ("Variables", "No variables yet"), ("Data structures", "No data yet")].into_iter().map(|(title, text)| {
                    v_flex().gap_2()
                        .child(div().text_xs().font_weight(FontWeight::MEDIUM).text_color(theme.muted_foreground).child(title))
                        .child(div().text_xs().text_color(theme.muted_foreground.opacity(0.7)).child(text))
                })).into_any_element()
        } else if let Some(reason) = &self.unsupported {
            empty(IconName::Footprints, reason, "Use Dry run in Assist for an AI trace of this language.", &theme)
        } else {
            match self.traces.get(self.selected) {
                None => empty(IconName::FlaskConical, "No test cases", "Add a case to debug it.", &theme),
                Some(Recording::Waiting | Recording::Running) => v_flex().size_full().items_center().justify_center().gap_2()
                    .child(Spinner::new()).child(div().text_sm().text_color(theme.muted_foreground).child("Recording every step…")).into_any_element(),
                Some(Recording::Failed(error)) => empty(IconName::TriangleAlert, "Could not record this case", error, &theme),
                Some(Recording::Ready(_)) => self.state(step, previous, trace, window, cx),
            }
        };
        let body = h_flex().size_full().min_h_0()
            .child(self.code(step, trace, window, cx))
            .child(v_flex().flex_1().min_w_0().h_full()
                .when_some(self.snapshot.as_ref().and_then(|s| s.cases.get(self.selected)), |el, case| {
                    el.child(v_flex().flex_shrink_0().p_4()
                        .child(v_flex().p_3().gap_2().rounded_lg().bg(theme.info.opacity(0.08))
                            .border_1().border_color(theme.info.opacity(0.3)).border_l_4()
                            .child(h_flex().gap_2().items_center().text_sm().font_weight(FontWeight::SEMIBOLD).text_color(theme.info)
                                .child(Icon::new(IconName::FlaskConical).size_4()).child("Input"))
                            .child(v_flex().id(("debug-input", self.selected)).max_h(px(160.)).overflow_scroll()
                                .text_size(px(16.)).font_weight(FontWeight::MEDIUM).text_color(theme.info).font_family(theme.mono_font_family.clone())
                                .children(case.input.lines().map(|line| div().whitespace_nowrap().child(line.to_owned()))))))
                })
                .child(div().flex().flex_1().min_h_0().child(state)));
        let markers: Vec<player::Marker> = trace.map(|t| {
            let every = (t.steps.len() / 160).max(1);
            t.steps.iter().enumerate().filter(|(i, s)| s.kind == StepKind::Exception || (s.kind != StepKind::Line && i % every == 0))
                .map(|(index, s)| player::Marker { index, color: match s.kind { StepKind::Call => theme.info, StepKind::Return => theme.success, _ => theme.danger } })
                .chain(wrong.map(|index| player::Marker { index, color: theme.warning }))
                .collect()
        }).unwrap_or_default();
        let banner = match &review {
            Some(Ok(TraceReview { wrong_step, explanation, fix_hint })) => {
                let target = *wrong_step;
                Some(v_flex().mx_3().mt_2().p_3().gap_1().rounded_lg().bg(theme.warning.opacity(0.08)).border_1().border_color(theme.warning.opacity(0.3))
                    .child(h_flex().gap_2().items_center().child(Icon::new(IconName::Sparkles).size_3p5().text_color(theme.warning))
                        .child(div().text_sm().font_weight(FontWeight::MEDIUM).child("Where it goes wrong"))
                        .child(div().flex_1())
                        .when_some(target, |el, index| el.child(Button::new("debug-goto-wrong").ghost().xsmall().label(format!("Go to step {}", index + 1))
                            .on_click(cx.listener(move |this, _, _, cx| this.control(player::Control::Seek(index), cx))))))
                    .child(div().text_sm().child(explanation.clone()))
                    .child(div().text_xs().text_color(theme.muted_foreground).child(fix_hint.clone())))
            }
            _ => None,
        };
        let entity = cx.entity().downgrade();
        let controls = player::controls("debugger", &self.playback, &markers, move |control, _, cx| { let _ = entity.update(cx, |this, cx| this.control(control, cx)); }, cx);
        v_flex().key_context("Debugger").track_focus(&self.focus).size_full().min_h_0().bg(theme.background)
            .on_action(cx.listener(|this, _: &StepBack, _, cx| this.control(player::Control::Step(-1), cx)))
            .on_action(cx.listener(|this, _: &StepForward, _, cx| this.control(player::Control::Step(1), cx)))
            .on_action(cx.listener(|this, _: &JumpBack, _, cx| this.jump(false, cx)))
            .on_action(cx.listener(|this, _: &JumpForward, _, cx| this.jump(true, cx)))
            .on_action(cx.listener(|this, _: &PlayPause, _, cx| this.control(player::Control::Toggle, cx)))
            .on_action(cx.listener(|this, _: &FirstStep, _, cx| this.control(player::Control::Seek(0), cx)))
            .on_action(cx.listener(|this, _: &LastStep, _, cx| this.control(player::Control::Seek(usize::MAX), cx)))
            .on_action(cx.listener(|this, _: &PrevCase, _, cx| { let i = this.selected.saturating_sub(1); this.select(i, cx); }))
            .on_action(cx.listener(|this, _: &NextCase, _, cx| { let i = this.selected + 1; this.select(i, cx); }))
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| this.focus.focus(window, cx)))
            .child(header)
            .when_some(self.install_error.as_ref(), |el, error| el.child(div().px_3().py_2().text_xs().text_color(theme.warning).child(error.clone())))
            .children(banner)
            .child(div().flex().flex_1().min_h_0().overflow_hidden().child(body))
            .child(div().px_3().py_2().border_t_1().border_color(theme.border).child(controls))
    }
}

impl Debugger {
    fn code(&self, step: Option<&Step>, trace: Option<&Trace>, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let mut heat = vec![0u32; self.lines.len() + 1];
        if let Some(trace) = trace {
            for s in trace.steps.iter().take(self.playback.index + 1) {
                if s.kind == StepKind::Line && let Some(h) = heat.get_mut(s.line as usize) { *h += 1; }
            }
        }
        let hottest = heat.iter().copied().max().unwrap_or(1).max(1) as f32;
        let current = step.map_or(0, |s| s.line as usize);
        let kind = step.map(|s| s.kind);
        let top = spring("debug-line", current.saturating_sub(1) as f32 * LINE_H, Spring::new(Duration::from_millis(220)), window, cx);
        let marker = match kind { Some(StepKind::Return) => theme.success, Some(StepKind::Exception) => theme.danger, Some(StepKind::Call) => theme.info, _ => theme.primary };
        div().w(relative(0.46)).h_full().min_w(px(260.)).border_r_1().border_color(theme.border)
            .child(div().id("debug-code").size_full().overflow_scroll().track_scroll(&self.code_scroll).py_2()
                .child(div().relative()
                    .when(current > 0, |el| el.child(div().absolute().left_0().right_0().top(px(top)).h(px(LINE_H)).bg(marker.opacity(0.12))
                        .border_l_2().border_color(marker)))
                    .children(self.lines.iter().enumerate().map(|(i, line)| {
                        let n = i + 1;
                        let count = heat.get(n).copied().unwrap_or(0);
                        let ran = count > 0;
                        h_flex().h(px(LINE_H)).items_center().pr_3()
                            .child(div().w(px(18.)).h_full().flex().items_center().justify_center()
                                .when(n == current, |el| el.child(Icon::new(match kind { Some(StepKind::Return) => IconName::CornerUpLeft, Some(StepKind::Exception) => IconName::TriangleAlert, Some(StepKind::Call) => IconName::CornerDownRight, _ => IconName::ArrowRight }).size_3().text_color(marker))))
                            .child(div().w(px(30.)).text_right().pr_2().text_size(px(11.)).font_family(theme.mono_font_family.clone())
                                .text_color(if n == current { theme.foreground } else { theme.muted_foreground.opacity(0.6) }).child(n.to_string()))
                            .child(div().w(px(4.)).h(px(LINE_H - 6.)).rounded_full().mr_2()
                                .bg(if ran { theme.warning.opacity(0.25 + 0.75 * (count as f32).ln_1p() / hottest.ln_1p()) } else { theme.muted.opacity(0.4) }))
                            .child(div().flex_1().min_w_0().whitespace_nowrap().font_family(theme.mono_font_family.clone()).text_size(px(12.5))
                                .text_color(theme.foreground).opacity(if trace.is_none() || ran || n == current { 1. } else { 0.65 })
                                .child(StyledText::new(line.text.clone()).with_highlights(line_highlights(&self.highlighter, &line.range, &theme.highlight_theme))))
                            .when(ran, |el| el.child(div().text_size(px(10.)).text_color(theme.muted_foreground.opacity(0.7)).child(format!("×{count}"))))
                    }))))
            .into_any_element()
    }

    fn state(&self, step: Option<&Step>, previous: Option<&Step>, trace: Option<&Trace>, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let Some(step) = step else {
            return empty(IconName::TriangleAlert, "No steps recorded", trace.and_then(|t| t.error.as_deref()).unwrap_or("The solution produced no traceable steps."), &theme);
        };
        let structures = debugger::structures(step, previous);
        let (variables, data): (Vec<_>, Vec<_>) = structures.iter().partition(|structure| matches!(structure, Structure::Vars { .. }));
        let heading = |title: &'static str| div().text_xs().font_weight(FontWeight::MEDIUM).text_color(theme.muted_foreground).child(title);
        let (icon, color, what) = match step.kind {
            StepKind::Call => (IconName::SquareFunction, theme.info, format!("Calling {}", step.function)),
            StepKind::Return => (IconName::SquareFunction, theme.success, format!("Returning from {}", step.function)),
            StepKind::Exception => (IconName::TriangleAlert, theme.danger, format!("Error in {}", step.function)),
            StepKind::Line => (IconName::ArrowRight, theme.primary, format!("Line {} · {}", step.line, step.function)),
        };
        let stack = h_flex().gap_1().flex_wrap().children(step.stack.iter().enumerate().map(|(i, frame)| {
            let last = i + 1 == step.stack.len();
            h_flex().gap_1().items_center()
                .when(i > 0, |el| el.child(Icon::new(IconName::ChevronRight).size_3().text_color(theme.muted_foreground)))
                .child(div().px_1p5().h(px(20.)).flex().items_center().gap_1().rounded_md().text_size(px(11.)).font_family(theme.mono_font_family.clone())
                    .bg(if last { color.opacity(0.14) } else { theme.secondary }).text_color(if last { color } else { theme.muted_foreground })
                    .child(Icon::new(IconName::SquareFunction).size_3())
                    .child(format!("{}:{}", frame.function, frame.line)))
        }));
        let stdout = trace.map(|t| { let end = step.stdout_len.min(t.stdout.len()); t.stdout.get(..end).unwrap_or_default().to_owned() }).unwrap_or_default();
        let last = trace.is_some_and(|t| self.playback.index + 1 == t.steps.len());
        let expected = self.snapshot.as_ref().and_then(|s| s.cases.get(self.selected)).and_then(|c| c.expected.clone());
        v_flex().id("debug-state").flex_1().min_w_0().h_full().overflow_y_scroll().p_4().gap_4()
            .child(h_flex().gap_2().items_center()
                .child(div().size(px(26.)).rounded_lg().flex().items_center().justify_center().bg(color.opacity(0.15)).child(Icon::new(icon).size_3p5().text_color(color)))
                .child(div().text_sm().font_weight(FontWeight::MEDIUM).child(what)))
            .child(v_flex().gap_1p5().child(heading("Call stack")).child(stack))
            .children(step.exception.clone().map(|e| div().p_2().rounded_md().bg(theme.danger.opacity(0.1)).text_xs().text_color(theme.danger).font_family(theme.mono_font_family.clone()).child(e)))
            .when(!variables.is_empty(), |el| el.child(v_flex().gap_2().child(heading("Variables"))
                .children(variables.iter().map(|s| crate::gen_ui::structure("debugger", s, window, cx)))))
            .when(!data.is_empty(), |el| el.child(v_flex().gap_4().child(heading("Data structures"))
                .children(data.iter().map(|s| crate::gen_ui::structure("debugger", s, window, cx)))))
            .when(structures.is_empty(), |el| el.child(div().text_xs().text_color(theme.muted_foreground).child("No variables yet")))
            .when(!stdout.is_empty(), |el| el.child(v_flex().gap_1().child(div().text_xs().text_color(theme.muted_foreground).child("Printed output"))
                .child(div().p_2().rounded_md().bg(theme.muted).text_xs().font_family(theme.mono_font_family.clone()).child(stdout))))
            .when(last, |el| el.children(trace.map(|t| {
                let ok = self.verdict(self.selected);
                let color = match ok { Some(true) => theme.success, Some(false) => theme.danger, None => theme.info };
                v_flex().gap_1().p_3().rounded_lg().bg(color.opacity(0.08)).border_1().border_color(color.opacity(0.3))
                    .child(h_flex().gap_2().items_center().child(Icon::new(if ok == Some(false) { IconName::CircleX } else { IconName::CircleCheck }).size_4().text_color(color))
                        .child(div().text_sm().font_weight(FontWeight::MEDIUM).child(match ok { Some(true) => "Matches expected", Some(false) => "Differs from expected", None => "Finished" })))
                    .child(div().text_xs().font_family(theme.mono_font_family.clone()).child(format!("output   {}", t.output.clone().or(t.error.clone()).unwrap_or_default())))
                    .when_some(expected.clone(), |el, e| el.child(div().text_xs().font_family(theme.mono_font_family.clone()).text_color(theme.muted_foreground).child(format!("expected {e}"))))
            })))
            .into_any_element()
    }
}

fn empty(icon: IconName, title: &str, detail: &str, theme: &gpui_kit::component::Theme) -> AnyElement {
    v_flex().size_full().items_center().justify_center().gap_2().p_6()
        .child(Icon::new(icon).size_8().text_color(theme.muted_foreground.opacity(0.6)))
        .child(div().text_sm().font_weight(FontWeight::MEDIUM).child(title.to_owned()))
        .when(!detail.is_empty(), |el| el.child(div().max_w(px(420.)).text_center().text_xs().text_color(theme.muted_foreground).child(detail.to_owned())))
        .into_any_element()
}

impl Workspace {
    pub fn preview_debug(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(snapshot) = self.debug_snapshot(cx) else { return };
        let selected = self.session.as_ref().map_or(0, |session| session.selected_case);
        self.debug_mode = true;
        self.debugger.update(cx, |debugger, cx| { debugger.preview(snapshot, selected, cx); debugger.focus(window, cx); });
        cx.notify();
    }

    /// Switches the center between the editor and the debugger, recording cases on entry.
    pub fn set_debug(&mut self, on: bool, window: &mut Window, cx: &mut Context<Self>) {
        if on {
            self.save_now(cx);
            let Some(snapshot) = self.debug_snapshot(cx) else { self.flash("Open a problem first", cx); return };
            let selected = self.session.as_ref().map_or(0, |s| s.selected_case);
            self.debug_mode = true;
            self.debugger.update(cx, |debugger, cx| { debugger.load(snapshot, selected, cx); debugger.focus(window, cx); });
            self.flash("Debug", cx);
        } else {
            self.debug_mode = false;
            self.focus_editor(window, cx);
        }
        cx.notify();
    }

    fn debug_snapshot(&self, cx: &App) -> Option<Snapshot> {
        let session = self.session.as_ref()?;
        let question = session.question.as_ref()?;
        Some(Snapshot {
            slug: session.slug.clone(), language: session.language, python: self.config.python.clone(),
            code: self.editor.read(cx).value().to_string(), starter: question.starter(session.language).unwrap_or_default().to_owned(), meta: question.meta.clone(), cases: session.cases.clone(),
            compare: Compare::for_statement(&question.content),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_kit::component::highlighter::HighlightTheme;

    #[::core::prelude::v1::test]
    fn debug_highlights_keep_multiline_context_and_row_byte_offsets() {
        let source = "def solve():\r\n\ttext = \"\"\"héllo\r\n世界\r\nend\"\"\"\r\n\treturn text\r\n".replace('\t', "    ");
        let lines = source_lines(&source);
        let mut highlighter = SyntaxHighlighter::new("python");
        highlighter.update(None, &ropey::Rope::from_str(&source), None);
        let theme = HighlightTheme::default_dark();
        assert_eq!(lines.len(), 5);
        assert_eq!(lines[2].text.as_ref(), "世界");
        assert!(lines[1].text.starts_with("    text"));
        for line in &lines {
            assert_eq!(&source[line.range.clone()], line.text.as_ref());
            for (range, _) in line_highlights(&highlighter, &line.range, &theme) {
                assert!(range.end <= line.text.len());
                assert!(line.text.is_char_boundary(range.start) && line.text.is_char_boundary(range.end));
            }
        }
        let keyword = line_highlights(&highlighter, &lines[0].range, &theme);
        assert!(keyword.iter().any(|(range, style)| range.start == 0 && range.end == 3 && style.color.is_some()));
        let string = line_highlights(&highlighter, &lines[2].range, &theme);
        assert!(string.iter().any(|(range, style)| range == &(0.."世界".len()) && style.color.is_some()));
    }
}
