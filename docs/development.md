# leet (GPUI desktop app)

`leet` is the standalone native desktop app in this repository: the NeetCode roadmap, a code editor, local tests, the LeetCode judge, Solution history, and AI learning prompts in one keyboard-driven window. On Linux it runs beside the Go `verd` TUI and shares its LeetCode session. For downloadable cross-platform builds, see [installation and releases](releases.md).

```sh
./ops local:deploy --json   # release build, link ~/.local/bin/leet, desktop entry + icon
./ops check --json     # cargo test --workspace
./ops local:deploy --json # update this PC’s leet desktop app after each milestone
leet                      # open; `leet <dir>` uses <dir> as the workspace for this launch
leet --version
```

Builds run with `nice` and at most 4 parallel jobs (`.cargo/config.toml`). Each install keeps a versioned binary in `~/.local/share/leet/bin` (the newest 3 are kept) and repoints the `leet` symlink. Each verified development milestone is installed into the same command and desktop launcher. A running window notices the new build and shows **Update ready**; `ctrl+shift+r` restarts into it, opening Home with the saved recent problems and panel layout.

## Layout

For GUI changes, use the fixture-only commands and evidence boundaries in
[isolated GUI verification](gui-testing.md).

One window with a pinned, icon-only Home tab and multiple problem tabs. Home
opens on every launch, showing up to 12 recent problems from the local cache.
Opening a recent problem uses the cached statement and existing Solution.
Each problem keeps its own editor, undo history, cursor, and test results.
Opening an already open problem selects its existing tab. Background runs finish
in their originating tab. Switching back to Home keeps every problem intact.
Hover the house icon for its tooltip. Ctrl+H opens Home (Ctrl+. also works); its tooltip shows the current shortcut. Ctrl+Tab and Ctrl+Shift+Tab cycle forward and backward through problem tabs, skipping Home. Typing on Home opens universal search with the typed text. Ctrl+W closes the problem tab after saving;
Home stays pinned. Up/down and Enter navigate the focused recent list.

Every panel has one toggle, and a spring animation shows or hides it. Drag the boundary above Results to resize its height; the app remembers it. Constraints remains accessible below the scrolling statement; opening it closes Description and Examples.

| Area | Toggle | Contents |
| --- | --- | --- |
| Explorer (far left) | `alt+s` | Roadmap topics and problems |
| Description (left of code) | `alt+a` | Statement, reference solution, hints, and Solution history (`ctrl+g`) |
| Center | `ctrl+1` | Python editor (tree-sitter highlighting) |
| AI (right) | `alt+d` | Actions, custom instructions, and follow-up questions |
| Bottom | `alt+x` | Test cases, outputs, LeetCode judge result |
| Roadmap | `alt+r` | neetcode.io/roadmap graph with per-topic progress |

`alt+z` hides everything but the editor. The status bar briefly shows each shortcut you use.

## Landing page

Run `pnpm --dir site dev` for `http://localhost:1337/leet/` (strict port 1337).

`site/index.html` is fully pre-rendered; the hero's CSS is inlined so the install command paints first. `src/main.js` (about 2.5 KB gzipped) owns platform tabs, copy, and Ctrl/Cmd+C. After the first animation frame, an idle callback requests `src/enhance.js`, which loads GSAP (ScrollTrigger, SplitText, DrawSVG, ScrambleText) and Lenis for the scroll scenes. OGL shaders and the particle logo load separately; React and Sonner load on idle or first feedback. `src/debugger.js` replays `src/trace.js`, a deterministic copy of leet's 46-step Container With Most Water recording. Loops pause offscreen, videos attach near the viewport, and reduced motion or Save-Data get static posters. Content remains visible if enhancement cannot load. The live Hz meter estimates browser animation-frame cadence, rather than querying monitor hardware.

