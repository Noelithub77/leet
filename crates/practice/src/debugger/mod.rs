//! Deterministic record-then-seek debugging: run one test case under a line tracer,
//! keep every step's state, and let the UI scrub through it.
//! Python uses `sys.settrace`; C++ uses gdb for function harnesses and stdin programs.

use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::language::Language;
use crate::runner::Case;

mod cpp;
mod cpp_driver;
mod python;
mod structures;

pub use structures::structures;

/// A recorded runtime value. Containers are capped; `Truncated` marks what was dropped.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum Value {
    None,
    Bool { v: bool },
    /// Integers as text so big values survive JSON.
    Int { v: String },
    Float { v: f64 },
    Str { v: String },
    Char { v: String },
    /// `kind` is the source type: list, tuple, deque, vector, array, stack, queue, heap.
    List { kind: String, items: Vec<Value>, #[serde(default)] truncated: usize },
    Set { kind: String, items: Vec<Value>, #[serde(default)] truncated: usize },
    Map { kind: String, entries: Vec<(Value, Value)>, #[serde(default)] truncated: usize },
    /// A `TreeNode` graph, flattened. `root` is a node id; ids are stable within one step.
    Tree { root: Option<String>, nodes: Vec<TreeNodeValue> },
    /// A `ListNode` chain in `next` order; `cycle_to` is the index the tail points back to.
    Linked { nodes: Vec<Value>, cycle_to: Option<usize> },
    /// Any other object: its class name and fields.
    Object { class: String, fields: Vec<(String, Value)> },
    /// A pointer or reference already shown elsewhere in this step.
    Ref { id: String },
    /// A value the recorder cannot show, with its type name.
    Opaque { type_name: String },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TreeNodeValue {
    pub id: String,
    pub value: Box<Value>,
    pub left: Option<String>,
    pub right: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepKind {
    Call,
    Line,
    Return,
    Exception,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StackFrame {
    pub function: String,
    pub line: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Variable {
    pub name: String,
    pub value: Value,
}

/// The state just before `line` runs (or at a call, return, or exception).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Step {
    pub kind: StepKind,
    /// 1-based line in the user's solution file.
    pub line: u32,
    pub function: String,
    /// Outermost first; the last frame is the current one.
    pub stack: Vec<StackFrame>,
    /// Locals of the current frame, in declaration order. `self` is omitted.
    pub locals: Vec<Variable>,
    /// Set on `Return` steps.
    #[serde(default)]
    pub returned: Option<Value>,
    /// Set on `Exception` steps.
    #[serde(default)]
    pub exception: Option<String>,
    /// Program stdout length at this step, to reveal output progressively.
    #[serde(default)]
    pub stdout_len: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Trace {
    pub language: Language,
    pub case_id: usize,
    pub steps: Vec<Step>,
    /// The step limit cut the recording short.
    pub truncated: bool,
    /// The result in the judge's output format, when the run finished.
    pub output: Option<String>,
    pub error: Option<String>,
    pub stdout: String,
}

#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub max_steps: usize,
    /// Items kept per container before `truncated` counts the rest.
    pub max_items: usize,
    pub timeout: Duration,
}

impl Default for Limits {
    fn default() -> Self {
        Self { max_steps: 4000, max_items: 64, timeout: Duration::from_secs(30) }
    }
}

/// Languages with a native recorder; others fall back to the AI dry run.
pub fn supported(language: Language) -> bool {
    matches!(language, Language::Python | Language::Cpp)
}

/// What the recorder needs on this machine, or why it cannot run.
pub fn requirement(language: Language, python: &str) -> Result<(), String> {
    match language {
        Language::Python => python::available(python),
        Language::Cpp => cpp::available(),
        _ => Err(format!("{} has no native debugger yet", language.label())),
    }
}

/// Records one LeetCode case. `meta` is the question's `metaData` JSON.
pub fn record(language: Language, python: &str, solution: &Path, meta: &serde_json::Value, case: &Case, limits: Limits) -> anyhow::Result<Trace> {
    match language {
        Language::Python => python::record(python, solution, meta, case, limits),
        Language::Cpp => cpp::record(solution, meta, case, limits),
        _ => anyhow::bail!("{} has no native debugger yet", language.label()),
    }
}

/// Records a complete stdin/stdout program, including its top-level entry point.
pub fn record_stdin(language: Language, python: &str, solution: &Path, case: &Case, limits: Limits) -> anyhow::Result<Trace> {
    match language {
        Language::Python => python::record_stdin(python, solution, case, limits),
        Language::Cpp => cpp::record_stdin(solution, case, limits),
        _ => anyhow::bail!("{} has no native debugger yet", language.label()),
    }
}

/// Captures a stdin program before recording so later editor changes cannot affect it.
pub fn record_stdin_code(language: Language, python: &str, code: &str, case: &Case, limits: Limits) -> anyhow::Result<Trace> {
    let dir = python::Scratch::new()?;
    let path = dir.0.join(format!("solution.{}", language.extension()));
    std::fs::write(&path, code)?;
    record_stdin(language, python, &path, case, limits)
}

/// Records the captured editor contents so later file edits cannot change a replay.
pub fn record_code(language: Language, python: &str, code: &str, meta: &serde_json::Value, case: &Case, limits: Limits) -> anyhow::Result<Trace> {
    let dir = python::Scratch::new()?;
    let path = dir.0.join(format!("solution.{}", language.extension()));
    std::fs::write(&path, code)?;
    record(language, python, &path, meta, case, limits)
}

#[cfg(test)]
mod snapshot_tests {
    use super::*;
    #[test]
    fn debugger_records_captured_source_without_a_solution_file() {
        let meta = serde_json::json!({"name":"solve","params":[{"type":"integer"}],"return":{"type":"integer"}});
        let case = Case { id: 0, input: "3".into(), expected: Some("4".into()), custom: false };
        let trace = record_code(Language::Python, "python3", "class Solution:\n def solve(self, n):\n  return n + 1\n", &meta, &case, Limits::default()).unwrap();
        assert_eq!(trace.output.as_deref(), Some("4"));
        assert!(trace.steps.iter().any(|step| step.line == 3));
    }
}

#[cfg(test)]
mod stdin_tests;
