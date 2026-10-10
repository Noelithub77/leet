//! Bounded stdio transport shared by the local CLI protocols.
use std::{io::{BufRead, BufReader, Write}, process::{Child, Command, Stdio}, sync::{Arc, Mutex, mpsc}, thread, time::{Duration, Instant}};
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use super::{Access, Cancel, Event, Outcome, Request};

pub(super) struct Process {
    child: Child, input: Option<mpsc::Sender<String>>, lines: mpsc::Receiver<String>, stderr: Arc<Mutex<String>>,
    readers: Vec<thread::JoinHandle<()>>, started: Instant, timeout: Duration, next_id: u64,
}
impl Process {
    pub fn spawn(command: &mut Command, timeout: Duration) -> Result<Self> {
        crate::background_process::hide_console(command);
        command.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
        #[cfg(unix)] { use std::os::unix::process::CommandExt; command.process_group(0); }
        let mut child = command.spawn().context("Start coding agent")?;
        let mut stdin = child.stdin.take().context("Agent stdin unavailable")?;
        let (input, writes) = mpsc::channel::<String>();
        let stdout = child.stdout.take().context("Agent stdout unavailable")?;
        let error = child.stderr.take().context("Agent stderr unavailable")?;
        let (tx, lines) = mpsc::channel();
        let stderr = Arc::new(Mutex::new(String::new())); let buffer = stderr.clone();
        let writer_error=stderr.clone();
        let readers = vec![thread::spawn(move || { for line in writes { if let Err(error)=writeln!(stdin,"{line}").and_then(|_|stdin.flush()) { if let Ok(mut buffer)=writer_error.lock() {buffer.push_str(&format!("Agent stdin: {error}"));} break; } } }),thread::spawn(move || { for line in BufReader::new(stdout).lines() { let Ok(line) = line else { break }; if tx.send(line).is_err() { break; } } }), thread::spawn(move || { for line in BufReader::new(error).lines() { let Ok(line) = line else { break }; if let Ok(mut text) = buffer.lock() { text.push_str(&line); text.push('\n'); if text.len() > 16_384 { let start = text.char_indices().find(|(i,_)| *i >= text.len()-8192).map(|(i,_)|i).unwrap_or(0); *text = text[start..].to_owned(); } } } })];
        Ok(Self { child, input:Some(input), lines, stderr, readers, started: Instant::now(), timeout, next_id: 0 })
    }
    pub fn send(&mut self, value: &Value) -> Result<()> { self.input.as_ref().context("Agent stdin closed")?.send(value.to_string()).context("Agent stopped reading stdin") }
    pub fn close_input(&mut self) { self.input.take(); }
    pub fn line(&mut self, cancel: &Cancel) -> Result<Option<String>> {
        loop {
            if cancel.is_cancelled() { bail!("Agent run was cancelled"); }
            if self.started.elapsed() > self.timeout { bail!("Agent timed out after {} seconds", self.timeout.as_secs()); }
            match self.lines.recv_timeout(Duration::from_millis(50)) {
                Ok(line) => return Ok(Some(line)),
                Err(mpsc::RecvTimeoutError::Timeout) => {},
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    let Some(status) = self.child.try_wait()? else {
                        thread::sleep(Duration::from_millis(50));
                        continue;
                    };
                    if !status.success() { bail!("Agent exited with {status}: {}", self.error()); }
                    return Ok(None);
                }
            }
        }
    }
    pub fn error(&self) -> String { self.stderr.lock().map(|s| s.trim().to_owned()).unwrap_or_default() }
    pub fn rpc(&mut self, method: &str, params: Value, cancel: &Cancel, handler: &mut dyn FnMut(&mut Self, &Value) -> Result<()>) -> Result<Value> {
        self.next_id += 1; let id = self.next_id;
        self.send(&json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}))?;
        while let Some(line) = self.line(cancel)? {
            let Ok(value) = serde_json::from_str::<Value>(&line) else { continue };
            if value["id"] == id { if let Some(error) = value.get("error") { bail!("{method}: {error}"); } return Ok(value["result"].clone()); }
            handler(self, &value)?;
        }
        bail!("Agent closed before answering {method}: {}", self.error())
    }
}
impl Drop for Process {
    fn drop(&mut self) {
        self.input.take();
        #[cfg(unix)] { let _ = Command::new("kill").args(["-KILL", "--", &format!("-{}", self.child.id())]).stdout(Stdio::null()).stderr(Stdio::null()).status(); }
        #[cfg(windows)] { let _ = crate::background_process::command("taskkill").args(["/PID", &self.child.id().to_string(), "/T", "/F"]).stdout(Stdio::null()).stderr(Stdio::null()).status(); }
        let _ = self.child.kill(); let _ = self.child.wait();
        for reader in self.readers.drain(..) { let _ = reader.join(); }
    }
}
pub(super) fn extract(text: &str) -> Option<Value> {
    if let Ok(value) = serde_json::from_str(text.trim()) { return Some(value); }
    if let Some(rest) = text.split("```json").nth(1).or_else(||text.split("```\n").nth(1)) && let Ok(value) = serde_json::from_str(rest.split("```").next()?.trim()) { return Some(value); }
    let start=text.find('{')?;
    // Some CLIs append finish-tool JSON to their final answer.
    serde_json::Deserializer::from_str(&text[start..]).into_iter::<Value>().next()?.ok()
}
pub(super) fn outcome(request: &Request, text: String, session: Option<String>, structured: Option<Value>) -> Result<Outcome> {
    let structured = structured.or_else(||request.schema.as_ref().and_then(|_|extract(&text)));
    Ok(Outcome { text, session, structured })
}
pub(super) fn prompt(request: &Request) -> String {
    let mut prompt = request.prompt.clone();
    if request.access == Access::ReadOnly { prompt.push_str("\nAnswer from the prompt alone. Do not invoke tools, execute commands, or edit files."); }
    if let Some(schema) = &request.schema { prompt.push_str(&format!("\nReturn only JSON matching this JSON Schema, including every required field: {schema}")); }
    prompt
}
pub(super) fn usage(value: &Value, events: &mut dyn FnMut(Event)) { if let Some(usage) = value.get("usage") { events(Event::Usage { input_tokens: usage["input_tokens"].as_u64().unwrap_or(0), output_tokens: usage["output_tokens"].as_u64().unwrap_or(0) }); } }
#[cfg(test)] mod tests { use super::*; #[test] fn agents_extract_json() { for text in ["{\"answer\":\"pong\"}","Answer:\n```json\n{\"answer\":\"pong\"}\n```", "Here: {\"answer\":\"pong\"} done"] { assert_eq!(extract(text),Some(json!({"answer":"pong"}))); } assert_eq!(extract("no json"),None); } }

