use super::*;
fn run(text: &str, cursor: usize, keys: &[&str]) -> (VimEngine, VimOutcome) {
    let mut engine = VimEngine::new(Rc::new(RefCell::new(VimShared::default())));
    let mut out = engine.outcome(text, cursor);
    for key in keys {
        out = engine.key(key, &out.text, out.cursor, true, 4);
    }
    (engine, out)
}
#[test]
fn motions_counts_and_operators() {
    let (_, out) = run(
        "one two three four five six seven",
        0,
        &["2", "d", "3", "w"],
    );
    assert_eq!(out.text, "seven");
    let (_, out) = run("one two\nthree\nfour", 0, &["d", "j"]);
    assert_eq!(out.text, "four");
}
#[test]
fn native_insert_paste_is_recorded_and_repeatable() {
    let (mut engine, mut out) = run("one two", 0, &["i", "ctrl-v"]);
    assert!(out.native_paste);
    assert_eq!(engine.mode(), VimMode::Insert);
    engine.native_edit(&out.text, "paste\ntextone two", 10);
    out = engine.key("escape", "paste\ntextone two", 10, true, 4);
    assert_eq!(engine.mode(), VimMode::Normal);
    out = engine.key(".", &out.text, out.cursor, true, 4);
    assert_eq!(out.text.len(), "one two".len() + 2 * "paste\ntext".len());
}
#[test]
fn control_v_mode_switches() {
    let (mut engine, mut out) = run("abcd\nefgh", 0, &["v", "ctrl-v"]);
    assert_eq!(engine.mode(), VimMode::VisualBlock);
    out = engine.key("ctrl-v", &out.text, out.cursor, true, 4);
    assert_eq!(engine.mode(), VimMode::Normal);
    assert!(out.selections.is_empty());
    let (engine, _) = run("abcd", 0, &["V", "ctrl-v"]);
    assert_eq!(engine.mode(), VimMode::VisualBlock);
}
#[test]
fn inclusive_visual_and_reversed_selections() {
    let (_, out) = run("abcdef", 3, &["v", "h", "h", "d"]);
    assert_eq!(out.text, "aef");
    let (engine, out) = run("abc\ndef", 0, &["v", "j", "V", "o"]);
    assert_eq!(engine.mode(), VimMode::VisualLine);
    assert_eq!(out.selections, vec![0..7]);
}
#[test]
fn block_delete_and_put() {
    let (_, out) = run("abcd\nefgh\nijkl", 1, &["ctrl-v", "j", "j", "l", "d", "P"]);
    assert_eq!(out.text, "abcd\nefgh\nijkl");
}
#[test]
fn block_insert_replication() {
    let (mut engine, out) = run("abcd\nefgh\nijkl", 1, &["ctrl-v", "j", "j", "I"]);
    assert_eq!(engine.mode(), VimMode::Insert);
    let mut text = out.text.clone();
    text.insert_str(out.cursor, "XY");
    engine.native_edit(&out.text, &text, out.cursor + 2);
    let out = engine.key("escape", &text, out.cursor + 2, true, 4);
    assert_eq!(out.text, "aXYbcd\neXYfgh\niXYjkl");
}
#[test]
fn block_geometry_tabs_wide_and_short_rows() {
    assert_eq!(column("\t界a", 4, 4), 6);
    assert_eq!(rectangle_text("\t界a", 0, 2, 4, 4), "  界");
    assert_eq!(rectangle_delete("\tab", 0, 1, 3, 4), (0..3, "  ab".into()));
    let (_, out) = run("abcd\nx\n\n界abc", 1, &["ctrl-v", "j", "j", "j", "l", "y"]);
    assert_eq!(out.text, "abcd\nx\n\n界abc");
}
#[test]
fn text_objects_and_change_words() {
    let (_, out) = run("call(one, [two])", 6, &["d", "i", "("]);
    assert_eq!(out.text, "call()");
    let (engine, out) = run("one two", 0, &["c", "w"]);
    assert_eq!(out.text, " two");
    assert_eq!(engine.mode(), VimMode::Insert);
}
#[test]
fn shared_register_search_repeat() {
    let shared = Rc::new(RefCell::new(VimShared::default()));
    let mut first = VimEngine::new(shared.clone());
    first.key("x", "abc", 0, true, 4);
    let mut second = VimEngine::new(shared);
    assert_eq!(second.key("P", "def", 0, true, 4).text, "adef");
    assert_eq!(second.key(".", "def", 0, true, 4).text, "adef");
}
#[test]
fn literal_query_history_is_shared_between_editors() {
    let shared = Rc::new(RefCell::new(VimShared::default()));
    let mut first = VimEngine::new(shared.clone());
    let mut out = first.outcome("alpha beta alpha", 0);
    for key in [
        "/", "a", "l", "p", "h", "a", "enter", "/", "b", "e", "t", "a", "enter",
    ] {
        out = first.key(key, &out.text, out.cursor, true, 4);
    }
    let mut second = VimEngine::new(shared);
    second.key("/", "alpha beta alpha", 0, true, 4);
    second.key("up", "alpha beta alpha", 0, true, 4);
    assert_eq!(second.pending(), "/beta");
    second.key("up", "alpha beta alpha", 0, true, 4);
    assert_eq!(second.pending(), "/alpha");
    second.key("down", "alpha beta alpha", 0, true, 4);
    assert_eq!(second.pending(), "/beta");
    second.key("down", "alpha beta alpha", 0, true, 4);
    assert_eq!(second.pending(), "/");
}
#[test]
fn literal_search_wrap_and_word_boundaries() {
    let (_, out) = run("one stone one", 0, &["*"]);
    assert_eq!(out.cursor, 10);
    let (_, out) = run("a.b aXb a.b", 0, &["/", "a", ".", "b", "enter", "n"]);
    assert_eq!(out.cursor, 0);
}
#[test]
fn read_only_never_modifies() {
    for keys in [
        vec!["d", "d"],
        vec!["i"],
        vec!["v", "l", "d"],
        vec!["x"],
        vec!["p"],
    ] {
        let mut engine = VimEngine::new(Rc::new(RefCell::new(VimShared::default())));
        let mut out = engine.outcome("abc", 0);
        for key in keys {
            out = engine.key(key, &out.text, out.cursor, false, 4);
        }
        assert_eq!(out.text, "abc");
        assert_ne!(engine.mode(), VimMode::Insert);
    }
}
#[test]
fn unsupported_visual_and_replacement_keys_do_not_edit() {
    for key in ["I", "A"] {
        let (_, out) = run("abcdef", 0, &["v", "l", key]);
        assert_eq!(out.text, "abcdef");
    }
    let (_, out) = run("abcdef", 0, &["r", "up"]);
    assert_eq!(out.text, "abcdef");
    let (_, out) = run("abcdef", 0, &["f5"]);
    assert!(
        !out.handled,
        "Function keys must remain available to app shortcuts"
    );
}
#[test]
fn visual_endpoints_survive_a_shorter_host_document() {
    let (mut engine, _) = run("readonly one readonly", 13, &["v", "$"]);
    let out = engine.key("y", "x", 0, false, 4);
    assert_eq!(out.text, "x");
    assert_eq!(engine.mode(), VimMode::Normal);
    assert!(out.cursor <= out.text.len());
}
#[test]
fn commands_history_and_scroll() {
    let (_, out) = run("abc", 0, &[":", "w", "q", "enter"]);
    assert_eq!(out.host, Some(VimHostCommand::SaveClose));
    let (_, out) = run("abc", 0, &["2", "u"]);
    assert_eq!(out.history, Some((true, 2)));
    let (_, out) = run("abc", 0, &["z", "z"]);
    assert_eq!(out.scroll, Some(VimScroll::Center));
}
#[test]
fn utf8_offsets_remain_safe() {
    for cursor in 0.."α界😀\nβ".len() {
        let (_, out) = run("α界😀\nβ", cursor, &["l", "h", "x"]);
        assert!(out.text.is_char_boundary(out.cursor));
    }
}
#[path = "reference.rs"]
mod reference;
#[test]
fn clean_vim_reference_cases() {
    check_references(reference::CASES);
}
#[path = "file_reference.rs"]
mod file_reference;
#[test]
fn clean_vim_file_reference_cases() {
    check_references(file_reference::CASES);
}
fn check_references(cases: &[(&str, &str, &str, &str, usize)]) {
    let mut failures = Vec::new();
    for (name, source, commands, expected, cursor) in cases {
        let mut engine = VimEngine::new(Rc::new(RefCell::new(VimShared::default())));
        let mut out = engine.outcome(source, 0);
        for c in commands.chars() {
            let key = match c {
                '\u{1b}' => "escape".to_string(),
                '\u{16}' => "ctrl-v".to_string(),
                '\n' | '\r' => "enter".to_string(),
                _ => c.to_string(),
            };
            out = engine.key(&key, &out.text, out.cursor, true, 4);
            if !out.handled && engine.mode() == VimMode::Insert {
                let before = out.text.clone();
                out.text.insert(out.cursor, c);
                out.cursor += c.len_utf8();
                engine.native_edit(&before, &out.text, out.cursor);
            }
        }
        if out.text != *expected || out.cursor != *cursor {
            failures.push(format!(
                "{name}: got {:?}@{}, expected {:?}@{}",
                out.text, out.cursor, expected, cursor
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