Liberation Sans is self-hosted from `site/public/fonts/` (OFL). Regenerate each weight with `pyftsubset /usr/share/fonts/liberation/LiberationSans-Regular.ttf --unicodes="U+0020-007E,U+00A0-00FF,U+2013,U+2014,U+2018,U+2019,U+201C,U+201D,U+2022,U+2026,U+2192,U+2193,U+21B5,U+2318,U+2726,U+00B7,U+00D7" --layout-features='kern,liga' --flavor=woff2`.

All titles use a self-hosted Nunito bold subset; its source and OFL notice are documented in `site/public/fonts/README.md`. The hero continuously morphs 1337 into the logo while visible, with a quick GSAP SplitText transition between Leeting and 1337ing. Reduced motion retains the static title and mark.

## Landing media

Run `pnpm --dir site ops media --help` for prerequisites and scenes. `pnpm --dir site ops media:capture --json` records the installed app in a disposable, invisible OmaBox at 2560×1600, scale 2, 120 Hz. Use `--scene flow` (or another listed scene) to replace one capture. The box has a fresh HOME, an onboarding-complete config, OpenCode selected, copied Zed language-server data, and an unreachable keyring bus; no personal leet state or AI credentials are copied. Only the task's box is torn down.

Full-resolution losslessly compressed PNGs, high-quality H.264 120 fps masters, measured unique-frame cadence, and an encoded asset inventory are retained in `site/media/captures/`. Temporary recorder files live in ignored `site/media/.scratch/`. Capture uses OpenCode's MiMo V2.6 Flash Free model; it waits up to 120 seconds and records exhausted retries as a failure. When the free models are rate limited, `--scene ai --codex-home PATH` uses Codex instead: only that home's `auth.json` is copied into the disposable box, and it is deleted with the box. Inspect every still for personal information and confirm the live AI response before publishing; encoding omits an unsuccessful AI video while retaining its still and source master.

`pnpm --dir site ops media:encode --json` regenerates AVIF/WebP stills at 1600×1000 and 800×500, 60 fps AV1/H.264 clips, and 1280×800 120 fps AV1 variants for Flow and Debugger, plus `site/public/media/manifest.json`, using only the tracked captures and system FFmpeg. JSON results identify `environment: "omabox"` and enumerate file sizes. Encoding does not launch an app. Run `pnpm --dir site ops check --json` after operator changes.

The README previews use a separate quality preset: `pnpm --dir site ops media:readme --json`. It encodes 12 fps looping WebP directly from the original masters with lossless compression, retaining the full workspace resolution and 1280px for the paired debugger/AI previews. The AI preview uses the final eight seconds of its completed capture. Outputs live in `docs/assets/`; website video variants are unchanged.

## Keys

Modifier shortcuts follow the user's VS Code bindings. Bare arrows, `enter` and `escape` navigate only the focused Home, sidebar, Settings, or roadmap. `ctrl+shift+p` lists every command with its key.

| Key | Action |
| --- | --- |
| `ctrl+h` / `ctrl+.` | Open the pinned Home view |
| `ctrl+tab` / `ctrl+shift+tab` | Next / previous tab |
| `ctrl+w` | Close current problem tab |
| `ctrl+shift+t` | Reopen the last closed problem tab |
| `ctrl+shift+w` | Close all problem tabs and return Home |
| `alt+t` | Show/hide problem tags |
| `ctrl+p` | Find any free LeetCode problem (fuzzy, by name or number) |
| `ctrl+enter` | Run tests locally |
| `ctrl+shift+enter` | Run on the LeetCode judge |
| `ctrl+alt+enter` | Submit; **Accepted** commits the Solution to history |
| `alt+n` / `alt+p` | Next / previous roadmap problem |
| `alt+.` / `alt+,` | Next / previous test case |
| `ctrl+alt+t` | Add a custom test (`ctrl+enter` saves) |
| `ctrl+alt+shift+t` | Edit the selected test case |
| `ctrl+alt+1`…`9`, `0` | Assist: hints, stuck, bugs, edge cases, complexity, optimize, visualize, dry run, pattern, explain |
| `alt+s` | Toggle the left problem explorer |
| `alt+a` | Toggle the separate description pane |
| `alt+d` | Toggle the right AI sidebar |
| `ctrl+enter` in chat | Send a question; Enter inserts a new line |
| `alt+b` | Toggle Debug mode (←/→ step, shift+←/→ next call or return, space play, ↑/↓ case) |
| `ctrl+shift+e` | Choose the AI agent, model, reasoning, and Fast tier |
| `ctrl+alt+a` | Switch between detected agents and the web chat |
| `ctrl+alt+h` | Reveal the next LeetCode hint |
| `ctrl+alt+l` | Cycle NeetCode 150 → 250 → All |
| `ctrl+0` / `ctrl+1` | Focus sidebar / editor |
| `ctrl+e` | Open the Solution in the external editor (changes reload automatically) |
| `ctrl+o` | Open the problem source (NeetCode for roadmap problems, otherwise LeetCode) |
| `ctrl+k` / `ctrl+shift+p` / `ctrl+t` | Search problems, commands, settings and themes |
| `ctrl+,` | Settings, AI, accounts, and keyboard shortcuts |
| `ctrl+=` / `ctrl+-` | Zoom in / out |
| `ctrl+alt+m` | Toggle NeetCode completion |
| `ctrl+alt+r` | Refresh the catalog and solved progress |
| `ctrl+q` | Quit |

