//! Portable adapter launch/cleanup, independent of the desktop UI.
use std::{fs, io::Read as _, net::TcpListener, path::{Path, PathBuf}, process::{Child, Command, Stdio}, time::Duration};
use anyhow::{Context, Result, ensure};
use serde_json::{Value as Json, json};
use crate::{language::Language, runner::Case, tool_setup};
use super::{Limits, StepKind, Trace, dap::Session, python::{Scratch, execute}};

struct Process(Child);
impl Process {
    fn start(command: &mut Command) -> Result<Self> {
        crate::background_process::hide_console(command);
        #[cfg(unix)] { use std::os::unix::process::CommandExt as _; command.process_group(0); }
        Ok(Self(command.spawn().context("Start private debug adapter")?))
    }
}
impl Drop for Process {
    fn drop(&mut self) {
        #[cfg(unix)] { let _ = Command::new("kill").args(["-TERM", "--", &format!("-{}", self.0.id())]).stdout(Stdio::null()).stderr(Stdio::null()).status(); }
        #[cfg(windows)] { let _ = crate::background_process::command("taskkill.exe").args(["/PID", &self.0.id().to_string(), "/T", "/F"]).stdout(Stdio::null()).stderr(Stdio::null()).status(); }
        let _ = self.0.kill(); let _ = self.0.wait();
    }
}
fn port() -> Result<u16> { Ok(TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))?.local_addr()?.port()) }
fn read_output(path: &Path) -> Result<String> {
    if !path.is_file() { return Ok(String::new()); }
    ensure!(fs::metadata(path)?.len() <= 1 << 20, "Program output exceeded 1 MiB");
    let mut output = String::new(); fs::File::open(path)?.take(1 << 20).read_to_string(&mut output)?; Ok(output)
}
struct JavaServer(std::sync::Arc<crate::lsp::Server>);
impl Drop for JavaServer { fn drop(&mut self) { self.0.stop(); } }

