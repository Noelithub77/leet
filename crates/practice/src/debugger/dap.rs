//! Bounded DAP transport and record-then-seek state capture shared by native adapters.
use std::{collections::{HashMap, VecDeque}, io::{BufRead, BufReader, Write}, net::{Shutdown, TcpStream}, sync::mpsc, time::{Duration, Instant}};
use anyhow::{Context, Result, bail, ensure};
use serde_json::{Value as Json, json};
use super::{Limits, StackFrame, Step, StepKind, Value, Variable};

const MESSAGE_LIMIT: usize = 8 << 20;
const OUTPUT_LIMIT: usize = 1 << 20;

fn read_message(reader: &mut impl BufRead) -> Result<Json> {
    let mut length = None;
    let mut bytes = 0;
    loop {
        let mut line = String::new();
        ensure!(reader.read_line(&mut line)? != 0, "Debug adapter closed its connection");
        bytes += line.len(); ensure!(bytes <= 8192, "Debug adapter header exceeded its limit");
        if line.trim().is_empty() { break; }
        if let Some(value) = line.strip_prefix("Content-Length:") { length = Some(value.trim().parse::<usize>()?); }
    }
    let length = length.context("Debug adapter omitted Content-Length")?;
    ensure!(length <= MESSAGE_LIMIT, "Debug adapter message exceeded 8 MiB");
    let mut bytes = vec![0; length]; reader.read_exact(&mut bytes)?;
    Ok(serde_json::from_slice(&bytes)?)
}

