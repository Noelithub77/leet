## 2026-10-07 · 1337 launch alias

- `1337` launches the same installed app as `leet` and `vg`. The Rust installer updates and verifies all three symlinks at each deployment.

## 2026-10-07 · Editable cases and calmer chrome

- Active problem tabs have white labels on a cyan-tinted background with 12 px rounded corners. Existing scrolling, Home, close actions, and tab shortcuts remain available.
- Removed persistent app-name and practice-progress text from the status bar.
- Results offer compact Add, Edit, and Restore icons. Input and expected output are editable through the existing textarea dialog; Ctrl+Enter or Save commits changes.
- Editing an original case offers Use original example, restoring only that case before saving.
- Ctrl+Alt+T adds a case; Ctrl+Alt+Shift+T edits the selected case. Restore question examples removes edits/custom cases from the active set.
- Per-question case overrides persist in the private Diesel/SQLite cache. Case mutations are blocked during runs and clear stale results; the original question remains intact.

## 2026-10-07 · leet

- Crates are named by responsibility: `gui` and `practice`; the executable is `leet`.
- Standalone Rust application and deployment operator; no Go/verd wrapper or runtime dependency.
- Extracted the Rust desktop app into its own repository with standalone safe-Rust operator commands and agent guidance.
- The app, command, and desktop entry are named leet. Existing vg settings, credentials, cache, and solutions are preserved; vg remains a launch alias.
- Original Vesper icon: an angled off-white L, peach baseline, and small peach code accent on a near-black rounded tile.
- Small 1337 Easter eggs: `leet --1337` and triple-click Home.
- Diagnostic messages appear below the cursor line on hover, keeping the source clear.

## 2026-10-07 · Offline practice and complete solution articles

- New users start with NeetCode 150. An 11.2 MB tracked base SQLite seeds full NeetCode 150 statements, solution articles, reference code, and metadata-only LeetCode/Codeforces catalogs without replacing personal data.
- Full solution articles show approaches, algorithms, complexity, prerequisites, and pitfalls. Video links open playback in the browser. Source data and upstream language omissions are documented in `docs/bundle.md`.
- Ctrl+K/Ctrl+P search every cached provider regardless of the explorer source, with accented SVG provider marks.
- Tags are hidden initially; the tag icon or Ctrl+Shift+T reveals chips with topic icons. Visibility persists across questions and restarts.
- Active tabs use cyan; constraints have a lavender box and examples use blue Input / teal Output labels.
- Explorer navigation is scoped to its focused view so Enter accepts editor completions instead of opening an explorer question. Home search keeps input focus.
- Diagnostics keep squiggles and reveal their message below hovered text. Completion results resolve documentation for the first 32 ranked suggestions and use the toolkit's adjacent documentation card.
- The right History tab combines local commits, the staged draft, account-scoped LeetCode submissions, and available NeetCode saved code tabs. Selecting a version stages the current code before restoring it; unrelated staged solutions stay out of acceptance commits.
- `./ops snapshot` refreshes the public bundle without reading account credentials.

## 2026-10-07 · Visible teal selections

- Selected onboarding languages, practice sources, and explorer topics use a teal border, label, and tinted background.
- Vesper uses teal for focused controls, active lists, and text selection.

## 2026-10-07 · Rich questions and cached reference solutions

- Larger problem headings, rounded example cards, themed Input/Output labels, highlighted values, and clearer constraints retain statement content.
- Icon tabs switch Question / Solution. Ctrl+Alt+V toggles the reference view; Ctrl+2 focuses the description for arrow and Page Up/Page Down scrolling.
- Fetch public NeetCode reference code inside the app, choose Python/C++/Go/C, and cache it in SQLite. Copy and source actions plus MIT attribution stay separate from your own solution.
- Language-server diagnostics wake the UI on notifications, eliminating idle document polling.
- The source dropdown also has a configurable Ctrl+Alt+O shortcut. Explicit refresh updates Codeforces’s catalog while preserving cached data after errors.

## 2026-10-07 · Native editor IntelliSense

