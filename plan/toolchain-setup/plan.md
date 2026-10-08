# Toolchain setup

- Hide Windows consoles for prerequisite checks and background language-server/run/debug processes.
- Allow onboarding to continue without language servers; runtime/compiler checks are informational too; local execution reports its own missing tools.
- Direct pinned downloads into Leet's private tools directory are [chosen]. No package managers, system PATH changes, terminal windows or browser launches for Python/clangd setup. Bootstrap a portable Python runtime and verify archives/wheels against pinned SHA-256 digests.
- Add focused core and isolated GUI regressions, document setup commands, run checks, commit, and deploy locally.

Verification boundaries: Linux fixtures and scripts can be exercised locally. Native Windows console behavior and macOS installation require those platforms.

Implemented and verified: 188 Rust tests and five installer regressions; isolated onboarding Continue with missing/pending tools, rendered setup and existing tour/debugger fixtures; real private Python and clangd downloads, checksum/version checks, repeat installation, dry-run isolation, and managed-server completion/hover/definition/diagnostics. Linux installed usage is 388 MiB for Python tools plus 225 MiB for clangd. Commit the milestone, then deploy the local build through `./ops local:deploy --json`.
