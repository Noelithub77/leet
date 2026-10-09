//! VS Code / TextMate snippet bodies: parsing, validation, and expansion.
//!
//! Supports `$1`, `${1:placeholder}` (nested), `${1|a,b|}` choices, `$0`, mirrors (a repeated
//! index), variables with defaults, and regex transforms on variables and mirrors. Malformed
//! constructs stay literal text, as in VS Code, so imported bodies never fail to insert.
use std::{ops::Range, path::PathBuf, time::{SystemTime, UNIX_EPOCH}};

use crate::language::Language;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Node {
    Text(String),
    Stop { index: u32, children: Vec<Node>, choices: Vec<String>, transform: Option<Transform> },
    Variable { name: String, children: Vec<Node>, transform: Option<Transform> },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Transform { pub regex: String, pub format: Vec<Format>, pub flags: String }

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Format {
    Text(String),
    Group { index: usize, case: Option<String>, when_set: Option<String>, otherwise: Option<String> },
}

#[derive(Debug)]
struct Parsed { nodes: Vec<Node>, unclosed: Option<usize> }

struct Parser { chars: Vec<char>, pos: usize, unclosed: Option<usize>, depth: usize }

impl Parser {
    fn peek(&self) -> Option<char> { self.chars.get(self.pos).copied() }
    fn eat(&mut self, ch: char) -> bool { if self.peek() == Some(ch) { self.pos += 1; true } else { false } }
    fn int(&mut self) -> Option<u32> {
        let start = self.pos;
        while self.peek().is_some_and(|ch| ch.is_ascii_digit()) { self.pos += 1; }
        if start == self.pos { return None; }
        self.chars[start..self.pos].iter().collect::<String>().parse().ok()
    }
    fn ident(&mut self) -> Option<String> {
        if !self.peek().is_some_and(|ch| ch.is_ascii_alphabetic() || ch == '_') { return None; }
        let start = self.pos;
        while self.peek().is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_') { self.pos += 1; }
        Some(self.chars[start..self.pos].iter().collect())
    }

    fn any(&mut self, nested: bool) -> Vec<Node> {
        let mut nodes = Vec::new(); let mut text = String::new();
        while let Some(ch) = self.peek() {
            match ch {
                '$' => match self.dollar() {
                    Some(node) => { if !text.is_empty() { nodes.push(Node::Text(std::mem::take(&mut text))); } nodes.push(node); }
                    None => { text.push('$'); self.pos += 1; }
                },
                '\\' if matches!(self.chars.get(self.pos + 1), Some('$' | '}' | '\\')) => { text.push(self.chars[self.pos + 1]); self.pos += 2; }
                '}' if nested => break,
                _ => { text.push(ch); self.pos += 1; }
            }
        }
        if !text.is_empty() { nodes.push(Node::Text(text)); }
        nodes
    }

    fn dollar(&mut self) -> Option<Node> {
        let start = self.pos;
        if self.depth >= 64 { self.unclosed.get_or_insert(start); return None; }
        self.pos += 1; self.depth += 1;
        let node = self.dollar_body(start);
        self.depth -= 1;
        if node.is_none() { self.pos = start; }
        node
    }

    fn dollar_body(&mut self, start: usize) -> Option<Node> {
        if let Some(index) = self.int() { return Some(Node::Stop { index, children: Vec::new(), choices: Vec::new(), transform: None }); }
        if let Some(name) = self.ident() { return Some(Node::Variable { name, children: Vec::new(), transform: None }); }
        if !self.eat('{') { return None; }
        let fail = |this: &mut Self| { this.unclosed.get_or_insert(start); None };
        if let Some(index) = self.int() {
            if self.eat('}') { return Some(Node::Stop { index, children: Vec::new(), choices: Vec::new(), transform: None }); }
            if self.eat(':') {
                let children = self.any(true);
                return if self.eat('}') { Some(Node::Stop { index, children, choices: Vec::new(), transform: None }) } else { fail(self) };
            }
            if self.eat('|') {
                let Some(choices) = self.choices() else { return fail(self) };
                return Some(Node::Stop { index, children: Vec::new(), choices, transform: None });
            }
            if self.eat('/') {
                let Some(transform) = self.transform() else { return fail(self) };
                return if self.eat('}') { Some(Node::Stop { index, children: Vec::new(), choices: Vec::new(), transform: Some(transform) }) } else { fail(self) };
            }
            return fail(self);
        }
        if let Some(name) = self.ident() {
            if self.eat('}') { return Some(Node::Variable { name, children: Vec::new(), transform: None }); }
            if self.eat(':') {
                let children = self.any(true);
                return if self.eat('}') { Some(Node::Variable { name, children, transform: None }) } else { fail(self) };
            }
            if self.eat('/') {
                let Some(transform) = self.transform() else { return fail(self) };
                return if self.eat('}') { Some(Node::Variable { name, children: Vec::new(), transform: Some(transform) }) } else { fail(self) };
            }
            return fail(self);
        }
        None
    }

    fn choices(&mut self) -> Option<Vec<String>> {
        let mut choices = Vec::new(); let mut current = String::new();
        while let Some(ch) = self.peek() {
            self.pos += 1;
            match ch {
                '\\' if matches!(self.peek(), Some(',' | '|' | '\\' | '$' | '}')) => { current.push(self.chars[self.pos]); self.pos += 1; }
                ',' => choices.push(std::mem::take(&mut current)),
                '|' if self.eat('}') => { choices.push(current); return Some(choices); }
                _ => current.push(ch),
            }
        }
        None
    }

    /// Reads `regex/format/flags` after the opening `/`, leaving the closing `}`.
    fn transform(&mut self) -> Option<Transform> {
        let mut regex = String::new();
        loop {
            let ch = self.peek()?; self.pos += 1;
            match ch {
                '\\' if self.peek() == Some('/') => { regex.push('/'); self.pos += 1; }
                '\\' => { regex.push('\\'); if let Some(next) = self.peek() { regex.push(next); self.pos += 1; } }
                '/' => break,
                _ => regex.push(ch),
            }
        }
        let mut format = Vec::new(); let mut text = String::new();
        loop {
            let ch = self.peek()?;
            match ch {
                '/' => { self.pos += 1; break; }
                '\\' if matches!(self.chars.get(self.pos + 1), Some('/' | '$' | '\\')) => { text.push(self.chars[self.pos + 1]); self.pos += 2; }
                '$' => {
                    let start = self.pos; self.pos += 1;
                    match self.format_group() {
                        Some(group) => { if !text.is_empty() { format.push(Format::Text(std::mem::take(&mut text))); } format.push(group); }
                        None => { self.pos = start + 1; text.push('$'); }
                    }
                }
                _ => { text.push(ch); self.pos += 1; }
            }
        }
        if !text.is_empty() { format.push(Format::Text(text)); }
        let mut flags = String::new();
        while let Some(ch) = self.peek().filter(|ch| ch.is_ascii_alphabetic()) { flags.push(ch); self.pos += 1; }
        if self.peek() != Some('}') { return None; }
        Some(Transform { regex, format, flags })
    }

    fn format_group(&mut self) -> Option<Format> {
        let group = |index: u32| Format::Group { index: index as usize, case: None, when_set: None, otherwise: None };
        if let Some(index) = self.int() { return Some(group(index)); }
        if !self.eat('{') { return None; }
        let index = self.int()? as usize;
        if self.eat('}') { return Some(Format::Group { index, case: None, when_set: None, otherwise: None }); }
        if !self.eat(':') { return None; }
        let until = |this: &mut Self, stops: &[char]| -> Option<String> {
            let mut out = String::new();
            loop {
                let ch = this.peek()?;
                if stops.contains(&ch) { return Some(out); }
                this.pos += 1;
                if ch == '\\' && let Some(next) = this.peek() { out.push(next); this.pos += 1; } else { out.push(ch); }
            }
        };
        let result = if self.eat('/') {
            let case = self.ident()?;
            Format::Group { index, case: Some(case), when_set: None, otherwise: None }
        } else if self.eat('+') {
            Format::Group { index, case: None, when_set: Some(until(self, &['}'])?), otherwise: None }
        } else if self.eat('?') {
            let when_set = until(self, &[':'])?; self.pos += 1;
            Format::Group { index, case: None, when_set: Some(when_set), otherwise: Some(until(self, &['}'])?) }
        } else {
            self.eat('-');
            Format::Group { index, case: None, when_set: None, otherwise: Some(until(self, &['}'])?) }
        };
        self.eat('}').then_some(result)
    }
}

fn parse_full(body: &str) -> Parsed {
    let mut parser = Parser { chars: body.chars().collect(), pos: 0, unclosed: None, depth: 0 };
    let mut nodes = parser.any(false);
    // A stray top-level `}` ends `any(false)` never, but keep the loop total.
    while parser.pos < parser.chars.len() { parser.pos += 1; nodes.push(Node::Text(parser.chars[parser.pos - 1].to_string())); }
    Parsed { nodes, unclosed: parser.unclosed }
}

/// Parses leniently: malformed constructs become text.
pub fn parse(body: &str) -> Vec<Node> { parse_full(body).nodes }

/// Checks that `body` is a usable snippet; the error names the problem and its character offset.
pub fn validate(body: &str) -> Result<(), String> {
    if body.len() > 256 * 1024 { return Err("Body exceeds 256 KiB".into()); }
    if body.trim().is_empty() { return Err("Body is empty".into()); }
    let parsed = parse_full(body);
    if let Some(at) = parsed.unclosed { return Err(format!("Unclosed ${{ at character {}", at + 1)); }
    fn transforms(nodes: &[Node]) -> Result<(), String> {
        for node in nodes {
            match node {
                Node::Stop { children, transform, .. } | Node::Variable { children, transform, .. } => {
                    if let Some(transform) = transform { transform.compile().map_err(|error| format!("Invalid transform: {error}"))?; }
                    transforms(children)?;
                }
                Node::Text(_) => {}
            }
        }
        Ok(())
    }
    transforms(&parsed.nodes)?;
    if preview(body).truncated { return Err("Expanded body exceeds the snippet limits".into()); }
    Ok(())
}

impl Transform {
    fn compile(&self) -> Result<regex::Regex, regex::Error> {
        regex::RegexBuilder::new(&self.regex)
            .case_insensitive(self.flags.contains('i')).multi_line(self.flags.contains('m')).dot_matches_new_line(self.flags.contains('s'))
            .size_limit(1 << 20).build()
    }

    pub fn apply(&self, text: &str) -> String {
        let Ok(regex) = self.compile() else { return text.to_owned() };
        let replace = |captures: &regex::Captures| -> String {
            let mut out = String::new();
            for item in &self.format {
                match item {
                    Format::Text(text) => out.push_str(text),
                    Format::Group { index, case, when_set, otherwise } => {
                        let value = captures.get(*index).map_or("", |m| m.as_str());
                        if let Some(case) = case { out.push_str(&change_case(value, case)); }
                        else if !value.is_empty() { out.push_str(when_set.as_deref().unwrap_or(value)); }
                        else if let Some(otherwise) = otherwise { out.push_str(otherwise); }
                    }
                }
            }
            out
        };
        if self.flags.contains('g') { regex.replace_all(text, replace).into_owned() } else { regex.replace(text, replace).into_owned() }
    }
}

fn change_case(value: &str, case: &str) -> String {
    let words = || value.split(|ch: char| !ch.is_alphanumeric()).filter(|word| !word.is_empty());
    let capital = |word: &str| { let mut chars = word.chars(); chars.next().map(|first| first.to_uppercase().chain(chars.flat_map(char::to_lowercase)).collect()).unwrap_or_default() };
    match case {
        "upcase" => value.to_uppercase(),
        "downcase" => value.to_lowercase(),
        "capitalize" => { let mut chars = value.chars(); chars.next().map(|first| first.to_uppercase().chain(chars).collect()).unwrap_or_default() }
        "pascalcase" => words().map(capital).collect(),
        "camelcase" => words().enumerate().map(|(index, word)| if index == 0 { word.to_lowercase() } else { capital(word) }).collect(),
        _ => value.to_owned(),
    }
}

/// Editor state that variables and indentation read from.
#[derive(Clone, Debug, Default)]
pub struct Context {
    pub selected: String,
    pub clipboard: String,
    pub path: Option<PathBuf>,
    pub workspace: Option<PathBuf>,
    pub line: String,
    pub word: String,
    pub line_index: usize,
    /// Leading whitespace of the line being expanded into; continued lines inherit it.
    pub indent: String,
    /// One indentation level; leading tabs in the body become this. Empty keeps tabs.
    pub unit: String,
    pub language: Option<Language>,
    /// Defaults to the current time.
    pub now: Option<SystemTime>,
    /// Seeds `RANDOM`, `RANDOM_HEX`, and `UUID`; zero uses the clock.
    pub seed: u64,
}

impl Context {
    fn variable(&self, name: &str, random: &mut u64) -> Option<String> {
        let file = self.path.as_ref();
        let (year, month, day, weekday, hour, minute, second, unix) = civil(self.now.unwrap_or_else(SystemTime::now));
        const MONTHS: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
        const DAYS: [&str; 7] = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];
        let mut next = || { *random ^= *random << 13; *random ^= *random >> 7; *random ^= *random << 17; *random };
        let comment = if self.language == Some(Language::Python) { ("#", "\"\"\"", "\"\"\"") } else { ("//", "/*", "*/") };
        Some(match name {
            "TM_SELECTED_TEXT" => self.selected.clone(),
            "TM_CURRENT_LINE" => self.line.clone(),
            "TM_CURRENT_WORD" => self.word.clone(),
            "TM_LINE_INDEX" => self.line_index.to_string(),
            "TM_LINE_NUMBER" => (self.line_index + 1).to_string(),
            "TM_FILENAME" => file.and_then(|path| path.file_name()).map(|name| name.to_string_lossy().into_owned()).unwrap_or_default(),
            "TM_FILENAME_BASE" => file.and_then(|path| path.file_stem()).map(|name| name.to_string_lossy().into_owned()).unwrap_or_default(),
            "TM_DIRECTORY" => file.and_then(|path| path.parent()).map(|path| path.display().to_string()).unwrap_or_default(),
            "TM_FILEPATH" => file.map(|path| path.display().to_string()).unwrap_or_default(),
            "RELATIVE_FILEPATH" => match (file, &self.workspace) {
                (Some(path), Some(root)) => path.strip_prefix(root).unwrap_or(path).display().to_string(),
                (Some(path), None) => path.display().to_string(),
                _ => String::new(),
            },
            "WORKSPACE_NAME" => self.workspace.as_ref().and_then(|root| root.file_name()).map(|name| name.to_string_lossy().into_owned()).unwrap_or_default(),
            "WORKSPACE_FOLDER" => self.workspace.as_ref().map(|root| root.display().to_string()).unwrap_or_default(),
            "CLIPBOARD" => self.clipboard.clone(),
            "CURRENT_YEAR" => year.to_string(),
            "CURRENT_YEAR_SHORT" => format!("{:02}", year % 100),
            "CURRENT_MONTH" => format!("{month:02}"),
            "CURRENT_MONTH_NAME" => MONTHS[month as usize - 1].into(),
            "CURRENT_MONTH_NAME_SHORT" => MONTHS[month as usize - 1][..3].into(),
            "CURRENT_DATE" => format!("{day:02}"),
            "CURRENT_DAY_NAME" => DAYS[weekday].into(),
            "CURRENT_DAY_NAME_SHORT" => DAYS[weekday][..3].into(),
            "CURRENT_HOUR" => format!("{hour:02}"),
            "CURRENT_MINUTE" => format!("{minute:02}"),
            "CURRENT_SECOND" => format!("{second:02}"),
            "CURRENT_SECONDS_UNIX" => unix.to_string(),
            "CURRENT_TIMEZONE_OFFSET" => "Z".into(),
            "RANDOM" => format!("{:06}", next() % 1_000_000),
            "RANDOM_HEX" => format!("{:06x}", next() & 0xff_ffff),
            "UUID" => { let (a, b) = (next(), next()); format!("{:08x}-{:04x}-4{:03x}-{:04x}-{:012x}", a >> 32, (a >> 16) & 0xffff, a & 0xfff, (b >> 48) & 0x3fff | 0x8000, b & 0xffff_ffff_ffff) }
            "LINE_COMMENT" => comment.0.into(),
            "BLOCK_COMMENT_START" => comment.1.into(),
            "BLOCK_COMMENT_END" => comment.2.into(),
            _ => return None,
        })
    }
}

