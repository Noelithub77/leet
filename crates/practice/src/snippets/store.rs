//! VS Code-compatible user snippet storage.
use std::{collections::BTreeMap, fs, io::Write, path::{Path, PathBuf}};
use serde_json::{Value, json};
use super::{Origin, Settings, Snippet, body};
use crate::language::Language;

pub fn dir() -> PathBuf { crate::config::config_dir().join("snippets") }

pub struct Loaded {
    pub snippets: Vec<Snippet>,
    pub errors: Vec<(PathBuf, String)>,
}

pub(crate) fn language(id: &str) -> Option<Language> {
    match id.trim() {
        "python" => Some(Language::Python),
        "cpp" | "c++" | "cuda-cpp" => Some(Language::Cpp),
        "c" => Some(Language::C), "go" | "golang" => Some(Language::Go),
        "java" => Some(Language::Java), _ => None,
    }
}

/// Remove comments and trailing commas without altering quoted strings.
pub(crate) fn jsonc(text: &str) -> Result<Value, String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let (mut i, mut quoted, mut escaped) = (0, false, false);
    while i < bytes.len() {
        let c = bytes[i];
        if quoted {
            out.push(c);
            if escaped { escaped = false; }
            else if c == b'\\' { escaped = true; }
            else if c == b'"' { quoted = false; }
            i += 1;
        } else if c == b'"' { quoted = true; out.push(c); i += 1; }
        else if c == b'/' && bytes.get(i + 1) == Some(&b'/') {
            i += 2;
            while i < bytes.len() && bytes[i] != b'\n' { i += 1; }
            out.push(b' ');
        } else if c == b'/' && bytes.get(i + 1) == Some(&b'*') {
            i += 2;
            while i + 1 < bytes.len() && &bytes[i..i + 2] != b"*/" {
                out.push(if bytes[i] == b'\n' { b'\n' } else { b' ' }); i += 1;
            }
            if i + 1 >= bytes.len() { return Err("Unclosed comment".into()); }
            out.push(b' '); i += 2;
        } else { out.push(c); i += 1; }
    }
    let (mut i, mut quoted, mut escaped) = (0, false, false);
    while i < out.len() {
        let c = out[i];
        if quoted {
            if escaped { escaped = false; } else if c == b'\\' { escaped = true; }
            else if c == b'"' { quoted = false; }
        } else if c == b'"' { quoted = true; }
        else if c == b',' {
            let next = out[i + 1..].iter().find(|c| !c.is_ascii_whitespace());
            if matches!(next, Some(b']' | b'}')) { out[i] = b' '; }
        }
        i += 1;
    }
    serde_json::from_slice(&out).map_err(|e| e.to_string())
}

fn strings(value: Option<&Value>) -> Option<Vec<String>> {
    match value? {
        Value::String(s) => Some(vec![s.clone()]),
        Value::Array(items) => items.iter().map(|v| v.as_str().map(str::to_owned)).collect(),
        _ => None,
    }
}

pub(crate) fn parse_json(text: &str, scope: Option<Language>, honor_scope: bool, origin: Origin)
    -> Result<(Vec<Snippet>, usize), String>
{
    let value = jsonc(text)?;
    let object = value.as_object().ok_or("Expected a snippet object")?;
    let (mut snippets, mut skipped) = (Vec::new(), 0);
    for (name, entry) in object {
        let Some(prefixes) = strings(entry.get("prefix")) else { skipped += 1; continue; };
        let Some(lines) = strings(entry.get("body")) else { skipped += 1; continue; };
        let snippet = Snippet {
            name: name.clone(), prefixes, body: lines.join("\n").replace("\r\n", "\n"),
            description: entry.get("description").and_then(Value::as_str).unwrap_or("").into(),
            scope, template: entry.get("isFileTemplate").and_then(Value::as_bool).unwrap_or(false),
            origin: if origin == Origin::User {
                match entry.get("origin").and_then(Value::as_str) {
                    Some("vs-code") => Origin::VsCode, Some("neovim") => Origin::Neovim,
                    Some("sublime") => Origin::Sublime, Some("ai") => Origin::Ai, _ => Origin::User,
                }
            } else { origin },
        };
        if check(&snippet, &[]).is_err() { skipped += 1; continue; }
        if honor_scope && let Some(ids) = entry.get("scope").and_then(Value::as_str).filter(|s| !s.trim().is_empty()) {
            let mut scopes = Vec::new();
            for id in ids.split(',') {
                if let Some(scope) = language(id) {
                    if !scopes.contains(&scope) { scopes.push(scope); }
                } else { skipped += 1; }
            }
            snippets.extend(scopes.into_iter().map(|scope| Snippet { scope: Some(scope), ..snippet.clone() }));
        } else { snippets.push(snippet); }
    }
    Ok((snippets, skipped))
}

