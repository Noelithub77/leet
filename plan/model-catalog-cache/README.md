# Session model catalogs and debug shortcut

- [CHOSEN] Ctrl+` toggles Code/Debug through the existing action; retain Alt+B as an alternative.
- [CHOSEN] Persist agent catalogs in the existing personal SQLite cache. Show cached catalogs immediately and revalidate once per agent per app session, sharing in-flight work with AI runs. Failures retain the previous cache and retry only in the next app session.
- [CHOSEN] Prefer the newest advertised Antigravity Flash model with low reasoning. Preserve explicit model choices.
- Verify shared fetching, persistence across sessions, failure retention, shuffled model versions, shortcut dispatch, native picker and debugger behavior, then check, commit and deploy.
