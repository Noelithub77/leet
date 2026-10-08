# Isolated GUI verification

[CHOSEN] GPUI interaction tests and direct offscreen PNGs by default, optional scripted 60 fps video, private Cage Wayland smoke checks, and invisible OmaBox for Hyprland exploration.

Verified: GPUI Kit 0.7.1 test support and gpui-pre-platform 0.3.8 provide working Linux offscreen rendering. `./ops gui:test` passes input assertions for all four provider/language fixtures; native PNG/video and private Cage captures pass. External OmaBox keyboard input and screenshots also pass on the Intel GPU.

- Test production Debug views for Python/C++ Codeforces and CodeChef fixtures.
- Keep account loading, background services, updates, and personal paths out of fixtures.
- Separate behavioral assertions, rendered pixel evidence, Wayland compatibility, and video timing.
- Bound child lifetimes; never fall back to the active desktop.
- Implemented tracked Rust operator surface, repository skill, and `docs/gui-testing.md`.
- Added the global OmaBox skill and canonical Codex guidance in dotfiles; both installed skill links resolve to the tracked source.
