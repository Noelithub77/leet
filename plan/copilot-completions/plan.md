# Copilot completions

## Chosen behavior

- Use GitHub's official native Copilot Language Server, pinned and checksum verified in Leet's private tools directory.
- Opt in through optional onboarding or Settings. Use its browser device flow; never copy Zed credentials.
- Fetch predictions after typing, but render only while Alt is held. Tab accepts the visible prediction; release or Escape hides it.
- Reuse GPUI Kit's inline renderer. Preserve snippets and language-server completion menus.
- A compact status-bar button opens the existing Editor setting; hover reports quota notifications when supplied by Copilot.

## Verification

- Deterministic protocol tests for sign-in, document versions, Unicode positions, acceptance, and quota notifications.
- Production GPUI input fixtures for modifier gating, acceptance, dismissal, stale-document rejection, status hover, and onboarding skip.
- Run `./ops check --json`, inspect offscreen captures, commit only owned changes, and run `./ops local:deploy --json`.

## Status

Implementation complete. Workspace checks passed (235 Rust tests and five installer
tests), and all 12 isolated GUI fixtures passed. Offscreen previews, optional setup,
and the usage hover were inspected. The installed 1.552.0 server passed initialization
without starting authentication. Live GitHub sign-in, account quota, and hosted
predictions require the user's browser authorization and remain unverified.
