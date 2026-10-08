# Agent chat and native sidebars

## Chosen

- Separate explorer and description panes to the left of the editor; AI/chat on the right.
- Alt+S toggles the explorer; Alt+A toggles description; Alt+D toggles the right AI pane.
- One locally saved conversation per problem. Actions accept optional instructions; questions use current statement, code, tests, and prior answers.
- Existing GPUI Kit resizable panels, textarea, menus, Markdown, and scroll containers. No new dependencies or custom drag implementation.
- Keep structured action results and Solve's explicit submission confirmation. Questions remain read-only.

## Work

- [x] Add bounded conversation prompts and preserve compatible saved action answers.
- [x] Add prompt composer, chronological conversation, and follow-up replies.
- [x] Integrate native resizable explorer, description, and AI panes; update shortcuts and tour.
- [x] Verify regression checks and native UI/keyboard/resize/history.
- Final milestone operation: focused commit and `./ops local:deploy --json`.

## Evidence

- Isolated native Wayland session: explorer, description, and AI shortcuts; all four panes; native dragging and saved widths; streamed question; action instructions; chronological history restored after restart.
- Deterministic Claude-protocol fixture confirmed current code, recent conversation, read-only flags, and Hints schema in actual requests. No live-agent response-quality claim.
