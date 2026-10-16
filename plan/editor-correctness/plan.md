# Editor correctness and patch release

- Reproduce missing C++ judge imports with real clangd; provide editor-only context without changing saved source or document coordinates.
- Reproduce snippet placeholder geometry through production GPUI input; align the marker and caret with selected text, including scrolling and zoom.
- Add configurable signature overload shortcuts and arrow tooltips; verify focus and existing input behavior.
- Run focused regressions, update release notes, commit task files, and start the next patch release using `ops release:publish`.

## Progress

- Real clangd regression reproduced the missing `vector` diagnostic, then passed with an editor-only forced header. Saved code and diagnostic positions remain unchanged.
- Native GPUI regression reproduced the placeholder offset. The overlay now uses window text bounds directly, including the editor’s existing scroll adjustment. Regular and scrolled/zoomed marker alignment, Tab/Shift+Tab, and the final body caret pass.
- Alt+Up / Alt+Down and the compact strip above completions are **[chosen]**. Native input and PNG checks pass on the combined checkout, including normal completion arrows, shortcut overrides, focus, and shortcut tooltips. Dark and light/zoomed renders inspected.
- Both live clangd regressions pass, including a C++17 project compile command that overrides the judge fallback.
- Chat-management work is committed separately as `c2af1a5`. Release version: `v0.4.2`, published through the tracked `release:publish` operator.
- Published Vim history is merged as `b5288d5`, preserving chat settings and both focused fixture routes. Native snippets/parameter hints, Vim, and tour interactions pass on the merged checkout; final compact dark/narrow renders inspected. Parameter hints are restricted to Vim Insert mode.

- Workspace tests with `gui/gui-test`: 274 passed, 22 ignored. Both real clangd tests passed separately. Native snippets/rendering, Copilot, Vim, tour, and chat fixtures passed. The Alt modifier regression reproduced completion loss, then passed after retaining completion state beneath the Copilot preview.
