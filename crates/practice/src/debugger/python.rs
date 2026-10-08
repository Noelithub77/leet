use std::{io::Read, fs, path::{Path, PathBuf}, process::Command, sync::atomic::{AtomicU64, Ordering}, time::{Duration, Instant}};
use anyhow::{Context, Result};
use super::{Limits, Trace};
const FILE_LIMIT: u64 = 8 << 20;
const EVENTS_LIMIT: u64 = 6 << 20;
const RESULT_LIMIT: u64 = 2 << 20;
fn read_bounded(path: &Path, limit: u64) -> Result<Vec<u8>> {
    let mut data = Vec::new();
    fs::File::open(path)?.take(limit).read_to_end(&mut data)?;
    Ok(data)
}
// Keep complete JSONL events even when a recorder is killed during a write.
fn bound_trace_files(dir: &Path) -> Result<bool> {
    let events = dir.join("steps.jsonl");
    let mut truncated = false;
    if events.exists() {
        let length = fs::metadata(&events)?.len();
        let data = read_bounded(&events, EVENTS_LIMIT)?;
        let end = data.iter().rposition(|b| *b == b'\n').map_or(0, |i| i + 1);
        if end as u64 != length {
            fs::OpenOptions::new().write(true).open(&events)?.set_len(end as u64)?;
            truncated = true;
        }
    }
    let result = dir.join("result.json");
    if result.exists() {
        let data = read_bounded(&result, RESULT_LIMIT)?;
        if fs::metadata(&result)?.len() > RESULT_LIMIT || serde_json::from_slice::<serde_json::Value>(&data).is_err() {
            fs::remove_file(result)?;
            truncated = true;
        }
    }
    Ok(truncated)
}
fn oversized_output(dir: &Path) -> bool {
    let files = ["stdout", "stderr", "program.stdout", "program.stderr", "steps.jsonl", "result.json"];
    files.iter().any(|name| fs::metadata(dir.join(name)).is_ok_and(|m| m.len() > FILE_LIMIT))
        || fs::metadata(dir.join("steps.jsonl")).map_or(0, |m| m.len()).saturating_add(fs::metadata(dir.join("result.json")).map_or(0, |m| m.len())) > FILE_LIMIT
}
static SEQUENCE: AtomicU64 = AtomicU64::new(0);
pub(super) struct Scratch(pub PathBuf);
impl Scratch {
    pub(super) fn new() -> Result<Self> {
        loop {
            let path = std::env::temp_dir().join(format!("leet-trace-{}-{}", std::process::id(), SEQUENCE.fetch_add(1, Ordering::Relaxed)));
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e.into()),
            }
        }
    }
}
impl Drop for Scratch { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); } }
pub(super) fn execute(command: &mut Command, input: &str, timeout: Duration, dir: &Path) -> Result<(bool, bool, String, String)> {
    crate::background_process::hide_console(command);
    let out = dir.join("stdout"); let err = dir.join("stderr"); let stdin = dir.join("stdin");
    fs::write(&stdin, input)?;
    command.stdin(fs::File::open(stdin)?).stdout(fs::File::create(&out)?).stderr(fs::File::create(&err)?);
    #[cfg(unix)]
    { use std::os::unix::process::CommandExt; command.process_group(0); }
    let mut child = command.spawn().context("Start debugger toolchain")?;
    let start = Instant::now(); let mut timed_out = false; let mut oversized;
    let status = loop {
        oversized = oversized_output(dir);
        if let Some(status) = child.try_wait()? { break status; }
        if start.elapsed() >= timeout || oversized {
            timed_out = !oversized;
            #[cfg(unix)]
            {
                // Let gdb terminate its inferior before forcing the process group down.
                let group=format!("-{}",child.id());
                let _ = Command::new("kill").args(["-TERM", "--", &group]).status();
                std::thread::sleep(Duration::from_millis(30));
                let _ = Command::new("kill").args(["-KILL", "--", &group]).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status();
            }
            let _ = child.kill(); break child.wait()?;
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    oversized |= oversized_output(dir);
    let incomplete = bound_trace_files(dir)?;
    if (incomplete || timed_out || oversized) && dir.join("steps.jsonl").exists() {
        let path = dir.join("result.json");
        let mut result = read_bounded(&path, RESULT_LIMIT).ok().and_then(|data| serde_json::from_slice::<serde_json::Value>(&data).ok()).filter(|value| value.is_object()).unwrap_or_else(|| serde_json::json!({}));
        result["truncated"] = true.into();
        result["error"] = serde_json::Value::String(if timed_out { "Recording timed out" } else if oversized { "Debugger output exceeded 8 MiB; reduce max_items or use a smaller test case" } else { "Trace interrupted or exceeded its byte budget; partial events were retained" }.into());
        let mut data = serde_json::to_vec(&result)?;
        if data.len() as u64 > RESULT_LIMIT {
            data = serde_json::to_vec(&serde_json::json!({"truncated":true,"error":result["error"]}))?;
        }
        fs::write(path, data)?;
    }
    for path in [&out, &err, &dir.join("program.stdout"), &dir.join("program.stderr")] {
        if fs::metadata(path).is_ok_and(|m| m.len() > FILE_LIMIT) {
            fs::OpenOptions::new().write(true).open(path)?.set_len(FILE_LIMIT)?;
        }
    }
    let stdout=String::from_utf8_lossy(&read_bounded(&out, FILE_LIMIT)?).into_owned();
    let mut stderr=String::from_utf8_lossy(&read_bounded(&err, FILE_LIMIT)?).into_owned();
    if incomplete {stderr.push_str("\nTrace interrupted or exceeded its byte budget; partial events were retained");}
    if oversized {stderr.push_str("\nDebugger output exceeded 8 MiB");}
    Ok((status.success() && !oversized, timed_out, stdout, stderr))
}
pub fn available(python: &str) -> Result<(), String> {
    let dir = Scratch::new().map_err(|e| e.to_string())?;
    let mut cmd = crate::background_process::command(python); cmd.args(["-X", "utf8", "-c", "print(1)"]);
    match execute(&mut cmd, "", Duration::from_secs(2), &dir.0) {
        Ok((true, false, _, _)) => Ok(()),
        _ => Err(format!("{python} interpreter unavailable")),
    }
}
pub fn record(python: &str, solution: &Path, meta: &serde_json::Value, case: &crate::runner::Case, limits: Limits) -> Result<Trace> {
    record_mode(python, solution, meta, case, limits, false)
}
pub fn record_stdin(python: &str, solution: &Path, case: &crate::runner::Case, limits: Limits) -> Result<Trace> {
    record_mode(python, solution, &serde_json::Value::Null, case, limits, true)
}
fn record_mode(python: &str, solution: &Path, meta: &serde_json::Value, case: &crate::runner::Case, limits: Limits, stdin: bool) -> Result<Trace> {
    let dir = Scratch::new()?; let events = dir.0.join("steps.jsonl"); let result = dir.0.join("result.json");
    let stdout_file = dir.0.join("program.stdout");
    let path = fs::canonicalize(solution)?;
    let stdin_file = dir.0.join("case.input");
    if stdin { fs::write(&stdin_file, &case.input)?; }
    let harness = include_str!("../harness.py").rsplit_once("\nmain()").context("Harness entry point")?.0;
    let script = format!("{harness}\n{}", include_str!("tracer.py"));
    let spec = serde_json::json!({"path":path,"meta":meta,"stdin":stdin,"stdin_file":stdin_file,"prelude":crate::python::PRELUDE,"input":case.input.lines().collect::<Vec<_>>(),"events":events,"result":result,"max_steps":limits.max_steps,"max_items":limits.max_items,"stdout_file":stdout_file});
    let mut cmd = crate::background_process::command(python); cmd.env("PYTHONHASHSEED", "0").args(["-X", "utf8", "-c"]).arg(script);
    let (_, timeout, _, stderr) = execute(&mut cmd, &spec.to_string(), limits.timeout, &dir.0)?;
    let mut trace = Trace { language: crate::language::Language::Python, case_id: case.id, steps: Vec::new(), truncated: false, output: None, error: None, stdout: String::new() };
    if let Ok(data) = read_bounded(&events, EVENTS_LIMIT).map(|data| String::from_utf8_lossy(&data).into_owned()) { for line in data.lines() { trace.steps.push(serde_json::from_str(line)?); } }
    if let Ok(data) = read_bounded(&result, RESULT_LIMIT).map(|data| String::from_utf8_lossy(&data).into_owned()) {
        let value: serde_json::Value = serde_json::from_str(&data)?;
        trace.output = value["output"].as_str().map(str::to_owned); trace.error = value["error"].as_str().map(str::to_owned);
        trace.stdout = value["stdout"].as_str().map(str::to_owned).unwrap_or_else(|| read_bounded(&stdout_file, FILE_LIMIT).map(|data| String::from_utf8_lossy(&data).into_owned()).unwrap_or_default()); trace.truncated = value["truncated"].as_bool().unwrap_or(false);
    } else { trace.stdout = read_bounded(&stdout_file, FILE_LIMIT).map(|data| String::from_utf8_lossy(&data).into_owned()).unwrap_or_default(); trace.truncated = true; trace.error = Some(if timeout { "Recording timed out".into() } else { stderr }); }
    Ok(trace)
}
#[cfg(test)] mod tests {
    use super::*;
    #[test] fn debugger_python_records_and_caps() {
        if available("python3").is_err() { eprintln!("Skipping Python recorder: python3 missing"); return; }
        let dir = Scratch::new().unwrap(); let path = dir.0.join("solution.py");
        fs::write(&path, "class Solution:\n def twoSum(self, nums, target):\n  for i, x in enumerate(nums):\n   for j in range(i+1,len(nums)):\n    if x+nums[j]==target: return [i,j]\n").unwrap();
        let meta = serde_json::json!({"name":"twoSum","params":[{"type":"integer[]"},{"type":"integer"}],"return":{"type":"integer[]"}});
        let case = crate::runner::Case { id: 7, input:"[2,7,11]\n9".into(), expected:None, custom:false };
        let trace = record("python3", &path, &meta, &case, Limits::default()).unwrap();
        assert_eq!(trace.output.as_deref(), Some("[0,1]")); assert!(trace.steps.iter().any(|s| s.returned.is_some()));
        let trace = record("python3", &path, &meta, &case, Limits { max_steps:3, ..Limits::default() }).unwrap();
        assert!(trace.truncated); assert_eq!(trace.steps.len(),3);
    }
}

