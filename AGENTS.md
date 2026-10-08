# leet

A native GPUI coding-practice IDE written in safe Rust. Keep the UI calm, compact, keyboard-first, and progressively disclosed. Default practice is NeetCode 150, Python, Liberation Sans, and Vesper with pastel accents.

## Layout and ownership

- `crates/gui`: desktop UI, editor adapters, navigation, accounts, themes, provider/language icons.
- `crates/practice`: UI-independent clients, Diesel/SQLite cache, language servers, runners, Git history, article parsing, bundled public data.
- `crates/practice/data/base.sqlite`: tracked public-only bundle. Full NeetCode 150 and 100 CodeChef practice statements/samples; metadata only elsewhere. Never put credentials, progress, submissions, personal solutions, or tests into it.
- `docs/development.md`, `docs/bundle.md`, `docs/changelog.md`: workflows, data provenance/limits, user-facing milestones.

## Work and decisions

Work on the current branch. Inspect Git status before editing and committing; preserve unrelated edits. Carry authorized changes through verification, focused commits, and local deployment. Ask before new dependencies or meaningful unresolved architecture/data choices. Reuse approved maintained packages and the current GPUI Kit editor hooks; do not embed Zed's unpublished workspace editor by assumption.

Use responsibility-based names, never generic catch-all names or product-name prefixes for internal modules. The `gui` package builds the `leet` binary; `practice` owns the problem-solving domain. Application and operator implementations are Rust; never add a Go/verd execution wrapper. Both crates forbid unsafe code. Maintain that rule. Keep responsibilities separate between UI and core. Add regression tests for material behavior; run appropriate checks without weakening them. No added formatters or formatting configs.

## Operator commands

Discover `./ops --help`. Run `./ops check --json` after changes, then `./ops local:deploy --json` after each completed milestone. Deployment builds the release app, installs versioned binaries and `leet.desktop`, points `~/.local/bin/leet` at the new build, and keeps `1337` as an Easter egg launch alias and removes the obsolete `vg` launcher. Running windows offer a restart. It preserves existing configuration, cache, credentials, and solution repositories; never run account setup during a rebuild.

Use `./ops snapshot --help` for public bundle refreshes. Data operations must identify their environment and report real outcomes. Snapshot tooling may use the approved DuckDB CLI only for Codeforces metadata; the app uses Diesel/SQLite. Inspect the SQLite bundle for private records and checkpoint/compact it before committing.

New installs use `~/.config/leet`, `~/.local/share/leet`, and `~/leet`. Existing vg paths remain supported so an upgrade does not lose saved state. Keyring identity remains compatible with verd. Do not relocate or overwrite personal data as a side effect of a build.

Update `docs/changelog.md` for user-visible milestones. Every release must publish concise, user-facing bullet lists classified as **Feat** and **Fix**. Use `<details open>` with `<summary>Feat</summary>` and `<details>` with `<summary>Fix</summary>` so Feat starts expanded and Fix collapsed on GitHub and in the native changelog. Keep one change per short bullet; omit implementation logs, repeated explanations, and installation requirements. Keep download/setup information in `docs/releases.md` and the release's separate Downloads section. Maintain the current release's categorized notes in `docs/changelog.md`; the release workflow publishes them and tests enforce the format. Verify source checks, rendered native UI/keyboard behavior, and installed commands separately, and report any unverified layer. Commit only task changes with a clear title and useful body. The user authorized public distribution of this repository. Creating unrelated repositories requires explicit authorization.

## Public distribution

`site/` is a minimal Vite landing page deployed by GitHub Pages. Discover `pnpm ops --help` there; use `pnpm ops check --json` after site changes. `./ops check --json` also exercises isolated Unix installer regressions. `./ops release:package --help` packages native CI builds without accessing user data. Version tags and the manual Release workflow build all native targets and publish complete GitHub releases; see `docs/releases.md` for requirements and operation. Linux account storage stays compatible; macOS/Windows use native OS stores through `keyring`.
