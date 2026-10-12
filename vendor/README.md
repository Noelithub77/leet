# Editor patch

`gpui-base` is the unmodified source distribution of GPUI Kit 0.7.1, apart from
the inline completion highlighting hook described below. Its Apache 2.0 license
is preserved in `gpui-base/LICENSE-APACHE`. Source: [GPUI Kit](https://github.com/longbridge/gpui-kit).
The published crate comes from upstream commit `87d10ae5e1299d1be18670a37570c5402f952e34`;
its crates.io SHA-256 checksum is `3cf698677b253118459f552fe15513858c669d79a25837a94e480db2ae330050`.

The local patch adds `set_inline_completion_highlights` to editor state and
passes those byte ranges through the native ghost-text renderer on every line.
It rejects stale completion text and invalid UTF-8 boundaries, clears styles
when the completion changes, and preserves the default gray styling when no
highlight ranges are supplied. Leet owns the syntax colors and opacity.

The workspace excludes this upstream crate from its members and selects it
through `[patch.crates-io]`. Registry bookkeeping and the upstream lockfile are
omitted. Keep the source version pinned; remove this patch when an upstream
release provides the equivalent hook.
