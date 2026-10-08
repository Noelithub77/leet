//! Local test runs: one Python process executes every case through `harness.py`.

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::Stdio;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

const HARNESS: &str = include_str!("harness.py");

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Case {
    pub id: usize,
    /// One argument per line, in LeetCode's input format.
    pub input: String,
    pub expected: Option<String>,
    pub custom: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Pass,
    Fail,
    Error,
    Timeout,
    /// No expected output: the result is shown without a judgement.
    Ran,
}

#[derive(Clone, Debug)]
pub struct CaseResult {
    pub id: usize,
    pub verdict: Verdict,
    pub output: String,
    pub stdout: String,
    pub error: String,
    pub ms: f64,
}

#[derive(Deserialize)]
struct Line {
    #[serde(default)]
    id: usize,
    output: Option<String>,
    error: Option<String>,
    compile_error: Option<String>,
    #[serde(default)]
    stdout: String,
    #[serde(default)]
    ms: f64,
}

/// How outputs are matched; LeetCode statements that accept "any order" compare as multisets.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Compare {
    #[default]
    Exact,
    AnyOrder,
}

impl Compare {
    pub fn for_statement(html: &str) -> Self {
        if html.contains("any order") { Compare::AnyOrder } else { Compare::Exact }
    }
}

/// Runs all cases, calling `on_result` as each finishes. Returns a compile error, if any.
pub fn run(
    python: &str,
    solution: &Path,
    meta: &Value,
    cases: &[Case],
    compare: Compare,
    timeout: Duration,
    mut on_result: impl FnMut(CaseResult),
) -> Result<Option<String>> {
    let spec = json!({
        "meta": meta,
        "cases": cases.iter().map(|c| json!({
            "id": c.id,
            "input": c.input.lines().filter(|l| !l.trim().is_empty()).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
    });
    let mut process = crate::background_process::command(python);
    let mut child = process
        .args(["-X", "utf8", "-c", HARNESS])
        .arg(solution)
        .current_dir(solution.parent().unwrap_or(Path::new(".")))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("start {python}"))?;
    child.stdin.take().context("stdin")?.write_all(spec.to_string().as_bytes())?;

    let stdout = child.stdout.take().context("stdout")?;
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if tx.send(line).is_err() {
                break;
            }
        }
    });

    let deadline = Instant::now() + timeout;
    let mut done = 0;
    while done < cases.len() {
        let left = deadline.saturating_duration_since(Instant::now());
        let line = match rx.recv_timeout(left) {
            Ok(line) => line,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                let _ = child.kill();
                for case in &cases[done..] {
                    on_result(CaseResult {
                        id: case.id,
                        verdict: Verdict::Timeout,
                        output: String::new(),
                        stdout: String::new(),
                        error: format!("exceeded {:.0}s", timeout.as_secs_f64()),
                        ms: timeout.as_millis() as f64,
                    });
                }
                break;
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                let mut stderr = String::new();
                if let Some(mut err) = child.stderr.take() {
                    let _ = std::io::Read::read_to_string(&mut err, &mut stderr);
                }
                let _ = child.wait();
                for case in &cases[done..] {
                    on_result(CaseResult {
                        id: case.id,
                        verdict: Verdict::Error,
                        output: String::new(),
                        stdout: String::new(),
                        error: if stderr.is_empty() { "process exited".into() } else { stderr.clone() },
                        ms: 0.0,
                    });
                }
                break;
            }
        };
        let Ok(parsed) = serde_json::from_str::<Line>(&line) else { continue };
        if let Some(err) = parsed.compile_error {
            let _ = child.wait();
            return Ok(Some(err));
        }
        let case = cases.iter().find(|c| c.id == parsed.id).unwrap_or(&cases[done]);
        let output = parsed.output.unwrap_or_default();
        let verdict = match (&parsed.error, &case.expected) {
            (Some(_), _) => Verdict::Error,
            (None, None) => Verdict::Ran,
            (None, Some(expected)) if outputs_match(&output, expected, compare) => Verdict::Pass,
            (None, Some(_)) => Verdict::Fail,
        };
        on_result(CaseResult {
            id: case.id,
            verdict,
            output,
            stdout: parsed.stdout,
            error: parsed.error.unwrap_or_default(),
            ms: parsed.ms,
        });
        done += 1;
    }
    let _ = child.wait();
    Ok(None)
}

pub fn outputs_match(actual: &str, expected: &str, compare: Compare) -> bool {
    match (serde_json::from_str::<Value>(actual), serde_json::from_str::<Value>(expected)) {
        (Ok(mut a), Ok(mut e)) => {
            if compare == Compare::AnyOrder {
                canonical_order(&mut a);
                canonical_order(&mut e);
            }
            values_match(&a, &e)
        }
        _ => actual.split_whitespace().eq(expected.split_whitespace()),
    }
}

fn canonical_order(v: &mut Value) {
    if let Value::Array(items) = v {
        items.iter_mut().for_each(canonical_order);
        items.sort_by_key(|x| x.to_string());
    }
}