pub(super) fn record(language: Language, python: &str, solution: &Path, meta: Option<&Json>, case: &Case, limits: Limits) -> Result<Trace> {
    let scratch = Scratch::new()?;
    let root = fs::canonicalize(&scratch.0)?;
    let original = fs::canonicalize(solution)?;
    let mut source = original.clone();
    let input = root.join("case.input"); fs::write(&input, &case.input)?;
    let stdout = root.join("program.stdout"); let stderr = root.join("program.stderr");
    let binary = root.join(format!("program{}", std::env::consts::EXE_SUFFIX));
    let code = fs::read_to_string(solution)?;
    let mut compiler = match language {
        Language::Cpp => {
            let mut command = crate::background_process::command("g++");
            command.args(["-std=c++20", "-O0", "-g", "-fno-omit-frame-pointer"]);
            if let Some(meta) = meta { source = root.join("driver.cpp"); fs::write(&source, super::cpp_driver::source(meta, &original, &code)?)?; }
            command.arg("-o").arg(&binary).arg(&source); Some(command)
        }
        Language::C => {
            if let Some(meta) = meta { source = root.join("driver.c"); fs::write(&source, super::c_driver::source(meta, &code, &case.input, &original)?)?; }
            let mut command = crate::background_process::command("gcc"); command.args(["-std=c17", "-O0", "-g"]).arg("-o").arg(&binary).arg(&source); Some(command)
        }
        Language::Go => {
            if let Some(meta) = meta { source = root.join("main.go"); fs::write(&source, super::drivers::go(meta, &code, &case.input, &original)?)?; }
            let mut command = crate::background_process::command("go"); command.args(["build", "-gcflags=all=-N -l", "-o"]).arg(&binary).arg(&source); Some(command)
        }
        Language::Java => {
            source = root.join("Main.java");
            if let Some(meta) = meta { fs::write(&source, super::drivers::java(meta, &code, &case.input)?)?; } else { fs::write(&source, &code)?; }
            let mut command = crate::background_process::command("javac"); command.args(["-g", "-encoding", "UTF-8", "-d"]).arg(&root).arg(&source); Some(command)
        }
        Language::Python => None,
    };
    if let Some(compiler) = &mut compiler {
        let (success, timeout, _, error) = execute(compiler, "", Duration::from_secs(120), &root)?;
        ensure!(success && !timeout, "{} compilation {}: {}", language.label(), if timeout { "timed out" } else { "failed" }, error.chars().take(8000).collect::<String>());
    }
    // C++ harness #line directives retain the user's original source identity.
    let captured_source = if language == Language::Cpp || (matches!(language, Language::Go | Language::C) && meta.is_some()) { original } else { fs::canonicalize(&source)? };
    let adapter_port;
    let mut processes = Vec::new();
    let mut java_server = None;
    let mut arguments = json!({"name":"Leet trace","request":"launch","cwd":root,"stopOnEntry":true});
    let mut request = "launch";
    match language {
        Language::Cpp | Language::C => {
            adapter_port = port()?;
            let mut command = crate::background_process::command(tool_setup::directory().join(format!("codelldb/adapter/codelldb{}", std::env::consts::EXE_SUFFIX)));
            let library = if cfg!(windows) { "lldb/bin/liblldb.dll" } else if cfg!(target_os = "macos") { "lldb/lib/liblldb.dylib" } else { "lldb/lib/liblldb.so" };
            command.arg("--liblldb").arg(tool_setup::directory().join("codelldb").join(library));
            command.args(["--port", &adapter_port.to_string()]).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
            processes.push(Process::start(&mut command)?);
            arguments["type"] = json!("lldb"); arguments["program"] = json!(binary);
            // CodeLLDB applies `stdio` only when a terminal is allocated. Set
            // LLDB's file paths directly for private, terminal-free recording.
            arguments["preRunCommands"] = json!([
                format!("settings set target.input-path {}", python_string(&input)),
                format!("settings set target.output-path {}", python_string(&stdout)),
                format!("settings set target.error-path {}", python_string(&stderr)),
            ]);
            arguments["terminal"] = json!("console");
        }
        Language::Go => {
            adapter_port = port()?;
            let mut command = crate::background_process::command(tool_setup::directory().join(format!("delve/dlv{}", std::env::consts::EXE_SUFFIX)));
            command.args(["dap", &format!("--listen=127.0.0.1:{adapter_port}")]).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
            processes.push(Process::start(&mut command)?);
            arguments["type"] = json!("go"); arguments["mode"] = json!("exec"); arguments["program"] = json!(binary);
            // Delve's synthetic entry stop precedes the first Go goroutine.
            arguments["stopOnEntry"] = json!(false);
            arguments["stdinFrom"] = json!(input); arguments["stdoutTo"] = json!(stdout); arguments["stderrTo"] = json!(stderr);
        }
        Language::Java => {
            let server = JavaServer(crate::lsp::Server::start(Language::Java, &root)?);
            adapter_port = async_io::block_on(async {
                let result = server.0.request::<lsp_types::request::ExecuteCommand>(lsp_types::ExecuteCommandParams {
                    command: "vscode.java.startDebugSession".into(), arguments: vec![], work_done_progress_params: Default::default(),
                }).await?;
                u16::try_from(result.and_then(|value| value.as_u64()).context("Java debugger did not return a port")?).context("Invalid Java debugger port")
            })?;
            let jvm_port = port()?;
            let mut command = crate::background_process::command("java");
            command.arg(format!("-agentlib:jdwp=transport=dt_socket,server=y,suspend=y,quiet=y,address=127.0.0.1:{jvm_port}"))
                .args(["-Dfile.encoding=UTF-8", "-cp"]).arg(&root).arg("Main")
                .stdin(fs::File::open(&input)?).stdout(fs::File::create(&stdout)?).stderr(fs::File::create(&stderr)?);
            processes.push(Process::start(&mut command)?);
            java_server = Some(server);
            request = "attach";
            arguments = json!({"type":"java","request":"attach","hostName":"127.0.0.1","port":jvm_port,"sourcePaths":[root],"classPaths":[root]});
        }
        Language::Python => {
            adapter_port = port()?;
            let mut command = crate::background_process::command(tool_setup::python(&tool_setup::directory()));
            command.args(["-m", "debugpy.adapter", "--host", "127.0.0.1", "--port", &adapter_port.to_string()])
                .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
            processes.push(Process::start(&mut command)?);
            let wrapper = root.join("entry.py");
            // Keep contest helpers available without flooding captured globals
            // with imported typing/module internals on every source stop.
            fs::write(&wrapper, format!("import sys, runpy, builtins\nhelpers = {{}}\nexec({}, helpers)\nvars(builtins).update({{name: value for name, value in helpers.items() if not name.startswith('__')}})\nsys.stdin = open({}, encoding='utf-8')\nsys.stdout = open({}, 'w', encoding='utf-8', buffering=1)\nsys.stderr = open({}, 'w', encoding='utf-8', buffering=1)\nrunpy.run_path({}, run_name='__main__')\n", serde_json::to_string(crate::python::PRELUDE)?, python_string(&input), python_string(&stdout), python_string(&stderr), python_string(&source)))?;
            arguments["type"] = json!("python"); arguments["program"] = json!(wrapper); arguments["python"] = json!(tool_setup::managed_command(std::ffi::OsStr::new(python)).map_or_else(|| PathBuf::from(python), |(path, _)| path));
            arguments["console"] = json!("internalConsole"); arguments["justMyCode"] = json!(true); arguments["redirectOutput"] = json!(false);
        }
    }
    let mut session = Session::connect(adapter_port, limits.timeout)?;
    session.output_files = vec![stdout.clone(), stderr.clone()];
    session.call("initialize", json!({"clientID":"leet","adapterID":language.id(),"pathFormat":"path","linesStartAt1":true,"columnsStartAt1":true,"supportsVariableType":true,"supportsVariablePaging":true,"supportsRunInTerminalRequest":false}))?;
    let launch = session.request(request, arguments)?;
    session.event("initialized")?;
    let line_count = code.lines().count();
    ensure!(line_count <= 50_000, "Source exceeds the debug recording line limit");
    let breakpoints: Vec<_> = (1..=line_count).map(|line| json!({"line":line})).collect();
    session.call("setBreakpoints", json!({"source":{"path":captured_source},"breakpoints":breakpoints}))?;
    session.call("setExceptionBreakpoints", json!({"filters":if matches!(language, Language::Java | Language::Python) { vec!["uncaught"] } else { vec![] }}))?;
    session.call("configurationDone", json!({}))?;
    session.response(launch)?;
    let mut trace = Trace { language, case_id: case.id, steps: vec![], truncated: false, output: None, error: None, stdout: String::new() };
    let mut trace_bytes = 0;
    let recording = (|| -> Result<()> {
        loop {
            let event = session.event("stopped")?;
            if event["event"] == "terminated" { break; }
            let thread = event["body"]["threadId"].as_i64().context("Debugger stop omitted its thread")?;
            let output_len = read_output(&stdout)?.len();
            if let Some(mut step) = session.capture(thread, &captured_source, trace.steps.last(), limits, output_len)? {
                if step.line as usize > line_count { session.call("continue", json!({"threadId":thread}))?; continue; }
                if event["body"]["reason"] == "exception" {
                    step.kind = StepKind::Exception;
                    step.exception = Some(event["body"]["text"].as_str().unwrap_or("Program exception").into());
                    trace.error = step.exception.clone();
                }
                trace_bytes += serde_json::to_vec(&step)?.len();
                if trace_bytes > 8 << 20 { trace.truncated = true; break; }
                trace.steps.push(step);
                if trace.error.is_some() { break; }
                if trace.steps.len() >= limits.max_steps { trace.truncated = true; break; }
            }
            read_output(&stderr)?;
            session.call("continue", json!({"threadId":thread}))?;
        }
        Ok(())
    })();
    session.disconnect();
    if let Err(error) = recording {
        trace.truncated |= error.to_string().contains("Recording timed out");
        trace.error = Some(format!("{error:#}"));
    }
    trace.stdout = read_output(&stdout)?;
    let error = read_output(&stderr)?;
    if !trace.truncated && session.exit_code.is_some_and(|code| code != 0) && trace.error.is_none() { trace.error = Some(if error.is_empty() { format!("Program exited with status {}", session.exit_code.unwrap_or_default()) } else { error }); }
    if !trace.truncated && trace.error.is_none() {
        if meta.is_some() {
            if let Some((before, result)) = trace.stdout.split_once("\n__LEET_TRACE_RESULT_91b6__\n") { trace.output = Some(result.trim_end().into()); trace.stdout = before.into(); }
            else { trace.error = Some("Function harness did not produce a result".into()); }
        } else { trace.output = Some(trace.stdout.clone()); }
    }
    // Dropping the adapter and JVM guards also reaps debuggees on timeout/error.
    drop(session); drop(processes); drop(java_server);
    Ok(trace)
}
fn python_string(path: &Path) -> String { serde_json::to_string(&path.to_string_lossy()).expect("Path string serialization") }
