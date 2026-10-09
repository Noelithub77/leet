//! Read-only, bounded import of the user's snippets from other editors.
use std::{collections::{HashMap, HashSet}, fs, io::Read, path::{Path, PathBuf}};
use super::{Origin, Snippet, store};
use crate::language::Language;

const MAX_FILES: usize = 500;
const MAX_BYTES: u64 = 2 * 1024 * 1024;

pub struct Roots { pub home: PathBuf, pub config: PathBuf, pub data: PathBuf }
impl Roots {
    pub fn detect() -> Self {
        Self {
            home: dirs::home_dir().unwrap_or_default(),
            config: dirs::config_dir().unwrap_or_default(),
            data: dirs::data_local_dir().unwrap_or_default(),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Editor { VsCode, Neovim, Sublime }
impl Editor {
    pub fn label(self) -> &'static str {
        match self { Self::VsCode => "VS Code", Self::Neovim => "Neovim", Self::Sublime => "Sublime Text" }
    }
    pub fn origin(self) -> Origin {
        match self { Self::VsCode => Origin::VsCode, Self::Neovim => Origin::Neovim, Self::Sublime => Origin::Sublime }
    }
}
pub struct Found { pub editor: Editor, pub snippets: Vec<Snippet>, pub skipped: usize, pub files: usize }
pub struct Report { pub added: usize, pub unchanged: usize, pub renamed: usize }

fn plugin(path: &Path) -> bool {
    path.components().any(|part| matches!(part.as_os_str().to_str(),
        Some("lazy" | "pack" | "plugged" | "site" | "friendly-snippets" | "bundle")))
}
fn supported(path: &Path, editor: Editor) -> bool {
    match path.extension().and_then(|s| s.to_str()) {
        Some("json" | "code-snippets") => editor != Editor::Sublime,
        Some("snippets") => editor == Editor::Neovim,
        Some("sublime-snippet") => editor == Editor::Sublime,
        _ => false,
    }
}
struct Walker { paths: Vec<PathBuf>, seen: HashSet<PathBuf>, entries: usize, skipped: usize }
impl Walker {
    fn new() -> Self { Self { paths: Vec::new(), seen: HashSet::new(), entries: 0, skipped: 0 } }
    fn walk(&mut self, path: &Path, depth: usize, editor: Editor) {
        if depth > 4 || self.paths.len() >= MAX_FILES || self.entries >= 20_000 || (editor == Editor::Neovim && plugin(path)) { return; }
        let Ok(meta) = fs::symlink_metadata(path) else { return; };
        if meta.file_type().is_symlink() { return; }
        if meta.is_file() {
            if supported(path, editor) && self.seen.insert(path.to_owned()) { self.paths.push(path.to_owned()); }
            return;
        }
        if !meta.is_dir() { return; }
        let Ok(entries) = fs::read_dir(path) else { self.skipped += 1; return; };
        // Bound traversal as well as file reads, even in unusually large trees.
        let mut children = Vec::new();
        for entry in entries.take(20_000usize.saturating_sub(self.entries)) {
            self.entries += 1;
            match entry { Ok(entry) => children.push(entry.path()), Err(_) => self.skipped += 1 }
        }
        children.sort();
        for child in children { self.walk(&child, depth + 1, editor); }
    }
}
fn read(path: &Path) -> Result<String, ()> {
    let meta = fs::symlink_metadata(path).map_err(|_| ())?;
    if !meta.is_file() || meta.len() > MAX_BYTES { return Err(()); }
    let mut bytes = Vec::new();
    fs::File::open(path).map_err(|_| ())?.take(MAX_BYTES + 1).read_to_end(&mut bytes).map_err(|_| ())?;
    if bytes.len() as u64 > MAX_BYTES { return Err(()); }
    String::from_utf8(bytes).map_err(|_| ())
}
fn filename_scope(path: &Path) -> Result<Option<Language>, ()> {
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    if matches!(stem, "all" | "_" | "global") { return Ok(None); }
    if let Some(language) = store::language(stem) { return Ok(Some(language)); }
    let parent = path.parent().and_then(Path::file_name).and_then(|s| s.to_str()).unwrap_or("");
    if matches!(parent, "all" | "_" | "global") { return Ok(None); }
    store::language(parent).map(Some).ok_or(())
}

pub fn scan(roots: &Roots) -> Vec<Found> {
    let mut results = Vec::new();
    for editor in [Editor::VsCode, Editor::Neovim, Editor::Sublime] {
        let mut walker = Walker::new();
        if editor != Editor::Neovim && roots.config.as_os_str().is_empty() { continue; }
        match editor {
            Editor::VsCode => {
                for app in ["Code", "Code - Insiders", "VSCodium", "Cursor", "Windsurf"] {
                    let user = roots.config.join(app).join("User");
                    walker.walk(&user.join("snippets"), 3, editor);
                    let profiles = user.join("profiles");
                    if fs::symlink_metadata(&profiles).is_ok_and(|m| m.is_dir() && !m.file_type().is_symlink())
                        && let Ok(entries) = fs::read_dir(&profiles)
                    {
                        let mut profiles: Vec<_> = entries.take(MAX_FILES).filter_map(Result::ok).map(|e| e.path()).collect();
                        profiles.sort();
                        for profile in profiles {
                            if fs::symlink_metadata(&profile).is_ok_and(|m| m.is_dir() && !m.file_type().is_symlink()) {
                                walker.walk(&profile.join("snippets"), 3, editor);
                            }
                        }
                    }
                }
            }
            Editor::Neovim => {
                for (base, folder) in [(&roots.config, "nvim"), (&roots.data, "nvim"), (&roots.home, ".vim")] {
                    if base.as_os_str().is_empty() { continue; }
                    let root = base.join(folder);
                    walker.walk(&root.join("package.json"), 0, editor);
                    for folder in ["snippets", "UltiSnips", "snips"] { walker.walk(&root.join(folder), 0, editor); }
                }
            }
            Editor::Sublime => {
                for app in ["sublime-text", "sublime-text-3", "Sublime Text", "Sublime Text 3"] {
                    walker.walk(&roots.config.join(app).join("Packages/User"), 0, editor);
                }
            }
        }
        let mut found = Found { editor, snippets: Vec::new(), skipped: walker.skipped, files: walker.paths.len() };
        let mut mappings: HashMap<PathBuf, Vec<Option<Language>>> = HashMap::new();
        if editor == Editor::Neovim {
            for path in walker.paths.iter().filter(|path| path.file_name().is_some_and(|s| s == "package.json")) {
                let text = match read(path) { Ok(text) => text, Err(()) => { found.skipped += 1; continue; } };
                if let Ok(value) = store::jsonc(&text)
                    && let Some(entries) = value.pointer("/contributes/snippets").and_then(|v| v.as_array())
                {
                    for entry in entries {
                        let Some(relative) = entry.get("path").and_then(|v| v.as_str()) else { continue; };
                        let relative = Path::new(relative);
                        // Manifests describe files already discovered; never read arbitrary paths.
                        if relative.is_absolute() || relative.components().any(|c| matches!(c, std::path::Component::ParentDir)) { continue; }
                        let target = path.parent().unwrap_or(Path::new("")).join(relative);
                        let ids: Vec<_> = match entry.get("language") {
                            Some(serde_json::Value::String(id)) => vec![id.as_str()],
                            Some(serde_json::Value::Array(ids)) => ids.iter().filter_map(|v| v.as_str()).collect(),
                            _ => Vec::new(),
                        };
                        let scopes = mappings.entry(target).or_default();
                        for id in ids {
                            if let Some(language) = store::language(id) {
                                if !scopes.contains(&Some(language)) { scopes.push(Some(language)); }
                            } else { found.skipped += 1; }
                        }
                    }
                }
            }
        }
        for path in walker.paths {
            if editor == Editor::Neovim && path.file_name().is_some_and(|s| s == "package.json") { continue; }
            let text = match read(&path) { Ok(text) => text, Err(()) => { found.skipped += 1; continue; } };
            let extension = path.extension().and_then(|s| s.to_str()).unwrap_or("");
            if editor == Editor::Sublime {
                let (items, skipped) = sublime(&path, &text); found.snippets.extend(items); found.skipped += skipped;
                continue;
            }
            let global = extension == "code-snippets";
            let scopes = if global { vec![None] }
                else if let Some(scopes) = mappings.get(&path) { scopes.clone() }
                else {
                    let scope = if editor == Editor::VsCode {
                        path.file_stem().and_then(|s| s.to_str()).and_then(store::language).map(Some).ok_or(())
                    } else { filename_scope(&path) };
                    match scope { Ok(scope) => vec![scope], Err(()) => { found.skipped += 1; continue; } }
                };
            for scope in scopes {
                let parsed = if extension == "snippets" { Ok(vim(&text, scope, path.components().any(|part| part.as_os_str() == "UltiSnips"))) }
                    else { store::parse_json(&text, scope, global, editor.origin()) };
                match parsed {
                    Ok((items, skipped)) => { found.snippets.extend(items); found.skipped += skipped; }
                    Err(_) => found.skipped += 1,
                }
            }
        }
        if !found.snippets.is_empty() || found.skipped > 0 { results.push(found); }
    }
    results
}

fn vim(text: &str, scope: Option<Language>, ulti: bool) -> (Vec<Snippet>, usize) {
    let normalized = text.replace("\r\n", "\n");
    let lines: Vec<_> = normalized.lines().collect();
    let ulti = ulti || lines.iter().any(|line| line.trim() == "endsnippet");
    let (mut snippets, mut skipped, mut i) = (Vec::new(), 0, 0);
    while i < lines.len() {
        let line = lines[i].trim();
        if line == "global" || line.starts_with("global ") {
            skipped += 1; i += 1;
            while i < lines.len() && lines[i].trim() != "endglobal" { i += 1; }
            i += usize::from(i < lines.len()); continue;
        }
        if line.starts_with("priority ") || line == "clearsnippets" || line.starts_with("clearsnippets ") {
            skipped += 1; i += 1; continue;
        }
        let Some(header) = line.strip_prefix("snippet ") else { i += 1; continue; };
        let mut words = header.splitn(2, char::is_whitespace);
        let trigger = words.next().unwrap_or("").trim_matches('"');
        let tail = words.next().unwrap_or("").trim();
        let (description, options) = if ulti && tail.starts_with('"') {
            match tail[1..].rfind('"') {
                Some(end) => (&tail[1..end + 1], tail[end + 2..].trim()),
                None => (tail, ""),
            }
        } else if ulti { ("", tail) } else { (tail, "") };
        i += 1;
        let mut body = Vec::new();
        let mut ended = false;
        while i < lines.len() {
            if ulti {
                if lines[i].trim() == "endsnippet" { ended = true; i += 1; break; }
                if lines[i].trim().starts_with("snippet ") { break; }
                body.push(lines[i]);
            } else {
                let Some(indented) = lines[i].strip_prefix('\t') else { break; };
                body.push(indented);
            }
            i += 1;
        }
        let snippet = Snippet {
            name: if description.is_empty() { trigger.into() } else { description.into() },
            prefixes: vec![trigger.into()], body: body.join("\n"), description: description.into(), scope,
            template: false, origin: Origin::Neovim,
        };
        if options.contains('r') || snippet.body.contains('`') || (ulti && !ended) || store::check(&snippet, &[]).is_err() {
            skipped += 1;
        } else { snippets.push(snippet); }
    }
    (snippets, skipped)
}

fn xml_field(text: &str, tag: &str) -> Option<String> {
    let start = text.find(&format!("<{tag}>"))? + tag.len() + 2;
    let rest = &text[start..];
    if let Some(cdata) = rest.trim_start().strip_prefix("<![CDATA[") {
        let end = cdata.find("]]>")?;
        return Some(cdata[..end].to_owned());
    }
    let end = rest.find(&format!("</{tag}>"))?;
    Some(rest[..end].replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&amp;", "&"))
}

fn sublime(path: &Path, text: &str) -> (Vec<Snippet>, usize) {
    let Some(body) = xml_field(text, "content") else { return (Vec::new(), 1); };
    let Some(trigger) = xml_field(text, "tabTrigger").map(|s| s.trim().to_owned()).filter(|s| !s.is_empty()) else { return (Vec::new(), 1); };
    let description = xml_field(text, "description").unwrap_or_default().trim().to_owned();
    let name = if description.is_empty() { path.file_stem().and_then(|s| s.to_str()).unwrap_or("Snippet").to_owned() } else { description.clone() };
    let scope = xml_field(text, "scope").unwrap_or_default();
    let mut scopes = Vec::new(); let mut skipped = 0;
    if scope.trim().is_empty() { scopes.push(None); }
    else {
        for selector in scope.split(',') {
            let source = selector.trim().split_whitespace().next().unwrap_or("");
            let language = match source {
                "source.c++" | "source.cpp" => Some(Language::Cpp), "source.c" => Some(Language::C),
                "source.python" => Some(Language::Python), "source.go" => Some(Language::Go), "source.java" => Some(Language::Java), _ => None,
            };
            if let Some(language) = language { if !scopes.contains(&Some(language)) { scopes.push(Some(language)); } }
            else { skipped += 1; }
        }
    }
    let mut snippets = Vec::new();
    for scope in scopes {
        let snippet = Snippet { name: name.clone(), prefixes: vec![trigger.clone()], body: body.replace("\r\n", "\n"), description: description.clone(), scope, template: false, origin: Origin::Sublime };
        if store::check(&snippet, &[]).is_ok() { snippets.push(snippet); } else { skipped += 1; }
    }
    (snippets, skipped)
}

pub fn merge(existing: &mut Vec<Snippet>, incoming: Vec<Snippet>) -> Report {
    let mut report = Report { added: 0, unchanged: 0, renamed: 0 };
    for mut snippet in incoming {
        if existing.iter().any(|s| s.scope == snippet.scope && s.prefixes == snippet.prefixes && s.body == snippet.body) {
            report.unchanged += 1; continue;
        }
        if existing.iter().any(|s| s.key() == snippet.key()) {
            snippet.name = store::unique_name(&snippet.name, snippet.scope, existing); report.renamed += 1;
        }
        existing.push(snippet); report.added += 1;
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    fn roots(dir: &Path) -> Roots { Roots { home: dir.join("home"), config: dir.join("config"), data: dir.join("data") } }
    fn put(path: PathBuf, text: &str) { fs::create_dir_all(path.parent().unwrap()).unwrap(); fs::write(path, text).unwrap(); }
    #[test]
    fn snippets_vscode_profiles_and_scopes() {
        let dir = tempfile::tempdir().unwrap(); let roots = roots(dir.path());
        let user = roots.config.join("Code/User");
        put(user.join("snippets/python.json"), r#"{"Print":{"prefix":["print","p"],"body":["print($1)","$0"]}}"#);
        put(user.join("snippets/shared.code-snippets"), r#"{"Loop":{"prefix":"loop","body":"$0","scope":"cpp,go"},"Global":{"prefix":"glob","body":"$0"}}"#);
        put(user.join("profiles/one/snippets/java.json"), r#"{"Main":{"prefix":"main","body":"$0"}}"#);
        let found = scan(&roots); assert_eq!(found.len(), 1); assert_eq!(found[0].files, 3); assert_eq!(found[0].snippets.len(), 5);
        assert_eq!(found[0].skipped, 0); assert!(found[0].snippets.iter().all(|s| s.origin == Origin::VsCode));
        assert!(found[0].snippets.iter().any(|s| s.scope.is_none()));
    }
    #[test]
    fn snippets_neovim_formats_manifest_and_plugins() {
        let dir = tempfile::tempdir().unwrap(); let roots = roots(dir.path()); let nvim = roots.config.join("nvim");
        put(nvim.join("snippets/python.snippets"), "# Comment\nextends all\nsnippet loop A loop\n\tfor ${1:x}:\n\t\t$0\nsnippet print\n\tprint($1)\n");
        put(nvim.join("UltiSnips/cpp.snippets"), "priority 10\nsnippet loop \"Loop\" b\nfor ($1) { $0 }\nendsnippet\nsnippet unsafe \"Unsafe\"\n`!p print(1)`\nendsnippet\nsnippet regex \"Regex\" r\n$0\nendsnippet\nglobal !p\nprint(1)\nendglobal\n");
        put(nvim.join("snips/package.json"), r#"{"contributes":{"snippets":[{"language":"java","path":"./custom.json"}]}}"#);
        put(nvim.join("snips/custom.json"), r#"{"Main":{"prefix":"main","body":"$0"}}"#);
        put(nvim.join("snips/cpp/loops.json"), r#"{"While":{"prefix":"while","body":"while ($1) { $0 }"}}"#);
        put(nvim.join("snips/lazy/python.json"), r#"{"Ignored":{"prefix":"ignore","body":"$0"}}"#);
        let found = scan(&roots); assert_eq!(found.len(), 1); assert_eq!(found[0].snippets.len(), 5); assert_eq!(found[0].skipped, 4);
        assert!(found[0].snippets.iter().any(|s| s.scope == Some(Language::Java)));
        assert!(found[0].snippets.iter().any(|s| s.body == "for ${1:x}:\n\t$0"));
    }
    #[test]
    fn snippets_sublime_cdata_escaping_and_multiscope() {
        let dir = tempfile::tempdir().unwrap(); let roots = roots(dir.path()); let user = roots.config.join("sublime-text/Packages/User");
        put(user.join("loop.sublime-snippet"), "<snippet><content><![CDATA[for ($1) {\n\t$0\n}]]></content><tabTrigger>loop</tabTrigger><scope>source.cpp, source.c</scope><description>Loop</description></snippet>");
        put(user.join("xml.sublime-snippet"), "<snippet><content>&lt;a&gt;&amp;&quot;&apos;&lt;/a&gt;</content><tabTrigger>xml</tabTrigger></snippet>");
        put(user.join("missing.sublime-snippet"), "<snippet><content>$0</content></snippet>");
        let found = scan(&roots); assert_eq!(found[0].snippets.len(), 3); assert_eq!(found[0].skipped, 1);
        assert!(found[0].snippets.iter().any(|s| s.body == "<a>&\"'</a>"));
        assert_eq!(xml_field("<content>  &lt;a&gt;  </content>", "content").unwrap(), "  <a>  ");
        assert_eq!(xml_field("<content><![CDATA[</content>]]></content>", "content").unwrap(), "</content>");
    }
    #[test]
    fn snippets_merge_rescan_and_conflict() {
        let dir = tempfile::tempdir().unwrap(); let roots = roots(dir.path());
        put(roots.config.join("Code/User/snippets/python.json"), r#"{"Print":{"prefix":"p","body":"print($1)"}}"#);
        let mut existing = Vec::new(); let first = merge(&mut existing, scan(&roots).remove(0).snippets); assert_eq!(first.added, 1);
        let second = merge(&mut existing, scan(&roots).remove(0).snippets); assert_eq!(second.added, 0); assert_eq!(second.unchanged, 1);
        let mut conflict = existing[0].clone(); conflict.body = "print($2)".into();
        let renamed = merge(&mut existing, vec![conflict.clone()]); assert_eq!(renamed.renamed, 1); assert_eq!(existing[1].name, "Print 2");
        assert_eq!(merge(&mut existing, vec![conflict]).unchanged, 1);
    }
    #[test]
    fn snippets_ultisnips_unquoted_and_invalid_bodies() {
        let (items, skipped) = vim("snippet good b\n\t$0\nendsnippet\nsnippet rx r\n$0\nendsnippet\nsnippet shell\n`date`\nendsnippet\n", Some(Language::Cpp), true);
        assert_eq!(items.len(), 1); assert_eq!(items[0].name, "good");
        assert_eq!(items[0].body, "\t$0"); assert_eq!(skipped, 2);
        let (items, skipped) = vim("snippet empty\nendsnippet\n", None, true);
        assert!(items.is_empty()); assert_eq!(skipped, 1);
    }
    #[test]
    fn snippets_file_and_depth_limits() {
        let dir = tempfile::tempdir().unwrap(); let roots = roots(dir.path());
        let snippets = roots.config.join("nvim/snippets");
        for i in 0..MAX_FILES + 1 {
            put(snippets.join(format!("{i:04}.code-snippets")), r#"{"Global":{"prefix":"global","body":"$0"}}"#);
        }
        assert_eq!(scan(&roots)[0].files, MAX_FILES);
        let dir = tempfile::tempdir().unwrap(); let roots = self::roots(dir.path());
        put(roots.config.join("nvim/snippets/a/b/c/python.json"), r#"{"Deep":{"prefix":"deep","body":"$0"}}"#);
        put(roots.config.join("nvim/snippets/a/b/c/d/python.json"), r#"{"Too deep":{"prefix":"deep","body":"$0"}}"#);
        let found = scan(&roots); assert_eq!(found[0].files, 1);
    }
    #[test]
    fn snippets_scan_bounds_and_unknown_language() {
        let dir = tempfile::tempdir().unwrap(); let roots = roots(dir.path()); let user = roots.config.join("Code/User/snippets");
        put(user.join("rust.json"), r#"{"Rust":{"prefix":"r","body":"$0"}}"#);
        put(user.join("cpp.json"), &" ".repeat(MAX_BYTES as usize + 1));
        let found = scan(&roots); assert_eq!(found[0].skipped, 2); assert!(found[0].snippets.is_empty());
        #[cfg(unix)] {
            std::os::unix::fs::symlink(&user, user.join("loop")).unwrap();
            assert_eq!(scan(&roots)[0].files, 2);
        }
    }
}
