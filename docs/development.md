# leet (GPUI desktop app)

`leet` is the standalone native desktop app in this repository: the NeetCode roadmap, a Python editor, local tests, the LeetCode judge, Solution history, and AI learning prompts in one keyboard-driven window. On Linux it runs beside the Go `verd` TUI and shares its LeetCode session. For downloadable cross-platform builds, see [installation and releases](releases.md).

```sh
./ops local:deploy --json   # release build, link ~/.local/bin/leet, desktop entry + icon
./ops check --json     # cargo test --workspace
./ops local:deploy --json # update this PC’s leet desktop app after each milestone
leet                      # open; `leet <dir>` uses <dir> as the workspace for this launch
leet --version
```

Builds run with `nice` and at most 4 parallel jobs (`.cargo/config.toml`). Each install keeps a versioned binary in `~/.local/share/leet/bin` (the newest 3 are kept) and repoints the `leet` symlink. Each verified development milestone is installed into the same command and desktop launcher. A running window notices the new build and shows **Update ready**; `ctrl+shift+r` restarts into it, opening Home with the saved recent problems and panel layout.

## Layout

One window with a pinned, icon-only Home tab and multiple problem tabs. Home
opens on every launch, showing up to 12 recent problems from the local cache.
Opening a recent problem uses the cached statement and existing Solution.
Each problem keeps its own editor, undo history, cursor, and test results.
Opening an already open problem selects its existing tab. Background runs finish
in their originating tab. Switching back to Home keeps every problem intact.
Hover the house icon for its tooltip. Ctrl+. opens Home; its tooltip shows the current shortcut. Ctrl+Tab and Ctrl+Shift+Tab cycle forward and backward through problem tabs, skipping Home. Typing on Home opens universal search with the typed text. Ctrl+W closes the problem tab after saving;
Home stays pinned. Up/down and Enter navigate the focused recent list.

Every panel has one toggle, and a spring animation shows or hides it.

| Area | Toggle | Contents |
| --- | --- | --- |
| Left | `alt+s` | Roadmap topics and problems; `ctrl+g` swaps to Solution history |
| Center | `ctrl+1` | Python editor (tree-sitter highlighting) |
| Right | `alt+d` | Statement, revealed hints; diffs while in history |
| Bottom | `alt+x` | Test cases, outputs, LeetCode judge result |
| Roadmap | `alt+r` | neetcode.io/roadmap graph with per-topic progress |

`alt+z` hides everything but the editor. The status bar briefly shows each shortcut you use.

## Keys

Modifier shortcuts follow the user's VS Code bindings. Bare arrows, `enter` and `escape` navigate only the focused Home, sidebar, Settings, or roadmap. `ctrl+shift+p` lists every command with its key.

| Key | Action |
| --- | --- |
| `ctrl+.` | Open the pinned Home view |
| `ctrl+tab` / `ctrl+shift+tab` | Next / previous tab |
| `ctrl+w` | Close current problem tab |
| `ctrl+p` | Find any free LeetCode problem (fuzzy, by name or number) |
| `ctrl+enter` | Run tests locally |
| `ctrl+shift+enter` | Run on the LeetCode judge |
| `ctrl+alt+enter` | Submit; **Accepted** commits the Solution to history |
| `alt+n` / `alt+p` | Next / previous roadmap problem |
| `alt+.` / `alt+,` | Next / previous test case |
| `ctrl+alt+t` | Add a custom test (`ctrl+enter` saves) |
| `ctrl+alt+shift+t` | Edit the selected test case |
| `ctrl+alt+1`…`4` | AI prompt: hints only, guided learning, full explanation, solution only |
| `alt+e` | AI assist with the default style |
| `ctrl+shift+e` | Configure AI in an editor overlay |
| `ctrl+alt+a` | Cycle AI between ChatGPT, Claude, and Gemini |
| `ctrl+alt+s` | Change the default prompt style (used by `alt+e`) |
| `ctrl+alt+h` | Reveal the next LeetCode hint |
| `ctrl+alt+l` | Cycle NeetCode 150 → 250 → All |
| `ctrl+0` / `ctrl+1` | Focus sidebar / editor |
| `ctrl+e` | Open the Solution in the external editor (changes reload automatically) |
| `ctrl+o` | Open the problem source (NeetCode for roadmap problems, otherwise LeetCode) |
| `ctrl+k` / `ctrl+shift+p` | Search problems, commands, settings and themes |
| `ctrl+,` | Settings, accounts, keyboard shortcuts and prompt instructions |
| `ctrl+=` / `ctrl+-` | Zoom in / out |
| `ctrl+alt+m` | Toggle NeetCode completion |
| `ctrl+alt+r` | Refresh the catalog and solved progress |
| `ctrl+q` | Quit |

