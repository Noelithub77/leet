# Setup feedback and cross-platform debugging

- Preserve unrelated operator/script edits in the shared checkout.
- Copilot: expose real installation stages and transfer progress, server startup,
  sign-in, and access checks in the existing setup controls. Keep cancellation,
  retry, and optional onboarding available; reject stale progress after cancel.
- [chosen] Maintained debug adapters and private portable toolchains for all five
  languages on the five distributed targets (Linux/macOS x64 and arm64, Windows
  x64). The app downloads/configures prerequisites in the background; no system
  package managers or PATH changes. Both tool and Copilot setup report status-bar
  progress and survive navigation away from setup.
- Add focused installer/protocol and isolated production-view regressions. Keep
  heavy verification to two CPUs/workers with low CPU and idle I/O priority.
- Document actual platform coverage and limits, update the changelog, and commit
  only task-owned changes on the current branch.

## Verification

- Passed on Linux x64 with an isolated data directory: private tool downloads,
  real debugger stdin cases for all five languages, C/C++/Go/Java function and
  array cases, Python timeout/step-limit child cleanup, and eight stdin regressions.
- Passed real completion, hover, definition, and diagnostics checks for all five
  private language servers. Copilot download/version verification and its
  unauthenticated server probe passed.
- Passed focused setup and Copilot headless input assertions and offscreen
  rendering; inspected simultaneous status-bar progress at compact width.
- Passed five Unix installer regressions and the workspace Rust suite:
  287 passed, zero failed, 25 opt-in tests ignored in the ordinary run.
  The aggregate operator check stops at an unrelated, unstaged punctuation scan
  over existing application and vendor files; preserve that work.
- Native macOS/Windows execution and authenticated Copilot predictions remain
  unverified. No installed release build or live-desktop verification was run.