Shortcut settings use one field: primary first, then optional alternatives separated by `|`. The primary appears in tooltips. Clear the field to restore defaults, or enter `none` to disable a command. Existing `[keybindings]` overrides keep the same format: primary first, then alternatives separated by `|`.

`ctrl+shift+t` restores closed tabs from newest to oldest during the current app session, retaining their solution, language, cursor, and undo history. `ctrl+shift+w` closes all problem tabs after saving them; an active run or save failure keeps the tabs open. `alt+t` toggles problem tags.

## Contests

Home lists up to three upcoming Codeforces rounds and both upcoming LeetCode contests,
ordered by start time. Expand past contests for the ten most recent rounds across
both providers. Rows open native problem explorers; the accent-colored link icon
opens the contest website. Unpublished problems appear when the contest starts.
LeetCode contest problems reuse the usual editor, local tests, and LeetCode judge.
Provider icons identify each source. Clicking NeetCode in the provider menu selects
the last-used list (NeetCode 150 by default). Hover it to reveal 150, 250, and All;
clicking a list saves it. Right/left arrows enter or leave the list choices, and
Enter selects the highlighted provider or list.

`./ops contests:refresh --json` refreshes both public lists in the local user cache,
retaining each provider's last successful list on failure. Use `--contest ID` for
Codeforces problems or `--leetcode-contest weekly-contest-522` for LeetCode problems.
The command reports its database, provider errors, and actual cached problem data;
partial provider refreshes exit unsuccessfully. No account setup or bundle writes.

## Tests

Local runs execute every case in one Python process through `crates/practice/src/harness.py`. The harness parses LeetCode inputs from the question's metadata: lists, strings, `ListNode`, `TreeNode`, in-place (`void`) outputs, and design-class call sequences. Common imports (`collections`, `heapq`, `typing`, ...) are preloaded like on LeetCode. Outputs compare as JSON; statements that say "any order" compare order-insensitively, and floats within 1e-5. The default timeout is 10 s for a whole run.

Judge runs and submissions use the shared LeetCode session (Secret Service entry `service=verd`, `username=leetcode`, or `~/.config/verd/credentials-leetcode.json`). Sign in through the LeetCode account row in Settings. The Go CLI’s `verd login leetcode --browser auto` remains an alternative.

Click a roadmap node or press Enter to open a topic panel beside the graph. It shows
cached progress, problems in the selected list, prerequisite topics, persistent
stars, and source/video links. Up/Down select a problem; Enter opens its tab. Left
returns focus to the graph and Right returns to the panel. Escape closes the
panel first, then leaves the roadmap. Stars are stored locally in SQLite.

## AI Assist

