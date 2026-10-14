# Vim technical design

Status: Frozen. Based on approved PRD dated 2026-10-13.

The vendored gpui-base InputBaseState is the authoritative text, selection and undo owner. Add a safe Rust pure modal engine in input/editor/vim.rs and a GPUI adapter beside it. The engine computes text, cursor and explicit selections; adapter applies a minimal byte-boundary-safe replacement through existing editing APIs, preserving notifications, autosave, LSP and history. Native Insert input remains on the current path.

## Contracts
VimMode: Normal, Insert, Visual, VisualLine, VisualBlock. VimShared: shared register/repeat/search. VimEngine::new(shared), mode(), pending(), handles_key(key), key(key,shared_text,cursor,editable,tab_size) -> VimOutcome. Outcome carries handled, shared text, cursor, selections, optional history (undo flag,count), host, scroll with scroll_count, native_paste and message. Host commands: Save, Close, SaveClose, SaveIfDirtyClose. Scroll: HalfPageDown/Up, PageDown/Up, Center/Top/Bottom.
EditorState: set_vim_enabled(bool,cx), vim_mode()->Option<VimMode>, vim_pending()->String; InputEvent adds VimCommand(VimHostCommand) and VimChanged. Workspace shares state via set_vim_shared(Rc<RefCell<VimShared>>). New engine starts Normal.

## Lifecycle and history
Pre-action keystroke interception uses focused editor identity. Ordinary inputs never enable Vim. Guard platform replacement and IME when not Insert. Insert repeat is derived from the session start and actual final text, including completion and paste. Native keystrokes bypass modal snapshots; navigation shares a revision-keyed document snapshot, with copy-on-write for edits. Enter Insert opens a logical undo transaction; Escape/blur/toggle closes it. Native paste remains an atomic transaction. Rectangular edits apply as one transaction. Existing undo remains authoritative.

## Rendering and host
Use existing multi-selection highlights for rectangle rows. Broaden caret to glyph width outside Insert. Show compact mode and pending input in editor pane. Emit save/close events to workspace and snippet host; save failure retains editor and reports error.

## Risk and compatibility
Unicode boundaries must never panic. Display columns count tab stops and Unicode widths through existing dependencies. Read-only edits and independent multi-cursors are rejected. Disabling restores conventional editor input while preserving text/history. No new dependencies, migrations or account behavior.

## Validation
Pure command/reference tests; focused headless real keyboard/pointer fixture; snippet and completion regression fixtures; workspace quality checks. One heavy job at a time with two CPUs/workers and idle I/O.

Renderer contract: block_geometry(text,tab_size) exposes exclusive logical row/display column ranges; width_at maps cells for partial-glyph selection painting.

Implementation details: shared literal query history is bounded to 100 entries and supports Up/Down recall. `cursor_window_layout` adds painted scroll-adjusted coordinates without changing the existing content-coordinate caret API. Disabling through the host finishes insertion transactions before restoring conventional input.
