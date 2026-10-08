# Sidebar and AI polish

[CHOSEN] Alt+S Explorer, Alt+A Description, Alt+D AI. Explorer and Description toggles beside Home; AI and bottom-panel toggles at the top right. Problem tabs show a Code icon. Stable native resize slots preserve pane identity. Rounded outlines use a very dim accent; native resize handles paint only on hover or drag.

[CHOSEN] AI tabs: General (action grid), My solution (one combined review), Conversation (per-problem history and questions). Review covers correctness, bugs, complexity, improvements, and an animated dry run. Edge cases live beside case tabs and open a native count/type/instructions dialog.

- [x] Implement controls, tabs, combined review, case dialog, rounded outlines, and stable slots.
- [x] Verify native toggling, resizing, animation settling, and prompt behavior (isolated Wayland session and deterministic agent fixture).
- [x] Run final checks: 30 GUI tests, 121 practice tests, doc tests, and five installer regressions.

Release the focused checkpoint with `./ops local:deploy --json`; keep concurrent provider work outside the source snapshot.
