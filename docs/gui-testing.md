# Isolated GUI verification

Use the repository skill `$gui-verification` for native UI changes. The default
runner constructs the production Debug view before normal application startup,
with fixture code and samples for Python and C++ on Codeforces and CodeChef.
It does not load accounts, personal databases, settings, solutions, updates, or
Companion. Its commands never send input to the active desktop.

```sh
./ops gui:test --json                    # GPUI input assertions and PNGs
./ops gui:test --interaction-only --json # input assertions without a GPU
./ops gui:test --video --json             # PNGs plus a scripted 60 fps video
./ops gui:test --backend cage --json      # private Wayland window smoke check
```

The JSON result names the artifact directory. Use `--output /absolute/new-dir`
to choose one; an existing directory is refused. Build, runner, and error logs
remain with the report and screenshots. The operator removes temporary session
data and bounds the fixture process to 180 seconds. Failure never falls back to
the user's display.

Run `./ops gui:test --fixture chat --json` for focused chat checks, including the merged Chats empty state, the slim one-row header and its Analysis toggle, the history dropdown, and right-aligned request bubbles. Shared tooltip checks hover long descriptions, short button labels, and shortcut keycaps in dark, light, compact, and zoomed windows; they assert wrapping, viewport containment, and dismissal, and save `tooltip-*.png`.

The chat fixture also checks Alt+Q quoting selected code and rendered answers as citation pills without sending, the automatic selection menu, Alt+E and Alt+F sending selection-scoped prompts, draft preservation, normal replacement typing, and workspace Alt+L accepting Copilot without quoting. It also selects every annotation dropdown mode and checks long source lines wrap without changing the file text. It saves `selection-menu.png`, `citation-hover.png`, `selection-ai.png`, and `wrapped-code.png`.

Run `./ops gui:test --fixture selection --json` for the selection toolbar and Ctrl+I. It checks that the toolbar sits beside the selection without covering it, that its Edit button opens the inline edit card below the target with a separate highlight, and that the agent note appears above the review diff. It also checks Enter applying the edit, undo, a right-click outside the selection keeping it for the context menu, stale-proposal rejection, and Ctrl+Shift+I history with Up/Down recall. Fixture replies replace agent runs. It saves `selection-menu.png`, `focused-edit-open.png`, `focused-edit-review.png`, `selection-context-menu.png`, and `focused-edit-history.png`.

Chat management checks archive from history, undo archiving without losing the draft, and open Archived chats in General settings. They restore and permanently delete chats from different problems, including cancellation and confirmation. Captures are saved as `chat-archive-icon-*.png`, `chat-archived-settings-*.png`, and `chat-delete-confirm-*.png`.

Prompt navigation checks click the chronological side rail and dispatch Shift+Up/Shift+Down through the focused composer, verify first/last boundaries and draft preservation, and assert the selected request scrolls to the top. Captures are saved as `chat-prompt-list-*.png`.

The chat composer fixture checks the one-line prompt expanding on focus, quick actions prefilling a removable action card, the card and text sending as one message, Enter sending, Shift+Enter line breaks, removal of Ctrl+Enter, empty and disabled input, and send/stop containment. It renders compact, light, dark, and 140%-zoom variants. It uses the production composer with a dropped fixture workspace, so sends do not start agents or open web chats. Its history includes an I'm stuck chat that renders as a colored Stuck tag in `threads-*.png`; asking about that chat's note must open it, unloaded, with the prompt prefilled and focused.

The snippet fixture checks C++ range-loop placeholder alignment in padded, zoomed, and scrolled editors, plus Tab/Shift+Tab and the final caret. It saves `snippet-cursor-regular.png` and `snippet-cursor-zoom-scroll.png`.

The snippet fixture also tests automatic parameter hints and Ctrl+Space through the production editor pane. Deterministic LSP-shaped responses cover active parameters, overload buttons, Alt+Up/Alt+Down with real Alt modifier events and retained completion state, customized commands, full-width signature strips above completions, compact parameter details beside or below the list, empty-completion-documentation fallback, Vim Insert/Normal hint gating, combined completion/signature geometry, focus preservation, Escape, selection, and late-result rejection. It saves `parameter-hints-dark.png`, `parameter-hints-compact.png`, and `parameter-hints-light-zoom.png`. The ignored `lsp::tests::live_cpp_judge_context` test uses installed clangd to check implicit standard-library names, unchanged saved files, hover positions, and genuine diagnostics after edits. `lsp::tests::live_cpp_compile_commands_override_judge_fallback` verifies that project defines and C++17 flags retain priority over the fallback. The ignored `lsp::tests::live_python_and_cpp_signature_help` test checks real basedpyright and clangd responses in temporary solution workspaces; it requires installed servers and does not download tools.

