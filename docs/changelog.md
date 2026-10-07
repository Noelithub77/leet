## 2026-10-07 · Cleaner shortcut labels

- Shortcut help and search show action names without the repeated “Shortcut:” prefix.

## 2026-10-07 · Launch updates and portable Linux releases

- Check the latest stable release once per launch. The bottom-right update button has a badge when an update is available; hover for the release changelog and click to download, verify, install, and restart.
- Save editor state before restarting and preserve settings, accounts, progress, caches, and solutions. Failed verification leaves the installed app intact.
- Move the changelog out of Settings. When release notes are unavailable, show the bundled changelog on hover.
- Build Linux archives and AppImages in the same native jobs, using the same executable. Cache and verify pinned AppImage packaging tools, and retain compiler flags in native build cache keys.

## 2026-10-07 · Contest explorer status

- The selected contest explorer has browser and refresh actions beside its heading, plus visible loading and unavailable status.
- Contest list refreshes no longer clear a problem download error. Cached problems remain available while refreshing.
- Refreshed contest 2275 into the local cache: eight problems are available.

## 2026-10-07 · Sarah's Pink themes

- Rename the light theme to Sarah's Pink and add a matching dark variant.

## 2026-10-07 · Home contest tab

- Home has separate Recent and Contests tabs, with Contests to the right. The contest list no longer sits below recent questions.
- Ctrl+/ opens the keyboard-bindings-only help view, including while editing code.
- Ctrl+Alt+C opens the Contests tab; Left/Right switches Home tabs, and Up/Down plus Enter navigates the active list.
- Opening a contest or toggling past contests selects the Contests tab. Existing cached-first refresh and contest explorer behavior remain available.

## 2026-10-07 · Shortcut help, pinned Home, and contests

- Ctrl+/ opens searchable shortcut help with native keycaps, all active alternatives, custom overrides, and contextual editor/form shortcuts. Select an application shortcut to edit it.
- Ctrl+K searches commands and current key combinations, including contextual bindings. Saving a shortcut refreshes the search index immediately.
- The clickable hint footer cycles through hints and hides them after the last. A close icon hides partial hints immediately; the displayed shortcut follows custom bindings.
- Home stays pinned outside the scrolling problem tabs. Tab labels have extra vertical room and line height for letters such as g and y.
- Home lists three upcoming/live Codeforces contests and a collapsed past-contest section. Ctrl+Alt+C focuses contests; arrows and Enter open the selected contest. Contests are also searchable through Ctrl+K.
- Opening a contest scopes the explorer to its problems. Saved contest data appears immediately; every Home/contest visit revalidates in the background, with one pending request per key and cached data retained on errors.
- Ctrl+Alt+Enter copies Codeforces solutions and opens that problem’s web submission page. Use the contest browser action for registration and contests whose problems are not published yet.
- `./ops contests:refresh [--contest ID] --json` refreshes and reports the private contest cache.

## 2026-10-07 · Codeforces snapshot compatibility

- Historical Codeforces statements load when the snapshot has null or omitted samples/tags, including contest 1001 quantum problems. No sample tests are fabricated.
- Malformed sample records and empty statements still produce actionable errors instead of entering the cache as complete questions.

## 2026-10-07 · Faster release builds

- Cache Cargo downloads and compiled dependencies separately for native release targets and Linux tests; cache the landing page's pnpm store by its lockfile.
- Run tests alongside platform builds and share the release profile between the app, checks, and packaging tool.
- Add a manual build-only run that warms caches without publishing. Keep release optimization enabled and avoid recompressing release archives during upload.

## 2026-10-07 · Shared question view preferences

- Video explanations have a compact play button beside the Question, Solution, and Tags toolbar icons. Question shortcut hints sit in a fixed footer below the scrolling content.
- Description, Examples, and Constraints expansion states are shared across every question and tab and saved in the private SQLite cache for future launches. New installs expand only Description.
- Ctrl+Up / Ctrl+Down open the previous / next question in the current explorer list for NeetCode, LeetCode, and Codeforces. Alt+P / Alt+N remain available; shortcuts are editable in Settings.

## 2026-10-07 · leet for the eleet landing page

- Larger Vesper teal landing page with the app logo, “leet for the eleet” tagline, and brief Rust/native performance copy.
- Linux and macOS share one install command. Ctrl+C or Cmd+C copies it, with shadcn keyboard badges in the Copy button; selected text and form controls retain normal copy behavior.
- Removed instructional filler and persistent copy-status text. Feedback appears briefly in the button.
- Removed the requirements section and footer; the GitHub link is a single header icon. Vesper peach accents complement the teal headline and primary action.
- Logo splash displays while the page loads and disappears when the page is ready, without a forced delay.

## 2026-10-07 · Public desktop releases

- Minimal platform-aware download page, with one command to install and open leet on Linux/macOS and a portable Windows x64 executable.
- GitHub Actions build native Linux x86_64/ARM64, macOS Intel/Apple Silicon, and Windows x64 artifacts from version tags or a manual release run. Releases publish after all builds succeed and include SHA-256 checksums.
- macOS accounts use Keychain; Windows accounts use Credential Manager. Linux retains compatible Secret Service/private-file storage.
- Windows local runners use executable suffixes and the platform's Python command. Windows launches without a terminal window.
- First builds are unsigned; OS security prompts remain possible. Requirements and local toolchain limits are documented in `docs/releases.md`.

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
## 2026-10-07 · Contest Python and reliable Codeforces loading

- Codeforces falls back to the approved historical Open-R1 snapshot when live statements fail. A 381 KB public identity index locates one row on demand; downloaded statements and examples persist in the private SQLite cache.
- Native Axum/Tokio receiver accepts Competitive Companion Codeforces samples on `127.0.0.1:13337`. Settings expose enable/disable and the port. Imported samples persist without replacing complete statements or solution files.
- Python IntelliSense recognises preloaded contest helpers and judge node types. Python diagnostics show syntax/indentation errors and suppress development type-checking noise; completion and documentation remain available.
- Local Python runners preload common contest modules/helpers. Solution only includes the active judge interface and language; a wrong method is caught before test execution.
- Description scrolling is faster. Description opens expanded; Examples and Constraints are separate expandable sections, initially collapsed.
- Sarah Pink is a bundled light theme with blush surfaces, rose accents, and pastel lavender details. Vesper remains the default.
- Restored the bottom-left leet label. The language uses its logo with a tooltip; clicking the AI mode opens its prompt configuration.
- Removed the obsolete `vg` launcher. Deployments maintain `leet` and `1337` and preserve legacy user-data paths.
