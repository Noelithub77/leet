# Contests and provider menu

- [CHOSEN] Show upcoming Codeforces and LeetCode contests together, ordered by start time; keep browser actions as accent-colored icons.
- [CHOSEN] Open LeetCode contests in the native explorer, reusing existing question loading, editor, and judging. Preserve each provider's cached data on refresh failure.
- Show provider icons and the active NeetCode list in a right-opening hover submenu.
- Verify cache and contest isolation regressions, live public APIs, native rendering/menu behavior, project checks, focused commit, and local deployment.

Status: implementation complete. Project checks, live LeetCode API/cache regression, and native contest/editor, hover, and keyboard checks passed. Install the focused checkpoint with `./ops local:deploy --json`. No new dependencies or database migration.