/// UTC calendar fields: year, month, day, weekday (0 = Sunday), hour, minute, second, unix seconds.
fn civil(time: SystemTime) -> (i64, u32, u32, usize, u64, u64, u64, u64) {
    let unix = time.duration_since(UNIX_EPOCH).map_or(0, |duration| duration.as_secs());
    let days = (unix / 86_400) as i64; let rest = unix % 86_400;
    // Howard Hinnant's days-to-civil algorithm.
    let z = days + 719_468; let era = z.div_euclid(146_097); let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32; let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day, (days + 4).rem_euclid(7) as usize, rest / 3600, rest / 60 % 60, rest % 60, unix)
}

/// A navigable stop. `ranges[primary]` is edited; the others mirror it, optionally transformed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stop {
    pub index: u32,
    pub ranges: Vec<Range<usize>>,
    pub transforms: Vec<Option<Transform>>,
    pub primary: usize,
    pub choices: Vec<String>,
}

impl Stop {
    pub fn range(&self) -> Range<usize> { self.ranges[self.primary].clone() }
}

/// Inserted text and its stops in navigation order (`$1`, `$2`, …, then `$0`). Byte offsets.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Expansion { pub text: String, pub stops: Vec<Stop>, pub truncated: bool }

struct Emitter<'a> {
    ctx: &'a Context,
    out: String,
    line_start: bool,
    occurrences: Vec<(u32, Range<usize>, Option<Transform>)>,
    defaults: std::collections::HashMap<u32, Vec<Node>>,
    choices: std::collections::HashMap<u32, Vec<String>>,
    random: u64,
    visiting: Vec<u32>,
    remaining: usize,
    truncated: bool,
}

