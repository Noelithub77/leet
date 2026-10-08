---
name: gui-verification
description: Verify Leet GPUI changes with fixture-only input tests, offscreen PNGs, optional motion video, and private compositor checks. Use when checking rendered native UI or keyboard/pointer behavior in this repository.
---

# Leet GUI verification

Read `docs/gui-testing.md` for supported commands and evidence boundaries, then `./ops gui:test --help`. Prefer the tracked Rust runner over disposable app launch/capture scripts.

- Default to `./ops gui:test --json`: production Debug views, real recorders, hit-tested pointer/keyboard input, and direct PNGs. Use `--interaction-only` for behavior without GPU rendering; use `--video` for intermediate animation frames encoded at 60 fps.
- Use `--backend cage` for a private Wayland window smoke check. For wider Omarchy desktop exploration, follow the installed global `$omabox` skill and select a unique box with isolated networking. Do not send input to the user's session.
- Add fixtures to the existing `gui_test` module for the view being changed. Enter fixture mode before normal configuration/workspace startup; keep account loading, network jobs, updates, Companion, credentials, and personal databases/solutions out of it.
- Reuse GPUI Kit test support and inspect the installed signatures. Exercise input dispatch and hit testing, not direct state mutation. Assert the observable state/verdict after each meaningful action.
- Inspect generated images and motion. Distinguish behavioral assertions, rendered pixels, compositor checks, encoded frame rate, and real-time performance. Never treat a nonblank image or exit-zero input command alone as visual correctness.
- Fail with logs on unavailable rendering/compositor support. No fallback to a live display. Preserve artifacts, clean only the runner's temporary session/owned box, and finish with the repository's required checks and local deployment.