fn scopes() -> impl Iterator<Item = Option<Language>> {
    Language::ALL.into_iter().map(Some).chain(std::iter::once(None))
}
fn scope_file(dir: &Path, scope: Option<Language>) -> PathBuf {
    dir.join(format!("{}.json", scope.map_or("all", Language::id)))
}

/// Cheap change detection for the six managed library files, without parsing bodies.
pub fn revision(dir: &Path) -> Vec<Option<(std::time::SystemTime, u64)>> {
    scopes().map(|scope| fs::metadata(scope_file(dir, scope)).ok().and_then(|m| Some((m.modified().ok()?, m.len())))).collect()
}

pub fn load(dir: &Path) -> Loaded {
    let mut loaded = Loaded { snippets: Vec::new(), errors: Vec::new() };
    for scope in scopes() {
        let path = scope_file(dir, scope);
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => { loaded.errors.push((path, e.to_string())); continue; }
        };
        match parse_json(&text, scope, scope.is_none(), Origin::User) {
            Ok((snippets, skipped)) => {
                loaded.snippets.extend(snippets);
                if skipped > 0 { loaded.errors.push((path, format!("Skipped {skipped} invalid snippet entries"))); }
            }
            Err(e) => loaded.errors.push((path, e)),
        }
    }
    loaded
}

pub fn save(dir: &Path, snippets: &[Snippet]) -> anyhow::Result<()> {
    fs::create_dir_all(dir)?;
    for scope in scopes() {
        let path = scope_file(dir, scope);
        let mut entries = BTreeMap::new();
        for snippet in snippets.iter().filter(|s| s.scope == scope && s.origin != Origin::Builtin) {
            let prefix = if snippet.prefixes.len() == 1 { json!(snippet.prefixes[0]) } else { json!(snippet.prefixes) };
            let mut entry = serde_json::Map::new();
            entry.insert("prefix".into(), prefix);
            entry.insert("body".into(), json!(snippet.body.replace("\r\n", "\n").split('\n').collect::<Vec<_>>()));
            if !snippet.description.is_empty() { entry.insert("description".into(), json!(snippet.description)); }
            if snippet.template { entry.insert("isFileTemplate".into(), json!(true)); }
            if snippet.origin != Origin::User { entry.insert("origin".into(), serde_json::to_value(snippet.origin)?); }
            entries.insert(&snippet.name, Value::Object(entry));
        }
        if entries.is_empty() {
            match fs::remove_file(&path) { Ok(()) => {}, Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}, Err(e) => return Err(e.into()) }
            continue;
        }
        let mut text = serde_json::to_vec_pretty(&entries)?; text.push(b'\n');
        // create_new prevents concurrent saves from sharing a temporary file.
        let mut attempt = 0;
        let (temp, mut file) = loop {
            let temp = dir.join(format!(".{}.{}.{}.tmp", scope.map_or("all", Language::id), std::process::id(), attempt));
            match fs::OpenOptions::new().write(true).create_new(true).open(&temp) {
                Ok(file) => break (temp, file),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => attempt += 1,
                Err(e) => return Err(e.into()),
            }
        };
        let result = (|| -> std::io::Result<()> { file.write_all(&text)?; file.sync_all()?; drop(file); fs::rename(&temp, &path) })();
        if result.is_err() { let _ = fs::remove_file(&temp); }
        result?;
    }
    Ok(())
}