## Tests

Local runs execute every case in one Python process through `crates/practice/src/harness.py`. The harness parses LeetCode inputs from the question's metadata: lists, strings, `ListNode`, `TreeNode`, in-place (`void`) outputs, and design-class call sequences. Common imports (`collections`, `heapq`, `typing`, ...) are preloaded like on LeetCode. Outputs compare as JSON; statements that say "any order" compare order-insensitively, and floats within 1e-5. The default timeout is 10 s for a whole run.

Judge runs and submissions use the shared LeetCode session (Secret Service entry `service=verd`, `username=leetcode`, or `~/.config/verd/credentials-leetcode.json`). Sign in through the LeetCode account row in Settings. The Go CLI’s `verd login leetcode --browser auto` remains an alternative.

Click a roadmap node or press Enter to open a topic panel beside the graph. It shows
cached progress, problems in the selected list, prerequisite topics, persistent
stars, and source/video links. Up/Down select a problem; Enter opens its tab. Left
returns focus to the graph and Right returns to the panel. Escape closes the
panel first, then leaves the roadmap. Stars are stored locally in SQLite.

## AI prompts

Alt+E opens AI assist with your configured defaults. Ctrl+Shift+E opens a compact
editor overlay for assistant, prompt style, and per-style
instructions. Ctrl+Enter saves; Escape returns to the editor.

A prompt includes the statement, your current code unless it is still the starter, and up to three failing cases. ChatGPT and Claude open with the prompt in `q`; Gemini opens `gemini.google.com/app` with the prompt copied for pasting. The selected account controls its model. Prompts are always copied to the clipboard in case a website drops a long URL. ChatGPT and Claude URL prefill is best-effort.

## Files

| Path | Contents |
| --- | --- |
| `~/.config/leet/config.toml` | New Linux installs: workspace, theme, Python, external editor, list, AI provider/style, timeout |
| `~/.local/share/leet/leet.db` | New Linux installs: catalog, questions, solved marks, custom tests, layout |
| `~/leet/leetcode/<id>-<slug>.py` | New solutions (the workspace is a git repository; commits only on Accepted) |
| `crates/gui/themes/` | Bundled themes; Vesper is the default |

Existing `vg` configuration/database paths are detected and retained. macOS and Windows use their OS configuration/data directories through `dirs`; credential storage uses the native OS keyring. User data is separate from release binaries on every platform.

## Settings and keyboard customization

The default UI font is Liberation Sans. **Font** opens a searchable list of all
installed system font families; Enter applies the choice immediately and saves
it across restarts and theme changes. Ctrl+K also finds fonts by family name or
`font <name>`. The Python editor retains its monospace font.

**Changelog** in Settings shows the running build version and bundled release
notes from [changelog.md](changelog.md). Search `changelog` with Ctrl+K
to jump to that row.

`ctrl+,` opens Settings. Up/down select a row, left/right change a choice,
Enter edits, and Escape returns. The page scrolls with keyboard selection.
Settings are also searchable from `ctrl+k`: prefix `>` for settings and
commands, or `#` for topics. `ctrl+p` searches problems directly. The roadmap
fits the available window; `ctrl+=` and `ctrl+-` adjust interface scale.

Open **Keyboard shortcuts** in Settings to edit any application command.
Use GPUI syntax such as `ctrl-alt-k`; separate alternatives with `|` and chord
steps with spaces. Each step requires Ctrl, Alt or Super. Conflicting keys and
chord prefixes are rejected. Empty restores the command's default; `none`
disables it. Changes apply immediately and persist in `[keybindings]` in
`config.toml`, keyed by stable action names. Search shows the current binding; searching `shortcut run tests`, for example,
opens that command directly in the editor. Visible application key hints also
use the current bindings.
Focused arrow/Enter/Escape navigation and the editor's own text-editing bindings
remain provided by GPUI.

Each AI style has a multiline instruction editor in Settings. Ctrl+Enter saves;
Escape cancels. Empty restores its built-in instructions. The app still adds
the problem, your attempt and failing tests as appropriate to the selected style.
Instructions persist under `[prompt_instructions]`.

## Native account setup

Open **LeetCode account** or **NeetCode account** in Settings. These forms and
network clients run in Rust; leet does not invoke `verd login`. Both apps share
the existing OS keyring/private-file credential format.

For LeetCode, open the browser, sign in, and copy the Cookie and User-Agent
values from the same `leetcode.com/graphql/` Network request into the form.
The Cookie field is masked. The session's signed-in identity is validated
before saving; leet then refreshes the catalog and LeetCode progress.
Automatic browser-cookie discovery is still available in the Go CLI, but is
not part of the Rust form.

