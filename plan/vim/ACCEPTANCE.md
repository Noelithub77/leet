# Vim acceptance contract

Status: Frozen. User-approved plan dated 2026-10-13. All criteria are release blocking.

| ID | Requirements | Observable scenario and required evidence |
|---|---|---|
| AC-001 | FR-001 | Default off, persisted round trip, live setting and palette toggle, new/parked/reopened/snippet editor propagation; config and production fixture assertions |
| AC-002 | FR-002, FR-005 | Ctrl+v in every mode, Insert multiline/empty/selection paste, undo/redo/dot, Escape, IME and shortcut isolation; engine and production input assertions |
| AC-003 | FR-003 | Standard motions/counts/operators/text objects and both-direction Visual mode changes/o/O/gv; clean headless Vim reference cases and regression assertions |
| AC-004 | FR-004 | Block I/A/c/delete/replace/put/repeat/undo across tabs/wide/uneven/empty/EOF; engine and production fixture assertions |
| AC-005 | FR-003, FR-006 | Typed register, shared repeat/search, wrap search, scrolling, focus transactions; engine and production fixture assertions |
| AC-006 | FR-007 | Read-only safety, existing save/close and failed save retention; production host assertions |
| AC-007 | FR-008 | Mode/pending/command display, cursor and rectangle images inspected, help and changelog; rendered screenshots and source review |
| AC-008 | All | Focused snippet/completion regressions and appropriate workspace checks pass; fixture-only isolated data and bounded resources |

Deferred scope is defined in PRD.md. Definition of done: all blocking criteria supported by evidence, implementation committed in focused changes, and no weakened verification or new unapproved dependencies.

## Verification record, 2026-10-14

| Criteria | Result | Evidence |
| --- | --- | --- |
| AC-001, AC-002 | Pass | Config persistence tests and final production Vim fixture, including real Settings/palette input, native clipboard, input-handler IME guard and tab lifecycle |
| AC-003, AC-004 | Pass | 18 engine tests with 171 clean Vim references; final production movement/Visual/block input cases |
| AC-005, AC-006 | Pass | Final production shared state, scroll, focus, read-only preview and successful/failed host save assertions |
| AC-007 | Pass | Inspected Normal, Visual, rectangle, command, Settings and snippet PNGs; supported-command help and changelog |
| AC-008 | Pass | Workspace check, focused snippets/completion fixture, final Vim rebuild/fixture, formatting and whitespace checks |

Exact artifact paths and scope limits are recorded in LOOP.md. This evidence establishes source and isolated headless behavior; no installed release or active-desktop verification was requested.
