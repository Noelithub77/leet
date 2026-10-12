//! Notes kept as comments in a solution: `#!` lines from the AI, `#?` questions, and
//! ordinary comments. C-family languages use `//!`, `//?` and `//`.
use crate::assist::{Answer, Severity};
use crate::language::Language;
use std::ops::Range;

const TEXT_LIMIT: usize = 160;

/// How notes appear in the editor.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum View {
    /// `#!` lines hidden, their code highlighted, details on hover.
    #[default]
    Rich,
    /// Plain comments.
    Raw,
    /// `#!` and `#?` lines hidden.
    Off,
}

impl View {
    pub const ALL: [Self; 3] = [Self::Rich, Self::Raw, Self::Off];
    pub fn label(self) -> &'static str { match self { Self::Rich => "Rich", Self::Raw => "Raw", Self::Off => "Off" } }
    pub fn next(self) -> Self { match self { Self::Rich => Self::Raw, Self::Raw => Self::Off, Self::Off => Self::Rich } }

    /// Whether this view leaves a line out of the editor.
    pub fn hides(self, line: &str) -> bool {
        match self { Self::Rich => is_ai_line(line), Self::Raw => false, Self::Off => is_note_line(line) }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    Bug,
    Risk,
    Stuck,
    Slow,
    Hint,
    Answer,
    /// A `#?` question for the AI.
    Ask,
    /// An ordinary full-line comment.
    Comment,
}

impl Kind {
    const TAGGED: [Self; 6] = [Self::Bug, Self::Risk, Self::Stuck, Self::Slow, Self::Hint, Self::Answer];

    pub fn tag(self) -> &'static str {
        match self {
            Self::Bug => "bug", Self::Risk => "risk", Self::Stuck => "stuck", Self::Slow => "slow",
            Self::Hint => "hint", Self::Answer => "answer", Self::Ask => "ask", Self::Comment => "note",
        }
    }

    /// Written by the AI, as opposed to the user.
    pub fn ai(self) -> bool { !matches!(self, Self::Ask | Self::Comment) }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Note {
    /// Zero-based line of the comment.
    pub line: usize,
    pub kind: Kind,
    /// `(step, of)` for a reasoning trail.
    pub step: Option<(u32, u32)>,
    pub text: String,
    /// The first backtick span of the text, highlighted on the target line.
    pub snippet: Option<String>,
    /// Byte range of the comment, from its marker to the end of the line.
    pub comment: Range<usize>,
    /// Byte range of the marker, such as `#! bug:` or `#?`.
    pub marker: Range<usize>,
    /// The next code line the note is about.
    pub target: Option<usize>,
    /// Byte range of the anchored code on the target line. `None` when the snippet is gone.
    pub anchor: Option<Range<usize>>,
}

impl Note {
    /// The quoted code no longer appears on the target line.
    pub fn outdated(&self) -> bool { self.snippet.is_some() && self.anchor.is_none() }
}

pub fn comment_prefix(language: Language) -> &'static str {
    if language == Language::Python { "#" } else { "//" }
}

/// Whether a line is a `#!` or `#?` note in any supported language. Shebangs are not notes.
pub fn is_note_line(line: &str) -> bool {
    let line = line.trim_start();
    let rest = line.strip_prefix('#').or_else(|| line.strip_prefix("//"));
    rest.is_some_and(|rest| (rest.starts_with('!') && !rest.starts_with("!/")) || rest.starts_with('?'))
}

/// Whether a line is a `#!` note from the AI.
pub fn is_ai_line(line: &str) -> bool { is_note_line(line) && !is_question_line(line) }

fn is_question_line(line: &str) -> bool {
    let line = line.trim_start();
    line.strip_prefix('#').or_else(|| line.strip_prefix("//")).is_some_and(|rest| rest.starts_with('?'))
}

/// Byte ranges of each line, without the `\n`.
fn line_ranges(code: &str) -> Vec<Range<usize>> {
    let mut start = 0;
    code.split('\n').map(|line| { let range = start..start + line.len(); start = range.end + 1; range }).collect()
}

fn is_comment_line(line: &str, prefix: &str) -> bool { line.trim_start().starts_with(prefix) }

