# Full solutions and unobtrusive editor help

- [CHOSEN] Reuse scraper, htmd, and GPUI Kit TextView for complete NeetCode articles. Cache per problem/language; retain existing public-code fallback. Preserve explanation, algorithms, complexity, pitfalls, links, and images. Show video links; Markdown cannot play iframe media.
- [CHOSEN] Cyan active tabs and pastel semantic colors, as requested.
- Use public editor hooks for diagnostics below the hovered text and completion documentation; keep squiggles and keyboard completion behavior.
- Check Rust workspace, live article/cache and LSP behavior, native rendered views and shortcuts. Update changelog, commit focused changes, then local:deploy.

Teal selection milestone: committed and installed daf66b6; 40 tests passed and onboarding pointer/keyboard selection verified.


- [CHOSEN] Open-R1 Codeforces title-only catalog and live refresh; statements fetch on demand (latest user request), approved by user; DuckDB is approved only for snapshot tooling.
- [CHOSEN] Track an embedded base SQLite, using the existing Diesel ORM. NeetCode 150 is the fresh-install default; all 150 public alternatives and articles are required. Preserve explicit saved lists.
- [CHOSEN] Universal provider search with SVG marks, lavender constraint boxes, cyan active tabs, and right-panel version history. Stage the current draft before restore; cache personal histories by account separately from the bundle.

- [CHOSEN] Latest user choice: full NeetCode 150; metadata only for the rest of LeetCode and Codeforces. Measured compacted base: 11,153,408 bytes. Fetch and cache statements on first open.

User-requested extraction: standalone `/home/noel/Projects/hobby/leet`, product/executable leet; role-based crates `gui` and `practice`. Preserve legacy vg paths and credentials, install leet.desktop, and retain vg launch compatibility. Native verification covers universal search, full article/constraint rendering, completions with documentation and acceptance, staged restore, hidden/persistent tags, and cursor-anchored hover diagnostics.

Verification: 44 Rust tests passed (6 live tests excluded from routine checks); verd Go vet/tests passed. Native headless Wayland verified the exact Expected indented block message below the cursor on hover, zero-width diagnostic hit targets, universal provider search, completion acceptance/docs, tags, and staged history restore. SQLite integrity passed with zero progress/custom-test rows.