- Shared Python, C++, Go, and C language servers provide completion menus, hover documentation, definitions, and versioned inline diagnostics.
- Ctrl+Space requests completions; Ctrl+F12 follows a definition. Current-file definitions stay in the editor, open solution tabs are reused, and library files open in the configured external editor.
- Reuse installed basedpyright/pyright, clangd, and gopls. Python can also reuse Zed’s installed basedpyright server.
- Settings can restart the current language server; the status bar exposes availability and server details on hover.
- Server processes and documents follow editor lifetimes; code actions use existing toolkit menus. Workspace-wide edits remain delegated to the external editor.

## 2026-10-07 · Compact explorer and native onboarding

- Topic icons form a compact grid; only the selected topic’s questions appear below it. Ctrl+0 focuses the grid, arrow keys select topics, and Enter opens the first question. Left from the question list returns to the grid.
- One dropdown switches NeetCode, LeetCode, and Codeforces, including NeetCode list selection.
- First-run onboarding offers Python, C++, Go, and C, account setup, and a Codeforces handle. Settings can run onboarding again with saved choices.
- New solutions use language-specific starters and files. Python LeetCode tests run locally; other LeetCode languages use its signed-in judge. Codeforces samples compile/run locally in all four languages.
- Native Codeforces catalog, progress, and statements are cached; failed requests preserve existing data. Codeforces submissions copy the solution and open its browser submission form.
- The roadmap title sits at the top; shortcut text was removed and selection fades on stationary nodes.

## 2026-10-07 · Roadmap topic panels and shortcuts

- Click a roadmap topic or press Enter to open its progress and problem list beside the graph.
- Prerequisite topics link back through the existing roadmap; problem rows show solved state, difficulty, persistent stars, and source/video actions.
- Arrow keys select problems, Enter opens a problem tab, Left returns to the graph, Right returns to the panel, and Escape closes the panel before leaving the roadmap.
- Ctrl+E opens the current solution in the external editor; Alt+E runs AI assist. Ctrl+Shift+E keeps the AI configuration popup.
- Ctrl+O opens NeetCode for roadmap problems and LeetCode for other LeetCode problems.

## 2026-10-07 · Navigation, fonts, and editor AI

- Ctrl+Tab and Ctrl+Shift+Tab cycle only problem tabs, skipping Home.
- Ctrl+. opens Home; its hover tooltip shows the current shortcut.
- Typing on Home opens universal search with the typed text, like Ctrl+K.
- Clicking a sidebar topic or pressing Enter now toggles expansion; collapse no longer reopens the topic.

- Liberation Sans is the default UI font; Settings lists every installed system font and universal search finds font families.
- Provider controls use ChatGPT, Claude, and Gemini brand icons with tooltips. Removed the model selector; websites use the account’s model.
- Rounded tabs, panels, and component corners keep the interface consistent across themes.
- Ctrl+E opens AI assist; Ctrl+Shift+E configures the assistant, style, and prompt instructions without leaving the editor.

## 2026-10-07 · Local deployment and release notes

- `./ops local:deploy --json` updates the desktop launcher, icon, `leet`, and `verd`, then verifies both installed commands.
- Settings now includes the installed build version and this changelog.

## 2026-10-07 · Home, problem tabs, and Settings

- Pinned, icon-only Home tab opens by default with cached recent problems.
- Multiple problem tabs keep independent edits, undo history, statements, and test results. Ctrl+Tab and Ctrl+Shift+Tab cycle tabs; Ctrl+W closes the current problem.
- Background loads and Python runs finish in their original tab.
- Keyboard shortcuts can be edited in Settings, with conflict detection and immediate application.
- Each AI prompt style supports custom instructions while retaining problem and failure context.
- Native Rust account settings validate LeetCode sessions and import, refresh, and sync NeetCode sessions.
- Cached fuzzy search tolerates typos; the roadmap scales to the window.

## Initial desktop app

- Vesper by default, bundled themes, Python editor, and NeetCode roadmap.
- Local tests, LeetCode runs and submissions, solution Git history, and AI prompts.
## 2026-10-07 · Smooth roadmap topic panels

- Topic panels open and close with a 280 ms eased reveal and content fade. The graph uses the same animated width, avoiding a sudden layout jump.
- Panel contents retain their final width during the reveal; rapid opening/closing reverses the current transition. The toolkit respects reduced-motion preferences.