#[cfg(test)] mod regression_tests {
    use super::*;
    fn run(code:&str, input:&str, limits:Limits)->Trace {
        let dir=Scratch::new().unwrap();let path=dir.0.join("solution.py");fs::write(&path,code).unwrap();
        let case=crate::runner::Case{id:0,input:input.into(),expected:None,custom:false};
        record("python3",&path,&serde_json::json!({"name":"solve","params":[{"type":"integer"}],"return":{"type":"integer"}}),&case,limits).unwrap()
    }
    #[test] fn debugger_python_raw_objects_cycles_depth_and_stdout() {
        if available("python3").is_err(){eprintln!("Skipping Python recorder: python3 missing");return;}
        let trace=run("class Dangerous:\n __slots__=('val','next')\n def __init__(self):\n  self.val=1\n  self.next=self\n def __repr__(self):\n  raise RuntimeError('repr invoked')\n def __eq__(self,other):\n  raise RuntimeError('eq invoked')\nclass Solution:\n def solve(self,n):\n  node=Dangerous()\n  alias=node\n  values=list(range(10))\n  mapping=Counter([1,1,2])\n  nested=[[[[[1]]]]]\n  print('hello')\n  return n\n","3",Limits{max_items:2,..Limits::default()});
        assert_eq!(trace.error,None);assert_eq!(trace.output.as_deref(),Some("3"));assert_eq!(trace.stdout,"hello\n");
        let step=trace.steps.last().unwrap();assert_eq!(step.stdout_len,6);
        assert!(step.locals.iter().any(|v|matches!(&v.value,super::super::Value::Linked{cycle_to:Some(0),..})));
        assert!(step.locals.iter().any(|v|matches!(&v.value,super::super::Value::Ref{id} if id=="linked:node:0")));
        assert!(step.locals.iter().any(|v|matches!(&v.value,super::super::Value::List{truncated:8,..})));
    }
    #[test] fn debugger_python_hides_generated_frames_but_keeps_user_helpers() {
        if available("python3").is_err() { return; }
        let trace = run("class Solution:\n def double(self,x):\n  return x*2\n def solve(self,n):\n  values=[i for i in range(n)]\n  total=sum(self.double(x) for x in values)\n  identity=lambda value:value\n  return identity(total)\n", "3", Limits::default());
        assert_eq!(trace.error, None);
        assert_eq!(trace.output.as_deref(), Some("6"));
        assert!(trace.steps.iter().any(|step| step.function == "double"));
        assert!(trace.steps.iter().any(|step| step.function == "lambda"));
        assert!(trace.steps.iter().all(|step| !step.function.starts_with('<')
            && step.stack.iter().all(|frame| !frame.function.starts_with('<'))
            && step.locals.iter().all(|variable| variable.name != ".0")));
        assert!(trace.steps.iter().any(|step| step.locals.iter().any(|variable| variable.name == "total")));
    }
    #[test] fn debugger_python_timeout_exception_and_void_output() {
        if available("python3").is_err(){eprintln!("Skipping Python recorder: python3 missing");return;}
        let trace=run("class Solution:\n def solve(self,n):\n  print('before',flush=True)\n  time.sleep(1)\n  return n\n","3",Limits{timeout:Duration::from_millis(150),..Limits::default()});
        assert_eq!(trace.error.as_deref(),Some("Recording timed out"));assert_eq!(trace.stdout,"before\n");assert!(!trace.steps.is_empty());
        let trace=run("class Solution:\n def solve(self,n):\n  raise ValueError('failure')\n","3",Limits::default());assert!(trace.error.as_deref().unwrap().contains("failure"));assert!(trace.steps.iter().any(|s|s.kind==super::super::StepKind::Exception));
        let dir=Scratch::new().unwrap();let path=dir.0.join("solution.py");fs::write(&path,"class Solution:\n def solve(self,nums):\n  nums.reverse()\n").unwrap();
        let case=crate::runner::Case{id:0,input:"[1,2,3]".into(),expected:None,custom:false};
        for output in [serde_json::json!({}),serde_json::json!({"paramindex":0})] {
            let meta=serde_json::json!({"name":"solve","params":[{"type":"integer[]"}],"return":{"type":"void"},"output":output});
            assert_eq!(record("python3",&path,&meta,&case,Limits::default()).unwrap().output.as_deref(),Some("[3,2,1]"));
        }
    }
}

