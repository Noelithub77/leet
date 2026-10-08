use super::*;

fn run(language: Language, code: &str, input: &str, limits: Limits) -> Trace {
    let case = Case { id: 9, input: input.into(), expected: None, custom: true };
    record_stdin_code(language, "python3", code, &case, limits).unwrap()
}

#[test]
fn python_stdin_records_module_helpers_binary_input_and_stdout() {
    let code = "import sys\ndef double(n):\n    return n * 2\nif __name__ == '__main__':\n    values = list(map(int, sys.stdin.buffer.read().split()))\n    for n in values[1:]:\n        answer = double(n)\n        print(answer)\n";
    let trace = run(Language::Python, code, "2\n3 5\n", Limits::default());
    assert_eq!(trace.error, None);
    assert_eq!(trace.output.as_deref(), Some("6\n10\n"));
    assert_eq!(trace.stdout, "6\n10\n");
    assert_eq!(trace.case_id, 9);
    assert!(trace.steps.iter().any(|s| s.function == "module" && s.locals.iter().any(|v| v.name == "answer")));
    assert!(trace.steps.iter().any(|s| s.function == "double" && s.stack.len() == 2));
    assert!(trace.steps.iter().any(|s| s.stdout_len == 2));
    assert!(trace.steps.iter().all(|s| s.line <= 8 && s.locals.iter().all(|v| v.name != "cap")));
}

#[test]
fn python_stdin_preserves_whitespace_eof_and_successful_exit() {
    let trace = run(Language::Python, "import sys\na = input()\nb = sys.stdin.readline()\nrest = sys.stdin.read()\nprint(repr((a, b, rest)))\nsys.exit(0)\n", "  hi  \n\nend  ", Limits::default());
    assert_eq!(trace.error, None);
    assert_eq!(trace.output.as_deref(), Some("('  hi  ', '\\n', 'end  ')\n"));
    let trace = run(Language::Python, "print(input())\nprint(input())\n", "one\n", Limits::default());
    assert!(trace.error.as_deref().unwrap().contains("EOFError"));
    assert_eq!(trace.stdout, "one\n");
    assert!(trace.steps.iter().any(|s| s.kind == StepKind::Exception));
}

#[test]
fn stdin_recorders_retain_partial_traces_and_report_errors() {
    for (language, loop_code, error_code) in [
        (Language::Python, "n = 0\nwhile True:\n    n += 1\n", "print('before')\nraise ValueError('failure')\n"),
        (Language::Cpp, "#include <iostream>\nint main() {\n int n = 0;\n while (true) {\n  n++;\n }\n}\n", "#include <iostream>\nint main() {\n std::cout << \"before\\n\" << std::flush;\n return 7;\n}\n"),
    ] {
        if requirement(language, "python3").is_err() { eprintln!("Skipping unavailable {} recorder", language.label()); continue; }
        let trace = run(language, loop_code, "", Limits { max_steps: 10, ..Limits::default() });
        assert!(trace.truncated, "{}", language.label());
        assert_eq!(trace.steps.len(), 10);
        assert!(trace.output.is_none());
        let trace = run(language, error_code, "", Limits::default());
        assert!(trace.error.is_some(), "{}", language.label());
        assert_eq!(trace.stdout, "before\n");
    }
}

#[test]
fn cpp_stdin_records_main_helpers_containers_and_raw_output() {
    if requirement(Language::Cpp, "python3").is_err() { eprintln!("Skipping unavailable C++ recorder"); return; }
    let code = "#include <bits/stdc++.h>\nusing namespace std;\nint twice(int x) {\n return x * 2;\n}\nint main() {\n int t;\n cin >> t;\n vector<int> answers;\n while (t--) {\n  int n;\n  cin >> n;\n  answers.push_back(twice(n));\n  cout << answers.back() << '\\n' << flush;\n }\n return 0;\n}\n";
    let trace = run(Language::Cpp, code, "2\n3\n5", Limits::default());
    assert_eq!(trace.error, None);
    assert_eq!(trace.output.as_deref(), Some("6\n10\n"));
    assert_eq!(trace.stdout, "6\n10\n");
    assert!(trace.steps.iter().any(|s| s.function.contains("twice") && s.stack.len() == 2));
    assert!(trace.steps.iter().any(|s| s.locals.iter().any(|v| v.name == "answers" && matches!(&v.value, Value::List { items, .. } if items.len() == 2))));
    assert!(trace.steps.iter().any(|s| s.stdout_len == 2));
}

#[test]
fn stdin_program_timeouts_keep_output_and_states() {
    for (language, code) in [
        (Language::Python, "import time\nprint('before')\ntime.sleep(10)\n"),
        (Language::Cpp, "#include <iostream>\n#include <thread>\n#include <chrono>\nint main() {\n std::cout << \"before\\n\" << std::flush;\n std::this_thread::sleep_for(std::chrono::seconds(10));\n}\n"),
    ] {
        if requirement(language, "python3").is_err() { eprintln!("Skipping unavailable {} recorder", language.label()); continue; }
        let trace = run(language, code, "", Limits { timeout: Duration::from_secs(3), ..Limits::default() });
        assert_eq!(trace.error.as_deref(), Some("Recording timed out"));
        assert_eq!(trace.stdout, "before\n");
        assert!(!trace.steps.is_empty());
        assert!(trace.truncated);
    }
}

#[test]
fn python_stdin_supports_fast_file_descriptor_input_and_prelude_helpers() {
    let trace = run(Language::Python, "import os, sys\nraw = os.read(sys.stdin.fileno(), os.fstat(sys.stdin.fileno()).st_size)\ncounts = Counter(map(int, raw.split()))\nprint(sum(counts.values()))\n", "1 1 2", Limits::default());
    assert_eq!(trace.error, None);
    assert_eq!(trace.output.as_deref(), Some("3\n"));
    assert!(trace.steps.iter().any(|s| s.locals.iter().any(|v| v.name == "counts")));
}

#[test]
fn cpp_stdin_records_globals_and_allows_stderr_logging() {
    if requirement(Language::Cpp, "python3").is_err() { eprintln!("Skipping unavailable C++ recorder"); return; }
    let trace = run(Language::Cpp, "#include <iostream>\nint answer = 1;\nint main() {\n std::cin >> answer;\n std::cerr << \"diagnostic\";\n std::cout << answer;\n}\n", "42", Limits::default());
    assert_eq!(trace.error, None);
    assert_eq!(trace.output.as_deref(), Some("42"));
    assert!(trace.steps.iter().any(|s| s.locals.iter().any(|v| v.name == "answer" && matches!(&v.value, Value::Int { v } if v == "42"))));
}

#[test]
fn stdin_invalid_source_reports_compiler_and_interpreter_errors() {
    let trace = run(Language::Python, "def broken(:\n", "", Limits::default());
    assert!(trace.error.as_deref().unwrap().contains("SyntaxError"));
    if requirement(Language::Cpp, "python3").is_err() { eprintln!("Skipping unavailable C++ recorder"); return; }
    let case = Case { id: 0, input: String::new(), expected: None, custom: true };
    let error = record_stdin_code(Language::Cpp, "python3", "int main() { broken syntax }", &case, Limits::default()).unwrap_err();
    assert!(error.to_string().contains("compilation failed"));
}