impl Emitter<'_> {
    /// The default text of stop `index`, used as the source of its transformed mirrors.
    fn plain(&mut self, index: u32) -> String {
        let flat = Context { indent: String::new(), ..self.ctx.clone() };
        let mut scratch = Emitter { ctx: &flat, out: String::new(), line_start: false, occurrences: Vec::new(), defaults: self.defaults.clone(), choices: self.choices.clone(), random: self.random, visiting: self.visiting.clone(), remaining: self.remaining, truncated: false };
        if let Some(choices) = self.choices.get(&index) { return choices[0].clone(); }
        if let Some(default) = self.defaults.get(&index).cloned() { scratch.render(&default, false); }
        self.remaining = scratch.remaining;
        self.truncated |= scratch.truncated;
        scratch.out
    }

    fn text(&mut self, text: &str) {
        for ch in text.chars() {
            if self.out.len() + ch.len_utf8() + self.ctx.indent.len() + self.ctx.unit.len() > 1024 * 1024 { self.truncated = true; break; }
            if ch == '\r' { continue; }
            if self.line_start && ch == '\t' && !self.ctx.unit.is_empty() { self.out.push_str(&self.ctx.unit); continue; }
            self.line_start = false;
            self.out.push(ch);
            if ch == '\n' { self.out.push_str(&self.ctx.indent); self.line_start = true; }
        }
    }

    fn render(&mut self, nodes: &[Node], record: bool) {
        for node in nodes {
            if self.remaining == 0 { self.truncated = true; break; }
            self.remaining -= 1;
            match node {
                Node::Text(text) => self.text(text),
                Node::Stop { index, children, choices, transform } => {
                    let start = self.out.len();
                    if self.visiting.contains(index) { continue; }
                    self.visiting.push(*index);
                    if let Some(transform) = transform {
                        let source = self.plain(*index);
                        self.text(&transform.apply(&source));
                    } else if !choices.is_empty() {
                        self.text(&choices[0]);
                    } else if !children.is_empty() {
                        self.render(children, record);
                    } else if let Some(choices) = self.choices.get(index).cloned() {
                        self.text(&choices[0]);
                    } else if let Some(default) = self.defaults.get(index).cloned() {
                        self.render(&default, false);
                    }
                    self.visiting.pop();
                    if record { self.occurrences.push((*index, start..self.out.len(), transform.clone())); }
                }
                Node::Variable { name, children, transform } => {
                    match self.ctx.variable(name, &mut self.random) {
                        Some(value) => {
                            let value = if value.is_empty() && !children.is_empty() { None } else { Some(value) };
                            match (value, transform) {
                                (Some(value), Some(transform)) => self.text(&transform.apply(&value)),
                                (Some(value), None) => self.text(&value),
                                (None, _) => self.render(children, record),
                            }
                        }
                        None => self.render(children, record),
                    }
                }
            }
        }
    }
}

