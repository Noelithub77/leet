//! Assist actions: what each asks the agent, and the typed answer it must return.
//! Local agents get a JSON Schema derived from these types; web chats get a prose prompt.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::agents::Access;
use crate::language::Language;
use crate::viz::Scene;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Action {
    Hints,
    Tests,
    Bugs,
    Analyze,
    Stuck,
    Explain,
    Visualize,
    Optimize,
    Pattern,
    DryRun,
    Solve,
    Ask,
    Review,
}

impl Action {
    pub const ALL: [Self; 11] = [
        Self::Hints, Self::Stuck, Self::Bugs, Self::Tests, Self::Analyze, Self::Optimize,
        Self::Visualize, Self::DryRun, Self::Pattern, Self::Explain, Self::Solve,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Self::Hints => "hints",
            Self::Tests => "tests",
            Self::Bugs => "bugs",
            Self::Analyze => "analyze",
            Self::Stuck => "stuck",
            Self::Explain => "explain",
            Self::Visualize => "visualize",
            Self::Optimize => "optimize",
            Self::Pattern => "pattern",
            Self::DryRun => "dry-run",
            Self::Solve => "solve",
            Self::Ask => "ask",
            Self::Review => "review",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Hints => "Hints",
            Self::Tests => "Edge cases",
            Self::Bugs => "Find bugs",
            Self::Analyze => "Complexity",
            Self::Stuck => "I'm stuck",
            Self::Explain => "Explain",
            Self::Visualize => "Visualize",
            Self::Optimize => "Optimize",
            Self::Pattern => "Pattern",
            Self::DryRun => "Dry run",
            Self::Solve => "Solve",
            Self::Ask => "Chat",
            Self::Review => "Analyse my solution",
        }
    }

    /// One line for the tooltip.
    pub fn summary(self) -> &'static str {
        match self {
            Self::Hints => "Escalating hints, one at a time",
            Self::Tests => "Edge and stress cases to add to your tests",
            Self::Bugs => "Bugs in your code, by line",
            Self::Analyze => "Time and space of your code, line by line",
            Self::Stuck => "Where you're stuck and the next step",
            Self::Explain => "Intuition, approaches, and an optimal solution",
            Self::Visualize => "Animated walkthrough of the algorithm",
            Self::Optimize => "Your complexity against the best, with a nudge",
            Self::Pattern => "The pattern, a template, and similar problems",
            Self::DryRun => "Trace your code on a case and find the first wrong step",
            Self::Solve => "Write a solution quickly, then confirm submission",
            Self::Ask => "Ask about this problem or your code",
            Self::Review => "Correctness, complexity, improvements, and a visual dry run",
        }
    }

    /// Actions that read the user's attempt and say nothing useful without one.
    pub fn needs_attempt(self) -> bool {
        matches!(self, Self::Bugs | Self::Analyze | Self::Stuck | Self::Optimize | Self::DryRun | Self::Review)
    }

    pub fn access(self) -> Access { Access::Full }

    /// Every shortcut and free-form turn shares the native response contract.
    pub fn schema(self) -> serde_json::Value {
        crate::agents::strict_schema(serde_json::to_value(schemars::schema_for!(Reply)).unwrap_or_default())
    }

    pub fn instructions(self) -> &'static str {
        match self {
            Self::Review => "Review my current solution as written. Return correctness bugs (or explicitly say none found), time and space complexity with line costs, improvements compared with the best approach, and a dry run of my code on the focus test case. Include an animated scene showing the actual changing variables and data structures. Explain the first wrong step if present. Do not modify my solution or supply a replacement solution. You may write native response artifacts. Keep every section concise.",
            Self::Hints => "Give 4 escalating hints that let me solve it myself. Level 1 is a small observation about the problem; level 2 names the useful data structure or pattern; level 3 states the key insight; level 4 outlines the approach in plain words. Never include code. If my attempt is present, aim the first hint at what my attempt misses. Each body is at most 2 sentences.",
            Self::Tests => "Use the requested count and case types from the additional instructions; default to 6 to 10 cases when no count is given. Propose test cases that are most likely to break solutions to this problem: empty and minimum inputs, boundaries from the constraints, duplicates, negatives, sorted and reversed orders, single elements, and one larger stress-style case that is still small enough to read. Compute every expected output carefully by reasoning step by step; it must be exactly what the judge returns. Inputs use the exact LeetCode input format: one argument per line, JSON values.",
            Self::Bugs => "Review my attempt for correctness bugs only: wrong logic, off-by-one errors, missed edge cases, wrong return values, mutation mistakes, overflow, and exceptions. Cite the 1-based line from the numbered code. Do not rewrite the solution; each fix is the smallest change in words or one short code fragment. If you find no bugs, return an empty list and say so in the summary. Use the failing tests when present.",
            Self::Analyze => "Analyze the time and space complexity of my attempt as written, not of the optimal solution. Explain the dominant term, attribute cost to the lines that create it, and compare with the best known complexity for this problem. Use n, m, k exactly as named in the constraints.",
            Self::Stuck => "Diagnose exactly where I'm stuck from my attempt and failing tests: the misconception or missing idea, the line where my reasoning goes wrong, and the single next step I should take. Do not give the solution or code. End with one Socratic question that leads me to the insight.",
            Self::Explain => "Explain the problem completely: the intuition, the pattern it belongs to, a brute-force approach and the optimal approach with why it works, an optimal idiomatic solution in the required language that preserves the judge interface, pitfalls, and a short visual walkthrough of the optimal approach on the first example (4 to 10 frames).",
            Self::Visualize => "Create an animated walkthrough of the optimal algorithm on the first example, as 6 to 16 frames. Each frame shows the data structures at that moment using the available structure kinds, marks pointers and windows, and highlights with tones what changed (changed), what is being compared (active), what was visited (visited), and what is final (done). Keep labels equal to variable names. Captions are one short sentence. If my attempt is present and correct, walk my approach and set `line` to my code's lines.",
            Self::Optimize => "Compare the complexity of my attempt with the best known for this problem. Name the bottleneck line, give one nudge toward the faster approach without code, then the idea in two or three sentences (shown only when I ask). If my attempt is already optimal, say so and suggest a constant-factor or space improvement instead.",
            Self::Pattern => "Identify the algorithmic pattern this problem belongs to, the cues in the statement that reveal it, a short generic code template of the pattern in the required language (not the solution to this problem), and 3 to 5 similar problems chosen ONLY from the candidate list below, using their exact slugs.",
            Self::DryRun => "Dry-run my attempt on the given case exactly as the code executes, not as it was intended. Produce frames for each meaningful step (at most 30) with `line` set to the executed 1-based line and structures showing the variables at that moment. Set `wrong_frame` to the first frame whose state diverges from a correct solution's state, explain why, and give a fix hint without rewriting the code.",
            Self::Ask => "Answer my question about this problem and my current code. Use the prior conversation when relevant, but treat the current code and test results as the latest state. Use native tools when needed and respond concisely. Edit files only when requested. Leave judge submission to the app confirmation. Do not reveal a full solution unless I ask for one.",
            Self::Solve => "Quick solve: use the supplied statement and code to write a correct, efficient solution directly to the specified solution file, preserving the judge interface and required language. Edit only that file. This is a single write-and-report task, not a verification loop. On the first attempt, do not plan aloud, research, inspect unrelated files, run commands or tests, perform verification, or create artifacts. Use a direct file write/edit and immediately return a solve report with one sentence for the approach, time and space complexity, and a brief summary. Do not claim tests passed. The app handles submission confirmation and judge feedback. When failure feedback is supplied, fix that specific failure and use only focused investigation or checks needed for the fix.",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HintLevel { Nudge, Pattern, Insight, Approach }

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Hint { pub level: HintLevel, pub title: String, pub body: String }

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Hints { pub hints: Vec<Hint> }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TestKind { Edge, Boundary, Tricky, Stress, Typical }

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TestIdea {
    pub kind: TestKind,
    pub title: String,
    /// One argument per line in LeetCode's input format.
    pub input: String,
    pub expected: String,
    pub why: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Tests { pub cases: Vec<TestIdea> }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Severity { Bug, Risk, Style }

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Bug { pub line: u32, pub severity: Severity, pub title: String, pub detail: String, pub fix: String }

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Bugs {
    pub summary: String,
    pub bugs: Vec<Bug>,
    /// A small input that exposes the first bug, in LeetCode's input format.
    pub failing_input: Option<String>,
}

/// Growth classes the UI can plot without trusting numbers from the model.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Growth { Constant, Logarithmic, Linear, Linearithmic, Quadratic, Cubic, Exponential, Factorial }

impl Growth {
    pub fn notation(self) -> &'static str {
        match self {
            Self::Constant => "O(1)",
            Self::Logarithmic => "O(log n)",
            Self::Linear => "O(n)",
            Self::Linearithmic => "O(n log n)",
            Self::Quadratic => "O(n²)",
            Self::Cubic => "O(n³)",
            Self::Exponential => "O(2ⁿ)",
            Self::Factorial => "O(n!)",
        }
    }

    /// Work at input size `n`, for plotting curves on a shared scale.
    pub fn cost(self, n: f64) -> f64 {
        let log = n.max(2.).log2();
        match self {
            Self::Constant => 1.,
            Self::Logarithmic => log,
            Self::Linear => n,
            Self::Linearithmic => n * log,
            Self::Quadratic => n * n,
            Self::Cubic => n * n * n,
            Self::Exponential => 2f64.powf(n.min(1000.)),
            Self::Factorial => (1..=(n.min(170.) as u32)).map(f64::from).product(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct LineCost { pub line: u32, pub cost: String, pub note: String }

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Analysis {
    /// Exact notation with the problem's variables, such as `O(n·m)`.
    pub time: String,
    pub space: String,
    pub time_growth: Growth,
    pub best_time: String,
    pub best_space: String,
    pub best_growth: Growth,
    pub summary: String,
    pub lines: Vec<LineCost>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Stuck {
    pub diagnosis: String,
    pub line: Option<u32>,
    /// The concept to revisit, in a few words.
    pub concept: String,
    pub next_step: String,
    pub question: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Approach { pub name: String, pub idea: String, pub time: String, pub space: String }

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Explanation {
    pub intuition: String,
    pub pattern: String,
    pub approaches: Vec<Approach>,
    /// The optimal solution in the required language.
    pub code: String,
    pub pitfalls: Vec<String>,
    pub walkthrough: Scene,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Optimization {
    pub already_optimal: bool,
    pub current: String,
    pub current_growth: Growth,
    pub best: String,
    pub best_growth: Growth,
    pub bottleneck_line: Option<u32>,
    pub bottleneck: String,
    pub nudge: String,
    pub idea: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Similar { pub slug: String, pub title: String, pub why: String }

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PatternMatch {
    pub pattern: String,
    pub why: String,
    pub signals: Vec<String>,
    pub template: String,
    pub similar: Vec<Similar>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DryRun {
    pub scene: Scene,
    pub wrong_frame: Option<usize>,
    /// With a recorded trace: the `#` number of the first wrong step.
    pub wrong_step: Option<usize>,
    pub explanation: String,
    pub fix_hint: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SolveReport { pub approach: String, pub time: String, pub space: String, pub summary: String }

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SolutionReview {
    pub correctness: Bugs,
    pub complexity: Analysis,
    pub improvements: Optimization,
    pub dry_run: DryRun,
}

/// The parsed answer of one action.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "action", content = "answer", rename_all = "kebab-case")]
pub enum Answer {
    Hints(Hints),
    Tests(Tests),
    Bugs(Bugs),
    Analyze(Analysis),
    Stuck(Stuck),
    Explain(Explanation),
    Visualize(Scene),
    Optimize(Optimization),
    Pattern(PatternMatch),
    DryRun(DryRun),
    Solve(SolveReport),
    Chat(String),
    Review(SolutionReview),
}

/// A single object envelope keeps the shared tagged response compatible with strict providers.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct Reply { pub response: Answer }

impl Answer {
    pub fn parse(action: Action, value: serde_json::Value) -> serde_json::Result<Self> {
        Ok(match action {
            Action::Hints => Self::Hints(serde_json::from_value(value)?),
            Action::Tests => Self::Tests(serde_json::from_value(value)?),
            Action::Bugs => Self::Bugs(serde_json::from_value(value)?),
            Action::Analyze => Self::Analyze(serde_json::from_value(value)?),
            Action::Stuck => Self::Stuck(serde_json::from_value(value)?),
            Action::Explain => Self::Explain(serde_json::from_value(value)?),
            Action::Visualize => Self::Visualize(serde_json::from_value(value)?),
            Action::Optimize => Self::Optimize(serde_json::from_value(value)?),
            Action::Pattern => Self::Pattern(serde_json::from_value(value)?),
            Action::DryRun => Self::DryRun(serde_json::from_value(value)?),
            Action::Solve => Self::Solve(serde_json::from_value(value)?),
            Action::Ask => Self::Chat(serde_json::from_value(value)?),
            Action::Review => Self::Review(serde_json::from_value(value)?),
        })
    }
}

/// Runs an action on a local agent and parses its typed answer, asking once for a
/// corrected answer when the first does not match. Solve returns its report when parseable.
pub fn execute(action: Action, request: &crate::agents::Request, events: &mut dyn FnMut(crate::agents::Event), cancel: &crate::agents::Cancel) -> anyhow::Result<(Option<Answer>, crate::agents::Outcome)> {
    execute_with(action, request, &mut |request| crate::agents::run(request, events, cancel))
}

fn execute_with(action: Action, request: &crate::agents::Request, run: &mut dyn FnMut(&crate::agents::Request) -> anyhow::Result<crate::agents::Outcome>) -> anyhow::Result<(Option<Answer>, crate::agents::Outcome)> {
    execute_with_artifact(action, request, run, None)
}

/// File-based native artifacts may be the result even when the final text is only a report.
pub fn execute_artifact(action: Action, request: &crate::agents::Request, path: &std::path::Path, events: &mut dyn FnMut(crate::agents::Event), cancel: &crate::agents::Cancel) -> anyhow::Result<(Option<Answer>, crate::agents::Outcome)> {
    execute_with_artifact(action, request, &mut |request| crate::agents::run(request, events, cancel), Some(path))
}

fn execute_with_artifact(action: Action, request: &crate::agents::Request, run: &mut dyn FnMut(&crate::agents::Request) -> anyhow::Result<crate::agents::Outcome>, artifact: Option<&std::path::Path>) -> anyhow::Result<(Option<Answer>, crate::agents::Outcome)> {
    let outcome = run(request)?;
    if let Some(path) = artifact.filter(|path| path.exists()) { return Ok((Some(crate::chat::artifacts::load(path)?), outcome)); }
    if action == Action::Ask && request.schema.is_none() { return Ok((Some(Answer::Chat(outcome.text.clone())), outcome)); }
    match parse(action, &outcome) {
        Ok(answer) => Ok((Some(answer), outcome)),
        Err(_) if action == Action::Solve => Ok((None, outcome)),
        Err(error) => {
            let resume = if request.agent.kind == crate::agents::AgentKind::Claude && request.access == Access::ReadOnly { None } else { outcome.session.clone() };
            let repair = repair_prompt(action, &error, &outcome.text);
            let prompt = if resume.is_some() { repair } else { format!("{}\n\n{repair}", request.prompt) };
            let retry = crate::agents::Request { prompt, resume, ..request.clone() };
            let outcome = run(&retry)?;
            let answer = parse(action, &outcome).map_err(|error| anyhow::anyhow!("The agent's answer was not valid: {error}"))?;
            Ok((Some(answer), outcome))
        }
    }
}

fn parse(action: Action, outcome: &crate::agents::Outcome) -> Result<Answer, String> {
    let value = match &outcome.structured {
        Some(value) => value.clone(),
        None => json_in(&outcome.text).ok_or_else(|| "no JSON object in the answer".to_owned())?,
    };
    if value.get("response").is_some() {
        return serde_json::from_value::<Reply>(value).map(|reply| reply.response).map_err(|error| error.to_string());
    }
    Answer::parse(action, value).map_err(|error| error.to_string())
}

/// The first JSON object in free text: raw, fenced, or embedded.
fn json_in(text: &str) -> Option<serde_json::Value> {
    let trimmed = text.trim();
    if let Ok(value) = serde_json::from_str(trimmed) { return Some(value); }
    if let Some(start) = trimmed.find("```") {
        let body = &trimmed[start + 3..];
        let body = body.strip_prefix("json").unwrap_or(body);
        if let Some(end) = body.find("```") { if let Ok(value) = serde_json::from_str(body[..end].trim()) { return Some(value); } }
    }
    let (start, end) = (trimmed.find('{')?, trimmed.rfind('}')?);
    serde_json::from_str(trimmed.get(start..=end)?).ok()
}

/// A test case with its latest local result, for context.
pub struct CaseContext<'a> {
    pub input: &'a str,
    pub expected: Option<&'a str>,
    /// Actual output or error, when the case has run.
    pub outcome: Option<String>,
}

/// Everything the prompt may use about the open problem.
pub struct Context<'a> {
    pub title: &'a str,
    pub difficulty: &'a str,
    pub url: &'a str,
    pub statement_html: &'a str,
    pub code: &'a str,
    pub starter: &'a str,
    pub language: Language,
    pub cases: Vec<CaseContext<'a>>,
    /// The case the dry run follows.
    pub focus_case: Option<usize>,
    /// Absolute path of the solution file, for Solve.
    pub solution_path: &'a str,
    /// The previous verdict, when Solve continues after a failed attempt.
    pub feedback: Option<&'a str>,
    /// A recorded execution of the focus case, from [`trace_digest`].
    pub trace: Option<&'a str>,
}

const STATEMENT_LIMIT: usize = 6000;
const CODE_LIMIT: usize = 8000;

/// Shared environment instructions describe real capabilities, not imaginary app tools.
pub fn environment_prompt(web: bool) -> String {
    let mut out = format!("You are an agent inside Leet, a safe Rust/GPUI coding-practice IDE. Environment: {} / {}. All requests and prompt shortcuts belong to ordinary conversation threads. Use prior conversation when relevant; current editor code and tests are the latest state.\n", std::env::consts::OS, std::env::consts::ARCH);
    if web {
        out.push_str("This is an external web conversation. Leet supplies context but exposes no local file, command, runner, or native rendering tools here. Do not claim to have executed local operations.\n");
    } else {
        out.push_str("Native agent capabilities: your selected provider's installed file read/write/edit, search, and command tools have full user-authorized access with non-interactive permissions. Use your actual tool catalog; other provider-specific capabilities depend on that provider. Work directly in files with native tools. Edit only files relevant to the user's request; report actual execution results.\nLeet renders validated native response kinds: chat (Markdown), hints, tests (addable cases), bugs (line links), analyze (complexity curves), stuck, explain, visualize (animated scenes), optimize, pattern (problem links), dry-run, review (combined analysis), and solve (solution report). Native scenes support arrays, grids, linked lists, trees, graphs, heaps, stacks/queues, sets, maps, variables and playback. These are typed response/artifact formats, not callable app tools or arbitrary executable UI.\nThe app provides editor reload, local tests, trace recording, judge runs and submission confirmation through its existing controls. You may run available runtimes or commands directly. Never submit to a judge via commands or HTTP: return a solve report and leave submission to the app's explicit confirmation. Never fabricate runtime results.\n");
    }
    if !web { out.push_str(&crate::snippets::cli::context()); }
    out
}

/// The prompt for a local agent; the schema travels separately.
pub fn prompt(action: Action, ctx: &Context) -> String {
    let mut out = environment_prompt(false);
    out.push_str(&format!("\n## Request shortcut\n{}\n{}\n", action.label(), action.instructions()));
    out.push_str("Return one JSON object with a `response` containing a tagged native answer: {\"response\":{\"action\":\"chat\",\"answer\":\"Markdown here\"}}. Choose any supported response kind appropriate to the user's request, regardless of the shortcut.\n");
    if action == Action::Solve { out.push_str("Use the solve response kind for this task. Return the report in your final JSON reply; do not write a response artifact.\n"); }
    push_problem(&mut out, ctx);
    if action != Action::Solve { out.push_str(VISUAL_GUIDE); }
    if let (Action::DryRun, Some(trace)) = (action, ctx.trace) {
        out.push_str("\n## Recorded execution of the focus case\nThis is the real trace from running my code, one step per line: `#step Lline event function | variables`. Do not simulate; read it. Set `wrong_step` to the `#` of the first step whose state diverges from a correct solution, and make `scene` 3 to 6 key frames around it.\n");
        out.push_str(trace);
        out.push('\n');
    }
    if action == Action::Pattern {
        out.push_str("\nCandidate similar problems (slug | title | topic):\n");
        for entry in crate::roadmap::ENTRIES.iter().filter(|e| e.in_list(crate::roadmap::List::NeetCode250) && e.title != ctx.title) {
            out.push_str(&format!("{} | {} | {}\n", entry.slug, entry.title, entry.topic));
        }
    }
    out.push_str(&format!("\nSolution file: {}\n", ctx.solution_path));
    if let Some(feedback) = ctx.feedback { out.push_str(&format!("\nThe previous attempt was judged:\n{feedback}\nFix the solution file accordingly.\n")); }
    out
}

/// A self-contained prompt for a web chat, which cannot enforce a schema.
pub fn web_prompt(action: Action, ctx: &Context) -> String {
    let mut out = environment_prompt(true);
    out.push_str(&format!("{}\nAnswer in Markdown.\n", action.instructions()));
    push_problem(&mut out, ctx);
    out
}

/// Asks the agent to restate an answer that failed to parse.
pub fn repair_prompt(action: Action, error: &str, text: &str) -> String {
    let mut text = text.to_owned();
    truncate(&mut text, CODE_LIMIT);
    format!("Your previous {} answer did not match the schema ({error}). Return only the corrected JSON object.\n\nPrevious answer:\n{text}", action.label())
}

/// A compact text form of a recorded trace for the agent: changed variables only, sampled when long.
pub fn trace_digest(trace: &crate::debugger::Trace, limit: usize) -> String {
    use crate::debugger::StepKind;
    let mut out = String::new();
    let mut last: std::collections::HashMap<&str, String> = std::collections::HashMap::new();
    let every = (trace.steps.len() / limit.max(1)).max(1);
    let mut frame = Vec::new();
    for (index, step) in trace.steps.iter().enumerate() {
        let identity: Vec<_> = step.stack.iter().map(|f| f.function.as_str()).collect();
        if identity != frame || step.kind == StepKind::Call { last.clear(); }
        frame = identity;
        let notable = step.kind != StepKind::Line || index + 1 == trace.steps.len();
        if index % every != 0 && !notable { continue; }
        let current: std::collections::HashMap<_, _> = step.locals.iter().map(|v| (v.name.as_str(), short(&v.value, 2))).collect();
        let mut vars: Vec<String> = step.locals.iter().filter_map(|v| {
            let value = &current[v.name.as_str()];
            (last.get(v.name.as_str()) != Some(value)).then(|| format!("{}={value}", v.name))
        }).collect();
        let mut removed: Vec<_> = last.keys().filter(|name| !current.contains_key(*name)).copied().collect();
        removed.sort_unstable();
        vars.extend(removed.into_iter().map(|name| format!("{name}=<removed>")));
        last = current;
        let kind = match step.kind { StepKind::Call => "call", StepKind::Line => "line", StepKind::Return => "return", StepKind::Exception => "raise" };
        let mut line = format!("#{index} L{} {kind} {} | {}", step.line, step.function, vars.join(" "));
        if let Some(value) = &step.returned { line.push_str(&format!(" -> {}", short(value, 2))); }
        if let Some(error) = &step.exception { line.push_str(&format!(" !! {error}")); }
        out.push_str(&line);
        out.push('\n');
    }
    if trace.truncated { out.push_str("(trace truncated)\n"); }
    if let Some(output) = &trace.output { out.push_str(&format!("output: {output}\n")); }
    if let Some(error) = &trace.error { out.push_str(&format!("error: {error}\n")); }
    truncate(&mut out, 24_000);
    out
}

fn short(value: &crate::debugger::Value, depth: usize) -> String {
    use crate::debugger::Value;
    let list = |items: &[Value], extra: usize, open: &str, close: &str| {
        if depth == 0 { return format!("{open}…{close}"); }
        let mut parts: Vec<String> = items.iter().take(12).map(|v| short(v, depth - 1)).collect();
        if items.len() > 12 || extra > 0 { parts.push("…".into()); }
        format!("{open}{}{close}", parts.join(","))
    };
    match value {
        Value::None => "None".into(),
        Value::Bool { v } => v.to_string(),
        Value::Int { v } => v.clone(),
        Value::Float { v } => v.to_string(),
        Value::Str { v } | Value::Char { v } => format!("{v:?}"),
        Value::List { items, truncated, .. } => list(items, *truncated, "[", "]"),
        Value::Set { items, truncated, .. } => list(items, *truncated, "{", "}"),
        Value::Map { entries, truncated, .. } => {
            if depth == 0 { return "{…}".into(); }
            let mut parts: Vec<String> = entries.iter().take(10).map(|(k, v)| format!("{}:{}", short(k, depth - 1), short(v, depth - 1))).collect();
            if entries.len() > 10 || *truncated > 0 { parts.push("…".into()); }
            format!("{{{}}}", parts.join(","))
        }
        Value::Tree { nodes, .. } => format!("tree({} nodes)", nodes.len()),
        Value::Linked { nodes, cycle_to } => format!("{}{}", list(nodes, 0, "[", "]").replace(',', "->"), if cycle_to.is_some() { "(cycle)" } else { "" }),
        Value::Object { class, .. } => class.clone(),
        Value::Ref { id } => format!("&{id}"),
        Value::Opaque { type_name } => type_name.clone(),
    }
}

fn push_problem(out: &mut String, ctx: &Context) {
    let mut statement = crate::prompts::statement_markdown(ctx.statement_html);
    truncate(&mut statement, STATEMENT_LIMIT);
    out.push_str(&format!("\n## Problem: {} ({})\n{}\n\n{}\n", ctx.title, ctx.difficulty, ctx.url, statement.trim()));
    out.push_str(&format!("\nRequired language: {}.\n", ctx.language.label()));
    if !ctx.starter.trim().is_empty() {
        out.push_str(&format!("Judge interface (preserve exactly):\n```{}\n{}\n```\n", ctx.language.id(), ctx.starter.trim()));
    }
    if ctx.url.contains("codeforces.com") {
        out.push_str("The program reads stdin and writes stdout.\n");
    }
    if crate::workspace::has_attempt(ctx.code, ctx.starter) {
        let mut numbered = numbered(ctx.code);
        truncate(&mut numbered, CODE_LIMIT);
        out.push_str(&format!("\n## My attempt (numbered lines)\n```{}\n{numbered}\n```\n", ctx.language.id()));
    } else {
        out.push_str("\nI have not written an attempt yet.\n");
    }
    if !ctx.cases.is_empty() {
        out.push_str("\n## Test cases\n");
        for (index, case) in ctx.cases.iter().enumerate().take(8) {
            let focus = if ctx.focus_case == Some(index) { " (dry-run this one)" } else { "" };
            out.push_str(&format!("Case {}{focus}:\ninput:\n{}\n", index + 1, case.input.trim()));
            if let Some(expected) = case.expected { out.push_str(&format!("expected: {}\n", expected.trim())); }
            if let Some(outcome) = &case.outcome { out.push_str(&format!("my result: {}\n", outcome.trim())); }
        }
    }
}

/// Code with `N | ` prefixes so answers can cite exact lines.
pub fn numbered(code: &str) -> String {
    let width = code.lines().count().max(1).to_string().len();
    code.lines().enumerate().map(|(index, line)| format!("{:>width$} | {line}", index + 1)).collect::<Vec<_>>().join("\n")
}

fn truncate(text: &mut String, limit: usize) {
    if text.len() > limit {
        let mut end = limit;
        while !text.is_char_boundary(end) { end -= 1; }
        text.truncate(end);
        text.push_str("\n…");
    }
}

const VISUAL_GUIDE: &str = "
## Visual structures
Frames hold `structures`, drawn natively by leet. Use the kind that matches the data:
- array: items with pointers (`i`, `l`, `r`, `mid`) by index and spans (inclusive windows).
- grid: matrices and DP tables; row/col labels optional; grid pointers by (row, col).
- tree: binary trees as nodes with ids and left/right child ids; root id.
- graph: nodes and edges (directed or not) for adjacency, BFS/DFS, topological order.
- linked_list: nodes in next order, pointers by index, cycle_to for a tail cycle.
- stack (top is last), queue (front is first), heap (array order), map, set, intervals.
- vars: scalars such as counters, sums, and the current answer.
Tones: active = being compared now, changed = written this step, visited = seen before, done = final or in the answer, muted = outside the window, warn and error for problems.
Keep 1 to 4 structures per frame, the same labels across frames, and values short.
";

#[cfg(test)]
mod tests {
    use super::*;

    pub(super) fn ctx<'a>(code: &'a str, cases: Vec<CaseContext<'a>>) -> Context<'a> {
        Context {
            title: "Two Sum", difficulty: "Easy", url: "https://leetcode.com/problems/two-sum/",
            statement_html: "<p>Given <code>nums</code>, 10<sup>4</sup>.</p>", code,
            starter: "class Solution:\n    def twoSum(self, nums: List[int], target: int) -> List[int]:",
            language: Language::Python, cases, focus_case: Some(0), solution_path: "/tmp/1-two-sum.py", feedback: None, trace: None,
        }
    }

    #[test]
    fn prompts_number_attempt_lines_and_include_results() {
        let cases = vec![CaseContext { input: "[2,7]\n9", expected: Some("[0,1]"), outcome: Some("[1,0]".into()) }];
        let prompt = prompt(Action::Bugs, &ctx("a = 1\nreturn a", cases));
        assert!(prompt.contains("1 | a = 1") && prompt.contains("2 | return a"));
        assert!(prompt.contains("my result: [1,0]") && prompt.contains("10^4") && prompt.contains("dry-run this one"));
        assert!(prompt.contains("Visual structures"));
    }

    #[test]
    fn starter_only_attempts_are_not_sent() {
        let context = ctx("class Solution:\n    def twoSum(self, nums: List[int], target: int) -> List[int]:", vec![]);
        assert!(prompt(Action::Hints, &context).contains("not written an attempt"));
    }

    #[test]
    fn visual_and_pattern_prompts_carry_their_guides() {
        assert!(prompt(Action::Visualize, &ctx("", vec![])).contains("linked_list"));
        let pattern = prompt(Action::Pattern, &ctx("", vec![]));
        assert!(pattern.contains("valid-anagram |") && !pattern.contains("two-sum | Two Sum"));
    }

    #[test]
    fn solve_targets_the_file_and_carries_feedback() {
        let mut context = ctx("", vec![]);
        context.feedback = Some("Wrong Answer on [3,3]");
        let prompt = prompt(Action::Solve, &context);
        assert!(prompt.contains("/tmp/1-two-sum.py") && prompt.contains("Wrong Answer on [3,3]"));
        assert!(!prompt.contains("Do not use tools"));
        assert_eq!(Action::Solve.access(), Access::Full);
        assert!(prompt.contains("single write-and-report task") && prompt.contains("do not plan aloud"));
        assert!(!prompt.contains(VISUAL_GUIDE));
        assert!(prompt.contains("do not write a response artifact"));
    }

    #[test]
    fn every_action_has_a_schema_and_parses_its_answer() {
        for action in Action::ALL { assert!(action.schema().is_object(), "{action:?}"); }
        let hints = serde_json::json!({"hints":[{"level":"nudge","title":"Look","body":"Pairs"}]});
        assert!(matches!(Answer::parse(Action::Hints, hints), Ok(Answer::Hints(h)) if h.hints.len() == 1));
        let analysis = serde_json::json!({"time":"O(n²)","space":"O(1)","time_growth":"quadratic","best_time":"O(n)","best_space":"O(n)","best_growth":"linear","summary":"s","lines":[]});
        assert!(matches!(Answer::parse(Action::Analyze, analysis), Ok(Answer::Analyze(a)) if a.time_growth > a.best_growth));
    }

    #[test]
    fn combined_review_uses_shared_capabilities_and_requires_every_section() {
        assert!(Action::Review.needs_attempt());
        assert_eq!(Action::Review.access(), Access::Full);
        let schema = Action::Review.schema();
        let properties = schema["properties"].as_object().unwrap();
        assert!(properties.contains_key("response"));
        assert!(schema.to_string().contains("correctness"));
        assert!(Answer::parse(Action::Review, serde_json::json!({"complexity": {}})).is_err());
        let prompt = prompt(Action::Review, &ctx("x = 1", vec![]));
        assert!(prompt.contains("visual") || prompt.contains("animated"));
        assert!(prompt.contains("x = 1") && prompt.contains("correctness"));
    }

    #[test]
    fn trace_digests_show_changes_and_events() {
        use crate::debugger::{StackFrame, Step, StepKind, Trace, Value, Variable};
        let step = |kind, line, i: &str| Step { kind, line, function: "twoSum".into(), stack: vec![StackFrame { function: "twoSum".into(), line }],
            locals: vec![Variable { name: "i".into(), value: Value::Int { v: i.into() } }, Variable { name: "nums".into(), value: Value::List { kind: "list".into(), items: vec![Value::Int { v: "2".into() }], truncated: 0 } }],
            returned: None, exception: None, stdout_len: 0 };
        let trace = Trace { language: Language::Python, case_id: 0, steps: vec![step(StepKind::Call, 1, "0"), step(StepKind::Line, 2, "0"), step(StepKind::Line, 3, "1")], truncated: false, output: Some("[0,1]".into()), error: None, stdout: String::new() };
        let digest = trace_digest(&trace, 100);
        assert!(digest.contains("#0 L1 call twoSum | i=0 nums=[2]"), "{digest}");
        assert!(digest.contains("#1 L2 line twoSum | \n") && digest.contains("#2 L3 line twoSum | i=1"), "{digest}");
        let mut context = ctx("x = 1", vec![]);
        context.trace = Some(&digest);
        assert!(prompt(Action::DryRun, &context).contains("Recorded execution") && !prompt(Action::Bugs, &context).contains("Recorded execution"));
    }

    #[test]
    fn answers_are_found_in_fenced_or_embedded_text() {
        assert_eq!(json_in("```json\n{\"a\":1}\n```"), Some(serde_json::json!({"a":1})));
        assert_eq!(json_in("Here: {\"a\":{\"b\":2}} done"), Some(serde_json::json!({"a":{"b":2}})));
        assert_eq!(json_in("no json"), None);
    }

    #[test]
    fn growth_curves_order_by_cost() {
        let n = 64.;
        let costs: Vec<f64> = [Growth::Constant, Growth::Logarithmic, Growth::Linear, Growth::Linearithmic, Growth::Quadratic, Growth::Cubic, Growth::Exponential, Growth::Factorial].iter().map(|g| g.cost(n)).collect();
        assert!(costs.windows(2).all(|w| w[0] < w[1]), "{costs:?}");
    }
}

#[cfg(test)]
mod reliability_tests {
    use super::*;
    use crate::agents::{AgentKind, Detected, Outcome, Request, Selection};
    fn request(kind: AgentKind) -> Request {
        Request { agent: Detected { kind, path: "mock".into(), version: None }, selection: Selection { agent: kind, model: "mock".into(), effort: None, fast: false }, prompt: "original problem context".into(), schema: Some(Action::Hints.schema()), cwd: std::env::temp_dir(), access: Access::ReadOnly, resume: None }
    }
    #[test]
    fn legacy_markdown_questions_still_parse_without_a_schema() {
        let mut request = request(AgentKind::Codex);
        request.schema = None;
        let mut calls = 0;
        let (answer, _) = execute_with(Action::Ask, &request, &mut |request| {
            calls += 1;
            assert_eq!(request.access, Access::ReadOnly);
            assert!(request.schema.is_none());
            Ok(Outcome { text: "Check **before** inserting.".into(), session: None, structured: None })
        }).unwrap();
        assert_eq!(calls, 1);
        assert!(matches!(answer, Some(Answer::Chat(text)) if text == "Check **before** inserting."));
        assert_eq!(Action::Ask.access(), Access::Full);
        let context = super::tests::ctx("latest_code()", vec![]);
        let prompt = prompt(Action::Ask, &context);
        assert!(prompt.contains("latest_code()"));
        assert!(prompt.contains("full user-authorized access"));
        assert!(prompt.contains("one JSON object"));
    }

    #[test]
    fn ordinary_questions_can_return_native_scenes_and_file_artifacts() {
        let mut request = request(AgentKind::Codex); request.access = Access::Full; request.schema = Some(Action::Ask.schema());
        let scene = serde_json::json!({"response":{"action":"visualize","answer":{"title":"Pointers","frames":[]}}});
        let (answer, _) = execute_with(Action::Ask, &request, &mut |_| Ok(Outcome { text: scene.to_string(), session: None, structured: Some(scene.clone()) })).unwrap();
        assert!(matches!(answer, Some(Answer::Visualize(_))));
        let dir = tempfile::tempdir().unwrap(); let path = dir.path().join("response.json");
        let (answer, _) = execute_with_artifact(Action::Ask, &request, &mut |_| {
            crate::chat::artifacts::save(&path, &Answer::Chat("Artifact content".into()))?;
            Ok(Outcome { text: "Wrote the artifact".into(), session: None, structured: None })
        }, Some(&path)).unwrap();
        assert_eq!(answer, Some(Answer::Chat("Artifact content".into())));
        for action in Action::ALL { assert_eq!(action.schema(), Action::Ask.schema()); assert_eq!(action.access(), Access::Full); }
        assert!(environment_prompt(false).contains("submission confirmation"));
        assert!(environment_prompt(true).contains("exposes no local file"));
    }

    #[test]
    #[ignore = "Uses the signed-in Codex account to verify full native file access and response schema"]
    fn unified_chat_live_native_artifact() {
        let agent = crate::agents::detect().into_iter().find(|agent| agent.kind == AgentKind::Codex).expect("Codex installed");
        let catalog = crate::agents::catalog(&agent).unwrap();
        let cwd = tempfile::tempdir().unwrap(); let artifacts = tempfile::tempdir().unwrap(); let file = artifacts.path().join("response.json");
        let request = Request { agent, selection: Selection { agent: AgentKind::Codex, model: catalog.default_model.unwrap(), effort: Some("low".into()), fast: false }, prompt: format!("{}\nUse your native file tools to write exactly this JSON to {} (outside the working directory): {{\"response\":{{\"action\":\"visualize\",\"answer\":{{\"title\":\"Native artifact probe\",\"input\":\"\",\"frames\":[]}}}}}}. Then return the same response JSON. Do not read or alter any other files.", environment_prompt(false), file.display()), schema: Some(Action::Ask.schema()), cwd: cwd.path().into(), access: Access::Full, resume: None };
        let (answer, _) = execute_artifact(Action::Ask, &request, &file, &mut |_| {}, &crate::agents::Cancel::default()).unwrap();
        assert!(file.exists()); assert!(matches!(answer, Some(Answer::Visualize(scene)) if scene.title == "Native artifact probe"));
    }

    #[test]
    fn assist_repairs_plain_and_invalid_answers_once_with_fresh_claude_context() {
        for text in ["plain answer", "{broken", "{}"] {
            let mut calls = 0;
            let answer = execute_with(Action::Hints, &request(AgentKind::Claude), &mut |req| {
                calls += 1;
                if calls == 1 { return Ok(Outcome { text: text.into(), session: Some("unavailable".into()), structured: None }); }
                assert!(req.resume.is_none());
                assert!(req.prompt.contains("original problem context") && req.prompt.contains(text));
                Ok(Outcome { text: "{\"hints\":[{\"level\":\"nudge\",\"title\":\"Look\",\"body\":\"Pairs\"}]}".into(), session: None, structured: None })
            }).unwrap();
            assert!(matches!(answer.0, Some(Answer::Hints(_))));
            assert_eq!(calls, 2);
        }
        let mut calls = 0;
        assert!(execute_with(Action::Hints, &request(AgentKind::Codex), &mut |_| {
            calls += 1; Ok(Outcome { text: "invalid".into(), session: None, structured: None })
        }).is_err());
        assert_eq!(calls, 2);
        calls = 0;
        let error = execute_with(Action::Hints, &request(AgentKind::Codex), &mut |_| {
            calls += 1; anyhow::bail!("transport failed")
        }).unwrap_err();
        assert_eq!(calls, 1); assert_eq!(error.to_string(), "transport failed");
    }
    #[test]
    fn assist_resumes_supported_session_and_propagates_repair_transport_failure() {
        let mut calls = 0;
        let error = execute_with(Action::Hints, &request(AgentKind::Codex), &mut |req| {
            calls += 1;
            if calls == 1 { return Ok(Outcome { text: "invalid".into(), session: Some("existing".into()), structured: None }); }
            assert_eq!(req.resume.as_deref(), Some("existing"));
            assert!(!req.prompt.contains("original problem context"));
            anyhow::bail!("repair transport failed")
        }).unwrap_err();
        assert_eq!(calls, 2); assert_eq!(error.to_string(), "repair transport failed");
    }
    #[test]
    fn assist_digest_samples_against_emitted_baseline_and_tracks_removals() {
        use crate::debugger::{StackFrame, Step, StepKind, Trace, Value, Variable};
        let mut steps = Vec::new();
        for index in 0..6 {
            steps.push(Step { kind: StepKind::Line, line: index + 1, function: "f".into(), stack: vec![StackFrame { function: "f".into(), line: index + 1 }], locals: if index == 4 { vec![] } else { vec![Variable { name: "x".into(), value: Value::Int { v: if index == 0 { "0" } else { "1" }.into() } }] }, returned: None, exception: None, stdout_len: 0 });
        }
        let trace = Trace { language: Language::Python, case_id: 0, steps, truncated: false, output: None, error: None, stdout: String::new() };
        let digest = trace_digest(&trace, 3);
        assert!(digest.contains("#2 L3 line f | x=1"), "{digest}");
        assert!(digest.contains("#4 L5 line f | x=<removed>"), "{digest}");
        assert!(digest.contains("#5 L6 line f | x=1"), "{digest}");
    }
}
