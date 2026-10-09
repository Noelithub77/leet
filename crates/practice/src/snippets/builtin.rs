//! Bundled competitive-programming snippets and file templates.
use std::{collections::BTreeMap, sync::OnceLock};

use serde::Deserialize;

use super::{Origin, Snippet};
use crate::language::Language;

const FILES: [(Language, &str); 5] = [
    (Language::Python, include_str!("builtin/python.json")),
    (Language::Cpp, include_str!("builtin/cpp.json")),
    (Language::Go, include_str!("builtin/go.json")),
    (Language::C, include_str!("builtin/c.json")),
    (Language::Java, include_str!("builtin/java.json")),
];

#[derive(Deserialize)]
#[serde(untagged)]
enum Prefixes {
    One(String),
    Many(Vec<String>),
}

#[derive(Deserialize)]
struct Entry {
    prefix: Prefixes,
    body: Vec<String>,
    description: String,
    #[serde(default, rename = "isFileTemplate")]
    template: bool,
}

fn parse(language: Language, json: &str) -> Result<Vec<Snippet>, serde_json::Error> {
    let entries: BTreeMap<String, Entry> = serde_json::from_str(json)?;
    Ok(entries.into_iter().map(|(name, entry)| Snippet {
        name,
        prefixes: match entry.prefix {
            Prefixes::One(prefix) => vec![prefix],
            Prefixes::Many(prefixes) => prefixes,
        },
        body: entry.body.join("\n"),
        description: entry.description,
        scope: Some(language),
        template: entry.template,
        origin: Origin::Builtin,
    }).collect())
}

/// Returns an independent copy of the embedded catalog, parsed once per process.
pub fn builtin() -> Vec<Snippet> {
    static SNIPPETS: OnceLock<Vec<Snippet>> = OnceLock::new();
    SNIPPETS.get_or_init(|| FILES.into_iter().flat_map(|(language, json)| {
        parse(language, json).expect("embedded snippet catalog must be valid JSON")
    }).collect()).clone()
}

#[cfg(test)]
mod tests {
    use std::{collections::{BTreeMap, HashSet}, io::Write, process::{Command, Stdio}};

    use super::*;

    #[test]
    fn snippets_builtin_catalog_contract() {
        for (language, json) in FILES {
            let snippets = parse(language, json).expect("language catalog parses");
            let range = match language {
                Language::Cpp => 35..=55,
                Language::Python => 30..=45,
                _ => 25..=35,
            };
            assert!(range.contains(&snippets.len()), "{}: {} snippets", language.id(), snippets.len());
            let mut names = HashSet::new();
            let mut prefixes = HashSet::new();
            for snippet in &snippets {
                assert!(names.insert(&snippet.name), "duplicate name: {}", snippet.name);
                assert!(!snippet.name.is_empty());
                assert!(!snippet.prefixes.is_empty(), "{}", snippet.name);
                for prefix in &snippet.prefixes {
                    assert!(!prefix.is_empty() && !prefix.chars().any(char::is_whitespace), "{}: {prefix}", snippet.name);
                    assert!(prefixes.insert(prefix), "duplicate prefix: {prefix}");
                    assert!(prefix.chars().all(|ch| !ch.is_ascii_uppercase()), "{prefix}");
                }
                assert!(!snippet.description.is_empty() && snippet.description.chars().count() <= 60, "{}", snippet.name);
                assert!(!snippet.description.ends_with('.'), "{}", snippet.name);
                assert_eq!(snippet.scope, Some(language));
                assert_eq!(snippet.origin, Origin::Builtin);
                super::super::body::validate(&snippet.body).unwrap_or_else(|error| panic!("{}/{}: {error}", language.id(), snippet.name));
            }
            assert!(snippets.iter().any(|snippet| snippet.template && snippet.prefixes.iter().any(|prefix| prefix.starts_with("cp"))));
        }
        let mut copy = builtin();
        let original = copy[0].body.clone();
        copy[0].body.clear();
        assert_eq!(builtin()[0].body, original);
    }

