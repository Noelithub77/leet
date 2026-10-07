//! Compile once, then run stdin/stdout samples with deadlines and bounded output.
use std::{fs, io::{Read, Write}, path::{Path, PathBuf}, process::{Command, Stdio}, sync::atomic::{AtomicU64, Ordering}, time::{Duration, Instant}};
use anyhow::{Context, Result};
use crate::{language::Language, runner::{Case, CaseResult, Verdict}};
static SEQUENCE: AtomicU64 = AtomicU64::new(0);
struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Result<Self> {
        loop {
            let path = std::env::temp_dir().join(format!("leet-run-{}-{}", std::process::id(), SEQUENCE.fetch_add(1, Ordering::Relaxed)));
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error.into()),
            }
        }
    }
}
impl Drop for Scratch { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); } }
struct Output { success: bool, timeout: bool, stdout: String, stderr: String, ms: f64 }
fn execute(mut command: Command, input: String, timeout: Duration, scratch: &Path) -> Result<Output> {
    let out = scratch.join("stdout"); let err = scratch.join("stderr");
    command.stdin(Stdio::piped()).stdout(fs::File::create(&out)?).stderr(fs::File::create(&err)?);
    let started = Instant::now();
    let mut child = command.spawn().context("Start language toolchain")?;
    let mut stdin = child.stdin.take().context("Child stdin")?;
    let writer = std::thread::spawn(move || { let _ = stdin.write_all(input.as_bytes()); });
    let mut timed_out = false;
    let mut oversized = false;
    let status = loop {
        if let Some(status) = child.try_wait()? { break status; }
        oversized = [&out, &err].into_iter().any(|path| fs::metadata(path).is_ok_and(|meta| meta.len() > 1 << 20));
        if started.elapsed() >= timeout || oversized {
            timed_out = !oversized;
            let _ = child.kill(); break child.wait()?;
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    let _ = writer.join();
    let read = |path: &Path| -> Result<String> {
        let mut bytes = Vec::new(); fs::File::open(path)?.take(1 << 20).read_to_end(&mut bytes)?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    };
    let mut stderr = read(&err)?;
    if oversized { stderr.push_str("\nOutput exceeded 1 MiB"); }
    Ok(Output { success: status.success() && !oversized, timeout: timed_out, stdout: read(&out)?, stderr, ms: started.elapsed().as_secs_f64() * 1000. })
}
pub fn run(language: Language, python: &str, path: &Path, cases: &[Case], timeout: Duration, mut result: impl FnMut(CaseResult)) -> Result<Option<String>> {
    let scratch = Scratch::new()?;
    let binary = scratch.0.join("solution");
    if language != Language::Python {
        let mut compiler = match language {
            Language::Cpp => { let mut cmd = Command::new("g++"); cmd.args(["-std=c++20", "-O2"]); cmd },
            Language::C => { let mut cmd = Command::new("gcc"); cmd.args(["-std=c17", "-O2"]); cmd },
            Language::Go => { let mut cmd = Command::new("go"); cmd.arg("build"); cmd },
            Language::Python => unreachable!(),
        };
        compiler.arg("-o").arg(&binary).arg(path);
        let output = execute(compiler, String::new(), Duration::from_secs(60), &scratch.0)?;
        if !output.success { return Ok(Some(if output.timeout { "Compilation timed out".into() } else { output.stderr })); }
    }
    for case in cases {
        let mut command = if language == Language::Python { let mut cmd = Command::new(python); cmd.arg("-u").arg(path); cmd } else { Command::new(&binary) };
        command.current_dir(path.parent().unwrap_or(Path::new(".")));
        let output = execute(command, format!("{}\n", case.input.trim_end_matches('\n')), timeout, &scratch.0)?;
        let verdict = if output.timeout { Verdict::Timeout } else if !output.success { Verdict::Error }
            else if let Some(expected) = &case.expected { if output.stdout.split_whitespace().eq(expected.split_whitespace()) { Verdict::Pass } else { Verdict::Fail } }
            else { Verdict::Ran };
        result(CaseResult { id: case.id, verdict, output: output.stdout.clone(), stdout: output.stdout, error: output.stderr, ms: output.ms });
    }
    Ok(None)
}
#[cfg(test)] mod tests {
    use super::*;
    #[test] fn compiled_languages_run_stdin_samples() {
        let dir = tempfile::tempdir().unwrap();
        for (language, code) in [
            (Language::Cpp, "#include <iostream>\nint main(){int n; std::cin>>n; std::cout<<n*2; }"),
            (Language::C, "#include <stdio.h>\nint main(){int n; scanf(\"%d\", &n); printf(\"%d\",n*2);}"),
            (Language::Go, "package main\nimport \"fmt\"\nfunc main(){var n int;fmt.Scan(&n);fmt.Print(n*2)}")
        ] {
            let path = dir.path().join(format!("main.{}", language.extension())); fs::write(&path, code).unwrap();
            let case = Case { id: 0, input: "3".into(), expected: Some("6".into()), custom: false };
            let mut results = Vec::new();
            run(language, "python3", &path, &[case], Duration::from_secs(2), |r| results.push(r)).unwrap();
            assert_eq!(results[0].verdict, Verdict::Pass, "{}: {}", language.label(), results[0].error);
        }
    }
    #[test] fn python_samples_compare_whitespace_and_enforce_timeout() {
        let dir = tempfile::tempdir().unwrap(); let path = dir.path().join("main.py");
        fs::write(&path, "print(int(input()) * 2)").unwrap();
        let case = Case { id: 0, input: "3".into(), expected: Some("6\n".into()), custom: false };
        let mut results = Vec::new();
        run(Language::Python, "python3", &path, &[case.clone()], Duration::from_secs(2), |r| results.push(r)).unwrap();
        assert_eq!(results[0].verdict, Verdict::Pass);
        fs::write(&path, "while True: pass").unwrap(); results.clear();
        run(Language::Python, "python3", &path, &[case], Duration::from_millis(80), |r| results.push(r)).unwrap();
        assert_eq!(results[0].verdict, Verdict::Timeout);
    }
}