fn values_match(a: &Value, e: &Value) -> bool {
    match (a, e) {
        (Value::Number(x), Value::Number(y)) => match (x.as_f64(), y.as_f64()) {
            (Some(x), Some(y)) if x.fract() != 0.0 || y.fract() != 0.0 => (x - y).abs() <= 1e-5,
            _ => x == y,
        },
        (Value::Array(x), Value::Array(y)) => x.len() == y.len() && x.iter().zip(y).all(|(a, e)| values_match(a, e)),
        _ => a == e,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn python() -> &'static str {
        "python3"
    }

    fn run_cases(code: &str, meta: Value, cases: &[Case], compare: Compare) -> (Option<String>, Vec<CaseResult>) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sol.py");
        std::fs::write(&path, code).unwrap();
        let mut results = vec![];
        let err = run(python(), &path, &meta, cases, compare, Duration::from_secs(10), |r| results.push(r)).unwrap();
        (err, results)
    }

    fn case(id: usize, input: &str, expected: &str) -> Case {
        Case { id, input: input.into(), expected: Some(expected.into()), custom: false }
    }

    #[test]
    fn wrong_question_method_is_reported_before_cases_run() {
        let meta = json!({"name":"insert", "params":[], "return":{"type":"integer[][]"}});
        let (error, results) = run_cases("class Solution:\n    def minWindow(self, s, t):\n        return s\n", meta, &[case(0, "", "[]")], Compare::Exact);
        let error = error.expect("interface mismatch");
        assert!(error.contains("Solution.insert")); assert!(error.contains("minWindow"));
        assert!(results.is_empty());
    }

    #[test]
    fn function_problem_passes_and_fails() {
        let code = "class Solution:\n    def twoSum(self, nums: List[int], target: int) -> List[int]:\n        seen = {}\n        for i, n in enumerate(nums):\n            if target - n in seen:\n                return [seen[target - n], i]\n            seen[n] = i\n";
        let meta = json!({"name": "twoSum", "params": [{"name": "nums", "type": "integer[]"}, {"name": "target", "type": "integer"}], "return": {"type": "integer[]"}});
        let (err, results) = run_cases(code, meta, &[case(0, "[2,7,11,15]\n9", "[0,1]"), case(1, "[3,3]\n6", "[1,0]")], Compare::Exact);
        assert!(err.is_none());
        assert_eq!(results[0].verdict, Verdict::Pass);
        assert_eq!(results[1].verdict, Verdict::Fail);
        assert_eq!(results[1].output, "[0,1]");
    }

    #[test]
    fn linked_list_and_tree_types_round_trip() {
        let code = "class Solution:\n    def reverseList(self, head):\n        prev = None\n        while head:\n            head.next, prev, head = prev, head, head.next\n        return prev\n    def invertTree(self, root):\n        if root:\n            root.left, root.right = self.invertTree(root.right), self.invertTree(root.left)\n        return root\n";
        let list = json!({"name": "reverseList", "params": [{"name": "head", "type": "ListNode"}], "return": {"type": "ListNode"}});
        let (_, r) = run_cases(code, list, &[case(0, "[1,2,3]", "[3,2,1]")], Compare::Exact);
        assert_eq!(r[0].verdict, Verdict::Pass);
        let tree = json!({"name": "invertTree", "params": [{"name": "root", "type": "TreeNode"}], "return": {"type": "TreeNode"}});
        let (_, r) = run_cases(code, tree, &[case(0, "[4,2,7,1,3,6,9]", "[4,7,2,9,6,3,1]")], Compare::Exact);
        assert_eq!(r[0].verdict, Verdict::Pass, "{:?}", r[0]);
    }

    #[test]
    fn design_problem_and_in_place_output() {
        let code = "class Counter:\n    def __init__(self, start):\n        self.n = start\n    def add(self, k):\n        self.n += k\n    def get(self):\n        return self.n\n\nclass Solution:\n    def reverseString(self, s: List[str]) -> None:\n        s.reverse()\n";
        let design = json!({"classname": "Counter", "systemdesign": true});
        let (_, r) = run_cases(code, design, &[case(0, "[\"Counter\",\"add\",\"get\"]\n[[1],[2],[]]", "[null, null, 3]")], Compare::Exact);
        assert_eq!(r[0].verdict, Verdict::Pass, "{:?}", r[0]);
        let void = json!({"name": "reverseString", "params": [{"name": "s", "type": "character[]"}], "return": {"type": "void"}, "output": {"paramindex": 0}});
        let (_, r) = run_cases(code, void, &[case(0, "[\"h\",\"i\"]", "[\"i\",\"h\"]")], Compare::Exact);
        assert_eq!(r[0].verdict, Verdict::Pass, "{:?}", r[0]);
    }

    #[test]
    fn errors_timeouts_and_syntax_errors_are_reported() {
        let meta = json!({"name": "f", "params": [{"name": "x", "type": "integer"}], "return": {"type": "integer"}});
        let (_, r) = run_cases("class Solution:\n    def f(self, x):\n        print('dbg')\n        return 1 // 0\n", meta.clone(), &[case(0, "1", "1")], Compare::Exact);
        assert_eq!(r[0].verdict, Verdict::Error);
        assert!(r[0].error.contains("ZeroDivisionError"));
        assert_eq!(r[0].stdout, "dbg\n");
        let (err, _) = run_cases("class Solution:\n    def f(self x):\n", meta.clone(), &[case(0, "1", "1")], Compare::Exact);
        assert!(err.unwrap().contains("SyntaxError"));

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sol.py");
        std::fs::write(&path, "class Solution:\n    def f(self, x):\n        while True: pass\n").unwrap();
        let mut results = vec![];
        run(python(), &path, &meta, &[case(0, "1", "1")], Compare::Exact, Duration::from_millis(500), |r| results.push(r)).unwrap();
        assert_eq!(results[0].verdict, Verdict::Timeout);
    }

    #[test]
    fn comparison_modes() {
        assert!(outputs_match("[0,1]", "[0, 1]", Compare::Exact));
        assert!(!outputs_match("[1,0]", "[0,1]", Compare::Exact));
        assert!(outputs_match("[[2,1],[3]]", "[[3],[1,2]]", Compare::AnyOrder));
        assert!(outputs_match("2.500001", "2.50000", Compare::Exact));
        assert!(outputs_match("\"fl\"", "\"fl\"", Compare::Exact));
    }
}