#[cfg(test)]
mod budget_tests {
    use super::*;
    #[test]
    fn debugger_trace_files_keep_only_complete_bounded_events() {
        let dir = Scratch::new().unwrap();
        let path = dir.0.join("steps.jsonl");
        fs::write(&path, "{\"complete\":true}\n{\"partial\":").unwrap();
        fs::write(dir.0.join("result.json"), "{partial").unwrap();
        assert!(bound_trace_files(&dir.0).unwrap());
        assert_eq!(fs::read_to_string(path).unwrap(), "{\"complete\":true}\n");
        assert!(!dir.0.join("result.json").exists());
    }
    #[test]
    fn debugger_python_large_values_stop_with_valid_partial_trace() {
        if available("python3").is_err() { return; }
        let dir = Scratch::new().unwrap(); let path = dir.0.join("solution.py");
        fs::write(&path, "class Solution:\n def solve(self,n):\n  values=['x'*8192 for _ in range(64)]\n  for i in range(1000):\n   n+=1\n  return n\n").unwrap();
        let case = crate::runner::Case { id: 0, input: "1".into(), expected: None, custom: false };
        let meta = serde_json::json!({"name":"solve","params":[{"type":"integer"}],"return":{"type":"integer"}});
        let trace = record("python3", &path, &meta, &case, Limits::default()).unwrap();
        assert!(trace.truncated); assert!(trace.error.as_deref().unwrap().contains("6 MiB"));
        assert!(!trace.steps.is_empty()); assert!(trace.steps.len() < 119);
        assert!(serde_json::to_vec(&trace.steps).unwrap().len() < FILE_LIMIT as usize);
    }
    #[test]
    fn debugger_python_large_result_is_bounded() {
        let dir = Scratch::new().unwrap(); let path = dir.0.join("solution.py");
        fs::write(&path, "class Solution:\n def solve(self,n):\n  print('retained')\n  return 'x'*(3*1024*1024)\n").unwrap();
        let case = crate::runner::Case { id: 0, input: "1".into(), expected: None, custom: false };
        let meta = serde_json::json!({"name":"solve","params":[{"type":"integer"}],"return":{"type":"string"}});
        let trace = record("python3", &path, &meta, &case, Limits::default()).unwrap();
        assert!(trace.truncated); assert!(trace.error.as_deref().unwrap().contains("2 MiB"));
        assert!(trace.output.is_none()); assert_eq!(trace.stdout, "retained\n");
    }
    #[test]
    fn debugger_execute_guards_trace_files_even_after_fast_exit() {
        for name in ["steps.jsonl", "result.json"] {
            let dir = Scratch::new().unwrap();
            let mut command = Command::new("python3");
            command.args(["-c", "import pathlib,sys; pathlib.Path(sys.argv[1]).write_bytes(b'x'*(9*1024*1024))", name]).current_dir(&dir.0);
            let (success, _, _, error) = execute(&mut command, "", Duration::from_secs(5), &dir.0).unwrap();
            assert!(!success); assert!(error.contains("exceeded 8 MiB"));
            assert!(fs::metadata(dir.0.join(name)).map_or(true, |m| m.len() <= EVENTS_LIMIT));
        }
    }
}

#[cfg(test)]
mod cpp_budget_tests {
    use super::*;
    #[test]
    fn debugger_cpp_large_values_stop_with_valid_partial_trace() {
        if super::super::cpp::available().is_err() { return; }
        let dir = Scratch::new().unwrap(); let path = dir.0.join("solution.cpp");
        fs::write(&path, "class Solution {\npublic:\n int solve(int n) {\n  vector<string> values(64,string(8192,'x'));\n  for(int i=0;i<1000;++i) {\n   n+=1;\n  }\n  return n;\n }\n};\n").unwrap();
        let case = crate::runner::Case { id: 0, input: "1".into(), expected: None, custom: false };
        let meta = serde_json::json!({"name":"solve","params":[{"type":"integer"}],"return":{"type":"integer"}});
        let trace = super::super::cpp::record(&path, &meta, &case, Limits::default()).unwrap();
        assert!(trace.truncated); assert!(trace.error.as_deref().unwrap().contains("6 MiB"), "{:?}", trace.error);
        assert!(!trace.steps.is_empty()); assert!(trace.steps.len() < 119);
        assert!(serde_json::to_vec(&trace.steps).unwrap().len() < FILE_LIMIT as usize);
    }
}
