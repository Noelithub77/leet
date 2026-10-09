# Snippets

Decisions (confirmed by the user):

- Expansion: prefix + Tab, completion-menu entries, and a Ctrl+J fuzzy picker with live preview. Tab / Shift+Tab move between stops; a caret glides between them.
- Import: copy once into leet's own VS Code–format files; editable; Rescan adds new ones without duplicates.
- Snippet editor: a full-width view like Settings (Ctrl+Shift+S, global search, command palette) with language icons, searchable list, editor, draggable stop/cursor drop points, and live preview.
- Motion: inserted snippets glide in; a glowing caret glides between stops; active stop highlighted.
- AI: ✦ panel in the snippet editor; chat agents also get snippet context. Both use the validated `leet snippets` CLI (list/show/set/remove, JSON). leet snapshots the library around agent turns and offers Undo.
- AI questions: ✦ in the snippet editor. The agent asks preference cards (ACP `elicitation/create`, or a structured `ask` turn for other agents), then creates and edits snippet files; leet validates and shows the change before saving.

Layout:

- `crates/practice/src/snippets/mod.rs`: shared types.
- `snippets/body.rs`: TextMate/VS Code body parser and expansion (stops, mirrors, choices, variables, transforms).
- `snippets/store.rs`: per-language JSON (JSONC tolerant) in `<config>/snippets/`, builtin merge, validation.
- `snippets/import.rs`: VS Code (+ Insiders, VSCodium, Cursor, Windsurf, profiles), Neovim (VS Code JSON, SnipMate, UltiSnips), Sublime (`.sublime-snippet`).
- `snippets/cli.rs`: the agent-facing `leet snippets` command.
- `snippets/builtin/*.json`: competitive-programming library and file templates.
- `crates/gui/src/snippets/`: editor session (expansion, stops, mirrors, glide overlay), picker, full-width editor, onboarding step, AI panel.
- Background processes must never open console windows on Windows.

Status: implementation complete and verified. Install the committed milestone with `./ops local:deploy --json`.

Verification covers parser/import/storage regressions, bundled template compilers, Unicode and nested linked edits, ACP question replay, and production GPUI input plus offscreen layouts. Native Windows console behavior and live provider generation remain unverified.

Release target: 0.3.0. Publishing is pending the requested scope decision: current `main` includes ten earlier unpublished commits in addition to this milestone. Preserve that work; do not publish or isolate it without the user's answer.