Statements and AI text share GPUI Kit TextView with render-only `ratex-gpui` plugins. Inline/display LaTeX, Codeforces triple-dollar formulas, bracket/parenthesis delimiters, and math/latex/tex fences render natively with bundled fonts. Tables, images, links, lists, and highlighted code retain the existing TextView behavior. HTML conversion protects raw formulas before Markdown escaping; literal code is preserved. A bounded cache keys math images by formula, font size, display scale, and text color. Invalid or unsupported TeX retains readable source; this is math typesetting, not a complete TeX document engine. Structured visualizations keep their native widgets, with rich-text captions.

The editor has separate native resizable panes: Explorer, Description, Code, and AI. Drag the boundaries to resize them; widths and visibility are saved locally. The explorer can be hidden to keep the description, code, and conversation visible together. AI has General, Analysis, and Chats tabs with a shared bottom prompt composer. General centers its action grid before any conversation; Analysis previews time and space complexity with placeholders, then runs one combined correctness, complexity, improvement, and visual dry-run review. Edge cases are generated from the test-case toolbar through a count, case-type, and instructions dialog. Type a question and press Enter to send; Shift+Enter adds a new line. The send/stop button sits inside the prompt box. Type optional instructions before choosing an action such as Explain or Hints. Chats stores multiple private threads per problem. Every action is a prompt shortcut in an ordinary thread; hover its compact request card to read the prompt. Follow-ups attach the latest statement, editor code, and tests plus bounded context from the selected thread only. Opening Chats focuses its search input; search matches the open problem’s thread titles and saved messages. The plus button creates a chat for that problem. Older Root history remains stored but is not shown. Thread drafts, finished answers, and interrupted requests survive restarts. The message menu forks through a reply or edits the request into a new branch; the original stays intact. Thread options delete a conversation, and Undo reverses the latest branch or deletion across restarts. Local chats use the selected agent’s native tools with full file/command permissions and a shared environment prompt. All turns can return the same native response kinds; solution reports use the existing submission confirmation. Quick Solve asks the agent for one direct write-and-report pass, skips first-attempt sample checks, and investigates or tests only after a rejection. The status-line chip shows the agent and model, and a stopwatch while a run is active; it opens the picker for agent, model, reasoning, and Fast tier. Settings → AI holds the same choices plus installers.

Native responses are editable files under the private data directory at
`chat-artifacts/<message-id>/response.json`. Each run also supplies `schema.json`
and the artifact path to the agent. The file contains a JSON object with a
`response` field using the tagged native response contract. Agents can write and
revise these files directly; Leet validates them and refreshes earlier artifacts
after follow-ups. The message menu also offers **Reload artifact**. Scene JSON is
rendered by the existing safe Rust components, not executed as HTML or scripts.
Web mode saves the outgoing request locally, but web answers and local execution
are unavailable to Leet unless supplied separately.

Thread/message tables and artifact files belong only to the private user cache.
Legacy `assist:<problem>` histories migrate atomically into **Previous conversation**
threads; a malformed record leaves the old histories intact and reports an error.
Undo affects chat history, not file edits, test runs, or judge submissions. Artifact
files remain available independently of deleting a conversation.

Local agents are detected on `PATH` and common user bin directories. Each uses its own native protocol with typed messages (`crates/practice/src/agents/`):

| Agent | Transport | Models |
| --- | --- | --- |
| Codex | `codex app-server` JSON-RPC | `model/list`: reasoning efforts and Fast tier |
| Claude Code | `claude -p` stream-json control protocol | `initialize`: aliases, effort levels, fast mode |
| Antigravity | `agy -p --output-format stream-json` | `agy models` |
| OpenCode, Gemini CLI | ACP (`agent-client-protocol`) | session config options |
| Cursor Agent | print/stream-json | `--list-models` |

