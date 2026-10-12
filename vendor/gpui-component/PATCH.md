# Tooltip presentation patch

This is the source distribution of `gpui-component` 0.7.1 from upstream commit
`87d10ae5e1299d1be18670a37570c5402f952e34`. The crates.io archive checksum is
`bc8b7def8e74b0d822d3e96f6b28fb5a2c20eb7a83d30d22fa59ca8088d1159b`.
Its Apache 2.0 license is preserved in `LICENSE-APACHE`.

Two areas differ.

- `src/tooltip.rs`: tooltip surfaces have a reading-width and viewport cap,
  wrapping content that can shrink beside shortcut keycaps, and balanced
  padding. Theme colors, native positioning, delays, transitions, and dismissal
  remain upstream behavior. Both native element tooltips and managed component
  tooltips use this presentation.
- `src/input/popovers/`: editor hovers, diagnostics, completion and code action
  menus sit on a rounded, translucent card with a hairline edge and a deeper
  shadow. Hovers and diagnostics use the UI font at a reading size, may grow to
  480px tall, and stay hidden while a completion or code action menu is open
  (`src/input/overlay.rs`). The new `input::PopoverMarkdown` global lets an app extend the
  Markdown those popovers render, such as with its own block renderers;
  without it, rendering is unchanged.

The workspace selects this existing dependency through `[patch.crates-io]`.
Registry bookkeeping and the upstream lockfile are omitted. Remove the patch
when upstream supplies equivalent sizing and wrapping.