    // This deliberately small default expander is independent of the production
    // parser so syntax checks can catch catalog mistakes in either implementation.
    fn defaults(body: &str) -> String {
        let chars: Vec<char> = body.chars().collect();
        let mut values: BTreeMap<String, String> = BTreeMap::new();
        let mut output = String::new();
        let mut i = 0;
        while i < chars.len() {
            match chars[i] {
                '\\' if i + 1 < chars.len() && matches!(chars[i + 1], '$' | '}' | '\\') => {
                    output.push(chars[i + 1]);
                    i += 2;
                }
                '$' if i + 1 < chars.len() && chars[i + 1] == '{' => {
                    i += 2;
                    let start = i;
                    while i < chars.len() && chars[i].is_ascii_digit() { i += 1; }
                    let index: String = chars[start..i].iter().collect();
                    let kind = chars[i];
                    i += 1;
                    if kind == '}' {
                        if let Some(value) = values.get(&index) { output.push_str(value); }
                        continue;
                    }
                    let mut value = String::new();
                    while i < chars.len() && chars[i] != '}' {
                        if chars[i] == '\\' && i + 1 < chars.len() { i += 1; }
                        value.push(chars[i]);
                        i += 1;
                    }
                    i += 1;
                    if kind == '|' { value = value.split(',').next().unwrap().to_owned(); }
                    values.entry(index).or_insert_with(|| value.clone());
                    output.push_str(&value);
                }
                '$' => {
                    i += 1;
                    let start = i;
                    while i < chars.len() && chars[i].is_ascii_digit() { i += 1; }
                    if i == start {
                        while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') { i += 1; }
                    } else {
                        let index: String = chars[start..i].iter().collect();
                        if let Some(value) = values.get(&index) { output.push_str(value); }
                    }
                }
                '\t' => { output.push_str("    "); i += 1; }
                ch => { output.push(ch); i += 1; }
            }
        }
        output
    }

