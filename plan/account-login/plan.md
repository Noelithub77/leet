# Embedded accounts and Striver sheets

- [chosen] Use Wry on Windows/macOS for dedicated provider login windows; retain manual import.
- Capture only provider-scoped sessions, validate identity through existing clients, then persist using existing credential storage.
- Cover cancellation, untrusted origins, malformed messages, and session handoff; distinguish Linux checks from native Windows/macOS proof.
- [chosen] Both Striver A2Z and SDE, using native sections, LeetCode editor handoff, and browser handoff for lessons/TUF-only exercises.
- Preserve unrelated Copilot, operator, and script edits. Commit only task-owned changes.

## Progress

- Current account forms and native WebView APIs inspected. Wry approved.
- Take U Forward's A2Z page now redirects to `/prep-hub/strivers-a2z-dsa-sheet`; inspect current payload rather than an old sheet API.
- Implemented public-only curriculum normalization, native sheet controls, TUF account validation and read-only progress import, and Wry capture with manual fallback.
- Verified a task-only Linux snapshot: GUI compile, four core Striver regressions, and all 57 GUI unit tests pass (including capture origin/shape checks).
- Focused Striver fixture passes real pointer input, expansion, sheet selection/persistence, account dialog open/dismiss, and Escape back to Home. Inspected offscreen A2Z/SDE PNGs; exercise rows align under section headers.
- Snapshot operator help passes; regeneration from the downloaded public pages matches the tracked curriculum byte for byte. Both capture scripts pass JavaScript syntax checks.
- Windows cross-check cannot reach the application: default MSVC build lacks `lib.exe`; explicit clang-cl/llvm-lib retry stops in existing `ring` on missing Windows C runtime header `assert.h`. Windows/macOS compilation and native login runtime remain unverified. No authenticated provider progress or external-browser handoff was exercised.
- No installed release build, live desktop interaction, account access, or provider mutation was performed.
- Concurrent tool-install/debugger edits remain with their owner; verification uses `/tmp/leet-account-review-20261025` from the current commit plus task-owned changes.
