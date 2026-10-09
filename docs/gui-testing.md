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

The chat composer fixture checks Enter sending, Shift+Enter line breaks, removal of Ctrl+Enter, empty and disabled input, and send/stop containment. It renders compact, light, dark, and 140%-zoom variants. It uses the production composer with a dropped fixture workspace, so sends do not start agents or open web chats.

Native mode also checks long statement cards at regular and compact sizes, in light and dark themes, and at 140% zoom: visible headers, independent expansion and scrolling, increased wheel speed, scrollbar thumb dragging, natural short-content sizing without overflow, shared saved flags across questions and reopened storage, PageDown, and Tab/Space activation. Statement PNGs are saved alongside debugger captures.

Hint controls share the problem toolbar. Statement captures verify their containment; the production workspace fixture exercises hint cycling, hiding, and a customized shortcut, and saves `hint-tooltip.png` with the shortcut shown on hover.

The onboarding fixture renders the production setup view with missing clangd, a missing compiler, and pending tool checks. Hit-tested Continue clicks advance in every case. The retained workspace receives deterministic installer events after leaving setup: progress stays visible, a hit-tested Retry restarts a failed job, and success shows Ready. Prerequisite results are deterministic; this fixture never detects or downloads tools. Setup PNGs are saved alongside the other captures. The ignored `live_private_python_and_cpp_servers` core test verifies completion, hover, definition, and diagnostics against managed tools; isolate its data directory and install the tools there before running it.

The guided-tour fixture uses isolated SQLite storage and a bundled public
question. It checks startup defaults, automatic display, search actions, Skip
and Done persistence, replay, Escape after shortcut reload, all eight steps,
Explorer, description and AI toggle actions, automatic advancement from actions and shortcuts,
editor input isolation, Left/Right tour navigation, full search on Ctrl+P and Ctrl+Shift+P, and layout restoration.
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

Run `./ops check --json` after source changes. Local deployment uses
`./ops local:deploy --json`; it installs the ordinary release binary without
the `gui-test` feature. Running windows may offer a restart, which remains a
user action.

To explore the statement cards in a named OmaBox, use the same isolated launch with `--gui-test --explore --statement --output /tmp/leet-statement-review`; its app id is `leet-statement-fixture`. This fixture stays open for 180 seconds and uses synthetic long content without a database.

Snippet fixtures use production editor input for prefix expansion, linked typing, Tab/Shift+Tab, picker search and dismissal, manual save, and cursor drag-and-drop. They render regular, compact, light-theme, zoomed, and assistant-open layouts. Onboarding checks that the snippet step can be skipped, and native agent-question fixtures validate required fields and submit enum/boolean choices. All snippet libraries and solution paths live under the test output's `session/` directory; fixtures do not scan personal editors or launch agents.
