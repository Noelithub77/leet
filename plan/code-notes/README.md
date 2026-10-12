# Code notes

AI and user annotations stored as ordinary comments in the solution file.

## Decisions

- [chosen] AI notes are real comments written into the file, saved with Git history, and read by the AI as context.
- [chosen] Syntax: `#!` for AI notes, `#?` for your questions, plain `#` for your own notes (`//!`, `//?`, `//` in C-family languages). A backtick snippet in a note anchors its highlight on the next code line.
- [chosen] Views: Rich hides `#!` lines and only highlights the code they are about, with every detail on hover. Raw shows plain text. Off hides `#!` and `#?` lines. Hidden lines renumber the visible ones and stay out of copies.
- [chosen] First version: lifecycle (outdated, resolved, stripped on submit), cost and trace badges, the reasoning trail, and asking in place.
- Rich and Off are projections: the editor holds the code without the hidden lines, which are reattached on every save, run, and AI request. This avoids patching cursor, selection, and copy in the vendored editor, and line numbers and copy follow naturally. Switching views resets undo history, and resolving or clearing hidden notes cannot be undone in the editor.
- Leet writes notes from typed answers, not from agent file edits: bugs, stuck (with optional steps), optimize bottleneck, and in-place answers. A rerun replaces earlier notes of the same kind. Notes go in as one undoable edit, and only into the active tab.
- Costs and trace values are hover details in Rich, not written to the file and not drawn inline, so the vendored editor stays unpatched. Trace values are the last values of variables each line names, for the selected case, after leaving Debug.
- Hovers come from a notes provider that adds the language server's hover below its own. Its part is a card (kind chip, step bar, summary, labelled detail, values, shortcut keys) carried as a `leet-note` fenced block and drawn by a Markdown plugin installed through the vendored `PopoverMarkdown` hook. Note shortcuts act on the hovered line while its card is open. A highlighted line takes one fill colored by its most severe note. Note actions are code actions on `alt-enter`: Ask AI (on a `#?` line), Resolve note (on a highlighted line), Ask about this line, Clear AI notes. All are also palette commands.
- Chat line chips peek at the cited line on hover, jump and flash it on click, and follow edits and the Off view's numbering.
- Lifecycle: notes whose quoted code changed fade. When all local tests pass and AI notes remain, the toast offers Clear notes.
- Submissions, judge runs, and external-judge clipboard copies strip `#!` and `#?` lines. Local runs use the file as is, since notes are comments.

## Work

- [x] `practice::notes`: parse, projection, stripping, note specs from answers, relocation. Unit tested.
- [x] Assist schemas: optional `snippet` on bugs and stuck, stuck `steps`. Older answers stay readable.
- [x] GUI: highlights, view toggle, hover and code actions, trail navigation, ask in place, chips, hover badges, lifecycle.
- [x] Focused GUI fixture: `./ops gui:test --fixture notes`.

## Status

Built. Not yet exercised against a live agent run.
