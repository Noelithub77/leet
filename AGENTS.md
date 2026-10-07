# leet

A native GPUI coding-practice IDE written in safe Rust. Keep the UI calm, compact, keyboard-first, and progressively disclosed. Default practice is NeetCode 150, Python, Liberation Sans, and Vesper with pastel accents.

## Layout and ownership

- `crates/gui`: desktop UI, editor adapters, navigation, accounts, themes, provider/language icons.
- `crates/practice`: UI-independent clients, Diesel/SQLite cache, language servers, runners, Git history, article parsing, bundled public data.
- `crates/practice/data/base.sqlite`: tracked public-only bundle. Full NeetCode 150; metadata only elsewhere. Never put credentials, progress, submissions, personal solutions, or tests into it.
- `docs/development.md`, `docs/bundle.md`, `docs/changelog.md`: workflows, data provenance/limits, user-facing milestones.

## Work and decisions

Work on the current branch. Inspect Git status before editing and committing; preserve unrelated edits. Carry authorized changes through verification, focused commits, and local deployment. Ask before new dependencies or meaningful unresolved architecture/data choices. Reuse approved maintained packages and the current GPUI Kit editor hooks; do not embed Zed's unpublished workspace editor by assumption.

Use responsibility-based names, never generic catch-all names or product-name prefixes for internal modules. The `gui` package builds the `leet` binary; `practice` owns the problem-solving domain. Application and operator implementations are Rust; never add a Go/verd execution wrapper. Both crates forbid unsafe code. Maintain that rule. Keep responsibilities separate between UI and core. Add regression tests for material behavior; run appropriate checks without weakening them. No added formatters or formatting configs.

## Operator commands

Discover `./ops --help`. Run `./ops check --json` after changes, then `./ops local:deploy --json` after each completed milestone. Deployment builds the release app, installs versioned binaries and `leet.desktop`, points `~/.local/bin/leet` at the new build, and keeps `vg` as a compatibility alias and `1337` as an Easter egg launch alias. Running windows offer a restart. It preserves existing configuration, cache, credentials, and solution repositories; never run account setup during a rebuild.

Use `./ops snapshot --help` for public bundle refreshes. Data operations must identify their environment and report real outcomes. Snapshot tooling may use the approved DuckDB CLI only for Codeforces metadata; the app uses Diesel/SQLite. Inspect the SQLite bundle for private records and checkpoint/compact it before committing.

New installs use `~/.config/leet`, `~/.local/share/leet`, and `~/leet`. Existing vg paths remain supported so an upgrade does not lose saved state. Keyring identity remains compatible with verd. Do not relocate or overwrite personal data as a side effect of a build.

Update `docs/changelog.md` for user-visible milestones. Verify source checks, rendered native UI/keyboard behavior, and installed commands separately, and report any unverified layer. Commit only task changes with a clear title and useful body. Creating/pushing repositories requires explicit user authorization; this private repository's creation was requested by the user.