Model catalogs are stored in the personal SQLite cache, shown immediately, and revalidated once per agent per app session. Picker visits and AI runs share the fetch; failed refreshes keep cached models and retry on the next app launch. No model list is hardcoded. Defaults are the newest Luna for Codex, Haiku for Claude, a free Zen model for OpenCode, and the newest Flash Low model for Antigravity and the first Flash model for Gemini. Answers are typed: `practice::assist` derives one shared JSON Schema for all turns (enforced natively by Codex, Claude, and agy), validates with serde, and asks once for a corrected answer. Local turns run in the solutions workspace with the provider’s full native file/command access. A returned Solve report enters the same test-and-confirm workflow whether requested through chat or a shortcut; leet then runs the tests, asks before every LeetCode submission, and feeds failures back for up to five attempts. Changing the problem or language stops the solve loop. Python and Codeforces cases run locally; other LeetCode languages use the existing online test endpoint. Codeforces submission copies the solution and opens its submit page; leet cannot observe acceptance there.

Web mode can be selected explicitly even with installed agents. Actions open ChatGPT, Claude, or Gemini with the prompt prefilled and copied. Settings → AI and the picker offer the official OpenCode and Antigravity installers.

`./ops agents --catalog --json` lists detected agents and their live catalogs. `./ops agents --smoke --json` sends a small read-only structured-output probe using each available default. Authentication and upstream model restrictions are reported per agent.

OpenCode free-model availability depends on its upstream service; a model appearing in the catalog does not guarantee that it accepts requests through ACP. Gemini and Cursor require their own CLI authentication. Fast mode also depends on account eligibility.

## Debug mode

Debug (`ctrl+backtick`, `alt+b`, or the header switch) records every test case with a deterministic tracer, then plays it back like a video: the code shows the current line and a heat gutter, and the state canvas draws locals as arrays with index pointers, grids, trees, linked lists, stacks, queues, heaps, maps, sets, and graphs (`practice::debugger::structures`). Native renderers live in `crates/gui/src/gen-ui/`, with one file per structure (for example `array.rs`, `tree.rs`, and `graph.rs`), shared styles in `style.rs`, drawing helpers in `drawing.rs`, and playback controls in `player.rs`; their UI-independent scene schema lives in `practice::viz`. Python uses `sys.settrace` and hides generated comprehension frames while retaining user-defined helpers and lambdas. C++ compiles a generated LeetCode driver with `g++ -O0 -g` and steps it under gdb's Python API, so it needs `g++` and `gdb` with Python. Recordings stop at 4,000 steps. **Explain** sends the real trace to the agent, which marks the first wrong step on the seek bar. Codeforces and CodeChef stdin programs (including CPH samples) use the same native player in Python and C++. Python records top-level code and `__main__` with text/binary stdin; C++ compiles the program’s own `main()` with debug symbols. Each sample is one recording, even when its input contains multiple internal test cases. Stdout is compared by whitespace-separated tokens. Buffered C++ output appears when the program flushes it; use `std::flush` when you want intermediate output in the timeline. Other languages use the AI Dry run action. Recordings use a captured copy of the editor source, so later file edits cannot change their states.

`./ops trace --help` records a trace from the command line.

Record a stdin sample without touching saved solutions or account data: `./ops trace --language python --solution /path/main.py --stdin --input /path/sample.txt --json` (also supports `--language cpp`). Function problems retain `--meta /path/meta.json` instead of `--stdin`.

## Files

| Path | Contents |
| --- | --- |
| `~/.config/leet/config.toml` | New Linux installs: workspace, theme, Python, external editor, list, AI agent, models, and web chat, timeout |
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

On first launch, choose Python, C++, Go, C, or Java and the practice source. Account forms are available in the second step; a Codeforces handle is validated with its public API before saving. A missing handle is prompted on startup and when choosing Codeforces; it is optional for other sources. Settings → Run onboarding again reopens saved choices without removing account sessions or solution files.

The explorer has a single source dropdown. NeetCode shows a topic icon grid and only the selected topic’s questions. Ctrl+0 focuses the grid; arrows change topics, Enter opens its first problem, and Left from the question list returns to the grid. LeetCode and Codeforces show their cached catalogs. Ctrl+P and Ctrl+Shift+P search problems, commands, and settings. Ctrl+O opens the actual source of the current problem.