pub fn parse(code: &str, language: Language) -> Vec<Note> {
    let prefix = comment_prefix(language);
    let ranges = line_ranges(code);
    let line = |index: usize| &code[ranges[index].clone()];
    let target = |from: usize| (from + 1..ranges.len()).find(|&i| !line(i).trim().is_empty() && !is_comment_line(line(i), prefix));
    let mut notes = Vec::new();
    for (index, range) in ranges.iter().enumerate() {
        let text = line(index);
        let indent = text.len() - text.trim_start().len();
        let Some(rest) = text[indent..].strip_prefix(prefix) else { continue };
        if index == 0 && rest.starts_with("!/") { continue; }
        let start = range.start + indent;
        let (kind, step, marker_len, body) = if let Some(body) = rest.strip_prefix('?') {
            (Kind::Ask, None, prefix.len() + 1, body)
        } else if let Some(body) = rest.strip_prefix('!') {
            let (kind, step, consumed) = tagged(body);
            (kind, step, prefix.len() + 1 + consumed, &body[consumed..])
        } else {
            (Kind::Comment, None, prefix.len(), rest)
        };
        let text = body.trim().trim_end_matches('\r').to_owned();
        let snippet = text.split('`').nth(1).filter(|_| text.matches('`').count() >= 2).filter(|s| !s.trim().is_empty()).map(str::to_owned);
        let target = if kind == Kind::Comment { None } else { target(index) };
        let anchor = target.and_then(|t| {
            let code_line = line(t);
            match &snippet {
                Some(snippet) => code_line.find(snippet.as_str()).map(|at| ranges[t].start + at..ranges[t].start + at + snippet.len()),
                None => {
                    let trimmed = code_line.trim_end_matches('\r').trim_end();
                    let lead = trimmed.len() - trimmed.trim_start().len();
                    Some(ranges[t].start + lead..ranges[t].start + trimmed.len())
                }
            }
        });
        notes.push(Note { line: index, kind, step, text, snippet, comment: start..range.end - usize::from(code[range.clone()].ends_with('\r')), marker: start..start + marker_len, target, anchor });
    }
    notes
}

/// Reads `bug:`, `stuck 2/3:` or `answer:` after `!`. Unknown forms are hints with no tag.
fn tagged(body: &str) -> (Kind, Option<(u32, u32)>, usize) {
    let lead = body.len() - body.trim_start().len();
    let rest = &body[lead..];
    let word: String = rest.chars().take_while(char::is_ascii_alphabetic).collect();
    let Some(kind) = Kind::TAGGED.into_iter().find(|kind| kind.tag() == word.to_ascii_lowercase()) else { return (Kind::Hint, None, 0) };
    let after = &rest[word.len()..];
    let (step, after_step) = match after.strip_prefix(' ').and_then(|s| s.split_once(':')) {
        Some((counts, tail)) => match counts.trim().split_once('/') {
            Some((step, of)) => match (step.parse(), of.parse()) {
                (Ok(step), Ok(of)) => (Some((step, of)), tail),
                _ => (None, after),
            },
            None => (None, after),
        },
        None => (None, after),
    };
    let after_colon = if step.is_some() { after_step } else if let Some(tail) = after.strip_prefix(':') { tail } else { return (Kind::Hint, None, 0) };
    (kind, step, body.len() - after_colon.len())
}

/// The code without `#!` and `#?` lines, for the judge.
pub fn strip(code: &str) -> String { hide(code, View::Off).0 }

/// The note lines removed from a view, each kept before the view line it preceded.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Hidden {
    lines: Vec<(usize, String)>,
}

pub fn hide(code: &str, shown: View) -> (String, Hidden) {
    let mut view = Vec::new();
    let mut hidden = Hidden::default();
    for (index, line) in code.split('\n').enumerate() {
        if shown.hides(line) && !(index == 0 && line.trim_start().starts_with("#!/")) { hidden.lines.push((view.len(), line.to_owned())); } else { view.push(line); }
    }
    (view.join("\n"), hidden)
}

impl Hidden {
    pub fn is_empty(&self) -> bool { self.lines.is_empty() }
    pub fn len(&self) -> usize { self.lines.len() }