pub(super) struct Session {
    stream: TcpStream,
    incoming: mpsc::Receiver<Result<Json, String>>,
    reader: Option<std::thread::JoinHandle<()>>,
    sequence: u64,
    deadline: Instant,
    responses: HashMap<u64, Json>,
    events: VecDeque<Json>,
    pub output: String,
    pub error_output: String,
    pub exit_code: Option<i64>,
    pub output_files: Vec<std::path::PathBuf>,
}
impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.stream.shutdown(Shutdown::Both);
        let (_, replacement) = mpsc::channel(); self.incoming = replacement;
        if let Some(reader) = self.reader.take() { let _ = reader.join(); }
    }
}
impl Session {
    pub fn connect(port: u16, timeout: Duration) -> Result<Self> {
        let started = Instant::now();
        let stream = loop {
            match TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, port)) {
                Ok(stream) => break stream,
                Err(error) if started.elapsed() >= Duration::from_secs(10).min(timeout) => return Err(error).context("Connect to private debug adapter"),
                Err(_) => std::thread::sleep(Duration::from_millis(25)),
            }
        };
        stream.set_read_timeout(Some(timeout + Duration::from_secs(2)))?;
        stream.set_write_timeout(Some(Duration::from_secs(2)))?;
        let mut input = BufReader::new(stream.try_clone()?);
        let (tx, incoming) = mpsc::sync_channel(64);
        let reader = std::thread::spawn(move || {
            loop {
                let result = read_message(&mut input).map_err(|error| error.to_string());
                let failed = result.is_err();
                if tx.send(result).is_err() || failed { break; }
            }
        });
        Ok(Self { stream, incoming, reader: Some(reader), sequence: 0, deadline: Instant::now() + timeout,
            responses: HashMap::new(), events: VecDeque::new(), output: String::new(), error_output: String::new(), exit_code: None, output_files: vec![] })
    }
    fn send(&mut self, message: Json) -> Result<()> {
        let body = serde_json::to_vec(&message)?;
        write!(self.stream, "Content-Length: {}\r\n\r\n", body.len())?;
        self.stream.write_all(&body)?; self.stream.flush()?; Ok(())
    }
    pub fn request(&mut self, command: &str, arguments: Json) -> Result<u64> {
        self.sequence += 1; let sequence = self.sequence;
        self.send(json!({"seq":sequence,"type":"request","command":command,"arguments":arguments}))?;
        Ok(sequence)
    }
    fn receive(&mut self) -> Result<()> {
        let message = loop {
            ensure!(self.output_files.iter().all(|path| std::fs::metadata(path).map_or(true, |metadata| metadata.len() <= OUTPUT_LIMIT as u64)), "Program output exceeded 1 MiB");
            let remaining = self.deadline.saturating_duration_since(Instant::now());
            ensure!(!remaining.is_zero(), "Recording timed out");
            match self.incoming.recv_timeout(remaining.min(Duration::from_millis(100))) {
                Ok(message) => break message.map_err(anyhow::Error::msg)?,
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(error) => return Err(error).context("Debug adapter stopped"),
            }
        };
        match message["type"].as_str() {
            Some("response") => { ensure!(self.responses.len() < 128, "Too many pending debugger responses"); self.responses.insert(message["request_seq"].as_u64().context("Missing debugger request id")?, message); }
            Some("event") => {
                match message["event"].as_str() {
                    Some("output") => {
                        let text = message["body"]["output"].as_str().unwrap_or_default();
                        let target = match message["body"]["category"].as_str() { Some("stdout") => Some(&mut self.output), Some("stderr") => Some(&mut self.error_output), _ => None };
                        if let Some(target) = target { ensure!(target.len() + text.len() <= OUTPUT_LIMIT, "Debugger output exceeded 1 MiB"); target.push_str(text); }
                    }
                    Some("exited") => self.exit_code = message["body"]["exitCode"].as_i64(),
                    Some("stopped" | "terminated" | "initialized") => { ensure!(self.events.len() < 128, "Too many pending debugger events"); self.events.push_back(message); }
                    _ => {}
                }
            }
            Some("request") => self.send(json!({"seq":0,"type":"response","request_seq":message["seq"],"command":message["command"],"success":false,"message":"Leet runs debug programs privately without a terminal"}))?,
            _ => bail!("Invalid debug adapter message"),
        }
        Ok(())
    }
    pub fn response(&mut self, sequence: u64) -> Result<Json> {
        loop {
            if let Some(response) = self.responses.remove(&sequence) {
                ensure!(response["success"].as_bool() == Some(true), "{}: {}", response["command"].as_str().unwrap_or("Debugger request"), response["message"].as_str().unwrap_or("request failed"));
                return Ok(response["body"].clone());
            }
            self.receive()?;
        }
    }
    pub fn call(&mut self, command: &str, arguments: Json) -> Result<Json> { let sequence = self.request(command, arguments)?; self.response(sequence) }
    pub fn disconnect(&mut self) {
        // Allow the adapter to reap detached debuggees even after the recording
        // deadline expires, before its own process is terminated.
        self.deadline = Instant::now() + Duration::from_secs(2);
        self.output_files.clear();
        let _ = self.call("disconnect", json!({"terminateDebuggee":true}));
    }
    pub fn event(&mut self, name: &str) -> Result<Json> {
        loop {
            if let Some(ix) = self.events.iter().position(|event| event["event"] == name || event["event"] == "terminated") {
                return Ok(self.events.remove(ix).context("Missing debugger event")?);
            }
            self.receive()?;
        }
    }
    fn variables(&mut self, reference: i64, max: usize) -> Result<Vec<Json>> {
        if reference == 0 { return Ok(vec![]); }
        let response = self.call("variables", json!({"variablesReference":reference,"start":0,"count":max}))?;
        Ok(response["variables"].as_array().map(|items| items.iter().filter(|variable| {
            !matches!(variable["name"].as_str(), Some("special variables" | "function variables" | "class variables" | "protected variables" | "len()"))
                && !matches!(variable["type"].as_str(), Some("module" | "type" | "function" | "builtin_function_or_method"))
        }).take(max).cloned().collect()).unwrap_or_default())
    }
    fn value(&mut self, variable: &Json, max_items: usize, depth: usize, budget: &mut usize) -> Result<Value> {
        let text = variable["value"].as_str().unwrap_or_default();
        let kind = variable["type"].as_str().unwrap_or_default();
        if *budget == 0 { return Ok(Value::Opaque { type_name: kind.into() }); }
        *budget -= 1;
        if text == "None" || text == "null" || text == "nil" { return Ok(Value::None); }
        if matches!(text, "true" | "false" | "True" | "False") { return Ok(Value::Bool { v: text.eq_ignore_ascii_case("true") }); }
        if text.parse::<i128>().is_ok() { return Ok(Value::Int { v: text.into() }); }
        if text.starts_with('"') { if let Ok(value) = serde_json::from_str::<String>(text) { return Ok(Value::Str { v: value }); } }
        if let Ok(value) = text.parse::<f64>() { if value.is_finite() { return Ok(Value::Float { v: value }); } }
        let reference = variable["variablesReference"].as_i64().unwrap_or_default();
        if reference != 0 && depth < 3 {
            let count = variable["indexedVariables"].as_u64().map(|count| count as usize);
            if count == Some(0) && variable["namedVariables"].as_u64().unwrap_or_default() == 0 { return Ok(Value::List { kind: kind.into(), truncated: 0, items: vec![] }); }
            let count = count.filter(|count| *count > 0);
            let children = self.variables(reference, count.map_or(max_items, |count| count.min(max_items)))?;
            let indexed = children.first().is_some_and(|child| child["name"].as_str().is_some_and(|name| name.parse::<usize>().is_ok() || name.starts_with('[')));
            if indexed {
                let mut items = Vec::new();
                for child in &children { if *budget == 0 { break; } items.push(self.value(child, max_items.min(16), depth + 1, budget)?); }
                let count = variable["indexedVariables"].as_u64().unwrap_or(items.len() as u64) as usize;
                return Ok(Value::List { kind: kind.into(), truncated: count.saturating_sub(items.len()), items });
            }
            let mut fields = Vec::new();
            for child in &children {
                if *budget == 0 { break; }
                if let Some(name) = child["name"].as_str() { fields.push((name.into(), self.value(child, max_items.min(16), depth + 1, budget)?)); }
            }
            return Ok(Value::Object { class: kind.into(), fields });
        }
        Ok(Value::Opaque { type_name: if kind.is_empty() { text.into() } else { format!("{kind} · {text}") } })
    }
    pub fn capture(&mut self, thread: i64, source: &std::path::Path, previous: Option<&Step>, limits: Limits, stdout_len: usize) -> Result<Option<Step>> {
        let response = self.call("stackTrace", json!({"threadId":thread,"startFrame":0,"levels":64}))?;
        // A Java thread may exit between its last stop and this request.
        let Some(frames) = response["stackFrames"].as_array() else { return Ok(None); };
        let same_source = |frame: &&Json| frame["source"]["path"].as_str().is_some_and(|path| std::fs::canonicalize(path).is_ok_and(|path| path == source));
        let Some(frame) = frames.iter().find(same_source) else { return Ok(None); };
        // Ignore a library breakpoint while a caller remains in the user's source.
        if !frames.first().is_some_and(|first| first == frame) { return Ok(None); }
        let frame_name = |frame: &Json| match frame["name"].as_str().unwrap_or("main") { "<module>" => "module".to_owned(), name => name.to_owned() };
        let function = frame_name(frame);
        let stack: Vec<_> = frames.iter().filter(same_source).rev().map(|frame| StackFrame {
            function: frame_name(frame), line: frame["line"].as_u64().unwrap_or_default() as u32,
        }).collect();
        let scopes = self.call("scopes", json!({"frameId":frame["id"]}))?;
        let mut locals = Vec::new();
        let mut budget = 512;
        for scope in scopes["scopes"].as_array().into_iter().flatten().take(4) {
            if scope["presentationHint"] == "registers" || scope["name"].as_str().is_some_and(|name| name.contains("Register")) { continue; }
            for variable in self.variables(scope["variablesReference"].as_i64().unwrap_or_default(), limits.max_items)? {
                if budget == 0 { break; }
                let Some(name) = variable["name"].as_str() else { continue; };
                let name = name.strip_prefix("::").unwrap_or(name);
                if name.starts_with("__") || name.starts_with("std::") || matches!(name, "self" | "this") || locals.iter().any(|local: &Variable| local.name == name) { continue; }
                locals.push(Variable { name: name.into(), value: self.value(&variable, limits.max_items, 0, &mut budget)? });
            }
        }
        let kind = match previous {
            Some(previous) if stack.len() > previous.stack.len() => StepKind::Call,
            Some(previous) if stack.len() < previous.stack.len() => StepKind::Return,
            None => StepKind::Call,
            _ => StepKind::Line,
        };
        Ok(Some(Step { kind, line: frame["line"].as_u64().unwrap_or_default() as u32, function, stack, locals, returned: None, exception: None, stdout_len }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dap_framing_rejects_oversized_or_incomplete_messages() {
        let message = b"Content-Length: 34\r\n\r\n{\"type\":\"event\",\"event\":\"stopped\"}";
        assert_eq!(read_message(&mut &message[..]).unwrap()["event"], "stopped");
        assert!(read_message(&mut &b"Content-Length: 999999999\r\n\r\n"[..]).is_err());
        assert!(read_message(&mut &b"Content-Length: 20\r\n\r\n{}"[..]).is_err());
        assert!(read_message(&mut &b"\r\n{}"[..]).is_err());
    }

    #[test]
    fn transport_keeps_events_while_waiting_for_a_response_and_reports_failures() {
        let listener = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let first = read_message(&mut reader).unwrap();
            for message in [
                json!({"type":"event","event":"stopped","body":{"threadId":7}}),
                json!({"type":"event","event":"output","body":{"category":"stdout","output":"before\n"}}),
                json!({"type":"response","request_seq":first["seq"],"success":true,"body":{"supported":true}}),
            ] {
                let body = message.to_string(); write!(stream, "Content-Length: {}\r\n\r\n{body}", body.len()).unwrap(); stream.flush().unwrap();
            }
            let second = read_message(&mut reader).unwrap();
            let body = json!({"type":"response","request_seq":second["seq"],"command":"continue","success":false,"message":"fixture failure"}).to_string();
            write!(stream, "Content-Length: {}\r\n\r\n{body}", body.len()).unwrap();
        });
        let mut session = Session::connect(port, Duration::from_secs(2)).unwrap();
        assert_eq!(session.call("initialize", json!({})).unwrap()["supported"], true);
        assert_eq!(session.output, "before\n");
        assert_eq!(session.event("stopped").unwrap()["body"]["threadId"], 7);
        assert!(session.call("continue", json!({"threadId":7})).unwrap_err().to_string().contains("fixture failure"));
        drop(session); server.join().unwrap();
    }
}
