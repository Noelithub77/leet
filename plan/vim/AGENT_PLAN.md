# Vim execution ownership

Status: Frozen. Approval: user instruction dated 2026-10-13. Shared contracts: TECH_DESIGN.md.

- TASK-001 engine: input/editor/vim.rs and input/editor/vim/ only. FR-002/003/004/006, AC-002/003/004/005. Pure modal engine, reference and unit tests. No edits to adapter or app.
- TASK-002 app integration: crates/practice/src/config.rs; crates/gui/src/ except gui_test.rs and gui_test/ and snippets/expansion.rs; docs/changelog.md and docs/vim.md. FR-001/007/008, AC-001/006/007. Settings propagation, save/close host flows, palette/help and config tests. Do not edit vendor or test runner. Coordinate any overlap before edits.
- TASK-003 production fixture: crates/gui/src/gui_test.rs, crates/gui/src/gui_test/vim.rs and crates/practice/examples/ops/gui_tests.rs and docs/gui-testing.md. All ACs: real input and screenshots. No editor implementation edits. Do not run heavy checks concurrently.
- Main: vendor adapter/state/cursor rendering/mod exports; snippets/expansion.rs; plan/vim/; integration and checks. FR-002/005/006/008 and final acceptance judgment.

Integrate engine + adapter first, app next, production fixture last. Engine and app are independent against frozen API, fixture can be written against it. Agents report Status, Acceptance IDs addressed, Files changed, Behavior implemented, Checks executed, Results, Evidence, Assumptions, Risks, Contract deviations. Stop and report requirement/interface/ownership changes. Main runs heavy checks serially.

## TASK-003 fixture report

- Status: Implemented source; integration compilation, input execution and rendered inspection pending parent checks.
- Acceptance IDs addressed: AC-001 through AC-008 via focused production input assertions and named PNG captures. No overall acceptance claim.
- Files changed: `crates/gui/src/gui_test.rs`, `crates/gui/src/gui_test/vim.rs`, `crates/practice/examples/ops/gui_tests.rs`, `docs/gui-testing.md`.
- Behavior implemented: `--fixture vim`; production source-pane modal commands, native Insert paste and text guard, all Ctrl+V modes, logical undo/redo/repeat, bidirectional Visual selection, block I/A/c/delete/replace/put/newline/repeat geometry, literal and whole-word search, focus pending reset, read-only safety, completion/snippet Escape, settings and palette persistence, new/parked/closed/reopened solutions, shared register/repeat/search, snippet source/preview, and successful/failed save routing. Named captures separate cursor/rectangle/command/settings/workspace/snippet evidence.
- Checks executed: Read fixture skill and documentation; inspected operator help before edits; rustfmt parsed/formatted the new fixture; clean headless Vim 9.2 reference checks for final-line deletion, around-word, endpoint swap, tab/wide/short block geometry, block insertion/append/change/put/newline/repeat and indentation.
- Results: Reference outcomes match asserted expected strings. Post-edit operator help attempt was cancelled while waiting for the parent's Cargo job, preserving serial heavy verification. No GUI checks or screenshots have yet run.
- Evidence: Parent-managed focused fixture report and screenshot paths will provide runtime evidence after integration. Reference probes used temporary files and `vim --clean -n -es`, starting with `gg0` and tabstop/shiftwidth 4.
- Assumptions: Shared API frozen in TECH_DESIGN.md; GPUI test `press` dispatches text-capable keys as in existing production fixtures; standalone tab size matches solution editor at four display columns.
- Risks: Compiler/API or input-routing failures may need follow-up fixture corrections. Visual images must be inspected independently. Live desktop, installed app, account services and real-time performance remain outside this isolated fixture.
- Contract deviations: None. AGENT_PLAN.md updated solely with this required task report. Parent retains final acceptance and commit ownership.

Engine agent hit model capacity during final repair. Main assumed TASK-001 ownership, retained the 109 buffer-reference cases and added 50 actual-file references, including terminal LF and CRLF. All 15 engine tests pass.
