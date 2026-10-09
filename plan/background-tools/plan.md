# Background tool setup

- Automatically request private pinned tools at onboarding start, language selection, and Continue. Keep Continue available.
- Let the workspace own one serialized install queue and progress/error state, independent of onboarding and Debug views.
- Show concise tool progress, ready state, and failure/retry in the existing status bar. Reuse the private installer without system package or PATH changes.
- Verify queue deduplication/retry and production onboarding/status input with deterministic installer events. Run one low-priority heavy job at a time on two CPUs/two workers, then commit and deploy locally.

Completed implementation and verification: `./ops gui:test --json` passed all isolated interaction/pixel fixtures, including background progress after the setup entity is dropped, pointer-driven retry, and Ready. `./ops check --json` passed workspace and installer regressions. Local deployment follows the focused commit.