    /// The full file for a view.
    pub fn restore(&self, view: &str) -> String {
        if self.lines.is_empty() { return view.to_owned(); }
        let parts: Vec<&str> = view.split('\n').collect();
        let mut out = Vec::with_capacity(parts.len() + self.lines.len());
        let mut hidden = self.lines.iter().peekable();
        for (index, part) in parts.iter().enumerate() {
            while let Some((_, line)) = hidden.next_if(|(at, _)| *at <= index) { out.push(line.as_str()); }
            out.push(part);
        }
        out.extend(hidden.map(|(_, line)| line.as_str()));
        out.join("\n")
    }

    /// Keeps hidden lines beside their code after the view is edited.
    pub fn edited(&mut self, before: &str, after: &str) {
        if self.lines.is_empty() || before == after { return; }
        let (old, new): (Vec<&str>, Vec<&str>) = (before.split('\n').collect(), after.split('\n').collect());
        let prefix = old.iter().zip(&new).take_while(|(a, b)| a == b).count();
        let limit = old.len().min(new.len()) - prefix;
        let suffix = old.iter().rev().zip(new.iter().rev()).take(limit).take_while(|(a, b)| a == b).count();
        let (old_end, new_end) = (old.len() - suffix, new.len() - suffix);
        for (at, _) in &mut self.lines {
            if *at <= prefix { continue; }
            *at = if *at >= old_end { *at + new_end - old_end } else { (*at).min(new_end) };
        }
    }

    /// Visible line for a file line; `None` for a hidden one. Both zero-based.
    pub fn view_line(&self, file_line: usize) -> Option<usize> {
        let mut file = 0;
        let mut hidden = self.lines.iter().peekable();
        for view in 0.. {
            while hidden.next_if(|(at, _)| *at <= view).is_some() {
                if file == file_line { return None; }
                file += 1;
            }
            if file == file_line { return Some(view); }
            file += 1;
            if hidden.peek().is_none() { return Some(view + file_line - file + 1); }
        }
        None
    }

    /// File line for a visible line. Both zero-based.
    pub fn file_line(&self, view_line: usize) -> usize {
        view_line + self.lines.iter().filter(|(at, _)| *at <= view_line).count()
    }
}

/// A note to write, cited by its 1-based line in the code the AI saw.
#[derive(Clone, Debug, PartialEq)]
pub struct Spec {
    pub line: u32,
    pub kind: Kind,
    pub step: Option<(u32, u32)>,
    pub text: String,
    pub snippet: Option<String>,
    /// The full explanation shown on hover.
    pub detail: String,
}

fn first_sentence(text: &str) -> &str {
    let text = text.trim();
    text.char_indices().find(|&(i, c)| matches!(c, '.' | '!' | '?') && text[i + 1..].starts_with(char::is_whitespace)).map_or(text, |(i, _)| &text[..=i])
}

/// The notes an answer leaves in the code.
pub fn specs(answer: &Answer) -> Vec<Spec> {
    let spec = |line: u32, kind, step, text: &str, snippet: &Option<String>, detail: String| Spec { line, kind, step, text: first_sentence(text).to_owned(), snippet: snippet.clone(), detail };
    match answer {
        Answer::Bugs(bugs) => bugs.bugs.iter().map(|bug| {
            let kind = match bug.severity { Severity::Bug => Kind::Bug, Severity::Risk => Kind::Risk, Severity::Style => Kind::Hint };
            spec(bug.line, kind, None, &bug.title, &bug.snippet, format!("{}\n\nFix: {}", bug.detail, bug.fix))
        }).collect(),
        Answer::Stuck(stuck) => {
            let detail = format!("{}\n\nNext step: {}\n\n_{}_", stuck.diagnosis, stuck.next_step, stuck.question);
            if stuck.steps.is_empty() { return stuck.line.map(|line| spec(line, Kind::Stuck, None, &stuck.diagnosis, &stuck.snippet, detail)).into_iter().collect(); }
            let of = stuck.steps.len() as u32;
            stuck.steps.iter().zip(1..).map(|(step, index)| spec(step.line, Kind::Stuck, Some((index, of)), &step.note, &step.snippet, detail.clone())).collect()
        }
        Answer::Optimize(opt) if !opt.already_optimal => opt.bottleneck_line.map(|line| spec(line, Kind::Slow, None, &opt.bottleneck, &None, format!("{}\n\n{}", opt.bottleneck, opt.nudge))).into_iter().collect(),
        Answer::Review(review) => {
            let mut out = specs(&Answer::Bugs(review.correctness.clone()));
            out.extend(specs(&Answer::Optimize(review.improvements.clone())));
            out
        }
        _ => vec![],
    }
}

