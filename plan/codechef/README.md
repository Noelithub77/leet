# CodeChef and CPH imports

[CHOSEN] Bundle 100 popular public practice problems: 50 beginner (rating 0–999), 50 intermediate (1000–1800), sorted by successful submissions. Use CodeChef's public catalog/problem endpoints; no new Rust dependency or account integration.

- Done: Validated identities, public client, statement/sample parsing, stdin starters, and resumable snapshot scraper.
- Done: Provider/logo/catalog/navigation, solution paths, local runner, browser submission handoff, and CPH imports.
- Done: Public-only bundle refreshed, audited, checkpointed, and compacted; provenance and coverage documented.
- Done: Workspace check passed: 32 GUI tests, 129 core tests, 12 ignored live tests, and five installer regressions. Native CodeChef samples ran successfully; imports reused problem tabs and focused the same window.
- Done: Commit `97d1e21` deployed locally; Linux login listener enabled and active, owning 10042, 10043, and 10045 while earlier candidates were occupied. Installed receiver accepted 2275/G and reused the existing GUI.
- Pending decision: Optional installed-curl fallback for Codeforces HTTP 403 responses. The normal HTTP client rejects 2275/G and this newer problem is absent from the historical snapshot; curl retrieves its complete public statement. The fallback has not been added.

[CHOSEN] Acquire the first three free default Companion ports from 1327, 4244, 6174, 10042, 10043, 10045; skip occupied ports and exclude VS Code CPH port 27121. Warn if none are available and keep retrying, deduplicate broadcasts, and remove CPH settings.

[CHOSEN] Run a background login listener, launch Leet only when no GUI exists, and open/select a problem tab in the existing window. Allow both VS Code and Leet to receive the browser click and bring Leet forward. Preserve concurrent sidebar/chat work.