fn collect(nodes: &[Node], defaults: &mut std::collections::HashMap<u32, Vec<Node>>, choices: &mut std::collections::HashMap<u32, Vec<String>>, max: &mut u32) {
    for node in nodes {
        match node {
            Node::Stop { index, children, choices: options, .. } => {
                *max = (*max).max(*index);
                if !children.is_empty() { defaults.entry(*index).or_insert_with(|| children.clone()); }
                if !options.is_empty() { choices.entry(*index).or_insert_with(|| options.clone()); }
                collect(children, defaults, choices, max);
            }
            Node::Variable { children, .. } => collect(children, defaults, choices, max),
            Node::Text(_) => {}
        }
    }
}

/// Unknown variables become stops named after themselves, as in VS Code.
fn resolve_unknown(nodes: Vec<Node>, ctx: &Context, next: &mut u32) -> Vec<Node> {
    nodes.into_iter().map(|node| match node {
        Node::Variable { name, children, .. } if ctx.variable(&name, &mut 1).is_none() => {
            *next = next.saturating_add(1);
            let children = if children.is_empty() { vec![Node::Text(name)] } else { resolve_unknown(children, ctx, next) };
            Node::Stop { index: *next, children, choices: Vec::new(), transform: None }
        }
        Node::Variable { name, children, transform } => Node::Variable { name, children: resolve_unknown(children, ctx, next), transform },
        Node::Stop { index, children, choices, transform } => Node::Stop { index, children: resolve_unknown(children, ctx, next), choices, transform },
        text => text,
    }).collect()
}

