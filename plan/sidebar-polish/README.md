# Sidebar and AI polish

[CHOSEN] Alt+S Explorer, Alt+A Description, Alt+D AI. Explorer and Description toggles beside Home; AI and bottom-panel toggles at the top right. Problem tabs show a Code icon. Stable native resize slots preserve pane identity. Rounded outlines use a very dim accent; native resize handles paint only on hover or drag.

[CHOSEN] AI tabs: General (action grid), My solution (one combined review), Conversation (per-problem history and questions). Review covers correctness, bugs, complexity, improvements, and an animated dry run. Edge cases live beside case tabs and open a native count/type/instructions dialog.

- [x] Implement controls, tabs, combined review, case dialog, rounded outlines, and stable slots.
- [x] Verify native toggling, resizing, animation settling, and prompt behavior (isolated Wayland session and deterministic agent fixture).
- [x] Run final checks: 30 GUI tests, 121 practice tests, doc tests, and five installer regressions.

Release the focused checkpoint with `./ops local:deploy --json`; keep concurrent provider work outside the source snapshot.

## Follow-up UI

[CHOSEN] One compact AI row with General, Analysis, and Chats; centered empty action and analysis views; `__` analysis placeholders with a Run analysis button. Edge cases use the AI sparkle. Results have an independent rounded frame and native vertical resizing with saved height. Constraints stays in the statement footer; opening it closes other sections.

Threaded chat storage, global and per-question threads, forks, editing, undo, deletion, and action request cards are owned by the separate Threaded AI conversations and history chat.

Verified follow-up in isolated native Wayland: centered General and analysis placeholders, in-place analysis result, vertical results drag and toggle restoration, and pinned Constraints after expanding Examples. Focused check passed 31 GUI tests, 121 practice tests, doc tests, and five installer regressions.

[CHOSEN] General shortcuts use a fixed three-column, two-row grid with larger icons. AI tabs use normal-size labels and more spacing within the single navigation row.
