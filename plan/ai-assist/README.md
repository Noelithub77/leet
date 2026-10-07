# AI Assist, local agents, and the native debugger

## Decisions

- [CHOSEN] Local agents use each CLI's best native protocol with typed serde messages: Codex `app-server` JSON-RPC (`model/list`, efforts, Fast tier), Claude `-p` stream-json control protocol (`initialize` models, effort, fast mode), Antigravity `agy -p` stream-json + `agy models`, and OpenCode, Gemini CLI, and Cursor Agent through ACP (`agent-client-protocol`). Nothing hardcodes model lists. Defaults: Codex newest Luna, Claude Haiku, OpenCode first free Zen model, Antigravity newest Flash Low, Gemini first Flash.
- [CHOSEN] Web mode stays: ChatGPT/Claude/Gemini URLs with the prompt prefilled.
- [CHOSEN] Structured results: Rust result types derive `schemars` schemas. Claude, Codex, and agy enforce them. ACP agents get the schema in the prompt, and every response is validated with serde, with one repair retry.
- [CHOSEN] Visuals are native GPUI only. A shared `practice::viz::Structure` model (array, grid, tree, graph, linked list, stack, queue, heap, map, set, intervals, vars) renders AI scenes and debugger frames alike. The schema goes into the agent's prompt context.
- [CHOSEN] Right panel gets Statement | Assist | History. Assist shows an icon grid of actions (tinted Lucide tiles with tooltips) and animated result cards. A status chip shows the agent/model and a live stopwatch with the phase.
- [CHOSEN] Actions: Hints, Tests & edge cases, Find bugs, Analyze, Where I'm stuck, Full explain, Visualize, Optimize, Pattern & similar, AI dry run, Solve. Old prompt styles and their custom instructions are dropped.
- [CHOSEN] Solve: the agent edits the solution with its own tools. leet runs the local tests after each turn and asks before every real submit. On a failed verdict it feeds the result back, capped at 5 attempts, and can be stopped any time.
- [CHOSEN] Missing agents: buttons run the official OpenCode or Antigravity installers.
- [CHOSEN] Debugger: a deterministic record-then-seek tracer for Python (`sys.settrace`) and C++ (generated LeetCode driver, `g++ -O0 -g`, gdb Python recorder). A center Code | Debug mode with a state canvas, heat gutter, and video-style seek bar. Other languages use the AI dry run.
- [CHOSEN] Settings gets an AI tab in the existing row style. Settings values are right-aligned.

## Work split

| Part | Owner | Files |
| --- | --- | --- |
| Shared contracts (`viz`, `agents` types, `debugger` types, `assist`) | parent | `crates/practice/src/{viz,assist}.rs`, `agents/mod.rs`, `debugger/mod.rs` |
| Agent drivers, detection, catalogs, installers | codex-3 | `crates/practice/src/agents/*` except `mod.rs` types |
| Python/C++ recorders, C++ driver, locals → structures | codex-3 | `crates/practice/src/debugger/*` |
| GUI: Assist panel, viz components, Debug mode, AI settings, status chip | parent | `crates/gui/src/*` |

## Integration status

- Agent drivers and Python/C++ recorders implemented; catalogs and representative traces verified locally.
- Assist cards, native Debug mode, and the model picker verified in an isolated headless Wayland window.
- Native visual components and playback controls live in `crates/gui/src/gen-ui/` (`gen_ui` in Rust).
- Reliability regressions cover closed stdout cancellation, ACP replay isolation, schema repair, sampled context, and bounded trace/result files.
- Live Luna Hints and Solve verified in an isolated window; Solve reached three passing cases and the submission confirmation. Real submission remains unverified.
- `./ops check --json` passed: 19 GUI tests, 106 core tests, and 5 Unix installer regressions; 11 live tests remain intentionally ignored.
- Local installation uses the existing `./ops local:deploy --json` workflow.

## Known limits

- C++ debugging needs gdb (Linux first). macOS needs an lldb recorder later, and Windows needs MinGW gdb.
- Traces are capped (steps and value sizes) to keep recording fast.