    fn check_program(program: &str, args: &[&str], source: &str, name: &str) {
        static COMPILER_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _guard = if matches!(program, "g++" | "gcc" | "go" | "javac") {
            Some(COMPILER_LOCK.lock().unwrap_or_else(|error| error.into_inner()))
        } else { None };
        let mut command = Command::new(program);
        crate::background_process::hide_console(&mut command);
        if program == "go" { command.env("GOMAXPROCS", "2"); }
        command.args(args).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return,
            Err(error) => panic!("{program}: {error}"),
        };
        child.stdin.take().unwrap().write_all(source.as_bytes()).unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success(), "{name}: {}\n{source}", String::from_utf8_lossy(&output.stderr));
    }

    #[test]
    fn snippets_builtin_templates_compile() {
        for snippet in builtin().into_iter().filter(|snippet| snippet.template) {
            let source = defaults(&snippet.body);
            match snippet.scope.unwrap() {
                Language::Cpp => check_program("g++", &["-std=c++17", "-x", "c++", "-fsyntax-only", "-"], &source, &snippet.name),
                Language::C => check_program("gcc", &["-std=c11", "-x", "c", "-fsyntax-only", "-"], &source, &snippet.name),
                Language::Python => check_program("python3", &["-c", "import ast,sys; ast.parse(sys.stdin.read())"], &source, &snippet.name),
                _ => {},
            }
        }
    }

    #[test]
    fn snippets_builtin_python_bodies_parse() {
        for snippet in builtin().into_iter().filter(|snippet| snippet.scope == Some(Language::Python)) {
            check_program("python3", &["-c", "import ast,sys; ast.parse(sys.stdin.read())"], &defaults(&snippet.body), &snippet.name);
        }
    }

    #[test]
    fn snippets_builtin_algorithm_definitions_compile() {
        let catalog = builtin();
        let names = ["Modular Power", "Modular Inverse", "Modular Arithmetic", "Gcd And Lcm",
            "Combinations", "Prime Sieve", "Prime Factors", "Disjoint Set", "Fenwick Tree",
            "Segment Tree", "Sparse Table", "Monotonic Stack", "Lowercase Trie", "Ordered Set",
            "Read Graph", "Graph Traversals", "Breadth First Search", "Iterative Depth First Search",
            "Dijkstra", "Topological Sort", "Zero One Bfs", "Floyd Warshall", "Grid Bfs",
            "Binary Search And Bounds", "Prefix Sums And Tables", "Two Pointers And Window",
            "Two Pointers", "Sliding Window", "String Matching", "Prefix Function", "Z Function",
            "Rolling Hash", "Contest Utilities"];
        for language in [Language::C, Language::Cpp] {
            let mut source = match language {
                Language::C => "#include <stdio.h>\n#include <stdlib.h>\n#include <string.h>\n#include <stdint.h>\n#include <limits.h>\n".to_owned(),
                _ => "#include <bits/stdc++.h>\nusing namespace std;\n".to_owned(),
            };
            if language == Language::C {
                let graph = catalog.iter().find(|snippet| snippet.scope == Some(language) && snippet.name == "Read Graph").unwrap();
                source.push_str(&defaults(&graph.body));
                source.push('\n');
            }
            for snippet in catalog.iter().filter(|snippet| snippet.scope == Some(language) && names.contains(&snippet.name.as_str())) {
                if snippet.name == "Read Graph" { continue; }
                // The C++ gcd entry is a statement for solve().
                if language == Language::Cpp && snippet.name == "Gcd And Lcm" { continue; }
                source.push_str(&defaults(&snippet.body));
                source.push('\n');
            }
            if language == Language::C {
                check_program("gcc", &["-std=c11", "-x", "c", "-fsyntax-only", "-"], &source, "C algorithms");
            } else {
                check_program("g++", &["-std=c++17", "-x", "c++", "-fsyntax-only", "-"], &source, "C++ algorithms");
            }
        }
    }

    #[test]
    fn snippets_builtin_python_algorithms() {
        let names = ["Combinations", "Prime Sieve", "Prime Factors", "Disjoint Set", "Fenwick Tree",
            "Segment Tree", "Sparse Table", "Monotonic Stack", "Trie", "Breadth First Search",
            "Iterative Depth First Search", "Dijkstra", "Topological Sort", "Zero One Bfs",
            "Floyd Warshall", "Grid Bfs", "Binary Search Answer", "Prefix Sums", "Grid Prefix Sums",
            "Two Pointers", "Sliding Window", "Prefix Function", "Z Function", "Rolling Hash"];
        let mut source = String::new();
        for snippet in builtin().iter().filter(|snippet| snippet.scope == Some(Language::Python) && names.contains(&snippet.name.as_str())) {
            source.push_str(&defaults(&snippet.body));
            source.push('\n');
        }
        source.push_str(r#"
assert build_combinations(10)(5, 2) == 10
assert sieve(0) == [] and sieve(20) == [2, 3, 5, 7, 11, 13, 17, 19]
assert factorize(1) == {} and factorize(360) == {2: 3, 3: 2, 5: 1}
d = DSU(4)
assert d.union(0, 1) and d.union(1, 2) and not d.union(2, 0)
assert d.find(0) == d.find(2) and d.find(0) != d.find(3)
f = Fenwick(4)
for i, value in enumerate([2, -1, 4, 3]): f.add(i, value)
assert f.query(1, 4) == 6 and f.query(2, 2) == 0
s = SegmentTree([2, -1, 4, 3])
assert s.query(1, 4) == 6
s.set(2, 7)
assert s.query(0, 4) == 11 and s.query(1, 1) == 0
assert SparseTable([5, 2, 7, 1, 4]).query(1, 4) == 1
assert next_greater([2, 2, 1, 3]) == [3, 3, 3, -1]
t = Trie(); t.insert('abc'); t.insert('')
assert t.contains('abc') and t.contains('') and not t.contains('ab')
g = [[1, 2], [3], [3], [], []]
assert bfs(g, 0) == [0, 1, 1, 2, -1] and sorted(dfs(g, 0)) == [0, 1, 2, 3]
weighted = [[(1, 1), (2, 0)], [(3, 1)], [(1, 0), (3, 1)], [], []]
expected = [0, 0, 0, 1, float('inf')]
assert dijkstra(weighted, 0) == expected and zero_one_bfs(weighted, 0) == expected
order = topological_sort(g)
assert all(order.index(u) < order.index(v) for u, edges in enumerate(g) for v in edges)
assert topological_sort([[1], [0]]) is None
inf = float('inf')
assert floyd_warshall([[0, 3, inf], [inf, 0, 2], [inf, inf, 0]])[0][2] == 5
assert grid_bfs(['..#', '#..'], (0, 0)) == [[0, 1, -1], [-1, 2, 3]]
assert first_true(0, 100, lambda x: x >= 37) == 37
assert first_true(0, 10, lambda x: False) == 10
assert prefix_sums([2, -1, 4]) == [0, 2, 1, 5]
assert prefix_grid([[1, 2], [3, 4]]) == [[0, 0, 0], [0, 1, 3], [0, 4, 10]]
assert has_pair_sum([-3, 1, 4, 8], 5) and not has_pair_sum([], 0)
assert longest_bounded_sum([1, 0, 2, 1, 3], 3) == 3
assert prefix_function('ababcabab') == [0, 0, 1, 2, 0, 1, 2, 3, 4]
assert z_function('aaaaa') == [0, 4, 3, 2, 1]
h = RollingHash('ababa')
assert h.query(0, 3) == h.query(2, 5) and h.query(1, 1) == 0
"#);
        check_program("python3", &["-"], &source, "Python algorithm examples");
    }

    #[test]
    fn snippets_builtin_go_java_compile() {
        let catalog = builtin();
        let names = ["Gcd And Lcm", "Modular Arithmetic", "Combinations", "Prime Sieve", "Prime Factors",
            "Disjoint Set", "Fenwick Tree", "Segment Tree", "Sparse Table", "Monotonic Stack",
            "Lowercase Trie", "Graph Traversals", "Dijkstra", "Topological Sort", "Zero One Bfs",
            "Floyd Warshall", "Grid Bfs", "Binary Search And Bounds", "Prefix Sums And Tables",
            "Two Pointers And Window", "String Matching", "Rolling Hash", "Contest Utilities"];
        let dir = tempfile::tempdir().unwrap();
        for language in [Language::Go, Language::Java] {
            let definitions: String = catalog.iter().filter(|snippet| snippet.scope == Some(language) && names.contains(&snippet.name.as_str()))
                .map(|snippet| format!("{}\n", defaults(&snippet.body))).collect();
            for snippet in catalog.iter().filter(|snippet| snippet.scope == Some(language) && snippet.template) {
                let mut source = defaults(&snippet.body);
                if snippet.name == "Single Test" {
                    if language == Language::Go {
                        source = source.replacen("import (", "import (\n\"container/heap\"\n\"container/list\"", 1);
                        source.push('\n');
                        source.push_str(&definitions);
                    } else {
                        source.truncate(source.rfind('}').unwrap());
                        source.push_str(&definitions);
                        source.push('}');
                    }
                }
                if language == Language::Go {
                    let path = dir.path().join("main.go");
                    let output = dir.path().join("snippet-go");
                    std::fs::write(&path, source).unwrap();
                    check_program("go", &["build", "-p", "1", "-o", &output.to_string_lossy(), &path.to_string_lossy()], "", &snippet.name);
                } else {
                    let path = dir.path().join("Main.java");
                    std::fs::write(&path, source).unwrap();
                    check_program("javac", &["-J-XX:ActiveProcessorCount=2", "-d", &dir.path().to_string_lossy(), &path.to_string_lossy()], "", &snippet.name);
                }
            }
        }
    }

    #[test]
    fn snippets_builtin_c_family_delimiters() {
        for snippet in builtin().into_iter().filter(|snippet| snippet.scope != Some(Language::Python)) {
            let source = defaults(&snippet.body);
            let mut chars = source.chars().peekable();
            let mut stack = Vec::new();
            while let Some(ch) = chars.next() {
                match ch {
                    '"' | '\'' | '`' => {
                        while let Some(next) = chars.next() {
                            if next == '\\' && ch != '`' { chars.next(); }
                            else if next == ch { break; }
                        }
                    }
                    '/' if chars.peek() == Some(&'/') => { for next in chars.by_ref() { if next == '\n' { break; } } }
                    '/' if chars.peek() == Some(&'*') => {
                        chars.next();
                        while let Some(next) = chars.next() { if next == '*' && chars.peek() == Some(&'/') { chars.next(); break; } }
                    }
                    '{' | '(' | '[' => stack.push(ch),
                    '}' | ')' | ']' => assert_eq!(stack.pop(), Some(match ch { '}' => '{', ')' => '(', _ => '[' }), "{}: {source}", snippet.name),
                    _ => {},
                }
            }
            assert!(stack.is_empty(), "{}: {source}", snippet.name);
        }
    }
}
