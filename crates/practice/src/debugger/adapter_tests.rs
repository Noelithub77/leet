use super::*;

#[test]
#[ignore = "requires private tools for all five languages; isolate XDG_DATA_HOME and install them first"]
fn managed_adapters_record_real_stdin_and_function_cases() {
    for (language, code) in [
        (Language::Python, "n = int(input())\nanswer = n * 2\nprint(answer)\n"),
        (Language::Cpp, "#include <iostream>\nint main() {\n int n; std::cin >> n;\n int answer = n * 2;\n std::cout << answer << std::endl;\n}\n"),
        (Language::C, "#include <stdio.h>\nint main(void) {\n int n; scanf(\"%d\", &n);\n int answer = n * 2;\n printf(\"%d\\n\", answer);\n return 0;\n}\n"),
        (Language::Go, "package main\nimport \"fmt\"\nfunc main() {\n var n int\n fmt.Scan(&n)\n answer := n * 2\n fmt.Println(answer)\n}\n"),
        (Language::Java, "import java.util.*;\npublic class Main {\n public static void main(String[] args) {\n  int n = new Scanner(System.in).nextInt();\n  int answer = n * 2;\n  System.out.println(answer);\n }\n}\n"),
    ] {
        assert!(crate::tool_setup::ready(language), "Install {} in the isolated tools directory first", language.label());
        let case = Case { id: 4, input: "3".into(), expected: Some("6".into()), custom: false };
        let trace = record_stdin_code(language, "python3", code, &case, Limits { timeout: Duration::from_secs(60), ..Default::default() }).unwrap();
        assert_eq!(trace.error, None, "{}: {trace:#?}", language.label());
        assert_eq!(trace.output.as_deref().map(str::trim), Some("6"), "{}", language.label());
        assert!(!trace.steps.is_empty(), "{} returned no real states", language.label());
        assert!(trace.steps.iter().any(|step| step.locals.iter().any(|variable| variable.name == "answer")), "{} omitted local variables: {trace:#?}", language.label());
        println!("{}: real stdin, output, source stops and locals passed", language.label());
    }
    let meta = serde_json::json!({"name":"twice","params":[{"type":"integer"}],"return":{"type":"integer"}});
    let case = Case { id: 2, input: "4".into(), expected: Some("8".into()), custom: false };
    for (language, code) in [
        (Language::C, "int twice(int n) {\n int answer = n * 2;\n return answer;\n}\n"),
        (Language::Cpp, "class Solution { public:\n int twice(int n) {\n  int answer = n * 2;\n  return answer;\n }\n};\n"),
        (Language::Go, "func twice(n int) int {\n answer := n * 2\n return answer\n}\n"),
        (Language::Java, "class Solution {\n public int twice(int n) {\n  int answer = n * 2;\n  return answer;\n }\n}\n"),
    ] {
        let trace = record_code(language, "python3", code, &meta, &case, Limits { timeout: Duration::from_secs(60), ..Default::default() }).unwrap();
        assert_eq!(trace.error, None, "{}: {trace:#?}", language.label());
        assert_eq!(trace.output.as_deref(), Some("8"), "{}", language.label());
        assert!(trace.steps.iter().all(|step| step.line as usize <= code.lines().count()), "Driver lines leaked into user source");
        println!("{}: real function harness passed", language.label());
    }

    let meta = serde_json::json!({"name":"sum","params":[{"type":"integer[]"}],"return":{"type":"integer"}});
    let case = Case { id: 3, input: "[3,5]".into(), expected: Some("8".into()), custom: false };
    for (language, code) in [
        (Language::C, "int sum(int* nums, int numsSize) {\n int answer = 0;\n for (int i = 0; i < numsSize; i++) answer += nums[i];\n return answer;\n}\n"),
        (Language::Cpp, "class Solution { public:\n int sum(vector<int>& nums) {\n  int answer = nums[0] + nums[1];\n  return answer;\n }\n};\n"),
        (Language::Go, "func sum(nums []int) int {\n answer := nums[0] + nums[1]\n return answer\n}\n"),
        (Language::Java, "class Solution {\n public int sum(int[] nums) {\n  int answer = nums[0] + nums[1];\n  return answer;\n }\n}\n"),
    ] {
        let trace = record_code(language, "python3", code, &meta, &case, Limits { timeout: Duration::from_secs(60), ..Default::default() }).unwrap();
        assert_eq!(trace.error, None, "{} array: {trace:#?}", language.label());
        assert_eq!(trace.output.as_deref(), Some("8"), "{} array", language.label());
        assert!(!trace.steps.is_empty(), "{} array returned no source states", language.label());
        println!("{}: real array argument passed", language.label());
    }
}

#[cfg(unix)]
#[test]
#[ignore = "requires private Python tools; isolate XDG_DATA_HOME and install them first"]
fn managed_python_reaps_debuggees_after_limits() {
    use std::process::{Command, Stdio};
    assert!(crate::tool_setup::ready(Language::Python));
    let temporary = tempfile::tempdir().unwrap();
    for (name, body, limits) in [
        ("steps", "n = 0\nwhile True:\n n += 1\n", Limits { max_steps: 8, ..Default::default() }),
        ("timeout", "import time\ntime.sleep(10)\n", Limits { timeout: Duration::from_secs(4), ..Default::default() }),
    ] {
        let pid_file = temporary.path().join(name);
        let code = format!("import os\nwith open({}, 'w') as __pid_output: __pid_output.write(str(os.getpid()))\n{body}", serde_json::to_string(&pid_file.to_string_lossy()).unwrap());
        let case = Case { id: 0, input: String::new(), expected: None, custom: true };
        let trace = record_stdin_code(Language::Python, "python3", &code, &case, limits).unwrap();
        assert!(trace.truncated, "{name}: {trace:#?}");
        if name == "steps" { assert_eq!(trace.steps.len(), 8); assert_eq!(trace.error, None); }
        else { assert_eq!(trace.error.as_deref(), Some("Recording timed out")); }
        let pid: u32 = std::fs::read_to_string(pid_file).unwrap().parse().unwrap();
        let started = std::time::Instant::now();
        while Command::new("kill").args(["-0", &pid.to_string()]).stdout(Stdio::null()).stderr(Stdio::null()).status().unwrap().success() {
            assert!(started.elapsed() < Duration::from_secs(2), "{name} left its debuggee running (pid {pid})");
            std::thread::sleep(Duration::from_millis(25));
        }
    }
}