pub fn effective(user: &[Snippet], builtin: &[Snippet], settings: &Settings) -> Vec<Snippet> {
    let mut entries = BTreeMap::new();
    if settings.builtin {
        for s in builtin.iter().filter(|s| !settings.hidden.contains(&s.key())) { entries.insert(s.key(), s.clone()); }
    }
    for s in user { entries.insert(s.key(), s.clone()); }
    let mut result: Vec<_> = entries.into_values().collect();
    result.sort_by(|a, b| {
        let rank = |scope| Language::ALL.iter().position(|l| Some(*l) == scope).unwrap_or(Language::ALL.len());
        (rank(a.scope), a.prefix(), &a.name).cmp(&(rank(b.scope), b.prefix(), &b.name))
    });
    result
}

pub fn for_language(snippets: &[Snippet], language: Language) -> Vec<&Snippet> {
    snippets.iter().filter(|s| s.applies_to(language)).collect()
}
pub fn check(snippet: &Snippet, others: &[Snippet]) -> Result<(), String> {
    if snippet.name.trim().is_empty() { return Err("Add a name".into()); }
    if snippet.prefixes.is_empty() || snippet.prefixes.iter().any(|p| p.is_empty()) { return Err("Add a prefix".into()); }
    if snippet.prefixes.iter().any(|p| p.chars().any(char::is_whitespace)) { return Err("Prefix can't contain spaces".into()); }
    if snippet.body.trim().is_empty() { return Err("Add a body".into()); }
    body::validate(&snippet.body)?;
    if others.iter().any(|s| s.key() == snippet.key()) { return Err("Name already used".into()); }
    Ok(())
}
pub fn unique_name(base: &str, scope: Option<Language>, existing: &[Snippet]) -> String {
    let mut name = base.to_owned(); let mut suffix = 2;
    while existing.iter().any(|s| s.scope == scope && s.name == name) {
        name = format!("{base} {suffix}"); suffix += 1;
    }
    name
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sample() -> Snippet { Snippet { name: "Loop".into(), prefixes: vec!["loop".into(), "for".into()], body: "for ${1:x}:\n\t$0\n".into(), description: "Loop".into(), scope: Some(Language::Python), template: true, origin: Origin::Ai } }
    #[test]
    fn stale_mutations_preserve_newer_library_and_release_lock() {
        let dir = tempfile::tempdir().unwrap();
        let before = vec![sample()];
        commit(dir.path(), &[], &before).unwrap();
        let mut after = before.clone(); after[0].body = "${1:new} $0".into();
        commit(dir.path(), &before, &after).unwrap();
        assert!(commit(dir.path(), &before, &[]).is_err());
        assert_eq!(load(dir.path()).snippets, after);
        assert!(!dir.path().join(".write-lock").exists());
        commit(dir.path(), &after, &before).unwrap();
        fs::write(dir.path().join(".write-lock"), "other writer").unwrap();
        assert!(commit(dir.path(), &before, &[]).is_err());
        assert_eq!(fs::read_to_string(dir.path().join(".write-lock")).unwrap(), "other writer");
    }
    #[test]
    fn snippets_jsonc() {
        let text = r#"{/* comment */ "url": {"prefix": "url", // comment
        "body": ["https://example.test/*hi*/", "\\\"//",],},}"#;
        let (items, skipped) = parse_json(text, None, true, Origin::User).unwrap();
        assert_eq!(skipped, 0); assert!(items[0].body.starts_with("https://example.test/*hi*/"));
        assert!(jsonc("{/*").is_err());
    }
    #[test]
    fn snippets_store_roundtrip_and_isolation() {
        let dir = tempfile::tempdir().unwrap(); let s = sample();
        fs::write(dir.path().join("notes.txt"), "keep").unwrap();
        save(dir.path(), std::slice::from_ref(&s)).unwrap();
        assert_eq!(load(dir.path()).snippets, vec![s]);
        fs::write(dir.path().join("cpp.json"), "broken").unwrap();
        let loaded = load(dir.path()); assert_eq!(loaded.snippets.len(), 1); assert_eq!(loaded.errors.len(), 1);
        fs::write(dir.path().join("all.json"), r#"{"Multi":{"prefix":"multi","body":"$0","scope":"go,java"}}"#).unwrap();
        assert_eq!(load(dir.path()).snippets.len(), 3);
        save(dir.path(), &[]).unwrap(); assert!(!dir.path().join("python.json").exists());
        assert_eq!(fs::read_to_string(dir.path().join("notes.txt")).unwrap(), "keep");
    }
    #[test]
    fn snippets_save_defaults_builtins_and_order() {
        let dir = tempfile::tempdir().unwrap();
        let mut first = sample(); first.name = "Z".into(); first.template = false;
        first.description.clear(); first.prefixes = vec!["z".into()]; first.origin = Origin::User;
        let mut second = first.clone(); second.name = "A".into();
        let mut builtin = first.clone(); builtin.scope = Some(Language::Cpp); builtin.origin = Origin::Builtin;
        save(dir.path(), &[first, second, builtin]).unwrap();
        let text = fs::read_to_string(dir.path().join("python.json")).unwrap();
        assert!(text.ends_with('\n')); assert!(text.find("\"A\"").unwrap() < text.find("\"Z\"").unwrap());
        assert!(!text.contains("origin")); assert!(!text.contains("description")); assert!(!text.contains("isFileTemplate"));
        assert!(!dir.path().join("cpp.json").exists());
        let value = jsonc(&text).unwrap(); assert!(value["A"]["prefix"].is_string()); assert!(value["A"]["body"].is_array());
    }
    #[test]
    fn snippets_effective_and_validation() {
        let user = sample(); let mut builtin = user.clone(); builtin.origin = Origin::Builtin; builtin.body = "$0".into();
        let mut hidden = builtin.clone(); hidden.name = "Hidden".into();
        let settings = Settings { hidden: vec![hidden.key()], ..Default::default() };
        assert_eq!(effective(std::slice::from_ref(&user), &[builtin.clone(), hidden], &settings), vec![user.clone()]);
        assert!(effective(&[], &[builtin], &Settings { builtin: false, ..settings }).is_empty());
        assert_eq!(check(&user, std::slice::from_ref(&user)).unwrap_err(), "Name already used");
        let mut bad = user.clone(); bad.prefixes.clear(); assert_eq!(check(&bad, &[]).unwrap_err(), "Add a prefix");
        bad.prefixes = vec!["a b".into()]; assert_eq!(check(&bad, &[]).unwrap_err(), "Prefix can't contain spaces");
        bad = user.clone(); bad.name.clear(); assert_eq!(check(&bad, &[]).unwrap_err(), "Add a name");
        bad = user.clone(); bad.body.clear(); assert_eq!(check(&bad, &[]).unwrap_err(), "Add a body");
        assert_eq!(unique_name("Loop", user.scope, &[user]), "Loop 2");
    }
}