/// The kinds a rerun of an answer replaces.
pub fn replaces(answer: &Answer) -> &'static [Kind] {
    match answer {
        Answer::Bugs(_) => &[Kind::Bug, Kind::Risk, Kind::Hint],
        Answer::Stuck(_) => &[Kind::Stuck],
        Answer::Optimize(_) => &[Kind::Slow],
        Answer::Review(_) => &[Kind::Bug, Kind::Risk, Kind::Hint, Kind::Slow],
        _ => &[],
    }
}

/// One comment line for a note, without indentation.
pub fn render(spec: &Spec, language: Language) -> String {
    let step = spec.step.map(|(step, of)| format!(" {step}/{of}")).unwrap_or_default();
    format!("{}! {}{step}: {}", comment_prefix(language), spec.kind.tag(), body(spec))
}

/// The text after a note's marker, as [`parse`] reads it back.
pub fn body(spec: &Spec) -> String {
    let mut text: String = spec.text.split_whitespace().collect::<Vec<_>>().join(" ");
    if let Some(snippet) = spec.snippet.as_deref().map(str::trim).filter(|s| !s.is_empty() && !s.contains('`') && !s.contains('\n')) {
        if !text.contains(&format!("`{snippet}`")) { text = format!("{} (`{snippet}`)", text.replace('`', "")); }
    }
    if text.len() > TEXT_LIMIT {
        let mut end = TEXT_LIMIT;
        while !text.is_char_boundary(end) { end -= 1; }
        text.truncate(end);
        text.push_str("...");
    }
    text
}

/// Finds a line of the code the AI saw in the current code. Zero-based.
pub fn relocate(seen: &str, current: &str, line: usize, snippet: Option<&str>) -> Option<usize> {
    let wanted = seen.split('\n').nth(line)?.trim();
    let lines: Vec<&str> = current.split('\n').collect();
    let matches = |i: usize| {
        let candidate = lines[i].trim();
        if wanted.is_empty() { return false; }
        candidate == wanted || snippet.is_some_and(|s| !s.is_empty() && candidate.contains(s) && !is_note_line(candidate))
    };
    if line < lines.len() && matches(line) { return Some(line); }
    (1..lines.len()).flat_map(|distance| [line.checked_sub(distance), Some(line + distance)]).flatten().filter(|&i| i < lines.len()).find(|&i| matches(i))
}

/// The code with earlier notes of `replace` removed and `specs` written above their lines.
/// `seen` is the code the AI read; specs cite its lines.
pub fn write(current: &str, seen: &str, specs: &[Spec], replace: &[Kind], language: Language) -> String {
    let notes = parse(current, language);
    let removed: Vec<usize> = notes.iter().filter(|note| replace.contains(&note.kind)).map(|note| note.line).collect();
    let lines: Vec<&str> = current.split('\n').collect();
    let mut inserts: Vec<(usize, String)> = Vec::new();
    for spec in specs {
        let Some(target) = spec.line.checked_sub(1).and_then(|line| relocate(seen, current, line as usize, spec.snippet.as_deref())) else { continue };
        let target = if is_note_line(lines[target]) { match notes.iter().find(|note| note.line == target).and_then(|note| note.target) { Some(t) => t, None => continue } } else { target };
        let code = lines[target];
        let indent = &code[..code.len() - code.trim_start().len()];
        let mut line = format!("{indent}{}", render(spec, language));
        if code.ends_with('\r') { line.push('\r'); }
        inserts.push((target, line));
    }
    let mut out = Vec::with_capacity(lines.len() + inserts.len());
    for (index, line) in lines.iter().enumerate() {
        out.extend(inserts.iter().filter(|(at, _)| *at == index).map(|(_, line)| line.clone()));
        if !removed.contains(&index) { out.push((*line).to_owned()); }
    }
    out.join("\n")
}

