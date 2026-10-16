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
- Chat-management work is committed separately as `c2af1a5`. Release target: `v0.4.2`, after workspace checks and committing these fixes.
