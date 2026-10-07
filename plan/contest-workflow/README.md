# Contest workflow

- [CHOSEN] Native Axum/Tokio Competitive Companion receiver, approved by the user. Implemented for Codeforces sample imports on loopback; configurable enable/port settings.
- [CHOSEN] Open-R1 historical snapshot fallback, approved by the user. Implemented as on-demand indexed rows; only a public identity index is bundled. Successful statements use the private Diesel/SQLite cache.
- Implemented Python contest globals, syntax-only editor diagnostics, exact judge-interface prompts, and pre-run interface checks. Core regressions and live Python LSP checks pass.
- Implemented faster statement scrolling, collapsed examples/constraints, status actions, and Sarah Pink. User requested to verify the UI themselves.
- Implemented upcoming/live contests, collapsed recent past contests, Ctrl+K contest search, keyboard navigation, and a contest-scoped explorer. [CHOSEN] cached-first with revalidation on every Home/contest open, one in-flight request per key, preserved cache on refresh failure. Existing Ctrl+Alt+Enter copies code and opens the specific web submission page. The tracked `contests:refresh` operator verifies list/problem refreshes in private SQLite.

Do not replace personal code or custom instructions. Snapshot rows must match their indexed identity and must not contain truncated required fields. Public bundle metadata must never contain account records.
