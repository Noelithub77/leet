# leet

A native GPUI coding-practice IDE written in safe Rust. Keep the UI calm, compact, keyboard-first, and progressively disclosed. Default practice is NeetCode 150, Python, Liberation Sans, and Vesper with pastel accents.

## Layout and ownership

- `crates/gui`: desktop UI, editor adapters, navigation, accounts, themes, provider/language icons.
- `crates/practice`: UI-independent clients, Diesel/SQLite cache, language servers, runners, Git history, article parsing, bundled public data.
- `crates/practice/data/base.sqlite`: tracked public-only bundle. Full NeetCode 150 and 100 CodeChef practice statements/samples; metadata only elsewhere. Never put credentials, progress, submissions, personal solutions, or tests into it.
- `docs/development.md`, `docs/bundle.md`, `docs/changelog.md`: workflows, data provenance/limits, user-facing milestones.

## Work and decisions

Work on the current branch. Inspect Git status before editing and committing; preserve unrelated edits. Carry authorized changes through appropriate verification, focused commits, and the development workflow. Ask before new dependencies or meaningful unresolved architecture/data choices. Reuse approved maintained packages and the current GPUI Kit editor hooks; do not embed Zed's unpublished workspace editor by assumption.

Use responsibility-based names, never generic catch-all names or product-name prefixes for internal modules. The `gui` package builds the `leet` binary; `practice` owns the problem-solving domain. Application and operator implementations are Rust; never add a Go/verd execution wrapper. Both crates forbid unsafe code. Maintain that rule. Keep responsibilities separate between UI and core. Add regression tests for material behavior; run appropriate checks without weakening them. No added formatters or formatting configs.

## Operator commands

Discover `./ops --help`. Keep verification proportional to the change: use the smallest relevant check for small fixes, and reserve `./ops check --json` and full GUI suites for substantial changes or failures that justify broader coverage. Do not repeatedly rerun passed checks without a new change or unresolved concern. Do not verify build scripts, installers, desktop entries, or the `leet`/`1337` launchers for routine app changes; check those only when their behavior changes or the user requests it. Documentation-only changes need no build or GUI run.

Default to `cargo run --jobs 2 -p gui --bin leet --features gui-test` for local development, or the `Dev` action declared in `t3.json`. This runs the normal development app and shares the focused GUI checks' build cache; fixture mode requires the separate `--gui-test` argument. Re-run after source changes; Rust code does not hot-reload. Keep the same build target and feature set to reuse cached artifacts. Do not automatically run release builds or `local:deploy` after routine fixes or milestones.

Use `./ops local:deploy --json` only when the user explicitly requests an installed release build. Run it once for that request; do not add separate launcher or installation checks afterward for small changes. Deployment builds the release app, installs versioned binaries and `leet.desktop`, points `~/.local/bin/leet` at the new build, and keeps `1337` as an Easter egg launch alias and removes the obsolete `vg` launcher. Running windows offer a restart. It preserves existing configuration, cache, credentials, and solution repositories; never run account setup during a rebuild.

Use `./ops snapshot --help` for public bundle refreshes. Data operations must identify their environment and report real outcomes. Snapshot tooling may use the approved DuckDB CLI only for Codeforces metadata; the app uses Diesel/SQLite. Inspect the SQLite bundle for private records and checkpoint/compact it before committing.

New installs use `~/.config/leet`, `~/.local/share/leet`, and `~/leet`. Existing vg paths remain supported so an upgrade does not lose saved state. Keyring identity remains compatible with verd. Do not relocate or overwrite personal data as a side effect of a build.

Update `docs/changelog.md` for user-visible milestones. Every release must publish concise, user-facing bullet lists classified as **Feat** and **Fix**. Use `<details open>` with `<summary>Feat</summary>` and `<details>` with `<summary>Fix</summary>` so Feat starts expanded and Fix collapsed on GitHub and in the native changelog. Keep one change per short bullet; omit implementation logs, repeated explanations, and installation requirements. Keep download/setup information in `docs/releases.md` and the release's separate Downloads section. Maintain the current release's categorized notes in `docs/changelog.md`; the release workflow publishes them and tests enforce the format. Report the verification actually performed; distinguish source, rendered UI, and installed behavior when relevant without checking every layer by default. Commit only task changes with a clear title and useful body. The user authorized public distribution of this repository. Creating unrelated repositories requires explicit authorization.

## GUI verification

Never slow the user's PC down for rendering or verification. Desktop responsiveness takes priority over throughput: run one heavy job at a time, use low CPU and idle I/O priority, and cap rendering at two allowed CPUs and two workers. Do not raise resource limits or run throughput benchmarks without explicit user authorization. If the desktop becomes sluggish, pause the job and reduce resource use before resuming; accept a longer render.

When GUI verification is warranted, prefer focused isolated headless interaction tests. Small copy, icon, spacing, or straightforward UI fixes do not require the full GUI suite, recordings, or compositor checks. Read `.agents/skills/gui-verification/SKILL.md` and `docs/gui-testing.md`; discover `./ops gui:test --help`. Exercise production views through real GPUI input dispatch and hit testing; assert behavior separately from rendered screenshots or recordings. Use deterministic fixtures and temporary settings, databases, and solution paths, without account loading, network jobs, updates, or Companion startup.

For compositor checks, use a private headless display and scope every capture/input tool to its socket. Never launch, focus, type into, click, restart, or record windows on the user's active desktop unless explicitly requested. Keep repeatable GUI workflows behind the tracked `ops` entry point; discover available commands with `./ops --help`. Report interaction, pixel-rendering, compositor, and real-time recording evidence separately.

## Public distribution

`site/` is a minimal Vite landing page deployed by Cloudflare Pages at `https://leet.allpyq.in`. Discover `pnpm ops --help` there; use `pnpm ops check --json` after site changes. `./ops check --json` also exercises isolated Unix installer regressions. `./ops release:package --help` packages native CI builds without accessing user data. Version tags and the manual Release workflow build all native targets and publish complete GitHub releases; see `docs/releases.md` for requirements and operation. Linux account storage stays compatible; macOS/Windows use native OS stores through `keyring`.
