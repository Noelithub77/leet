//! Selected-language prerequisites; checking never starts a language server.
use std::{process::{Command, Stdio}, time::{Duration, Instant}};
use crate::{language::Language, lsp};

#[derive(Clone, Debug)]
pub struct Requirement {
    pub label: String,
    pub ready: bool,
    pub detail: String,
    pub setup_url: &'static str,
}
#[derive(Clone, Debug)]
pub struct Setup { pub requirements: Vec<Requirement> }
impl Setup { pub fn ready(&self) -> bool { self.requirements.iter().all(|tool| tool.ready) } }

fn version(command: &str, argument: &str) -> Result<String, String> {
    let mut child = Command::new(command).arg(argument).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped())
        .spawn().map_err(|_| format!("{command} not found"))?;
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if started.elapsed() < Duration::from_secs(3) => std::thread::sleep(Duration::from_millis(20)),
            _ => { let _ = child.kill(); let _ = child.wait(); return Err(format!("{command} did not respond")); }
        }
    }
    let output = child.wait_with_output().map_err(|error| error.to_string())?;
    if !output.status.success() { return Err(format!("{command} could not run")); }
    let text = if output.stdout.is_empty() { output.stderr } else { output.stdout };
    Ok(String::from_utf8_lossy(&text).lines().next().unwrap_or_default().trim().to_owned())
}
fn java_major(version: &str) -> Option<u32> {
    let token = version.split_whitespace().find(|part| part.trim_matches('"').starts_with(|c: char| c.is_ascii_digit()))?.trim_matches('"');
    let mut parts = token.split(['.', '-', '+']);
    let major: u32 = parts.next()?.parse().ok()?;
    if major == 1 { parts.next()?.parse().ok() } else { Some(major) }
}
pub fn check(language: Language, python: &str) -> Setup {
    let mut requirements = Vec::new();
    let tools: Vec<(&str, &str, &str, &'static str)> = match language {
        Language::Python => vec![("Python", python, "--version", "https://www.python.org/downloads/")],
        Language::Cpp => vec![("C++ compiler", "g++", "--version", "https://gcc.gnu.org/install/")],
        Language::C => vec![("C compiler", "gcc", "--version", "https://gcc.gnu.org/install/")],
        Language::Go => vec![("Go", "go", "version", "https://go.dev/doc/install")],
        Language::Java => vec![("JDK 21+ runtime", "java", "-version", "https://adoptium.net/installation/"), ("Java compiler", "javac", "-version", "https://adoptium.net/installation/")],
    };
    for (label, command, argument, setup_url) in tools {
        let result = version(command, argument).and_then(|version| {
            if language == Language::Python && !version.starts_with("Python 3.") {
                Err(format!("Python 3 required · {version}"))
            } else if language == Language::Java && java_major(&version).is_none_or(|major| major < 21) {
                Err(format!("JDK 21+ required · {version}"))
            } else { Ok(version) }
        });
        requirements.push(Requirement { label: label.into(), ready: result.is_ok(), detail: result.unwrap_or_else(|error| error), setup_url });
    }
    let (label, setup_url) = match language {
        Language::Python => ("Python language server", "https://docs.basedpyright.com/latest/installation/command-line-and-language-server/"),
        Language::Cpp | Language::C => ("clangd", "https://clangd.llvm.org/installation"),
        Language::Go => ("gopls", "https://go.dev/gopls/"),
        Language::Java => ("Eclipse JDT LS", "https://github.com/eclipse-jdtls/eclipse.jdt.ls#installation"),
    };
    let server = lsp::command(language);
    requirements.push(Requirement { label: label.into(), ready: server.is_ok(), detail: server.map(|(path, _, _)| path.display().to_string()).unwrap_or_else(|error| error.to_string()), setup_url });
    Setup { requirements }
}
#[cfg(test)] mod tests {
    use super::*;
    #[test] fn java_versions_include_legacy_and_modern_jdks() {
        assert_eq!(java_major("openjdk version \"21.0.5\" 2024-10-15"), Some(21));
        assert_eq!(java_major("javac 25.0.1"), Some(25));
        assert_eq!(java_major("java version \"1.8.0_442\""), Some(8));
        assert_eq!(java_major("unavailable"), None);
    }
    #[test] fn checks_only_selected_language_and_configured_interpreter() {
        let setup = check(Language::Python, "leet-missing-python-for-test");
        assert!(!setup.ready());
        assert_eq!(setup.requirements.len(), 2);
        assert!(setup.requirements[0].detail.contains("leet-missing-python-for-test"));
        assert!(setup.requirements.iter().all(|requirement| !requirement.label.contains("Java") && !requirement.label.contains("Go")));
    }
}