/// Expands `body` for insertion at a cursor described by `ctx`.
pub fn expand(body: &str, ctx: &Context) -> Expansion {
    let nodes = parse(body);
    let (mut defaults, mut choices, mut max) = (Default::default(), Default::default(), 0);
    collect(&nodes, &mut defaults, &mut choices, &mut max);
    let nodes = resolve_unknown(nodes, ctx, &mut max);
    let (mut defaults2, mut choices2, mut unused) = (Default::default(), Default::default(), 0);
    collect(&nodes, &mut defaults2, &mut choices2, &mut unused);
    defaults.extend(defaults2); choices.extend(choices2);
    let seed = if ctx.seed != 0 { ctx.seed } else { SystemTime::now().duration_since(UNIX_EPOCH).map_or(0x9e37_79b9, |d| d.as_nanos() as u64) | 1 };
    let mut emitter = Emitter { ctx, out: String::new(), line_start: false, occurrences: Vec::new(), defaults, choices, random: seed, visiting: Vec::new(), remaining: 100_000, truncated: false };
    emitter.render(&nodes, true);

    let truncated = emitter.truncated;
    let (text, occurrences) = (emitter.out, emitter.occurrences);

    let mut indices: Vec<u32> = occurrences.iter().map(|(index, _, _)| *index).filter(|&index| index != 0).collect();
    indices.sort_unstable(); indices.dedup();
    let mut stops: Vec<Stop> = indices.into_iter().chain([0]).filter_map(|index| {
        let mut entries: Vec<_> = occurrences.iter().filter(|(other, _, _)| *other == index).cloned().collect();
        if entries.is_empty() { return (index == 0).then(|| Stop { index: 0, ranges: vec![text.len()..text.len()], transforms: vec![None], primary: 0, choices: Vec::new() }); }
        entries.sort_by_key(|(_, range, _)| (range.start, range.end));
        let primary = entries.iter().position(|(_, _, transform)| transform.is_none()).unwrap_or(0);
        Some(Stop { index, choices: choices_for(&nodes, index), primary, ranges: entries.iter().map(|(_, range, _)| range.clone()).collect(), transforms: entries.into_iter().map(|(_, _, transform)| transform).collect() })
    }).collect();
    if let Some(last) = stops.last_mut() && last.index == 0 { last.transforms.iter_mut().for_each(|transform| *transform = None); }
    Expansion { text, stops, truncated }
}