The language preference applies when opening a new solution. LeetCode files live under `leetcode/<id>-<slug>.<extension>`; Codeforces files use `codeforces/<contest><index>/main.<extension>`. Existing files are retained. Python LeetCode tests use the local harness; C++, Go, C, and Java LeetCode runs use the signed-in remote judge. Codeforces samples use stdin/stdout locally: Python needs the configured interpreter, C++ needs `g++`, C needs `gcc`, and Go needs `go` in PATH. Java needs `javac` and `java`; the runner copies the saved source into isolated `Main.java`, compiles once, and runs each sample without writing class files into the solution folder. Compilers have a 60-second deadline; samples use the configured timeout and a 1 MiB output limit. Codeforces’s normal whitespace-insensitive sample comparison cannot replace a custom checker or its online judge.

Codeforces uses native Rust public API/HTML clients, cached catalog/progress/statements, and bounded account-history pagination. If a live statement request fails, Leet tries the optional installed `curl` executable before its historical snapshot fallback. The curl request ignores personal curl configuration, permits only HTTPS, and limits the transfer to 30 seconds and 8 MiB. Missing curl, HTTP errors, and invalid statement HTML fall through to the snapshot; a failed fetch reports an error and retains the cache. CPH sample-only statements are upgraded without changing personal solutions or imported cases. Ctrl+Alt+Enter copies Codeforces code and opens the submission page; no authenticated Codeforces submission API is claimed.

## IntelliSense

`async-lsp` manages native LSP transport, initialization, notifications, and requests. A shared process per language serves open solution documents; changes use increasing versions, and diagnostics with an outdated version are ignored. Coalesced server notifications wake the editor directly, with no idle diagnostic polling. The editor reuses GPUI’s completion, hover, definition, code-action, and diagnostic overlays. Both application crates continue to forbid unsafe code.

Python discovers `basedpyright-langserver` or `pyright-langserver` in PATH, then Zed’s existing basedpyright installation if Node is available. C/C++ discover `clangd`; Go discovers `gopls`; Java discovers Eclipse JDT LS through `jdtls` (JDK 21+ required by current JDT LS). No server is downloaded automatically. Server errors appear in the status tooltip, and Settings → Restart language server retries the active document. Ctrl+Space requests completion; ordinary typing also triggers it. Ctrl+F12 follows definitions: the current document stays in the editor, known solution tabs are selected, and other file targets open with the configured external editor. Code actions that edit only the current document are supported; workspace document edits are rejected for handling in the external editor.

The GPUI editor uses lsp-types 0.97, while async-lsp uses 0.95. A typed serde adapter bridges compatible messages; no casts or unchecked types are used. Requests have deadlines, closed documents send didClose, and dropping the client terminates its server process. The local live integration test verified completion, hover, definitions, and diagnostics with all four installed servers. Native Python completion and diagnostic rendering were also verified in an isolated window.

## Statements and reference solutions

The right panel preserves the statement in ordered blocks, with a larger title, rounded examples, colored input/output labels, highlighted code values, and colored constraints. Unknown content stays in Markdown. Ctrl+2 focuses the panel; arrows and Page Up/Page Down scroll it. Ctrl+Alt+V toggles its Question/Solution icons. Ctrl+Alt+O cycles the same practice sources offered by the dropdown. All these modifier shortcuts are editable in Settings.

