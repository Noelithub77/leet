# Parameter hints

- Complete: language-server signature help reuses the existing versioned document connection; no new dependencies.
- Complete: Ctrl+Space requests completions and signature help. Automatic hints follow nested and incomplete calls and the server's active parameter.
- Complete: shared completion surface or compact caret popup, with wrapping, focus preservation, Escape, overload commands, and stale-result rejection.
- Verified: 55 GUI and 215 practice tests pass (20 optional integration tests skipped); the full isolated GUI suite passes, including compact/light/zoomed hints.
- Verified separately: real basedpyright and clangd signature responses and first/second active parameters in temporary workspaces with isolated data/cache paths.
- Complete: production compilation, development guidance, fixture documentation, and user-facing release notes. No release publication or installed deployment requested for this task.
