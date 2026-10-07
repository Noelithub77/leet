# Test case editing and compact tabs

- [CHOSEN] Reuse toolkit base Tab/Tabs for rounded cyan backgrounds and white active labels; retain scrolling, tooltips, close actions, and existing keyboard navigation.
- [CHOSEN] Reuse the existing Textarea dialog for adding/editing cases. Store per-question case overrides in the existing Diesel-backed private KV cache. Preserve legacy custom cases until overridden.
- Restore original question examples, clearing edited/custom overrides and stale results. Block mutations during runs; retain separate state across question tabs.
- Remove persistent app/practice progress text from the status bar.
- Verify SQLite persistence/reset regressions and native add/edit/reset controls, run required checks, commit, and local:deploy.

Verification: 45 Rust tests passed, including persisted edit/add/reset isolation from bundled question content. Native Wayland verified white rounded cyan tabs, removed status label, add/edit shortcuts, input/expected persistence across restart, individual reset preserving the custom case, and full reset restoring three original examples.