/// An answer written below a `#?` question, found by its text.
pub fn answer(current: &str, question: &str, reply: &str, language: Language) -> Option<String> {
    let notes = parse(current, language);
    let ask = notes.iter().find(|note| note.kind == Kind::Ask && note.text == question)?;
    let lines: Vec<&str> = current.split('\n').collect();
    let indent = { let line = lines[ask.line]; &line[..line.len() - line.trim_start().len()] };
    let spec = Spec { line: 0, kind: Kind::Answer, step: None, text: reply.to_owned(), snippet: None, detail: String::new() };
    let mut out: Vec<String> = lines.iter().map(|line| (*line).to_owned()).collect();
    let after = notes.iter().filter(|note| note.line > ask.line && note.kind == Kind::Answer).take_while(|note| note.target == ask.target).map(|note| note.line).max().unwrap_or(ask.line);
    out.insert(after + 1, format!("{indent}{}", render(&spec, language)));
    Some(out.join("\n"))
}

/// The smallest replacement turning `old` into `new`: a byte range of `old` and its new text.
pub fn changed_bytes(old: &str, new: &str) -> (Range<usize>, String) {
    let mut prefix = old.bytes().zip(new.bytes()).take_while(|(a, b)| a == b).count();
    while !old.is_char_boundary(prefix) { prefix -= 1; }
    let limit = old.len().min(new.len()) - prefix;
    let mut suffix = old.bytes().rev().zip(new.bytes().rev()).take(limit).take_while(|(a, b)| a == b).count();
    while !old.is_char_boundary(old.len() - suffix) || !new.is_char_boundary(new.len() - suffix) { suffix -= 1; }
    (prefix..old.len() - suffix, new[prefix..new.len() - suffix].to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    const CODE: &str = "class Solution:\n    def search(self, nums, target):\n        lo, hi = 0, len(nums)\n        # hi is exclusive\n        while lo < hi:\n            mid = (lo + hi) // 2\n            #! stuck 1/2: `lo = mid` never moves\n            #? why does this loop forever\n            lo = mid\n        return -1\n";

    #[test]
    fn parses_markers_snippets_and_targets() {
        let notes = parse(CODE, Language::Python);
        let kinds: Vec<_> = notes.iter().map(|note| (note.line, note.kind)).collect();
        assert_eq!(kinds, [(3, Kind::Comment), (6, Kind::Stuck), (7, Kind::Ask)]);
        let stuck = &notes[1];
        assert_eq!(stuck.step, Some((1, 2)));
        assert_eq!(stuck.snippet.as_deref(), Some("lo = mid"));
        assert_eq!(stuck.target, Some(8));
        assert_eq!(&CODE[stuck.anchor.clone().unwrap()], "lo = mid");
        assert_eq!(&CODE[stuck.marker.clone()], "#! stuck 1/2:");
        assert_eq!(&CODE[stuck.comment.clone()], "#! stuck 1/2: `lo = mid` never moves");
        assert!(!stuck.outdated());
        let ask = &notes[2];
        assert_eq!(ask.text, "why does this loop forever");
        assert_eq!(&CODE[ask.anchor.clone().unwrap()], "lo = mid");
    }

    #[test]
    fn edited_snippets_are_outdated_and_unknown_tags_are_hints() {
        let code = "#!/usr/bin/env python3\n//! not python\n#! bug: `x + 1` off\ny = x + 2\n#!plain words\nz = 1";
        let notes = parse(code, Language::Python);
        assert_eq!(notes.len(), 2);
        assert!(notes[0].outdated());
        assert_eq!(notes[1].kind, Kind::Hint);
        assert_eq!(notes[1].text, "plain words");
        let cpp = parse("int a;\n//! risk: overflow\nint b = a * a;\n// mine", Language::Cpp);
        assert_eq!(cpp.iter().map(|n| n.kind).collect::<Vec<_>>(), [Kind::Risk, Kind::Comment]);
    }

    #[test]
    fn hide_and_restore_round_trip_with_edits() {
        let (rich, _) = hide(CODE, View::Rich);
        assert!(!rich.contains("#!") && rich.contains("#? why") && rich.contains("# hi is exclusive"));
        assert_eq!(hide(CODE, View::Raw).0, CODE);
        let (view, mut hidden) = hide(CODE, View::Off);
        assert!(!view.contains("#!") && !view.contains("#?") && view.contains("# hi is exclusive"));
        assert_eq!(hidden.restore(&view), CODE);
        assert_eq!(strip(CODE), view);
        // Line 8 of the file is line 6 of the view.
        assert_eq!(hidden.view_line(8), Some(6));
        assert_eq!(hidden.view_line(6), None);
        assert_eq!(hidden.view_line(2), Some(2));
        assert_eq!(hidden.file_line(6), 8);
        assert_eq!(hidden.view_line(9), Some(7));
        // A new line above the loop keeps the notes beside `lo = mid`.
        let edited = view.replacen("        while lo < hi:\n", "        steps = 0\n        while lo < hi:\n", 1);
        hidden.edited(&view, &edited);
        let restored = hidden.restore(&edited);
        assert!(restored.contains("            #? why does this loop forever\n            lo = mid\n"));
        assert!(restored.contains("        steps = 0\n        while lo < hi:\n"));
        // Typing on the target line keeps notes above it.
        let typed = edited.replacen("            lo = mid\n", "            lo = mid + 1\n", 1);
        hidden.edited(&edited, &typed);
        assert!(hidden.restore(&typed).contains("forever\n            lo = mid + 1\n"));
    }

    #[test]
    fn trailing_note_without_newline_restores() {
        let code = "x = 1\n#! hint: done";
        let (view, hidden) = hide(code, View::Off);
        assert_eq!(view, "x = 1");
        assert_eq!(hidden.restore(&view), code);
    }

    #[test]
    fn writes_notes_above_relocated_lines_and_replaces_reruns() {
        let seen = "def f(a):\n    i = 0\n    while i <= len(a):\n        i += 1\n";
        let current = "def f(a):\n    # start\n    i = 0\n    while i <= len(a):\n        i += 1\n";
        let spec = Spec { line: 3, kind: Kind::Bug, step: None, text: "Loop runs one past the end.".into(), snippet: Some("i <= len(a)".into()), detail: String::new() };
        let first = write(current, seen, std::slice::from_ref(&spec), &[Kind::Bug], Language::Python);
        assert_eq!(first, "def f(a):\n    # start\n    i = 0\n    #! bug: Loop runs one past the end. (`i <= len(a)`)\n    while i <= len(a):\n        i += 1\n");
        let again = write(&first, seen, &[spec], &[Kind::Bug], Language::Python);
        assert_eq!(again, first);
        assert_eq!(parse(&again, Language::Python)[1].snippet.as_deref(), Some("i <= len(a)"));
        let (range, text) = changed_bytes(current, &first);
        let mut edited = current.to_owned();
        edited.replace_range(range.clone(), &text);
        assert_eq!(edited, first);
        assert!(text.contains("#! bug: Loop runs one past the end."), "{text:?}");
        let (range, text) = changed_bytes("é1é", "é2é");
        assert_eq!((range, text.as_str()), (2..3, "2"));
    }

    #[test]
    fn stuck_steps_become_a_trail() {
        let stuck: crate::assist::Stuck = serde_json::from_value(serde_json::json!({
            "diagnosis": "The range never shrinks. Look again.", "line": 9, "snippet": "lo = mid", "concept": "Half-open range",
            "next_step": "Trace it", "question": "Why?",
            "steps": [{"line": 5, "snippet": "lo < hi", "note": "Loop runs while the range is non-empty"}, {"line": 9, "snippet": null, "note": "lo stays put"}],
        })).unwrap();
        let trail = specs(&Answer::Stuck(stuck.clone()));
        assert_eq!(trail.iter().map(|s| (s.line, s.step)).collect::<Vec<_>>(), [(5, Some((1, 2))), (9, Some((2, 2)))]);
        assert_eq!(render(&trail[0], Language::Python), "#! stuck 1/2: Loop runs while the range is non-empty (`lo < hi`)");
        let single = specs(&Answer::Stuck(crate::assist::Stuck { steps: vec![], ..stuck }));
        assert_eq!(single[0].text, "The range never shrinks.");
        let legacy: crate::assist::Bug = serde_json::from_value(serde_json::json!({"line": 1, "severity": "bug", "title": "t", "detail": "d", "fix": "f"})).unwrap();
        assert_eq!(legacy.snippet, None);
    }

    #[test]
    fn answers_go_below_their_question() {
        let code = "    #? why\n    lo = mid";
        let answered = answer(code, "why", "Because mid repeats.\nSecond line", Language::Python).unwrap();
        assert_eq!(answered, "    #? why\n    #! answer: Because mid repeats. Second line\n    lo = mid");
        let twice = answer(&answered, "why", "Again", Language::Python).unwrap();
        assert!(twice.ends_with("Second line\n    #! answer: Again\n    lo = mid"));
    }
}