fn choices_for(nodes: &[Node], index: u32) -> Vec<String> {
    for node in nodes {
        match node {
            Node::Stop { index: other, choices, .. } if *other == index && !choices.is_empty() => return choices.clone(),
            Node::Stop { children, .. } | Node::Variable { children, .. } => { let found = choices_for(children, index); if !found.is_empty() { return found; } }
            Node::Text(_) => {}
        }
    }
    Vec::new()
}

/// The body with every stop at its default and no indentation context, for previews.
pub fn preview(body: &str) -> Expansion { expand(body, &Context { unit: "    ".into(), seed: 1, ..Context::default() }) }

/// Places the one final cursor marker, preserving escaped literal markers.
pub fn place_cursor(body: &str, position: usize) -> (String, usize) {
    let mut out=String::new();let mut at=position.min(body.len());let mut i=0;
    while i<body.len() {
        let rest=&body[i..];
        if rest.starts_with('\\') {
            let count=rest.chars().take(2).map(char::len_utf8).sum::<usize>();out.push_str(&rest[..count]);i+=count;continue;
        }
        let marker=if rest.starts_with("$0") && !rest.as_bytes().get(2).is_some_and(u8::is_ascii_digit){2}else if rest.starts_with("${0}"){4}else{0};
        if marker>0 {if i<position{at=at.saturating_sub(marker.min(position-i));}i+=marker;}
        else {let c=rest.chars().next().unwrap();out.push(c);i+=c.len_utf8();}
    }
    at=at.min(out.len());while !out.is_char_boundary(at){at-=1;}out.insert_str(at,"$0");(out,at)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snippets_cursor_drag_preserves_literals_and_moves_only_final_marker() {
        assert_eq!(place_cursor("a $0 b",6),("a  b$0".into(),4));
        assert_eq!(place_cursor("\\$0 $01 ${0}",0),("$0\\$0 $01 ".into(),0));
    }
    #[test]
    fn snippets_recursive_defaults_are_bounded() {
        assert_eq!(preview("${1:$1}").text,"");
        assert!(validate(&format!("{}x{}","${1:".repeat(70),"}".repeat(70))).is_err());
    }
    fn ctx() -> Context { Context { unit: "    ".into(), seed: 7, now: Some(UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000)), ..Context::default() } }

    #[test]
    fn repeated_defaults_cannot_amplify_without_bound() {
        let mut text = "${1:x}".to_owned();
        for index in 2..28 { text.push_str(&format!("${{{index}:${}${}}}", index - 1, index - 1)); }
        assert!(preview(&text).truncated);
        assert!(validate(&text).is_err());
    }

    #[test]
    fn snippets_body_stops_mirrors_and_final_cursor() {
        let expansion = expand("for (int ${1:i} = 0; $1 < ${2:n}; $1++) {\n\t$0\n}", &ctx());
        assert_eq!(expansion.text, "for (int i = 0; i < n; i++) {\n    \n}");
        let indices: Vec<u32> = expansion.stops.iter().map(|stop| stop.index).collect();
        assert_eq!(indices, [1, 2, 0]);
        assert_eq!(expansion.stops[0].ranges.len(), 3);
        assert_eq!(&expansion.text[expansion.stops[0].range()], "i");
        assert_eq!(&expansion.text[expansion.stops[1].range()], "n");
        let end = expansion.text.rfind("\n}").unwrap();
        assert_eq!(expansion.stops[2].range(), end..end);
    }

    #[test]
    fn snippets_body_implicit_final_stop_and_indentation() {
        let expansion = expand("if ($1) {\n\treturn;\n}", &Context { indent: "  ".into(), ..ctx() });
        assert_eq!(expansion.text, "if () {\n      return;\n  }");
        assert_eq!(expansion.stops.last().unwrap().range(), expansion.text.len()..expansion.text.len());
    }

    #[test]
    fn snippets_body_nested_placeholders_choices_and_escapes() {
        let expansion = expand("${1:vector<${2:int}>} \\$x ${3|YES,NO|} \\}", &ctx());
        assert_eq!(expansion.text, "vector<int> $x YES }");
        assert_eq!(&expansion.text[expansion.stops[0].range()], "vector<int>");
        assert_eq!(&expansion.text[expansion.stops[1].range()], "int");
        assert_eq!(expansion.stops[2].choices, ["YES", "NO"]);
    }

    #[test]
    fn snippets_body_variables_defaults_unknown_and_transforms() {
        let context = Context { selected: "solve();".into(), path: Some("/w/two_sum.cpp".into()), ..ctx() };
        let expansion = expand("${TM_SELECTED_TEXT} ${TM_FILENAME_BASE/(.*)/${1:/upcase}/} ${CLIPBOARD:none} $CURRENT_YEAR $MYSTERY", &context);
        assert_eq!(expansion.text, "solve(); TWO_SUM none 2023 MYSTERY");
        assert_eq!(&expansion.text[expansion.stops[0].range()], "MYSTERY");
    }

    #[test]
    fn snippets_body_transformed_mirror_follows_primary() {
        let expansion = expand("${1:name} ${1/(.)(.*)/${1:/upcase}$2/}", &ctx());
        assert_eq!(expansion.text, "name Name");
        let stop = &expansion.stops[0];
        assert_eq!(stop.ranges, [0..4, 5..9]);
        assert!(stop.transforms[1].is_some());
        assert_eq!(stop.primary, 0);
    }

    #[test]
    fn snippets_body_malformed_constructs_stay_text_but_fail_validation() {
        assert_eq!(expand("cost $ and ${", &ctx()).text, "cost $ and ${");
        assert_eq!(expand("${1:open", &ctx()).text, "${1:open");
        assert!(validate("${1:open").unwrap_err().contains("Unclosed"));
        assert!(validate("  ").is_err());
        assert!(validate("${1/(/x/}").unwrap_err().contains("transform"));
        assert!(validate("int a = $1; // done $0").is_ok());
    }

    #[test]
    fn snippets_body_conditional_formats_and_case_changes() {
        let transform = |body: &str| expand(body, &Context { word: "max_flow value".into(), ..ctx() }).text;
        assert_eq!(transform("${TM_CURRENT_WORD/(\\w+)/${1:/pascalcase}/g}"), "MaxFlow Value");
        assert_eq!(transform("${TM_CURRENT_WORD/(max)?.*/${1:+yes}${1:?a:b}/}"), "yesa");
        assert_eq!(transform("${TM_CURRENT_WORD/(min)?.*/${1:-none}/}"), "none");
        assert_eq!(change_case("two sum", "camelcase"), "twoSum");
    }

    #[test]
    fn snippets_body_civil_dates_and_unicode_offsets() {
        assert_eq!(civil(UNIX_EPOCH + std::time::Duration::from_secs(951_782_400)).0..=civil(UNIX_EPOCH + std::time::Duration::from_secs(951_782_400)).0, 2000..=2000);
        let (year, month, day, weekday, ..) = civil(UNIX_EPOCH + std::time::Duration::from_secs(951_782_400));
        assert_eq!((year, month, day, weekday), (2000, 2, 29, 2));
        let expansion = expand("λ ${1:é} $0", &ctx());
        assert_eq!(&expansion.text[expansion.stops[0].range()], "é");
    }
}