Parameter-popup coverage includes long documentation in dark, compact, and zoomed light layouts, wheel scrolling, scrollbar thumb dragging, and bottom padding. Captures are saved as `parameter-scroll-*.png`. The selection fixture checks visible shortcut hints for Quote, Explain, and Fix and saves `selection-shortcuts.png`.

Run `./ops gui:test --fixture notes --json` for code notes in the production editor pane: Rich hiding AI notes behind line highlights, note cards with a step bar, costs and shortcut keys, question and noted-line code actions, the Off view's hidden lines, renumbering and copy, edits that keep hidden notes in place, one-step undo after clearing AI notes in Raw, and resolving the hovered line while the cursor is elsewhere. It saves `notes-rich-hover.png`, `notes-actions.png`, and `notes-off.png`.

Run `./ops gui:test --fixture statement --json` for focused statement cards and complete LeetCode, NeetCode, Codeforces, and CodeChef payload rendering with an isolated local HTTP fixture and disk cache. No platform accounts or external requests are used.

Native mode also checks long statement cards at regular and compact sizes, in light and dark themes, and at 140% zoom: visible headers, independent expansion and scrolling, increased wheel speed, scrollbar thumb dragging, natural short-content sizing without overflow, shared saved flags across questions and reopened storage, PageDown, and Tab/Space activation. Statement PNGs are saved alongside debugger captures.

Hint controls share the problem toolbar. Statement captures verify their containment; the production workspace fixture exercises hint cycling, hiding, and a customized shortcut, and saves `hint-tooltip.png` with the shortcut shown on hover.

The onboarding fixture renders the production setup view with missing clangd, a missing compiler, and pending tool checks. Hit-tested Continue clicks advance in every case. The retained workspace receives deterministic installer events after leaving setup: progress stays visible, a hit-tested Retry restarts a failed job, and success shows Ready. Prerequisite results are deterministic; this fixture never detects or downloads tools. Setup PNGs are saved alongside the other captures. The ignored `live_private_python_and_cpp_servers` core test verifies completion, hover, definition, and diagnostics against managed tools; isolate its data directory and install the tools there before running it.

Run `./ops gui:test --fixture tour --json` for focused guided-tour checks.

Run `./ops gui:test --fixture striver --json` for native A2Z/SDE sheet navigation.
It opens sheets through the production source picker, expands a real curriculum
section, switches and persists the sheet choice, and opens/dismisses the TUF
account dialog. It saves `striver-a2z.png` and `striver-sde.png`. It never opens
a provider login window, accesses credentials, or sends provider requests.
This fixture proves sheet interaction and pixels; it does not prove native
WebView2/WKWebView login or authenticated progress sync.

The guided-tour fixture uses isolated SQLite storage and a bundled public
question. It checks startup defaults, automatic display, search actions, Skip
and Done persistence, replay, Escape after shortcut reload, all eight steps,
Explorer, description and AI toggle actions, automatic advancement from actions and shortcuts,
brief shortcut confirmation, cancellation on navigation, editor input isolation, Left/Right tour navigation, full search on Ctrl+P and Ctrl+Shift+P, and layout restoration.
It checks minimum window size, a light theme, and 140% zoom.
Tour PNGs are saved alongside the other captures; no
account loading, agent detection, language servers, or network jobs run.
With `--video`, `tour-motion.mp4` shows the eight step transitions using 240
offscreen frames encoded at 60 fps. This does not measure real-time performance.

Native mode checks real recorder results, case selection, step buttons, arrow
keys, seek dragging, Home/End, play/pause, and expected output. Inputs go through
GPUI dispatch and hit testing. Debugger PNGs are 2560×1600 for a 1280×800 logical window
at GPUI's 2× test scale. Inspect the images separately from the assertions.

Video uses 240 offscreen frames, advances the test clock at 60 Hz, and presses
Right every twelve frames. FFmpeg encodes a four-second 1280×800 MP4. Frames
include the view's transitions between steps. This is a scripted motion review;
encoding at 60 fps does not establish real-time compositor or hardware performance.

Cage mode requires installed `cage` and `grim`. It creates a private runtime
directory, strips inherited display and session-bus variables, and uses the
headless Wayland backend. It checks window rendering and GPUI-dispatched input,
then captures initial and final screenshots. It does not test external input
delivery through Hyprland.

## OmaBox exploration

Use the global `$omabox` skill when the check needs Hyprland, desktop focus, or
external keyboard/pointer delivery. Keep the session invisible and use a unique
name for every task. The following launches the same fixture-only Debug view;
it exits after a bounded 90-second exploration period once recording is ready.