#[cfg(all(test,unix))] mod process_tests {
    use super::*;
    #[test] fn agents_cancel_reaps_process_group() {
        let pidfile=tempfile::NamedTempFile::new().unwrap();
        let mut command=Command::new("sh");command.args(["-c","sleep 60 & echo $! > \"$1\"; wait","sh"]).arg(pidfile.path());
        let mut process=Process::spawn(&mut command,Duration::from_secs(5)).unwrap();
        let ready_deadline=Instant::now()+Duration::from_secs(2);
        let pid=loop {
            if let Ok(pid)=std::fs::read_to_string(pidfile.path()).unwrap().trim().parse::<u32>() {break pid;}
            assert!(Instant::now()<ready_deadline,"Agent fixture did not start its child");
            thread::sleep(Duration::from_millis(10));
        };
        let cancel=Cancel::default();let signal=cancel.clone();
        let trigger=thread::spawn(move || {thread::sleep(Duration::from_millis(100));signal.cancel();});
        let started=Instant::now();assert!(process.line(&cancel).unwrap_err().to_string().contains("cancelled"));
        drop(process);trigger.join().unwrap();assert!(started.elapsed()<Duration::from_secs(2));
        // Linux can retain a killed grandchild as a zombie until init reaps it.
        #[cfg(target_os="linux")] {
            let exit_deadline=Instant::now()+Duration::from_secs(2);
            loop {
                match std::fs::read_to_string(format!("/proc/{pid}/status")) {
                    Ok(status)=>{
                        let state=status.lines().find(|line|line.starts_with("State:")).unwrap_or("State unavailable");
                        if state.split_whitespace().nth(1).is_some_and(|state|matches!(state,"Z"|"X"|"x")) {break;}
                        assert!(Instant::now()<exit_deadline,"Cancelled agent child {pid} is still alive: {state}");
                    }
                    Err(error) if error.kind()==std::io::ErrorKind::NotFound=>break,
                    Err(error)=>panic!("Read cancelled agent child {pid}: {error}"),
                }
                thread::sleep(Duration::from_millis(10));
            }
        }
    }
    #[test] fn agents_cancel_with_blocked_stdin() {
        let mut process=Process::spawn(Command::new("sleep").arg("60"),Duration::from_secs(5)).unwrap();
        process.send(&json!({"prompt":"x".repeat(1_000_000)})).unwrap();
        let cancel=Cancel::default();cancel.cancel();assert!(process.line(&cancel).unwrap_err().to_string().contains("cancelled"));
        let started=Instant::now();drop(process);assert!(started.elapsed()<Duration::from_secs(2));
    }
}

#[cfg(all(test, unix))]
mod closed_stdout_tests {
    use super::*;
    #[test]
    fn agents_closed_stdout_obeys_timeout_and_cancel() {
        for cancelled in [false, true] {
            let mut process = Process::spawn(Command::new("sh").args(["-c", "exec 1>&-; sleep 60"]), Duration::from_millis(200)).unwrap();
            let cancel = Cancel::default();
            let signal = cancel.clone();
            let trigger = thread::spawn(move || { thread::sleep(Duration::from_millis(100)); if cancelled { signal.cancel(); } });
            let start = Instant::now();
            let error = process.line(&cancel).unwrap_err().to_string();
            assert!(error.contains(if cancelled { "cancelled" } else { "timed out" }), "{error}");
            drop(process);
            trigger.join().unwrap();
            assert!(start.elapsed() < Duration::from_secs(2));
        }
    }
}

#[cfg(test)]
mod answer_tests {
    use super::*;
    use crate::agents::{AgentKind, Detected, Selection};
    #[test]
    fn agents_non_json_answers_reach_schema_repair() {
        let request = Request { agent: Detected { kind: AgentKind::Claude, path: "mock".into(), version: None }, selection: Selection { agent: AgentKind::Claude, model: "mock".into(), effort: None, fast: false }, prompt: "context".into(), schema: Some(json!({"type":"object"})), cwd: std::env::temp_dir(), access: Access::ReadOnly, resume: None };
        for text in ["plain answer", "{broken"] {
            let answer = outcome(&request, text.into(), Some("session".into()), None).unwrap();
            assert_eq!(answer.text, text); assert!(answer.structured.is_none());
        }
    }
}
