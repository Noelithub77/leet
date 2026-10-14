#!/usr/bin/env python3
"""Generate deterministic standard behavior with clean headless system Vim.

Run from any directory. Requires Vim; uses only Python's standard library.
Insert Ctrl+v is intentionally excluded because Leet specifies native paste.
"""
import json
from pathlib import Path
import subprocess
import tempfile

CASES = [
    ("motion-count", "one two three four five six seven", "2d3w"),
    ("delete-lines", "one two\nthree\nfour", "dj"),
    ("delete-last-line", "one\ntwo", "Gdd"),
    ("change-word", "one two", "cwX\x1b"),
    ("change-single-character-word", "x two", "cwY\x1b"),
    ("change-word-count", "one two three", "c2wX\x1b"),
    ("delete-final-word", "one two\nthree", "$bdw"),
    ("word-end", "one two three", "2e"),
    ("previous-word-end", "one two three", "wge"),
    ("WORD-previous-end", "one.two three.four", "WgE"),
    ("WORD-motion", "one.two three", "WE"),
    ("paragraph", "one\ntwo\n\nthree\n\nfour", "}"),
    ("matching-brackets", "call(one, [two])", "%"),
    ("find-till-repeat", "abcabcabc", "tc;"),
    ("find-backward", "abcabcabc", "$Fa"),
    ("inner-word", "one two", "wdiw"),
    ("around-word", "one two", "wdaw"),
    ("inner-parens", "call(one, [two])", "6ldi("),
    ("inner-quotes", 'say "hello" now', "6ldi\""),
    ("around-braces", "x{a{b}c}y", "4lda}"),
    ("visual-delete-forward", "abcdef", "lv2ld"),
    ("visual-delete-reverse", "abcdef", "3lv2hd"),
    ("visual-text-object", "one two three", "wvaw d".replace(" ", "")),
    ("visual-lines", "abc\ndef\nghi", "Vjd"),
    ("visual-switch", "abc\ndef\nghi", "vjVd"),
    ("visual-swap", "abcdef", "v2lod"),
    ("visual-reselect", "abcdef", "v2lylgvd"),
    ("visual-replace", "abcdef", "v2lrx"),
    ("visual-line-change", "abc\ndef\nghi", "VjcX\x1b"),
    ("visual-put", "one two three", "yiwwvep"),
    ("block-delete", "abcd\nefgh\nijkl", "l\x16jjld"),
    ("block-delete-tabs", "\tab\n1234ab", "\x16j3ld"),
    ("block-delete-wide", "界ab\n1234", "\x16jld"),
    ("block-short-rows", "abcd\nx\n\nwxyz", "l\x163jld"),
    ("block-insert", "abc\ndef\nghi", "\x16jIx\x1b"),
    ("block-append", "abc\ndef\nghi", "\x16jAx\x1b"),
    ("block-change", "abc\ndef\nghi", "\x16jlcx\x1b"),
    ("block-yank-put", "abcd\nefgh\nijkl", "l\x16jjldP"),
    ("block-repeat", "abc\ndef\nghi\njkl", "\x16jIx\x1bj."),
    ("block-insert-short", "abcd\nx\n\nwxyz", "2l\x163jIx\x1b"),
    ("block-append-short", "abcd\nx\n\nwxyz", "2l\x163jAx\x1b"),
    ("block-replace", "abcd\nefgh", "l\x16jlrX"),
    ("block-replace-tabs", "\tab\n1234ab", "\x16jrX"),
    ("block-insert-newline", "abc\ndef", "\x16jIx\ny\x1b"),
    ("delete-char", "abcdef", "2x"),
    ("delete-before", "abcdef", "3l2X"),
    ("replace-char", "abcdef", "3rx"),
    ("join", "abc\n  def\nghi", "3J"),
    ("indent", "abc\ndef", ">>"),
    ("line-put", "abc\ndef", "yyGp"),
    ("repeat-delete", "one two three", "dw."),
    ("repeat-insert", "abc", "iXYZ\x1b."),
    ("repeat-visual", "abcdefgh", "v2ld."),
]