/// Compare before writing so an agent or another window cannot overwrite a stale draft.
pub fn commit(dir: &Path, expected: &[Snippet], snippets: &[Snippet]) -> anyhow::Result<()> {
    fs::create_dir_all(dir)?;
    let path = dir.join(".write-lock");
    let file = fs::OpenOptions::new().write(true).create_new(true).open(&path)
        .map_err(|e| anyhow::anyhow!("Snippet library is busy ({}): {e}", path.display()))?;
    struct Lock(PathBuf, Option<fs::File>);
    impl Drop for Lock { fn drop(&mut self) { drop(self.1.take()); let _ = fs::remove_file(&self.0); } }
    let _lock = Lock(path, Some(file));
    let loaded = load(dir);
    anyhow::ensure!(loaded.errors.is_empty(), "Repair invalid snippet files before saving: {:?}", loaded.errors);
    let mut current: Vec<_> = loaded.snippets.iter().collect(); let mut baseline: Vec<_> = expected.iter().collect();
    current.sort_by_key(|s| s.key()); baseline.sort_by_key(|s| s.key());
    anyhow::ensure!(current == baseline, "Snippets changed in another window or agent; reload before saving");
    let mut keys = std::collections::HashSet::new();
    for snippet in snippets {
        check(snippet, &[]).map_err(anyhow::Error::msg)?;
        anyhow::ensure!(keys.insert(snippet.key()), "Duplicate snippet name: {}", snippet.name);
    }
    save(dir, snippets)
}