```sh
cargo build -p gui --features gui-test
omabox up leet-review --net isolated --no-shell --size 1280x800@60 --json
omabox run -b leet-review -d -- /home/noel/Projects/hobby/leet/target/debug/leet \
  --gui-test --explore --output /tmp/leet-review
omabox wait -b leet-review window leet-gui-fixture
omabox windows -b leet-review --json
omabox shot -b leet-review --window leet-gui-fixture -o /tmp/leet-review-initial.png
omabox keys -b leet-review --window leet-gui-fixture Down End
omabox shot -b leet-review --window leet-gui-fixture -o /tmp/leet-review-final.png
omabox down leet-review
```

Wait for the fixture's `ready.json` inside the box before sending input. Use
`omabox run -b leet-review -- cat /tmp/leet-review/ready.json` to inspect it.
The fixture's `/tmp` is private to the box; `shot -o` writes the capture on the
host. Adapt the absolute binary path when using another checkout. View the final
image and confirm Case 2, its final step, output `6` and `10`, and the matching
verdict; an exit-zero key command alone is insufficient evidence.

The verified local OmaBox session used the Intel `i915` render node. OmaBox
reports its selected node and driver in the `render` field from `up --json`.
The current command selects the node automatically and has no `--gpu` flag.
Do not change the live compositor to resolve a fixture failure. OmaBox does not prove physical input,
real monitors, native system services, or performance on another GPU.

`./ops gui:test --fixture copilot --json` runs focused Copilot checks.
Copilot fixtures use the production source editor with deterministic predictions,
without downloading a server, reading credentials, or making network requests.
They dispatch real modifier and keyboard events to verify hidden predictions,
Alt-held previews, release dismissal, Alt+L acceptance, Alt+K reversion, Escape, and stale-document
rejection. `copilot-alt-preview.png` and `copilot-hidden.png` separate rendered
evidence from assertions. Onboarding fixtures verify that Copilot can be skipped
without starting authentication. Live GitHub sign-in and subscription-backed
predictions require separate verification after browser authorization.

Run `./ops check --json` after source changes. Local deployment uses
`./ops local:deploy --json`; it installs the ordinary release binary without
the `gui-test` feature. Running windows may offer a restart, which remains a
user action.

To explore the statement cards in a named OmaBox, use the same isolated launch with `--gui-test --explore --statement --output /tmp/leet-statement-review`; its app id is `leet-statement-fixture`. This fixture stays open for 180 seconds and uses synthetic long content without a database.

Snippet fixtures use production editor input for prefix expansion, linked typing, Tab/Shift+Tab, native cursor completions, merged language-server results, placeholder acceptance, and dismissal, manual save, and cursor drag-and-drop. They render regular, compact, light-theme, zoomed, and assistant-open layouts. Onboarding checks that the snippet step can be skipped, and native agent-question fixtures validate required fields and submit enum/boolean choices. All snippet libraries and solution paths live under the test output's `session/` directory; fixtures do not scan personal editors or launch agents.

C++ highlighting checks assert actual token colors for shared C rules and C++ additions in Vesper and Solarized Light, then render the production source pane in `cpp-syntax-dark.png` and `cpp-syntax-light.png`.

Autocomplete fixtures also type `dijktsra` and `rés`, verify the corresponding local declarations immediately and after delayed language-server results, then accept them.

Completion placement checks type in the production editor at the top, middle, and bottom of a narrow pane, plus wide, light, and zoomed variants. They assert that the full popup stays inside the editor and clear of the typing line, and save `completion-placement-*.png`.

Use `./ops gui:test --fixture snippets --json` for focused snippet rendering and input checks. It also checks the workspace tab, Explorer toggle, field titles, Ctrl+W closing, and retention of invalid edits. Add `--interaction-only` to skip pixels.

Use `./ops gui:test --fixture vim --json` for Vim input in production solution and
snippet editors. It checks mode-dependent Ctrl+V, native Insert clipboard paste,
selection replacement, undo/redo and repeat, Visual and rectangular edits,
tab/wide/short/empty line geometry, literal search, read-only safety, setting and
palette propagation, retained tabs, and failed-save retention. Insert Ctrl+V is
the documented native-paste exception. The fixture uses temporary SQLite,
configuration, solution, and snippet paths; it never loads accounts or starts
language servers. `vim-normal.png`, `vim-visual.png`, `vim-block.png`,
`vim-command.png`, `vim-settings.png`, `vim-workspace.png`, and `vim-snippet.png`
are separate pixel evidence. Inspect cursor shapes, rectangle highlights, mode
labels, and command entry independently of the input assertions. Add
`--interaction-only` for behavior without rendering.

The Vim engine has dependency-free regression tests, including checked-in clean
headless Vim references. Run `rustc --edition=2024 --test
vendor/gpui-base/src/input/editor/vim/verify.rs -o /tmp/leet-vim-engine-tests`,
then `/tmp/leet-vim-engine-tests --test-threads=2`. The `reference.py` generator
uses clean temporary buffers; `file_reference.py` reads/writes actual temporary
files to cover terminal newlines and CRLF without changing personal settings.
