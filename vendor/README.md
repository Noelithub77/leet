# Editor patch

`gpui-base` is the unmodified source distribution of GPUI Kit 0.7.1, apart from
the editor hooks described below. Its Apache 2.0 license
is preserved in `gpui-base/LICENSE-APACHE`. Source: [GPUI Kit](https://github.com/longbridge/gpui-kit).
The published crate comes from upstream commit `87d10ae5e1299d1be18670a37570c5402f952e34`;
its crates.io SHA-256 checksum is `3cf698677b253118459f552fe15513858c669d79a25837a94e480db2ae330050`.

The local patch adds `set_inline_completion_highlights` to editor state and
passes those byte ranges through the native ghost-text renderer on every line.
It rejects stale completion text and invalid UTF-8 boundaries, clears styles
when the completion changes, and preserves the default gray styling when no
highlight ranges are supplied. Leet owns the syntax colors and opacity.

The second local patch adds `LineEndHint`, `set_line_end_hints`, and
`line_end_hints` for presentation-only labels after the last wrapped row of a
buffer line. Only visible, non-folded lines are shaped; labels preserve their
supplied colors and order, separated by two spaces, without changing text,
wrapping, scrolling, selection, or history. Control characters and line
separators are rejected, and all hints clear on text edits so Leet can resend
fresh annotations.

The workspace excludes this upstream crate from its members and selects it
through `[patch.crates-io]`. Registry bookkeeping and the upstream lockfile are
omitted. Keep the source version pinned; remove this patch when an upstream
release provides the equivalent hook.
