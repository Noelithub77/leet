# Source menu

- [x] Confirm saved NeetCode list and menu API behavior.
- [x] Use the existing Popover pattern for a clickable NeetCode row and hover list choices.
- [x] Preserve arrow-key, Enter, and Escape navigation; add selection regressions.
- [x] Run workspace and installer checks; inspect native mouse and keyboard behavior.
- [x] Commit scoped changes and deploy locally.

Clicking NeetCode selects the saved list, initially NeetCode 150. Hover reveals
150, 250, and All; clicking a list saves it and selects NeetCode. No config migration
or dependencies are needed.

Verified the native debug app with isolated configuration: default 150, hover
choices, persisted 250 after switching providers, and keyboard provider/list
selection plus Escape dismissal. `./ops check --json` passed 36 GUI tests,
136 practice tests (14 ignored), and five installer regressions.

Implementation committed as `a607208`. `./ops local:deploy --json` succeeded,
installing `v0.1.0-81-ga607208` and verifying the launcher links and desktop entry.
Native interaction was inspected in the debug build; release installation was
verified by the operator.