For NeetCode, open neetcode.io and sign in. **Copy export script** copies the
existing read-only browser-session export helper. Run it in that page's browser
console, then paste the resulting JSON into the masked form. leet exchanges the
refresh token using the [Firebase Auth REST API](https://firebase.google.com/docs/reference/rest/auth#section-refresh-token),
checks the returned user identity, and saves only after validation. Refreshes
are serialized, rotated tokens are persisted, and rejected authorization is
retried once. Transient errors preserve the saved session and cached progress.

`ctrl+alt+r` refreshes the catalog and NeetCode progress. `ctrl+alt+m` toggles
NeetCode completion for the focused sidebar problem or the open problem.
The local mark changes only after server acknowledgement. NeetCode completion
is cached per user ID and contributes to roadmap progress separately from
LeetCode Accepted results. Signing out clears the displayed NeetCode marks;
the account's cache remains available for a later sign-in.

The sign-out action removes the shared leet/verd session, leaving browser login
and Solution files untouched. Settings distinguish saved sessions from a
verified login or successful progress sync. NeetCode's callable endpoint is an
integration with its current website, rather than a documented public API.

Both Rust application crates enforce `#![forbid(unsafe_code)]`. This applies to
project-owned Rust code; GPUI, graphics drivers, SQLite and other dependencies
have their own implementations and may use unsafe Rust or native code.

## Onboarding, languages, and source explorer

On first launch, choose Python, C++, Go, or C and the practice source. Account forms are available in the second step; a Codeforces handle is validated with its public API before saving. A missing handle is prompted on startup and when choosing Codeforces; it is optional for other sources. Settings → Run onboarding again reopens saved choices without removing account sessions or solution files.

The explorer has a single source dropdown. NeetCode shows a topic icon grid and only the selected topic’s questions. Ctrl+0 focuses the grid; arrows change topics, Enter opens its first problem, and Left from the question list returns to the grid. LeetCode and Codeforces show their cached catalogs. Ctrl+P searches the selected source. Ctrl+O opens the actual source of the current problem.

The language preference applies when opening a new solution. LeetCode files live under `leetcode/<id>-<slug>.<extension>`; Codeforces files use `codeforces/<contest><index>/main.<extension>`. Existing files are retained. Python LeetCode tests use the local harness; C++, Go, and C LeetCode runs use the signed-in remote judge. Codeforces samples use stdin/stdout locally: Python needs the configured interpreter, C++ needs `g++`, C needs `gcc`, and Go needs `go` in PATH. Compilers have a 60-second deadline; samples use the configured timeout and a 1 MiB output limit. Codeforces’s normal whitespace-insensitive sample comparison cannot replace a custom checker or its online judge.

Codeforces uses native Rust public API/HTML clients, cached catalog/progress/statements, and bounded account-history pagination. During development the live service returned HTTP 403 from this machine; fixtures and all four local sample runners passed. A failed fetch reports an error and retains the cache. Ctrl+Alt+Enter copies Codeforces code and opens the submission page; no authenticated Codeforces submission API is claimed.

## IntelliSense

`async-lsp` manages native LSP transport, initialization, notifications, and requests. A shared process per language serves open solution documents; changes use increasing versions, and diagnostics with an outdated version are ignored. Coalesced server notifications wake the editor directly, with no idle diagnostic polling. The editor reuses GPUI’s completion, hover, definition, code-action, and diagnostic overlays. Both application crates continue to forbid unsafe code.

Python discovers `basedpyright-langserver` or `pyright-langserver` in PATH, then Zed’s existing basedpyright installation if Node is available. C/C++ discover `clangd`; Go discovers `gopls`. No server is downloaded automatically. Server errors appear in the status tooltip, and Settings → Restart language server retries the active document. Ctrl+Space requests completion; ordinary typing also triggers it. Ctrl+F12 follows definitions: the current document stays in the editor, known solution tabs are selected, and other file targets open with the configured external editor. Code actions that edit only the current document are supported; workspace document edits are rejected for handling in the external editor.

The GPUI editor uses lsp-types 0.97, while async-lsp uses 0.95. A typed serde adapter bridges compatible messages; no casts or unchecked types are used. Requests have deadlines, closed documents send didClose, and dropping the client terminates its server process. The local live integration test verified completion, hover, definitions, and diagnostics with all four installed servers. Native Python completion and diagnostic rendering were also verified in an isolated window.

## Statements and reference solutions

The right panel preserves the statement in ordered blocks, with a larger title, rounded examples, colored input/output labels, highlighted code values, and colored constraints. Unknown content stays in Markdown. Ctrl+2 focuses the panel; arrows and Page Up/Page Down scroll it. Ctrl+Alt+V toggles its Question/Solution icons. Ctrl+Alt+O cycles the same practice sources offered by the dropdown. All these modifier shortcuts are editable in Settings.

The Solution view fetches public reference code from [NeetCode’s MIT-licensed repository](https://github.com/neetcode-gh/leetcode), using its own problem metadata to map filenames and available languages. References and the index are cached in SQLite, with one loader per cache key and malformed-cache repair. Python/C++/Go/C are selectable; unavailable problem/language combinations show a clear message. Copy puts reference code on the clipboard; it never replaces your solution file. The source action links to the actual GitHub file, and the license action shows the included MIT notice. This view does not claim access to premium LeetCode editorials or Codeforces editorial fetching.

Fixture tests cover classic preformatted and modern paragraph examples, constraints, and retained unknown content. Live NeetCode fetches passed for all four reference languages, along with a cached reread and malformed-index repair. Native question, reference-code, constraint-color, and keyboard-scroll rendering were verified in an isolated window.

## Offline data and versions

Fresh installs start with NeetCode 150 and import the embedded base SQLite through Diesel. See [bundle coverage, sources, and refresh commands](bundle.md). Existing selections and personal data are preserved.

The right panel's Git icon opens History (the existing configurable history shortcut also works). Click a version or use arrow keys and Enter to restore it. leet stages the current solution before changing versions; Staged draft can restore it later. LeetCode history requires its owning account; NeetCode cloud variants require a NeetCode session. Network failures keep the account-scoped cache. Unsupported remote languages remain listed but cannot be restored into the four-language editor.

Problem tags start hidden. Click the tag icon beside Question/Solution or press Ctrl+Shift+T to toggle them; the preference is saved across questions and restarts. Each revealed chip includes a topic icon.

New installations use `~/.config/leet/config.toml`, `~/.local/share/leet/leet.db`, and `~/leet`. Upgrades retain existing vg paths, saved workspace choice, and shared keyring sessions. The `vg` launcher is removed; the desktop entry is `leet.desktop`.

`leet --1337` and a triple-click on Home reveal the two small Easter eggs.

## Test case editing

The results header offers Add, Edit, and Restore icons with tooltips. Add/Edit open the existing input/expected-output dialog; Save or Ctrl+Enter persists the case. Expected output may be empty for an unjudged run. LeetCode uses one argument per line; Codeforces uses stdin.

For original examples, Use original example restores just that case in the dialog before saving. Restore in the results header restores all examples from the question, removing edited/custom cases from the active set. Per-question overrides live in the user SQLite cache, outside the bundled public database, and survive restarts. Changes clear old results and are blocked while a run or judge request is active.
# Current browser imports and contest defaults

Competitive Companion imports Codeforces samples through a native loopback receiver. Add **13337** to the browser extension's custom ports, then use its plus button on a Codeforces problem page while leet is running. Settings → Competitive Companion enables/disables the receiver; Browser import port changes its port. The receiver accepts bounded JSON POSTs, rejects unsupported/interactive problems, and reports an occupied port in Settings. It queues imports without blocking GPUI. This imports samples and limits, not full browser HTML. Complete existing statements and solution files are preserved.

Python LSP documents include hidden contest imports and judge node declarations after user text, retaining user positions. Syntax errors stay visible; Python type-rule diagnostics and warnings are filtered. Common helpers include Counter/defaultdict/deque, bisect, heap operations, cache/lru_cache, itertools, math, typing, and standard module names. Local stdin and function runners provide these helpers. External Codeforces submission still requires explicit imports; generated AI prompts require them.

`./ops cache:fetch --slug cf:4:A --json` fetches/cache-checks a single Codeforces statement in the **local user cache**, reports its database path, cache hit, source, sample count, and statement size. It preserves personal solutions and sample overrides. `./ops local:deploy --json` installs `leet` and `1337`, removes the former app-owned `vg` symlink, and preserves all legacy user data.

Contest cache operations use `./ops contests:refresh --json`; add `--contest 1` to refresh a specific contest’s problems as well. Results identify the private local SQLite target and report contest/problem counts. Failed downloads preserve previous data. Home and contest navigation perform this revalidation in the background on every visit. The public API follows [Codeforces documentation](https://codeforces.com/apiHelp/methods#contest.standings): regular contest standings use only `contestId`.

Ctrl+/ opens shortcut help. It reads the shared command configuration and live GPUI contextual bindings; Ctrl+K indexes those names and current keys. Contextual rows are informational because editor and form actions require their own focus context. Application shortcut rows open the existing shortcut editor.
