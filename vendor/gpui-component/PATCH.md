# Tooltip presentation patch

This is the source distribution of `gpui-component` 0.7.1 from upstream commit
`87d10ae5e1299d1be18670a37570c5402f952e34`. The crates.io archive checksum is
`bc8b7def8e74b0d822d3e96f6b28fb5a2c20eb7a83d30d22fa59ca8088d1159b`.
Its Apache 2.0 license is preserved in `LICENSE-APACHE`.

Only `src/tooltip.rs` differs: tooltip surfaces have a reading-width and viewport
cap, wrapping content that can shrink beside shortcut keycaps, and balanced
padding. Theme colors, native positioning, delays, transitions, and dismissal
remain upstream behavior. Both native element tooltips and managed component
tooltips use this presentation.

The workspace selects this existing dependency through `[patch.crates-io]`.
Registry bookkeeping and the upstream lockfile are omitted. Remove the patch
when upstream supplies equivalent sizing and wrapping.
