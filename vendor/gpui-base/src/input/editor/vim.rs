//! Safe modal editing over an externally owned buffer and undo history.
//!
//! Byte offsets are always UTF-8 boundaries. Visual endpoints are inclusive
//! character positions, while register ranges and host selections are exclusive.
use std::{cell::RefCell, ops::Range, rc::Rc};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum VimMode {
    #[default]
    Normal,
    Insert,
    Visual,
    VisualLine,
    VisualBlock,
}
impl VimMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Normal => "NORMAL",
            Self::Insert => "INSERT",
            Self::Visual => "VISUAL",
            Self::VisualLine => "VISUAL LINE",
            Self::VisualBlock => "VISUAL BLOCK",
        }
    }
    fn visual(self) -> bool {
        matches!(self, Self::Visual | Self::VisualLine | Self::VisualBlock)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VimHostCommand {
    Save,
    Close,
    SaveClose,
    SaveIfDirtyClose,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum VimScroll {
    HalfPageDown,
    HalfPageUp,
    PageDown,
    PageUp,
    Center,
    Top,
    Bottom,
}
#[derive(Clone, Debug)]
pub(super) struct VimOutcome {
    pub(super) handled: bool,
    pub(super) text: String,
    pub(super) cursor: usize,
    pub(super) selections: Vec<Range<usize>>,
    pub(super) history: Option<(bool, usize)>,
    pub(super) host: Option<VimHostCommand>,
    pub(super) scroll: Option<VimScroll>,
    pub(super) native_paste: bool,
    pub(super) scroll_count: usize,
    pub(super) message: Option<String>,
}
#[derive(Clone, Debug, Default)]
enum Register {
    #[default]
    Empty,
    Character(String),
    Line(String),
    Block {
        rows: Vec<String>,
        width: usize,
    },
}
#[derive(Clone, Debug, Default)]
struct Change {
    keys: Vec<String>,
    insert: Option<InsertPatch>,
    visual: Option<VisualRepeat>,
}
#[derive(Clone, Debug)]
struct VisualRepeat {
    mode: VimMode,
    rows: usize,
    columns: usize,
    characters: usize,
}
#[derive(Clone, Debug)]
struct InsertPatch {
    back: usize,
    remove: usize,
    value: String,
}
#[derive(Clone, Debug, Default)]
pub struct VimShared {
    register: Register,
    change: Change,
    search: String,
    search_history: Vec<String>,
    search_backward: bool,
    search_word: bool,
}
#[derive(Clone, Debug)]
struct Insertion {
    before: String,
    start: usize,
    latest: String,
    cursor: usize,
    block: Option<(Vec<usize>, usize, bool, usize)>,
    count: usize,
    open_line: bool,
}
#[derive(Clone, Copy, Debug)]
struct Motion {
    offset: usize,
    inclusive: bool,
    line: bool,
}
#[derive(Debug)]
pub(super) struct VimEngine {
    shared: Rc<RefCell<VimShared>>,
    mode: VimMode,
    anchor: usize,
    head: usize,
    previous_visual: Option<(VimMode, usize, usize)>,
    command: Vec<String>,
    entry: Option<(char, String)>,
    entry_history: Option<usize>,
    last_find: Option<(char, char)>,
    insert: Option<Insertion>,
    change_keys: Vec<String>,
    replaying: bool,
    desired_column: Option<usize>,
    block_anchor_column: usize,
    block_head_column: usize,
    repeat_visual: Option<VisualRepeat>,
    terminal_empty: bool,
    empty_line_ending: Option<&'static str>,
}
impl VimEngine {
    pub(super) fn new(shared: Rc<RefCell<VimShared>>) -> Self {
        Self {
            shared,
            mode: VimMode::Normal,
            anchor: 0,
            head: 0,
            previous_visual: None,
            command: vec![],
            entry: None,
            entry_history: None,
            last_find: None,
            insert: None,
            change_keys: vec![],
            replaying: false,
            desired_column: None,
            block_anchor_column: 0,
            block_head_column: 0,
            repeat_visual: None,
            terminal_empty: false,
            empty_line_ending: None,
        }
    }
    pub(super) fn block_insertion(&self) -> bool {
        self.insert.as_ref().is_some_and(|s| s.block.is_some())
    }
    pub(super) fn mode(&self) -> VimMode {
        self.mode
    }
    fn block_columns(&self, text: &str, tab_size: usize) -> (usize, usize) {
        let a = self.block_anchor_column;
        let h = self.block_head_column;
        let ar = a + if character(text, self.anchor) == '\t' {
            width_at('\t', a, tab_size) - 1
        } else {
            0
        };
        let hr = h + if character(text, self.head) == '\t' {
            width_at('\t', h, tab_size) - 1
        } else {
            0
        };
        (a.min(h), ar.max(hr))
    }
    /// Logical row indices and display columns, both exclusive ranges.
    pub(super) fn block_geometry(
        &self,
        text: &str,
        tab_size: usize,
    ) -> Option<(Range<usize>, Range<usize>)> {
        if self.mode != VimMode::VisualBlock {
            return None;
        }
        let a = text[..boundary(text, self.anchor)]
            .bytes()
            .filter(|c| *c == b'\n')
            .count();
        let h = text[..boundary(text, self.head)]
            .bytes()
            .filter(|c| *c == b'\n')
            .count();
        Some((
            a.min(h)..a.max(h) + 1,
            self.block_columns(text, tab_size).0..self.block_columns(text, tab_size).1 + 1,
        ))
    }
    pub(super) fn pending(&self) -> String {
        self.entry.as_ref().map_or_else(
            || self.command.concat(),
            |(kind, value)| format!("{kind}{value}"),
        )
    }
    pub(super) fn native_edit(&mut self, before: &str, after: &str, cursor: usize) {
        if self.mode != VimMode::Insert {
            return;
        }
        let session = self.insert.get_or_insert_with(|| Insertion {
            before: before.into(),
            start: cursor.min(before.len()),
            latest: before.into(),
            cursor,
            block: None,
            count: 1,
            open_line: false,
        });
        session.latest = after.into();
        session.cursor = boundary(after, cursor);
    }
    /// Finish an insertion and clear incomplete input when focus leaves the editor.
    pub(super) fn blur(&mut self, text: &str, cursor: usize, tab_size: usize) -> VimOutcome {
        let mut out = self.outcome(text, cursor);
        if !text.is_empty() {
            self.empty_line_ending = None;
        }
        if self.mode == VimMode::Insert {
            self.finish_insert(&mut out, tab_size);
        }
        self.command.clear();
        self.entry = None;
        self.with_selections(out, tab_size)
    }
    fn outcome(&self, text: &str, cursor: usize) -> VimOutcome {
        VimOutcome {
            handled: true,
            text: text.into(),
            cursor: boundary(text, cursor),
            selections: vec![],
            history: None,
            host: None,
            scroll: None,
            native_paste: false,
            scroll_count: 1,
            message: None,
        }
    }
    pub(super) fn key(
        &mut self,
        key: &str,
        text: &str,
        cursor: usize,
        editable: bool,
        tab_size: usize,
    ) -> VimOutcome {
        let mut out = self.outcome(text, cursor);
        self.anchor = boundary(text, self.anchor);
        self.head = boundary(text, self.head);
        if !text.is_empty() {
            self.empty_line_ending = None;
        }
        if self.mode == VimMode::Insert {
            match key {
                "escape" | "ctrl-[" => self.finish_insert(&mut out, tab_size),
                "ctrl-v" => out.native_paste = editable,
                "ctrl-w" if editable => {
                    let start = word_backward(text, cursor, false);
                    out.text.replace_range(start..cursor, "");
                    out.cursor = start;
                    self.native_edit(text, &out.text, out.cursor);
                }
                "ctrl-u" if editable => {
                    let start = line(text, cursor).start;
                    out.text.replace_range(start..cursor, "");
                    out.cursor = start;
                    self.native_edit(text, &out.text, out.cursor);
                }
                _ => out.handled = false,
            }
            return out;
        }
        if !(self.terminal_empty && out.cursor == text.len()) {
            out.cursor = normal_cursor(text, out.cursor);
        }
        if key == "escape" || key == "ctrl-[" {
            self.leave_visual();
            self.command.clear();
            self.entry = None;
            return out;
        }
        if self.entry.is_some() {
            return self.entry_key(key, out, editable);
        }
        if key == ":" || key == "/" || key == "?" {
            self.entry = Some((key.chars().next().unwrap(), String::new()));
            self.entry_history = None;
            return out;
        }
        if self.command.is_empty() && (key == "ctrl-v" || key == "v" || key == "V") {
            let mode = match key {
                "ctrl-v" => VimMode::VisualBlock,
                "V" => VimMode::VisualLine,
                _ => VimMode::Visual,
            };
            if self.mode == mode {
                self.leave_visual();
            } else {
                if !self.mode.visual() {
                    self.anchor = out.cursor;
                    self.head = out.cursor;
                }
                self.mode = mode;
                if mode == VimMode::VisualBlock {
                    self.block_anchor_column = column(text, self.anchor, tab_size);
                    self.block_head_column = column(text, self.head, tab_size);
                    self.desired_column = Some(
                        self.block_head_column
                            + if character(text, self.head) == '\t' {
                                width_at('\t', self.block_head_column, tab_size) - 1
                            } else {
                                0
                            },
                    );
                }
            }
            self.command.clear();
            return self.with_selections(out, tab_size);
        }
        if self.mode.visual() && (key == "o" || key == "O") {
            if key == "O" && self.mode == VimMode::VisualBlock {
                let a = line(text, self.anchor);
                let h = line(text, self.head);
                let ac = column(text, self.anchor, tab_size);
                let hc = column(text, self.head, tab_size);
                self.anchor = at_column(text, a.start, hc, tab_size, false);
                self.head = at_column(text, h.start, ac, tab_size, false);
            } else {
                std::mem::swap(&mut self.anchor, &mut self.head);
                std::mem::swap(&mut self.block_anchor_column, &mut self.block_head_column);
            }
            out.cursor = self.head;
            return self.with_selections(out, tab_size);
        }
        self.command.push(key.into());
        let keys = self.command.clone();
        let (count, mut at, explicit) = count_at(&keys, 0);
        if at == keys.len() {
            return self.with_selections(out, tab_size);
        }
        let first = keys[at].as_str();
        at += 1;
        if first == "g" && at == keys.len()
            || matches!(first, "f" | "F" | "t" | "T" | "r" | "z") && at == keys.len()
        {
            return self.with_selections(out, tab_size);
        }
        if !self.mode.visual() {
            self.repeat_visual = None;
        }
        out.scroll_count = count;
        let mut changed = false;
        let mut insert = false;
        if self.mode.visual()
            && (!matches!(first, "I" | "A") || self.mode == VimMode::VisualBlock)
            && matches!(
                first,
                "d" | "x"
                    | "c"
                    | "s"
                    | "y"
                    | "r"
                    | ">"
                    | "<"
                    | "p"
                    | "P"
                    | "I"
                    | "A"
                    | "D"
                    | "C"
                    | "Y"
            )
        {
            if !editable && first != "y" && first != "Y" {
                out.message = Some("Read-only editor".into());
            } else {
                let ranges = self.ranges(text, tab_size);
                let kind = self.mode;
                self.repeat_visual = Some(VisualRepeat {
                    mode: kind,
                    rows: ranges.len().max(1).max(
                        text[self.anchor.min(self.head)..self.anchor.max(self.head)]
                            .bytes()
                            .filter(|c| *c == b'\n')
                            .count()
                            + 1,
                    ),
                    columns: {
                        let (left, right) = self.block_columns(text, tab_size);
                        right - left + 1
                    },
                    characters: ranges.iter().map(|r| text[r.clone()].chars().count()).sum(),
                });
                let (left, right) = self.block_columns(text, tab_size);
                self.previous_visual = Some((self.mode, self.anchor, self.head));
                if matches!(first, "I" | "A") && kind == VimMode::VisualBlock {
                    let col = if first == "A" { right + 1 } else { left };
                    let rows = ranges
                        .iter()
                        .map(|r| line(text, r.start).start)
                        .collect::<Vec<_>>();
                    let rows = rows
                        .into_iter()
                        .filter(|row| {
                            first == "A"
                                || col == 0
                                || display_width(&text[line(text, *row)], tab_size) > col
                        })
                        .collect::<Vec<_>>();
                    if let Some(row) = rows.first() {
                        out.cursor =
                            prepare_insert(&mut out.text, *row, col, tab_size, first == "A");
                        self.begin_insert(
                            &out.text,
                            out.cursor,
                            Some((rows, col, first == "A", left)),
                        );
                    } else {
                        self.mode = VimMode::Normal;
                    }

                    insert = true;
                } else if matches!(first, "p" | "P") {
                    let register = self.shared.borrow().register.clone();
                    if first == "p" {
                        self.yank(text, &ranges, kind, tab_size);
                    }
                    self.delete_ranges(&mut out, &ranges, kind, false, tab_size);
                    self.put(&mut out, &register, false, count, tab_size);
                    changed = true;
                } else if first == ">" || first == "<" {
                    indent(&mut out, &ranges, first == ">", count, tab_size);
                    changed = true;
                } else if first == "r" {
                    let Some(value) = keys.get(at).and_then(|key| literal_key(key)) else {
                        return self.with_selections(out, tab_size);
                    };
                    let value = if value == '\n' { '\r' } else { value };
                    for r in ranges.iter().rev() {
                        if kind == VimMode::VisualBlock {
                            let row = line(text, r.start).start;
                            let replacement =
                                rectangle_replace(text, row, left, right + 1, value, tab_size);
                            out.text.replace_range(replacement.0, &replacement.1);
                        } else {
                            let replacement = text[r.clone()]
                                .chars()
                                .map(|c| if c == '\n' { '\n' } else { value })
                                .collect::<String>();
                            out.text.replace_range(r.clone(), &replacement);
                        }
                    }
                    out.cursor = ranges.first().map_or(out.cursor, |r| r.start);
                    changed = true;
                } else {
                    self.yank(text, &ranges, kind, tab_size);
                    if !matches!(first, "y" | "Y") {
                        self.delete_ranges(
                            &mut out,
                            &ranges,
                            kind,
                            first == "c" || first == "s" || first == "C",
                            tab_size,
                        );
                        changed = true;
                        if matches!(first, "c" | "s" | "C") {
                            let block = (kind == VimMode::VisualBlock).then(|| {
                                let mut row = line(&out.text, out.cursor).start;
                                let mut rows = vec![row];
                                for _ in 1..ranges.len() {
                                    row = line_after(&out.text, row, 1);
                                    rows.push(row);
                                }
                                (rows, left, false, left)
                            });
                            self.begin_insert(&out.text, out.cursor, block);
                            insert = true;
                        }
                    } else {
                        out.cursor = ranges.first().map_or(out.cursor, |r| r.start);
                    }
                }
                if !insert {
                    self.mode = VimMode::Normal;
                }
            }
        } else if matches!(first, "d" | "c" | "y" | ">" | "<") {
            let (motion_count, motion_at, _) = count_at(&keys, at);
            at = motion_at;
            if at == keys.len() {
                return self.with_selections(out, tab_size);
            }
            let total = count.saturating_mul(motion_count).min(1_000_000);
            let motion_key = keys[at].as_str();
            let range = if motion_key == first {
                let start = line(text, out.cursor).start;
                let end = line_after(text, start, total);
                let rows = text[start..end].bytes().filter(|c| *c == b'\n').count()
                    + usize::from(end == text.len() && !text.ends_with('\n'));
                if matches!(first, ">" | "<") && rows < total {
                    None
                } else {
                    Some((start..end, VimMode::VisualLine))
                }
            } else if matches!(motion_key, "i" | "a") {
                if at + 1 == keys.len() {
                    return self.with_selections(out, tab_size);
                }
                let object = text_object(text, out.cursor, &keys[at + 1], motion_key == "a", total);
                if object.is_none() && matches!(keys[at + 1].as_str(), "w" | "W") {
                    out.cursor = normal_cursor(text, text.len());
                }
                object.map(|r| (r, VimMode::Visual))
            } else {
                match self.motion(
                    text,
                    out.cursor,
                    &keys[at..],
                    total,
                    explicit || motion_count != 1,
                    tab_size,
                ) {
                    Some(m) => {
                        let mut end = m.offset;
                        if first == "c"
                            && matches!(motion_key, "w" | "W")
                            && !character(text, out.cursor).is_whitespace()
                        {
                            let mut stop = out.cursor;
                            for n in 0..total {
                                if n > 0 {
                                    while stop < text.len() && character(text, stop).is_whitespace()
                                    {
                                        stop = next(text, stop);
                                    }
                                }
                                let big = motion_key == "W";
                                let kind = class(character(text, stop), big);
                                while stop < text.len() && class(character(text, stop), big) == kind
                                {
                                    stop = next(text, stop);
                                }
                            }
                            end = prev(text, stop);
                        }
                        if matches!(first, "d" | "c" | "y")
                            && matches!(motion_key, "w" | "W")
                            && total == 1
                            && end > line(text, out.cursor).end
                        {
                            end = line(text, out.cursor).end;
                        }
                        let (start, end) = if m.line {
                            (
                                line(text, out.cursor.min(end)).start,
                                line_after(text, line(text, out.cursor.max(end)).start, 1),
                            )
                        } else if end < out.cursor {
                            (
                                end,
                                if m.inclusive {
                                    next(text, out.cursor)
                                } else {
                                    out.cursor
                                },
                            )
                        } else {
                            (
                                out.cursor,
                                if m.inclusive || first == "c" && matches!(motion_key, "w" | "W") {
                                    next(text, end)
                                } else {
                                    end
                                },
                            )
                        };
                        Some((
                            start..end,
                            if m.line {
                                VimMode::VisualLine
                            } else {
                                VimMode::Visual
                            },
                        ))
                    }
                    None if incomplete_motion(&keys[at..]) => {
                        return self.with_selections(out, tab_size);
                    }
                    None => None,
                }
            };
            if let Some((range, kind)) = range {
                if first == "y" {
                    self.yank(text, std::slice::from_ref(&range), kind, tab_size);
                } else if !editable {
                    out.message = Some("Read-only editor".into());
                } else if first == ">" || first == "<" {
                    indent(
                        &mut out,
                        std::slice::from_ref(&range),
                        first == ">",
                        1,
                        tab_size,
                    );
                    changed = true;
                } else {
                    self.yank(text, std::slice::from_ref(&range), kind, tab_size);
                    self.delete_ranges(
                        &mut out,
                        std::slice::from_ref(&range),
                        kind,
                        first == "c",
                        tab_size,
                    );
                    changed = true;
                    if first == "c" {
                        self.begin_insert(&out.text, out.cursor, None);
                        insert = true;
                    }
                }
            }
        } else {
            match first {
                "i" | "I" | "a" | "A" | "o" | "O" if editable && !self.mode.visual() => {
                    out.cursor = match first {
                        "I" => first_nonblank(text, out.cursor),
                        "a" => next(text, out.cursor).min(line(text, out.cursor).end),
                        "A" => line(text, out.cursor).end,
                        _ => out.cursor,
                    };
                    if first == "o" || first == "O" {
                        let l = line(text, out.cursor);
                        let ws = text[l.clone()]
                            .chars()
                            .take_while(|c| *c == ' ' || *c == '\t')
                            .collect::<String>();
                        if first == "O" {
                            out.text.insert_str(l.start, &format!("{ws}\n"));
                            out.cursor = l.start + ws.len();
                        } else {
                            out.text.insert_str(l.end, &format!("\n{ws}"));
                            out.cursor = l.end + 1 + ws.len();
                        }
                    }
                    self.begin_insert(&out.text, out.cursor, None);
                    if let Some(session) = self.insert.as_mut() {
                        session.count = count;
                        session.open_line = matches!(first, "o" | "O");
                    }
                    insert = true;
                }
                "D" | "C" | "Y" | "x" | "X" | "s" | "S" if editable || first == "Y" => {
                    let kind = if matches!(first, "S" | "Y") {
                        VimMode::VisualLine
                    } else {
                        VimMode::Visual
                    };
                    let r = match first {
                        "Y" | "S" => {
                            line(text, out.cursor).start
                                ..line_after(text, line(text, out.cursor).start, count)
                        }
                        "D" | "C" => {
                            out.cursor
                                ..line(
                                    text,
                                    line_after(
                                        text,
                                        line(text, out.cursor).start,
                                        count.saturating_sub(1),
                                    ),
                                )
                                .end
                        }
                        "X" => {
                            let mut start = out.cursor;
                            for _ in 0..count {
                                start = prev(text, start).max(line(text, out.cursor).start);
                            }
                            start..out.cursor
                        }
                        _ => {
                            let mut end = out.cursor;
                            for _ in 0..count {
                                end = next(text, end).min(line(text, out.cursor).end);
                            }
                            out.cursor..end
                        }
                    };
                    self.yank(text, std::slice::from_ref(&r), kind, tab_size);
                    if first != "Y" {
                        self.delete_ranges(
                            &mut out,
                            std::slice::from_ref(&r),
                            kind,
                            matches!(first, "C" | "s" | "S"),
                            tab_size,
                        );
                        changed = true;
                    }
                    if matches!(first, "C" | "s" | "S") {
                        self.begin_insert(&out.text, out.cursor, None);
                        insert = true;
                    }
                }
                "r" if editable => {
                    if let Some(value) = keys.get(at).and_then(|key| literal_key(key)) {
                        let limit = boundary(text, line(text, out.cursor).end);
                        let mut end = out.cursor;
                        let mut completed = 0;
                        for _ in 0..count {
                            let target = next(text, end).min(limit);
                            if target == end {
                                break;
                            }
                            end = target;
                            completed += 1;
                        }
                        if completed == count {
                            let replacement = if value == '\n' {
                                "\n".into()
                            } else {
                                value.to_string().repeat(count)
                            };
                            out.text.replace_range(out.cursor..end, &replacement);
                            out.cursor = if value == '\n' {
                                first_nonblank(&out.text, out.cursor + 1)
                            } else {
                                prev(&out.text, out.cursor + replacement.len())
                            };
                            changed = true;
                        } else {
                            out.message = Some("Not enough characters to replace".into());
                        }
                    } else if at < keys.len() {
                        out.message = Some("Expected a replacement character".into());
                    }
                }
                "J" if editable => {
                    for _ in 0..count.max(2) - 1 {
                        let l = line(&out.text, out.cursor);
                        if l.end == out.text.len()
                            || (l.end + 1 == out.text.len() && !self.terminal_empty)
                        {
                            break;
                        }
                        let end = if l.end > l.start && out.text.as_bytes()[l.end - 1] == b'\r' {
                            l.end - 1
                        } else {
                            l.end
                        };
                        let next_row = l.end + 1;
                        let next_line = line(&out.text, next_row);
                        let after = first_nonblank(&out.text, next_row);
                        let empty = out.text[next_line].trim().is_empty();
                        let previous = if end > 0 {
                            character(&out.text, prev(&out.text, end))
                        } else {
                            '\n'
                        };
                        let space = if empty || previous.is_whitespace() || previous == '\n' {
                            ""
                        } else if matches!(previous, '.' | '!' | '?') {
                            "  "
                        } else {
                            " "
                        };
                        out.text.replace_range(end..after, space);
                        out.cursor = end;
                    }
                    changed = out.text != text;
                }
                "p" | "P" if editable => {
                    let register = self.shared.borrow().register.clone();
                    self.put(&mut out, &register, first == "p", count, tab_size);
                    changed = true;
                }
                "u" if editable => out.history = Some((true, count)),
                "ctrl-r" if editable => out.history = Some((false, count)),
                "." if editable => {
                    self.command.clear();
                    return self.repeat(out, count, explicit, tab_size);
                }
                "n" | "N" => {
                    let shared = self.shared.borrow();
                    out.cursor = search(
                        text,
                        out.cursor,
                        &shared.search,
                        shared.search_backward ^ (first == "N"),
                        shared.search_word,
                        count,
                    )
                    .unwrap_or(out.cursor);
                }
                "*" | "#" => {
                    if let Some(r) = word_object(text, out.cursor, false, false, 1) {
                        let value = text[r].to_string();
                        let backward = first == "#";
                        out.cursor = search(text, out.cursor, &value, backward, true, count)
                            .unwrap_or(out.cursor);
                        let mut shared = self.shared.borrow_mut();
                        shared.search = value;
                        shared.search_backward = backward;
                        shared.search_word = true;
                    }
                }
                "ctrl-d" => out.scroll = Some(VimScroll::HalfPageDown),
                "ctrl-u" => out.scroll = Some(VimScroll::HalfPageUp),
                "ctrl-f" => out.scroll = Some(VimScroll::PageDown),
                "ctrl-b" => out.scroll = Some(VimScroll::PageUp),
                "z" => {
                    out.scroll = match keys.get(at).map(String::as_str) {
                        Some("z") => Some(VimScroll::Center),
                        Some("t") => Some(VimScroll::Top),
                        Some("b") => Some(VimScroll::Bottom),
                        _ => None,
                    }
                }
                "g" if keys.get(at).is_some_and(|k| k == "v") => {
                    if let Some((mode, a, h)) = self.previous_visual {
                        self.mode = mode;
                        self.anchor = normal_cursor(text, a);
                        self.head = normal_cursor(text, h);
                        out.cursor = self.head;
                    }
                }
                _ => {
                    if matches!(first, "i" | "a") && self.mode.visual() {
                        if at == keys.len() {
                            return self.with_selections(out, tab_size);
                        }
                        if let Some(r) =
                            text_object(text, out.cursor, &keys[at], first == "a", count)
                        {
                            self.anchor = r.start;
                            self.head = prev(text, r.end);
                            out.cursor = self.head;
                        }
                    } else if let Some(m) =
                        self.motion(text, out.cursor, &keys[at - 1..], count, explicit, tab_size)
                    {
                        out.cursor = m.offset;
                        if self.mode.visual() {
                            self.head = out.cursor;
                            if self.mode == VimMode::VisualBlock {
                                self.block_head_column = column(text, out.cursor, tab_size);
                            }
                        }
                    } else if incomplete_motion(&keys[at - 1..]) {
                        return self.with_selections(out, tab_size);
                    } else if !editable
                        && matches!(
                            first,
                            "i" | "I"
                                | "a"
                                | "A"
                                | "o"
                                | "O"
                                | "d"
                                | "c"
                                | "x"
                                | "X"
                                | "s"
                                | "S"
                                | "r"
                                | "J"
                                | "p"
                                | "P"
                        )
                    {
                        out.message = Some("Read-only editor".into());
                    } else if !key.starts_with("ctrl-") && key.chars().count() == 1 {
                        out.message = Some(format!("Unsupported Vim command: {}", keys.concat()));
                    } else if key.starts_with("ctrl-")
                        || matches!(
                            key.strip_prefix('f')
                                .and_then(|value| value.parse::<u8>().ok()),
                            Some(1..=24)
                        )
                    {
                        out.handled = false;
                    } else {
                        out.message = Some(format!("Unsupported Vim key: {key}"));
                    }
                }
            }
        }
        if changed || insert {
            self.change_keys = keys.clone();
            if !insert && !self.replaying {
                self.shared.borrow_mut().change = Change {
                    keys,
                    insert: None,
                    visual: self.repeat_visual.take(),
                };
            }
        }
        self.command.clear();
        if self.mode != VimMode::Insert && !(self.terminal_empty && out.cursor == out.text.len()) {
            out.cursor = normal_cursor(&out.text, out.cursor);
        }
        self.with_selections(out, tab_size)
    }
    fn entry_key(&mut self, key: &str, mut out: VimOutcome, editable: bool) -> VimOutcome {
        let (kind, value) = self.entry.as_mut().unwrap();
        match key {
            "up" | "down" if *kind != ':' => {
                let shared = self.shared.borrow();
                if !shared.search_history.is_empty() {
                    self.entry_history = if key == "up" {
                        Some(
                            self.entry_history
                                .map_or(shared.search_history.len() - 1, |index| {
                                    index.saturating_sub(1)
                                }),
                        )
                    } else {
                        self.entry_history.and_then(|index| {
                            (index + 1 < shared.search_history.len()).then_some(index + 1)
                        })
                    };
                    *value = self
                        .entry_history
                        .map_or_else(String::new, |index| shared.search_history[index].clone());
                }
            }
            "backspace" => {
                value.pop();
            }
            "enter" => {
                let kind = *kind;
                let value = value.clone();
                self.entry = None;
                if kind == ':' {
                    match value.trim() {
                        "w" if editable => out.host = Some(VimHostCommand::Save),
                        "q" => out.host = Some(VimHostCommand::Close),
                        "wq" if editable => out.host = Some(VimHostCommand::SaveClose),
                        "x" if editable => out.host = Some(VimHostCommand::SaveIfDirtyClose),
                        "$" => {
                            out.cursor = first_nonblank(&out.text, self.last_line_start(&out.text))
                        }
                        number if number.parse::<usize>().is_ok() => {
                            out.cursor = first_nonblank(
                                &out.text,
                                line_after(
                                    &out.text,
                                    0,
                                    number.parse::<usize>().unwrap().saturating_sub(1),
                                )
                                .min(self.last_line_start(&out.text)),
                            )
                        }
                        _ => out.message = Some(format!("Unsupported Vim command: :{value}")),
                    }
                } else {
                    let backward = kind == '?';
                    let mut shared = self.shared.borrow_mut();
                    if !value.is_empty() {
                        if shared.search_history.last() != Some(&value) {
                            shared.search_history.push(value.clone());
                            if shared.search_history.len() > 100 {
                                shared.search_history.remove(0);
                            }
                        }
                        shared.search = value;
                        shared.search_word = false;
                    }
                    shared.search_backward = backward;
                    if let Some(found) = search(
                        &out.text,
                        out.cursor,
                        &shared.search,
                        backward,
                        shared.search_word,
                        1,
                    ) {
                        out.cursor = found;
                        if self.mode.visual() {
                            self.head = found;
                        }
                    } else {
                        out.message = Some(format!("Pattern not found: {}", shared.search));
                    }
                }
            }
            _ if key.chars().count() == 1 => value.push_str(key),
            _ => {}
        }
        out
    }
    fn begin_insert(
        &mut self,
        text: &str,
        cursor: usize,
        block: Option<(Vec<usize>, usize, bool, usize)>,
    ) {
        self.mode = VimMode::Insert;
        self.insert = Some(Insertion {
            before: text.into(),
            latest: text.into(),
            start: cursor,
            cursor,
            block,
            count: 1,
            open_line: false,
        });
    }
    fn finish_insert(&mut self, out: &mut VimOutcome, tab_size: usize) {
        let mut block_cursor = None;
        if let Some(session) = self.insert.take() {
            let (range, value) = difference(&session.before, &out.text);
            let patch = InsertPatch {
                back: session.start.saturating_sub(range.start),
                remove: range.end - range.start,
                value: value.into(),
            };
            if session.count > 1 && !patch.value.is_empty() {
                let repeated = if session.open_line {
                    format!("\n{}", patch.value)
                } else {
                    patch.value.clone()
                }
                .repeat(session.count - 1);
                out.text.insert_str(out.cursor, &repeated);
                out.cursor += repeated.len();
            }
            if let Some((rows, col, pad, _normal_col)) = session.block.clone() {
                // Vim stops block replication when an insertion leaves its original line.
                if !patch.value.contains('\n') && !session.before[range.clone()].contains('\n') {
                    for row in rows.into_iter().skip(1).rev() {
                        if row > session.before.len() {
                            continue;
                        }
                        let shifted = if row >= range.end {
                            row.saturating_add(patch.value.len())
                                .saturating_sub(patch.remove)
                        } else {
                            row
                        };
                        if !pad
                            && display_width(&out.text[line(&out.text, shifted)], tab_size) < col
                        {
                            continue;
                        }
                        let point = prepare_insert(&mut out.text, shifted, col, tab_size, pad);
                        out.text.insert_str(point, &patch.value);
                    }
                }
            }
            if let Some((rows, _, _, normal_col)) = &session.block {
                if !patch.value.contains('\n') {
                    block_cursor = Some((rows.first().copied().unwrap_or(0), *normal_col));
                }
            }
            if !self.replaying
                && (session.before != session.latest
                    || session.before != out.text
                    || !self.change_keys.is_empty())
            {
                self.shared.borrow_mut().change = Change {
                    keys: self.change_keys.clone(),
                    insert: Some(patch),
                    visual: self.repeat_visual.take(),
                };
            }
            self.change_keys.clear();
        }
        self.mode = VimMode::Normal;
        out.cursor = if let Some((row, col)) = block_cursor {
            at_column(&out.text, row, col, tab_size, false)
        } else {
            let start = line(&out.text, out.cursor).start;
            if out.cursor == out.text.len() && out.cursor == start {
                self.terminal_empty = out.text.ends_with('\n');
                out.cursor
            } else {
                normal_cursor(
                    &out.text,
                    if out.cursor > start {
                        prev(&out.text, out.cursor)
                    } else {
                        out.cursor
                    },
                )
            }
        };
    }
    fn repeat(
        &mut self,
        mut out: VimOutcome,
        count: usize,
        explicit: bool,
        tab_size: usize,
    ) -> VimOutcome {
        let change = self.shared.borrow().change.clone();
        if change.keys.is_empty() {
            return out;
        }
        self.replaying = true;
        let (_, count_end, _) = count_at(&change.keys, 0);
        let mut repeat_keys = change.keys.clone();
        if explicit && change.visual.is_none() {
            repeat_keys.splice(
                0..count_end,
                count.to_string().chars().map(|c| c.to_string()),
            );
        }
        for _ in 0..1 {
            if let Some(original) = &change.visual {
                let shape = original.clone();

                self.mode = shape.mode;
                self.anchor = out.cursor;
                self.head = out.cursor;
                self.block_anchor_column = column(&out.text, out.cursor, tab_size);
                self.block_head_column = self.block_anchor_column + shape.columns - 1;
                match shape.mode {
                    VimMode::VisualBlock => {
                        self.head = at_column(
                            &out.text,
                            line_after(
                                &out.text,
                                line(&out.text, out.cursor).start,
                                shape.rows - 1,
                            ),
                            self.block_head_column,
                            tab_size,
                            false,
                        )
                    }
                    VimMode::VisualLine => {
                        self.head =
                            line_after(&out.text, line(&out.text, out.cursor).start, shape.rows - 1)
                    }
                    _ => {
                        for _ in 1..shape.characters {
                            self.head = next(&out.text, self.head);
                        }
                    }
                }
                out.cursor = self.head;
            }
            for key in &repeat_keys {
                out = self.key(key, &out.text, out.cursor, true, tab_size);
            }
            if let Some(patch) = &change.insert {
                let start = boundary(&out.text, out.cursor.saturating_sub(patch.back));
                let end = boundary(&out.text, start.saturating_add(patch.remove));
                let before = out.text.clone();
                out.text.replace_range(start..end, &patch.value);
                out.cursor = start + patch.value.len();
                self.native_edit(&before, &out.text, out.cursor);
                self.finish_insert(&mut out, tab_size);
            }
        }
        self.replaying = false;
        out
    }
    fn leave_visual(&mut self) {
        if self.mode.visual() {
            self.previous_visual = Some((self.mode, self.anchor, self.head));
        }
        self.mode = VimMode::Normal;
    }
    fn ranges(&self, text: &str, tab_size: usize) -> Vec<Range<usize>> {
        let a = boundary(text, self.anchor);
        let h = boundary(text, self.head);
        match self.mode {
            VimMode::Visual => vec![a.min(h)..next(text, a.max(h))],
            VimMode::VisualLine => {
                vec![line(text, a.min(h)).start..line_after(text, line(text, a.max(h)).start, 1)]
            }
            VimMode::VisualBlock => {
                let (left, right) = self.block_columns(text, tab_size);
                let mut row = line(text, a.min(h)).start;
                let last = line(text, a.max(h)).start;
                let mut ranges = vec![];
                loop {
                    let l = line(text, row);
                    let start = at_column(text, row, left, tab_size, true);
                    let end = at_column(text, row, right + 1, tab_size, true);
                    ranges.push(start..end);
                    if row == last || l.end == text.len() {
                        break;
                    }
                    row = l.end + 1;
                }
                ranges
            }
            _ => vec![],
        }
    }
    fn with_selections(&self, mut out: VimOutcome, tab_size: usize) -> VimOutcome {
        if self.mode.visual() {
            out.selections = self.ranges(&out.text, tab_size);
            out.cursor = boundary(&out.text, self.head);
        }
        out
    }
    fn yank(&mut self, text: &str, ranges: &[Range<usize>], kind: VimMode, tab_size: usize) {
        let register = match kind {
            VimMode::VisualBlock => {
                let (left, right) = self.block_columns(text, tab_size);
                let width = right - left + 1;
                let rows = ranges
                    .iter()
                    .map(|r| rectangle_text(text, line(text, r.start).start, left, width, tab_size))
                    .collect();
                Register::Block { rows, width }
            }
            VimMode::VisualLine => {
                let mut value = ranges.iter().map(|r| &text[r.clone()]).collect::<String>();
                if !value.ends_with('\n') {
                    value.push('\n');
                }
                Register::Line(value)
            }
            _ => Register::Character(
                ranges
                    .iter()
                    .map(|r| &text[boundary(text, r.start)..boundary(text, r.end)])
                    .collect(),
            ),
        };
        self.shared.borrow_mut().register = register;
    }
    fn delete_ranges(
        &mut self,
        out: &mut VimOutcome,
        ranges: &[Range<usize>],
        kind: VimMode,
        change: bool,
        tab_size: usize,
    ) {
        let terminated = out.text.ends_with('\n') && !self.terminal_empty;
        let crlf = out.text.contains("\r\n");
        out.cursor = ranges.first().map_or(out.cursor, |r| r.start);
        if kind == VimMode::VisualBlock {
            let (left, last) = self.block_columns(&out.text, tab_size);
            let right = last + 1;
            for r in ranges.iter().rev() {
                let row = line(&out.text, r.start).start;
                let edit = rectangle_delete(&out.text, row, left, right, tab_size);
                out.text.replace_range(edit.0, &edit.1);
            }
            out.cursor = at_column(
                &out.text,
                line(&out.text, out.cursor.min(out.text.len())).start,
                left,
                tab_size,
                true,
            );
        } else {
            for range in ranges.iter().rev() {
                let mut range = range.clone();
                if kind == VimMode::Visual {
                    range = boundary(&out.text, range.start)..boundary(&out.text, range.end);
                }
                if kind == VimMode::VisualLine
                    && range.end == out.text.len()
                    && range.start > 0
                    && !change
                    && !terminated
                {
                    range.start = prev(&out.text, range.start);
                    out.cursor = first_nonblank(&out.text, range.start);
                }
                if change && kind == VimMode::VisualLine {
                    let ws = out.text[range.clone()]
                        .chars()
                        .take_while(|c| *c == ' ' || *c == '\t')
                        .collect::<String>();
                    let suffix =
                        if range.end < out.text.len() || out.text[range.clone()].ends_with('\n') {
                            if crlf { "\r\n" } else { "\n" }
                        } else {
                            ""
                        };
                    out.text
                        .replace_range(range.clone(), &format!("{ws}{suffix}"));
                    out.cursor = range.start + ws.len();
                } else {
                    out.text.replace_range(range, "");
                }
            }
        }
        self.empty_line_ending =
            (out.text.is_empty() && terminated).then_some(if crlf { "\r\n" } else { "\n" });
        self.terminal_empty = !terminated && out.text.ends_with('\n');
        out.cursor = if kind == VimMode::VisualLine && !change {
            first_nonblank(&out.text, normal_cursor(&out.text, out.cursor))
        } else {
            boundary(&out.text, out.cursor)
        };
    }
    fn put(
        &mut self,
        out: &mut VimOutcome,
        register: &Register,
        after: bool,
        count: usize,
        tab_size: usize,
    ) {
        match register {
            Register::Empty => {}
            Register::Character(value) => {
                let at = if after {
                    next(&out.text, out.cursor).min(line(&out.text, out.cursor).end)
                } else {
                    out.cursor
                };
                let value = value.repeat(count);
                out.text.insert_str(at, &value);
                out.cursor = if value.contains('\n') {
                    at
                } else {
                    prev(&out.text, at + value.len())
                };
            }
            Register::Line(value) => {
                let l = line(&out.text, out.cursor);
                let at = if after {
                    if l.end < out.text.len() {
                        l.end + 1
                    } else {
                        l.end
                    }
                } else {
                    l.start
                };
                let mut value = value.repeat(count);
                if after && l.end == out.text.len() {
                    let ending =
                        if out.text.contains("\r\n") || self.empty_line_ending == Some("\r\n") {
                            "\r\n"
                        } else {
                            "\n"
                        };
                    value.insert_str(0, ending);
                    if value.ends_with(ending)
                        && !(out.text.is_empty() && self.empty_line_ending.is_some())
                    {
                        value.truncate(value.len() - ending.len());
                    }
                }
                out.text.insert_str(at, &value);
                out.cursor = first_nonblank(
                    &out.text,
                    if after && l.end == at {
                        at + if out.text[at..].starts_with("\r\n") {
                            2
                        } else {
                            1
                        }
                    } else {
                        at
                    },
                );
            }
            Register::Block { rows, width } => {
                let col = column(&out.text, out.cursor, tab_size)
                    + usize::from(
                        after
                            && character(&out.text, out.cursor) != '\n'
                            && out.cursor < out.text.len(),
                    );
                let start = line(&out.text, out.cursor).start;
                let mut row = start;
                for (index, value) in rows.iter().enumerate() {
                    if index > 0 {
                        let l = line(&out.text, row);
                        if l.end == out.text.len() {
                            out.text.push('\n');
                        }
                        row = line_after(&out.text, row, 1);
                    }
                    let at = at_column(&out.text, row, col, tab_size, true);
                    let current = column(&out.text, at, tab_size);
                    let padding = width.saturating_sub(display_width(value, tab_size));
                    let unit = format!("{value}{}", " ".repeat(padding));
                    out.text.insert_str(
                        at,
                        &format!(
                            "{}{}",
                            " ".repeat(col.saturating_sub(current)),
                            unit.repeat(count)
                        ),
                    );
                }
                out.cursor = at_column(&out.text, start, col, tab_size, true);
            }
        }
    }
    fn last_line_start(&self, text: &str) -> usize {
        let offset = if self.terminal_empty {
            text.len()
        } else {
            normal_cursor(text, text.len())
        };
        line(text, offset).start
    }

    fn motion(
        &mut self,
        text: &str,
        cursor: usize,
        keys: &[String],
        count: usize,
        explicit: bool,
        tab_size: usize,
    ) -> Option<Motion> {
        let key = keys.first()?.as_str();
        let mut at = cursor;
        let mut inclusive = false;
        let mut linewise = false;
        if !matches!(key, "j" | "k" | "up" | "down") {
            self.desired_column = None;
        }
        match key {
            "h" | "left" => {
                for _ in 0..count {
                    at = prev(text, at).max(line(text, cursor).start);
                }
            }
            "l" | "right" => {
                for _ in 0..count {
                    at = normal_cursor(text, next(text, at))
                        .min(normal_cursor(text, line(text, cursor).end));
                }
            }
            "j" | "down" | "k" | "up" => {
                let col = *self
                    .desired_column
                    .get_or_insert_with(|| column(text, cursor, tab_size));
                let down = matches!(key, "j" | "down");
                for _ in 0..count {
                    let l = line(text, at);
                    let row = if down {
                        if l.end == text.len() {
                            break;
                        }
                        l.end + 1
                    } else {
                        if l.start == 0 {
                            break;
                        }
                        line(text, l.start - 1).start
                    };
                    at = at_column(text, row, col, tab_size, false);
                }
                linewise = true;
            }
            "w" | "W" => {
                for _ in 0..count {
                    at = word_forward(text, at, key == "W");
                }
            }
            "b" | "B" => {
                for _ in 0..count {
                    at = word_backward(text, at, key == "B");
                }
            }
            "e" | "E" => {
                at = word_end(text, at, key == "E", count);
                inclusive = true;
            }
            "0" => at = line(text, cursor).start,
            "^" => at = first_nonblank(text, cursor),
            "$" => {
                at = normal_cursor(
                    text,
                    line(text, line_after(text, line(text, cursor).start, count - 1)).end,
                );
                inclusive = true;
            }
            "G" => {
                at = first_nonblank(
                    text,
                    if explicit {
                        line_after(text, 0, count - 1).min(self.last_line_start(text))
                    } else {
                        self.last_line_start(text)
                    },
                );
                linewise = true;
            }
            "g" => match keys.get(1)?.as_str() {
                "g" => {
                    at = first_nonblank(
                        text,
                        line_after(text, 0, count - 1).min(self.last_line_start(text)),
                    );
                    linewise = true;
                }
                "e" | "E" => {
                    let big = keys[1] == "E";
                    for _ in 0..count {
                        if !character(text, at).is_whitespace() {
                            let kind = class(character(text, at), big);
                            while at > 0 && class(character(text, prev(text, at)), big) == kind {
                                at = prev(text, at);
                            }
                        }
                        if at > 0 {
                            at = prev(text, at);
                        }
                        while at > 0 && character(text, at).is_whitespace() {
                            at = prev(text, at);
                        }
                    }
                    inclusive = true;
                }
                _ => return None,
            },
            "{" | "}" => {
                for _ in 0..count {
                    let backward = key == "{";
                    let mut row = line(text, at).start;
                    let mut seen_nonblank = !text[line(text, row)].trim().is_empty();
                    loop {
                        let l = line(text, row);
                        let next_row = if backward {
                            if row == 0 {
                                at = 0;
                                break;
                            }
                            line(text, row - 1).start
                        } else {
                            if l.end == text.len() {
                                at = normal_cursor(text, text.len());
                                break;
                            }
                            l.end + 1
                        };
                        let blank = text[line(text, next_row)].trim().is_empty();
                        row = next_row;
                        if blank && seen_nonblank {
                            at = row;
                            break;
                        }
                        seen_nonblank |= !blank;
                    }
                }
            }
            "%" => {
                if explicit {
                    let rows = (text.bytes().filter(|c| *c == b'\n').count() + 1
                        - usize::from(text.ends_with('\n') && !self.terminal_empty))
                    .max(1);
                    at = first_nonblank(
                        text,
                        line_after(
                            text,
                            0,
                            (rows * count.min(100)).div_ceil(100).saturating_sub(1),
                        ),
                    );
                    linewise = true;
                } else {
                    at = matching_pair(text, cursor)?;
                    inclusive = true;
                }
            }
            "f" | "F" | "t" | "T" => {
                let target = literal_key(keys.get(1)?)?;
                let kind = key.chars().next()?;
                at = find_char(text, cursor, target, kind, count)?;
                self.last_find = Some((kind, target));
                inclusive = matches!(kind, 'f' | 't');
            }
            ";" | "," => {
                let (mut kind, target) = self.last_find?;
                if key == "," {
                    kind = match kind {
                        'f' => 'F',
                        'F' => 'f',
                        't' => 'T',
                        _ => 't',
                    };
                }
                let origin = match kind {
                    't' => next(text, cursor),
                    'T' => prev(text, cursor),
                    _ => cursor,
                };
                at = find_char(text, origin, target, kind, count)?;
                inclusive = matches!(kind, 'f' | 't');
            }
            _ => return None,
        }
        Some(Motion {
            offset: boundary(text, at),
            inclusive,
            line: linewise,
        })
    }
}

fn literal_key(key: &str) -> Option<char> {
    match key {
        "enter" => Some('\n'),
        "tab" => Some('\t'),
        _ if key.chars().count() == 1 => key.chars().next(),
        _ => None,
    }
}

fn boundary(text: &str, offset: usize) -> usize {
    let mut at = offset.min(text.len());
    while !text.is_char_boundary(at) {
        at -= 1;
    }
    if at > 0 && at < text.len() && text.as_bytes()[at] == b'\n' && text.as_bytes()[at - 1] == b'\r'
    {
        at -= 1;
    }
    at
}
fn character(text: &str, offset: usize) -> char {
    text[boundary(text, offset)..]
        .chars()
        .next()
        .unwrap_or('\n')
}
fn next(text: &str, offset: usize) -> usize {
    let at = boundary(text, offset);
    let mut end = at + text[at..].chars().next().map_or(0, char::len_utf8);
    if text.as_bytes().get(at) == Some(&b'\r') && text.as_bytes().get(end) == Some(&b'\n') {
        end += 1;
    }
    while end < text.len()
        && glyph_width(character(text, end)) == 0
        && !character(text, end).is_control()
    {
        end += character(text, end).len_utf8();
    }
    end
}
fn prev(text: &str, offset: usize) -> usize {
    let mut at = boundary(text, offset);
    at = text[..at].char_indices().next_back().map_or(0, |(i, _)| i);
    if at > 0 && text.as_bytes()[at] == b'\n' && text.as_bytes()[at - 1] == b'\r' {
        at -= 1;
    }
    while at > 0 && glyph_width(character(text, at)) == 0 && !character(text, at).is_control() {
        at = text[..at].char_indices().next_back().map_or(0, |(i, _)| i);
    }
    at
}

fn line(text: &str, offset: usize) -> Range<usize> {
    let at = boundary(text, offset);
    let start = text[..at].rfind('\n').map_or(0, |i| i + 1);
    let end = text[at..].find('\n').map_or(text.len(), |i| at + i);
    start..end
}
fn normal_cursor(text: &str, offset: usize) -> usize {
    let at = boundary(text, offset);
    let at = if at == text.len() && text.ends_with('\n') && at > 0 {
        prev(text, at)
    } else {
        at
    };
    let l = line(text, at);
    if (at == l.end || at + 1 == l.end && text.as_bytes().get(at) == Some(&b'\r')) && l.start < at {
        prev(text, at)
    } else {
        at
    }
}
fn line_after(text: &str, start: usize, count: usize) -> usize {
    let mut at = start.min(text.len());
    for _ in 0..count {
        let l = line(text, at);
        if l.end == text.len() {
            return text.len();
        }
        at = l.end + 1;
    }
    at
}
fn first_nonblank(text: &str, at: usize) -> usize {
    let l = line(text, at);
    l.start
        + text[l.clone()]
            .char_indices()
            .find(|(_, c)| !c.is_whitespace())
            .map_or(0, |(i, _)| i)
}
fn count_at(keys: &[String], mut at: usize) -> (usize, usize, bool) {
    let mut count = 0usize;
    let start = at;
    while let Some(digit) = keys
        .get(at)
        .filter(|k| k.len() == 1)
        .and_then(|k| k.chars().next())
        .and_then(|c| c.to_digit(10))
    {
        if digit == 0 && at == start {
            break;
        }
        count = count
            .saturating_mul(10)
            .saturating_add(digit as usize)
            .min(1_000_000);
        at += 1;
    }
    (count.max(1), at, at != start)
}
fn incomplete_motion(keys: &[String]) -> bool {
    keys.len() == 1 && matches!(keys[0].as_str(), "g" | "f" | "F" | "t" | "T")
}
fn class(c: char, big: bool) -> u8 {
    if c.is_whitespace() {
        0
    } else if big || c.is_alphanumeric() || c == '_' {
        1
    } else {
        2
    }
}
fn word_forward(text: &str, mut at: usize, big: bool) -> usize {
    let kind = class(character(text, at), big);
    if kind != 0 {
        while at < text.len() && class(character(text, at), big) == kind {
            at = next(text, at);
        }
    } else if character(text, at) == '\n' && line(text, at).start == at {
        at = next(text, at);
    }
    while at < text.len() && character(text, at).is_whitespace() {
        if character(text, at) == '\n' && line(text, at).start == at {
            break;
        }
        at = next(text, at);
    }
    at
}

fn word_backward(text: &str, mut at: usize, big: bool) -> usize {
    at = prev(text, at);
    while at > 0 && character(text, at).is_whitespace() {
        at = prev(text, at);
    }
    let kind = class(character(text, at), big);
    while at > 0 && class(character(text, prev(text, at)), big) == kind {
        at = prev(text, at);
    }
    at
}
fn word_end(text: &str, mut at: usize, big: bool, count: usize) -> usize {
    for _ in 0..count {
        at = next(text, at);
        while at < text.len() && character(text, at).is_whitespace() {
            at = next(text, at);
        }
        let kind = class(character(text, at), big);
        while next(text, at) < text.len() && class(character(text, next(text, at)), big) == kind {
            at = next(text, at);
        }
    }
    normal_cursor(text, at)
}
fn find_char(text: &str, cursor: usize, target: char, kind: char, count: usize) -> Option<usize> {
    let l = line(text, cursor);
    let backward = matches!(kind, 'F' | 'T');
    let found = if backward {
        text[l.start..cursor]
            .char_indices()
            .rev()
            .filter(|(_, c)| *c == target)
            .nth(count - 1)
            .map(|(i, _)| l.start + i)
    } else {
        let start = next(text, cursor);
        text[start.min(l.end)..l.end]
            .char_indices()
            .filter(|(_, c)| *c == target)
            .nth(count - 1)
            .map(|(i, _)| start + i)
    }?;
    Some(match kind {
        't' => prev(text, found),
        'T' => next(text, found),
        _ => found,
    })
}
fn matching_pair(text: &str, cursor: usize) -> Option<usize> {
    let l = line(text, cursor);
    let (at, c) = text[cursor..l.end]
        .char_indices()
        .find(|(_, c)| "()[]{}".contains(*c))
        .map(|(i, c)| (cursor + i, c))?;
    let (open, close, back) = match c {
        '(' => ('(', ')', false),
        ')' => ('(', ')', true),
        '[' => ('[', ']', false),
        ']' => ('[', ']', true),
        '{' => ('{', '}', false),
        _ => ('{', '}', true),
    };
    let mut depth = 0usize;
    if back {
        for (i, c) in text[..=at].char_indices().rev() {
            if c == close {
                depth += 1;
            } else if c == open {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
        }
    } else {
        for (i, c) in text[at..].char_indices() {
            if c == open {
                depth += 1;
            } else if c == close {
                depth -= 1;
                if depth == 0 {
                    return Some(at + i);
                }
            }
        }
    }
    None
}
fn word_object(
    text: &str,
    cursor: usize,
    big: bool,
    around: bool,
    count: usize,
) -> Option<Range<usize>> {
    if text.is_empty() {
        return None;
    }
    let mut start = normal_cursor(text, cursor);
    let initial = class(character(text, start), big);
    while start > 0 && class(character(text, prev(text, start)), big) == initial {
        start = prev(text, start);
    }
    let mut end = start;
    if !around {
        for _ in 0..count {
            if end == text.len() {
                return None;
            }
            let kind = class(character(text, end), big);
            while end < text.len() && class(character(text, end), big) == kind {
                end = next(text, end);
            }
        }
    } else {
        for n in 0..count {
            if initial == 0 || n > 0 {
                while end < text.len() && character(text, end).is_whitespace() {
                    end = next(text, end);
                }
            }
            if end == text.len() {
                return None;
            }
            let kind = class(character(text, end), big);
            while end < text.len() && class(character(text, end), big) == kind {
                end = next(text, end);
            }
        }
        if initial != 0 {
            let before = end;
            while end < text.len() && matches!(character(text, end), ' ' | '\t') {
                end = next(text, end);
            }
            if end == before {
                while start > line(text, start).start
                    && matches!(character(text, prev(text, start)), ' ' | '\t')
                {
                    start = prev(text, start);
                }
            }
        }
    }
    Some(start..end)
}

fn text_object(
    text: &str,
    cursor: usize,
    key: &str,
    around: bool,
    count: usize,
) -> Option<Range<usize>> {
    if matches!(key, "w" | "W") {
        return word_object(text, cursor, key == "W", around, count);
    }
    let (open, close) = match key {
        "\"" => ('"', '"'),
        "'" => ('\'', '\''),
        "`" => ('`', '`'),
        "(" | ")" | "b" => ('(', ')'),
        "[" | "]" => ('[', ']'),
        "{" | "}" | "B" => ('{', '}'),
        _ => return None,
    };
    if open == close {
        let l = line(text, cursor);
        let quotes = text[l.clone()]
            .char_indices()
            .filter(|(i, c)| {
                *c == open
                    && text[l.start..l.start + i]
                        .chars()
                        .rev()
                        .take_while(|c| *c == '\\')
                        .count()
                        % 2
                        == 0
            })
            .map(|(i, _)| l.start + i)
            .collect::<Vec<_>>();
        for pair in quotes.chunks_exact(2) {
            if pair[1] >= cursor {
                let mut range = if around || count > 1 {
                    pair[0]..next(text, pair[1])
                } else {
                    next(text, pair[0])..pair[1]
                };
                if around {
                    let before = range.end;
                    while range.end < l.end && matches!(character(text, range.end), ' ' | '\t') {
                        range.end = next(text, range.end);
                    }
                    if range.end == before {
                        while range.start > l.start
                            && matches!(character(text, prev(text, range.start)), ' ' | '\t')
                        {
                            range.start = prev(text, range.start);
                        }
                    }
                }
                return Some(range);
            }
        }
        return None;
    }
    let mut stack = vec![];
    let mut pairs = vec![];
    for (i, c) in text.char_indices() {
        if c == open {
            stack.push(i);
        } else if c == close {
            if let Some(start) = stack.pop() {
                pairs.push(start..i);
            }
        }
    }
    let mut enclosing = pairs
        .iter()
        .filter(|r| r.start <= cursor && cursor <= r.end)
        .cloned()
        .collect::<Vec<_>>();
    if enclosing.is_empty() {
        let start = pairs
            .iter()
            .filter(|r| r.start > cursor)
            .map(|r| r.start)
            .min()?;
        enclosing = pairs.into_iter().filter(|r| r.start == start).collect();
    }
    enclosing.sort_by_key(|r| r.end - r.start);
    let r = enclosing.get(count - 1)?;
    Some(if around {
        r.start..next(text, r.end)
    } else {
        next(text, r.start)..r.end
    })
}
fn search(
    text: &str,
    cursor: usize,
    value: &str,
    backward: bool,
    word: bool,
    count: usize,
) -> Option<usize> {
    if value.is_empty() {
        return None;
    }
    let mut matches = text
        .match_indices(value)
        .filter(|(at, _)| {
            !word
                || ((*at == 0 || class(character(text, prev(text, *at)), false) != 1)
                    && (*at + value.len() == text.len()
                        || class(character(text, *at + value.len()), false) != 1))
        })
        .map(|(i, _)| i)
        .collect::<Vec<_>>();
    if matches.is_empty() {
        return None;
    }
    if backward {
        matches.reverse();
    }
    let mut at = cursor;
    for _ in 0..count {
        at = matches
            .iter()
            .copied()
            .find(|p| if backward { *p < at } else { *p > at })
            .unwrap_or(matches[0]);
    }
    Some(at)
}
fn difference<'a>(before: &str, after: &'a str) -> (Range<usize>, &'a str) {
    let mut start = 0;
    for (a, b) in before.chars().zip(after.chars()) {
        if a != b {
            break;
        }
        start += a.len_utf8();
    }
    let mut end = before.len();
    let mut after_end = after.len();
    while end > start && after_end > start {
        let a = prev(before, end);
        let b = prev(after, after_end);
        if before[a..end] != after[b..after_end] {
            break;
        }
        end = a;
        after_end = b;
    }
    (start..end, &after[start..after_end])
}
fn indent(
    out: &mut VimOutcome,
    ranges: &[Range<usize>],
    right: bool,
    count: usize,
    tab_size: usize,
) {
    let mut rows = vec![];
    for r in ranges {
        let mut row = line(&out.text, r.start).start;
        let last = line(
            &out.text,
            if r.end > r.start {
                prev(&out.text, r.end)
            } else {
                r.end
            },
        )
        .start;
        loop {
            rows.push(row);
            if row >= last {
                break;
            }
            let next = line_after(&out.text, row, 1);
            if next <= row {
                break;
            }
            row = next;
        }
    }
    rows.sort_unstable();
    rows.dedup();
    let width = tab_size.max(1) * count;
    for row in rows.iter().rev() {
        if right {
            out.text.insert_str(*row, &" ".repeat(width));
        } else {
            let mut end = *row;
            let mut removed = 0;
            while end < out.text.len() && removed < width {
                match character(&out.text, end) {
                    ' ' => {
                        removed += 1;
                        end += 1;
                    }
                    '\t' => {
                        removed += tab_size.max(1);
                        end += 1;
                    }
                    _ => break,
                }
            }
            out.text.replace_range(*row..end, "");
        }
    }
    if let Some(row) = rows.first() {
        out.cursor = first_nonblank(&out.text, *row);
    }
}

// Unicode display columns: zero-width marks and selectors do not occupy a
// column; East Asian wide/fullwidth and emoji occupy two. Tabs use tab stops.
fn glyph_width(c: char) -> usize {
    match c as u32 {
        0x0000..=0x001f
        | 0x007f..=0x009f
        | 0x0300..=0x036f
        | 0x0483..=0x0489
        | 0x0591..=0x05bd
        | 0x05bf
        | 0x05c1..=0x05c2
        | 0x0610..=0x061a
        | 0x064b..=0x065f
        | 0x0670
        | 0x06d6..=0x06ed
        | 0x0900..=0x0903
        | 0x093a..=0x094f
        | 0x0951..=0x0957
        | 0x0962..=0x0963
        | 0x1ab0..=0x1aff
        | 0x1dc0..=0x1dff
        | 0x200b..=0x200f
        | 0x202a..=0x202e
        | 0x2060..=0x206f
        | 0x20d0..=0x20ff
        | 0xfe00..=0xfe0f
        | 0xfe20..=0xfe2f
        | 0xe0100..=0xe01ef => 0,
        0x1100..=0x115f
        | 0x2329..=0x232a
        | 0x2e80..=0xa4cf
        | 0xac00..=0xd7a3
        | 0xf900..=0xfaff
        | 0xfe10..=0xfe19
        | 0xfe30..=0xfe6f
        | 0xff00..=0xff60
        | 0xffe0..=0xffe6
        | 0x1f300..=0x1faff
        | 0x20000..=0x3ffff => 2,
        _ => 1,
    }
}
pub(super) fn width_at(c: char, col: usize, tab_size: usize) -> usize {
    if c == '\t' {
        tab_size.max(1) - col % tab_size.max(1)
    } else {
        glyph_width(c)
    }
}
fn display_width(text: &str, tab_size: usize) -> usize {
    text.chars()
        .fold(0, |col, c| col + width_at(c, col, tab_size))
}
fn column(text: &str, at: usize, tab_size: usize) -> usize {
    let at = boundary(text, at);
    display_width(&text[line(text, at).start..at], tab_size)
}
fn at_column(text: &str, row: usize, target: usize, tab_size: usize, allow_end: bool) -> usize {
    let l = line(text, row);
    let mut col = 0;
    for (i, c) in text[l.clone()].char_indices() {
        let width = width_at(c, col, tab_size);
        if col + width > target {
            return l.start + i;
        }
        col += width;
    }
    if allow_end {
        l.end
    } else {
        normal_cursor(text, l.end)
    }
}
fn rectangle_text(text: &str, row: usize, left: usize, width: usize, tab_size: usize) -> String {
    let l = line(text, row);
    let right = left + width;
    let mut col = 0;
    let mut value = String::new();
    for c in text[l].chars() {
        let w = width_at(c, col, tab_size);
        let end = col + w;
        if end > left && col < right {
            if col < left || end > right {
                value.push_str(&" ".repeat(end.min(right).saturating_sub(col.max(left))));
            } else {
                value.push(c);
            }
        }
        col = end;
    }
    value.push_str(&" ".repeat(right.saturating_sub(col.max(left))));
    value
}
fn rectangle_delete(
    text: &str,
    row: usize,
    left: usize,
    right: usize,
    tab_size: usize,
) -> (Range<usize>, String) {
    let l = line(text, row);
    let mut col = 0;
    let mut value = String::new();
    for c in text[l.clone()].chars() {
        let width = width_at(c, col, tab_size);
        let end = col + width;
        if end <= left || col >= right {
            value.push(c);
        } else {
            value.push_str(&" ".repeat(left.saturating_sub(col).min(width)));
            value.push_str(&" ".repeat(end.saturating_sub(right).min(width)));
        }
        col = end;
    }
    (l, value)
}

#[cfg(test)]
#[path = "vim/tests.rs"]
mod tests;

fn prepare_insert(text: &mut String, row: usize, col: usize, tab_size: usize, pad: bool) -> usize {
    let at = at_column(text, row, col, tab_size, true);
    let current = column(text, at, tab_size);
    if character(text, at) == '\t' && current < col {
        let width = width_at('\t', current, tab_size);
        text.replace_range(at..at + 1, &" ".repeat(width));
        return at + col - current;
    }
    if pad && current < col {
        text.insert_str(at, &" ".repeat(col - current));
        return at + col - current;
    }
    at
}
fn rectangle_replace(
    text: &str,
    row: usize,
    left: usize,
    right: usize,
    value: char,
    tab_size: usize,
) -> (Range<usize>, String) {
    let l = line(text, row);
    let mut col = 0;
    let mut result = String::new();
    for c in text[l.clone()].chars() {
        let width = width_at(c, col, tab_size);
        let end = col + width;
        if end <= left || col >= right {
            result.push(c);
        } else {
            result.push_str(&" ".repeat(left.saturating_sub(col).min(width)));
            result.push_str(
                &value
                    .to_string()
                    .repeat(end.min(right).saturating_sub(col.max(left))),
            );
            result.push_str(&" ".repeat(end.saturating_sub(right).min(width)));
        }
        col = end;
    }
    (l, result)
}