CASES += [('audit-0', 'one   two  ', '2diw'), ('audit-1', 'one   two  ', '3ldaw'), ('audit-2', 'one   two  ', '3l2daw'), ('audit-3', 'one   two  ', '$daw'), ('audit-4', 'one\t two\nthree', '2diw'), ('audit-5', 'one\t two\nthree', '3ldaw'), ('audit-6', 'one\t two\nthree', '3l2daw'), ('audit-7', 'one\n  two', '2diw'), ('audit-8', 'xx "one" yy', '2diw'), ('audit-9', 'aa (b(c)d) ee', '2diw'), ('audit-10', 'x [a[b]c] y', '2diw'), ('audit-11', 'x (a(b)c) y', 'di('), ('audit-12', 'x (a(b)c) y', 'da('), ('audit-13', 'x [a[b]c] y', 'di['), ('audit-14', 'x [a[b]c] y', 'da['), ('audit-15', 'x {a{b}c} y', 'di{'), ('audit-16', 'x {a{b}c} y', 'da{'), ('audit-17', 'x (a(b)c) y', 'di('), ('audit-18', 'x (a(b)c) y', 'da('), ('audit-19', 'before "a\\"b" after', '9ldi"'), ('audit-20', 'before "a\\"b" after', '9lda"'), ('audit-21', 'before "a\\"b" after', 'di"'), ('audit-22', 'before "a\\"b" after', 'da"'), ('audit-23', 'before "a" "b" after', '11l2di"'), ('audit-24', "before 'a\\'b' after", "9ldi'"), ('audit-25', "before 'a\\'b' after", "9lda'"), ('audit-26', 'one   two  ', '3ldaw'), ('audit-27', 'one   two  ', '$daw'), ('audit-28', 'one   two  ', '3lcawX\x1b'), ('audit-37', 'one', 'ddp'), ('audit-41', 'one   \ntwo', 'wdw'), ('audit-42', 'one\n\ntwo', '2dw'), ('audit-43', 'one\n  two\n\nthree', '3J'), ('audit-44', 'one\n  two\n\nthree', '4J'), ('audit-45', 'one\n  two\n\nthree', 'jJ'), ('audit-46', 'one.\ntwo', 'J'), ('audit-47', 'a\n b\n  c\n    d', '>>3.'), ('audit-48', 'a\n b\n  c\n    d', 'G2<<')]

CASES += [
    ("combining-horizontal", "a\u0301bc", "lx"),
    ("combining-delete", "a\u0301bc", "x"),
    ("wide-horizontal", "界ab", "lx"),
    ("emoji-horizontal", "😀abc", "lx"),
    ("combining-block", "a\u0301bc\ndefg", "\x16jld"),
    ("block-wide-replace", "界abc\n12345", "\x16jlrX"),
    ("block-corners", "abcd\nefgh\nijkl", "l\x16jjlOd"),
    ("block-corners-swap", "abcd\nefgh\nijkl", "l\x16jjlod"),
    ("block-put-tabs", "\tab\n1234ab", "\x16jy$p"),
    ("block-I-partial-tab", "abcd\n\tab", "l\x16jIx\x1b"),
    ("block-A-partial-tab", "abcd\n\tab", "l\x16jAx\x1b"),
    ("block-change-short", "abcd\nx\n\nwxyz", "2l\x163jcx\x1b"),
    ("block-I-empty-first", "\nabc\ndef", "\x16jIx\x1b"),
    ("block-delete-paste-repeat", "abcd\nefgh\nijkl", "l\x16jldPj."),
    ("counted-insert", "abc", "3iX\x1b"),
    ("counted-open", "abc\ndef", "3oX\x1b"),
    ("repeat-count", "abcdefghi", "v2ld2."),
    ("counted-shift", "a\n b\n  c\n    d", ">>3."),
]

def vim_string(value):
    return json.dumps(value, ensure_ascii=False).replace("\\u001b", "\\<Esc>").replace("\\u0016", "\\<C-v>")


def generate():
    result = []
    with tempfile.TemporaryDirectory(prefix="leet-vim-reference-") as temporary:
        folder = Path(temporary)
        for name, source, commands in CASES:
            script = folder / "case.vim"
            output = folder / "result.json"
            script.write_text("set encoding=utf-8 tabstop=4 shiftwidth=4 expandtab noautoindent\n"
                + "call setline(1, " + json.dumps(source.split("\n"), ensure_ascii=False) + ")\n"
                + "execute \"normal! gg0\"\nexecute \"normal! \" . " + vim_string(commands) + "\n"
                + "let result = {'text': join(getline(1, '$'), \"\\n\"), 'line': line('.') - 1, 'column': col('.') - 1}\n"
                + "call writefile([json_encode(result)], '" + str(output) + "')\nqa!\n")
            subprocess.run(["vim", "-Nu", "NONE", "-U", "NONE", "-i", "NONE", "-n", "-N", "-es", "-S", str(script)], check=True)
            expected = json.loads(output.read_text())
            result.append({"name": name, "source": source, "commands": commands, **expected})
    destination = Path(__file__).with_name("reference.json")
    destination.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n")
    # A dependency-free Rust fixture consumed by the pure engine tests.
    rust = "// Generated by reference.py using vim -Nu NONE -U NONE -i NONE -n -N -es.\n"
    rust += "pub(super) const CASES: &[(&str, &str, &str, &str, usize)] = &[\n"
    def literal(value):
        return 'r###"' + value + '"###'
    for case in result:
        offset = sum(len(row.encode()) + 1 for row in case["text"].split("\n")[:case["line"]]) + case["column"]
        rust += "    (" + ", ".join(literal(case[field]) for field in ("name", "source", "commands", "text")) + ", " + str(offset) + "),\n"
    rust += "];\n"
    Path(__file__).with_name("reference.rs").write_text(rust)
    print(f"Generated {len(result)} clean Vim cases at {destination}")

if __name__ == "__main__":
    generate()
