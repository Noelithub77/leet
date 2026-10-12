# Code notes

AI and user annotations stored as ordinary comments in the solution file.

## Decisions

- [chosen] AI notes are real comments written into the file, saved with Git history, and read by the AI as context.
- [chosen] Syntax: `#!` for AI notes, `#?` for your questions, plain `#` for your own notes (`//!`, `//?`, `//` in C-family languages). A backtick snippet in a note anchors its highlight on the next code line.
- [chosen] Views: Rich (decorated), Raw (plain text), Off. Off hides only `#!` and `#?` lines, renumbers visible lines, and copying excludes the hidden lines.
- [chosen] First version: lifecycle (outdated, resolved, stripped on submit), cost and trace badges, the reasoning trail, and asking in place.
- Off is a projection: the editor holds the code without note lines, and the hidden lines are reattached on every save, run, and AI request. This avoids patching cursor, selection, and copy in the vendored editor, and line numbers and copy follow naturally. Switching views resets undo history.
- Leet writes notes from typed answers, not from agent file edits: bugs, stuck (with optional steps), optimize bottleneck, and in-place answers. A rerun replaces earlier notes of the same kind. Notes go in as one undoable edit, and only into the active tab.
- Cost and trace badges are virtual line-end hints, shown in Rich only and not written to the file. The vendored `gpui-base` gains a narrow `set_line_end_hints` hook; hints clear on edits, so the pane resends them after each change. Trace badges show the last values of variables each line names, for the selected case, after leaving Debug.
- Hovers come from a notes provider that falls back to the language server. Note actions are code actions on `alt-enter`: Ask AI (on a `#?` line), Ask about this line, Resolve note, Clear AI notes. All are also palette commands.
- Chat line chips peek at the cited line on hover, jump and flash it on click, and follow edits and the Off view's numbering.
- Lifecycle: notes whose quoted code changed fade. When all local tests pass and AI notes remain, the toast offers Clear notes.
- Submissions, judge runs, and external-judge clipboard copies strip `#!` and `#?` lines. Local runs use the file as is, since notes are comments.

## Work

- [x] `practice::notes`: parse, projection, stripping, note specs from answers, relocation. Unit tested.
- [x] Assist schemas: optional `snippet` on bugs and stuck, stuck `steps`. Older answers stay readable.
- [x] Vendored hook: line-end hints.
- [x] GUI: decorations, view toggle, hover and code actions, trail navigation, ask in place, chips, badges, lifecycle.
- [x] Focused GUI fixture: `./ops gui:test --fixture notes`.

## Status

Built. Not yet exercised against a live agent run.