The Solution view fetches public reference code from [NeetCode’s MIT-licensed repository](https://github.com/neetcode-gh/leetcode), using its own problem metadata to map filenames and available languages. References and the index are cached in SQLite, with one loader per cache key and malformed-cache repair. Python/C++/Go/C/Java are selectable; unavailable problem/language combinations show a clear message. Copy puts reference code on the clipboard; it never replaces your solution file. The source action links to the actual GitHub file, and the license action shows the included MIT notice. This view does not claim access to premium LeetCode editorials or Codeforces editorial fetching.

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

**CPH** imports are always enabled; there are no port or enable/disable settings. Competitive Companion's green plus button sends a Codeforces or CodeChef problem to Leet using its existing default ports. The listener acquires the first three free ports from **1327, 4244, 6174, 10042, 10043, 10045**, skipping occupied ports and excluding VS Code CPH's usual **27121**. It continues retrying until three ports are acquired and warns if all eligible ports are occupied. Broadcasts across several ports are deduplicated, while new browser clicks remain distinct.

The headless `leet --companion-service` receiver persists validated imports in the private local data directory before acknowledging them. A running GUI consumes these records; a closed GUI is launched when an import arrives. Imports open a new problem tab in the existing window, or select the tab if that problem is already open; they never launch another window while a Leet GUI is running. Successful imports open the samples and request foreground activation. An older running build keeps its window; imports remain queued until it is restarted into the installed update. The window manager ultimately controls focus. Both Leet and VS Code can receive the same click; Competitive Companion has no receiver priority mechanism. No `leet://` link or custom browser port is required.

On Linux, `./ops companion install --json` installs/enables the systemd user service at login, `./ops companion status --json` reports its state, and `./ops companion stop --json` disables it. Install the desktop app first. A development GUI can start its own headless receiver when no service is available; only the installed Linux service has verified login integration. Restart the service after upgrading its executable. Imports, settings, credentials, and solutions are preserved; stopping the service leaves pending imports intact.

The receiver accepts loopback-only POSTs up to 1 MiB and rejects unsupported or interactive problems. CodeChef imports accept practice and contest problem URLs. Existing solution files and full statements are preserved; browser imports provide samples and limits, while CodeChef's public statement scraper supplies full content on demand. CodeChef's provider includes 100 bundled problems, their logo, local stdin tests in Python/C++/C/Go/Java, and a browser submission handoff that copies the solution. No CodeChef login, account-progress sync, contest listing, or in-app judge submission is implemented. Refresh catalog metadata with the existing Refresh catalog action and rebuild the bundled statements with `./ops snapshot --source codechef --json`.

Python LSP documents include hidden contest imports and judge node declarations after user text, retaining user positions. Syntax errors stay visible; Python type-rule diagnostics and warnings are filtered. Common helpers include Counter/defaultdict/deque, bisect, heap operations, cache/lru_cache, itertools, math, typing, and standard module names. Local stdin and function runners provide these helpers. External Codeforces submission still requires explicit imports; generated AI prompts require them.

`./ops cache:fetch --slug cf:4:A --json` fetches/cache-checks a single Codeforces statement in the **local user cache**, reports its database path, cache hit, source, sample count, and statement size. It preserves personal solutions and sample overrides. `./ops local:deploy --json` installs `leet` and `1337`, removes the former app-owned `vg` symlink, and preserves all legacy user data.

Contest cache operations use `./ops contests:refresh --json`; add `--contest 1` to refresh a specific contest’s problems as well. Results identify the private local SQLite target and report contest/problem counts. Failed downloads preserve previous data. Home and contest navigation perform this revalidation in the background on every visit. The public API follows [Codeforces documentation](https://codeforces.com/apiHelp/methods#contest.standings): regular contest standings use only `contestId`.

Ctrl+/ opens shortcut help. It reads the shared command configuration and live GPUI contextual bindings; Ctrl+K indexes those names and current keys. Contextual rows are informational because editor and form actions require their own focus context. Application shortcut rows open the existing shortcut editor.

Onboarding checks only the selected language's interpreter/compiler and language server in the background. Missing requirements show **Set up** links to official instructions and **Recheck**; Continue waits until the selected setup passes. Python uses the configured interpreter, including custom executable paths. JDK versions below 21 are reported for Java setup. Checking does not launch or download a language server.

The app keeps one active language server. Changing languages stops the old server, clears parked editor adapters and diagnostics, and invalidates pending attachments. Returning to a different-language tab starts its server on demand; background problem loads do not start servers.

`./ops workspace:move --path /absolute/path --json` moves the configured solutions folder and Git history on the same filesystem, updates the local configuration, and keeps a compatibility symlink for running instances. It refuses an existing destination and rolls the move back if the configuration cannot be saved. Repeating it after success reports no move.
