# Tour and shortcut hints

[CHOSEN] Optional whole-IDE tour with short labels and keycaps; one universal Skip control. Use existing GPUI controls, no new dependency.

- Add accurate debugger transport and case shortcut tooltips.
- Add six tour steps: problem, editor, tests, Assist, debugger, playback. Navigate panels without running code or AI; restore layout on exit.
- Make Assist result cards collapsible.
- Verify checks and native layout/keyboard behavior, commit only owned hunks, deploy locally. Preserve the other agent's staged and working changes.

Implementation and verification complete: 24 GUI and 113 core tests, five installer tests, native tour navigation/skip, transport tooltips, card collapse/expand, and a non-executing preview with normal tracing as a positive control. The full check passed with serial test execution after a timing-sensitive 150 ms test failed during concurrent builds and passed its focused rerun. Test limits and assertions were unchanged.
