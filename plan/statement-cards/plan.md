[CHOSEN] Keep all statement headers visible with one expanded card. Scroll only the open body; opening Examples closes Description. Use quiet theme surfaces and a thin keyboard focus outline.

- Implemented bounded card stack, exclusive expansion, and hint rendering when collapsed.
- Added fixture checks for long content, compact panes, light/dark themes, 140% zoom, wheel/PageDown, and Tab/Space.
- Headless interaction and pixel checks passed; OmaBox confirmed pointer/keyboard switching, bottom header visibility, and the final focus outline in a narrow native window.
- Final repository check passed: 185 Rust tests and five installer regressions. One preceding run hit a Companion port-release failure; the complete rerun passed.
- Deliver through a focused commit and `./ops local:deploy --json`; keep unrelated site changes outside the commit.
