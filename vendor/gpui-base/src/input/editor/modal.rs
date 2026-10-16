//! GPUI routing for modal source editors. The existing buffer owns all edits.
use std::{cell::RefCell, rc::Rc};

use gpui::{Context, KeystrokeEvent, Window, px};

use crate::input::{
    EditorState, InputBaseState, InputEvent, InputModeKind, Paste, RopeExt as _,
    cursor::CursorSelection,
    undo_manager::EditIntent,
    vim::{VimEngine, VimMode, VimOutcome, VimScroll, VimShared},
};

impl EditorState {
    pub fn set_vim_shared(&mut self, shared: Rc<RefCell<VimShared>>) {
        self.vim_shared = Some(shared);
    }

    /// Enable modal editing without replacing the document or history.
    /// Use `set_vim_enabled_in_window` when disabling a live Insert session.
    pub fn set_vim_enabled(&mut self, enabled: bool, cx: &mut Context<Self>) {
        if enabled == self.vim.is_some() {
            return;
        }
        self.commit_vim_insert();
        self.undo_manager.break_transaction_coalescing();
        self.selections.remove_all_but_active();
        self.ime_marked_range = None;
        self.vim_message = None;
        self.vim_text_snapshot = None;
        self.vim_cursor = None;
        self.vim = enabled
            .then(|| VimEngine::new(self.vim_shared.get_or_insert_with(Default::default).clone()));
        cx.emit(InputEvent::VimChanged);
        cx.notify();
    }

    pub fn set_vim_enabled_in_window(
        &mut self,
        enabled: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !enabled {
            self.finish_modal_input(window, cx);
        }
        self.set_vim_enabled(enabled, cx);
    }

    pub fn finish_vim(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.finish_modal_input(window, cx);
    }

    /// Group native edits as one undo step. Nested transactions stay grouped.
    pub fn with_edit_transaction<R>(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        edit: impl FnOnce(&mut Self, &mut Window, &mut Context<Self>) -> R,
    ) -> R {
        self.undo_manager.break_transaction_coalescing();
        self.undo_manager.begin_transaction();
        let result = edit(self, window, cx);
        self.undo_manager.commit_transaction();
        result
    }

    pub fn vim_mode(&self) -> Option<VimMode> {
        self.vim.as_ref().map(VimEngine::mode)
    }

    pub fn vim_pending(&self) -> String {
        self.vim
            .as_ref()
            .map(VimEngine::pending)
            .unwrap_or_default()
    }

    pub fn vim_message(&self) -> Option<&str> {
        self.vim_message.as_deref()
    }

    /// The painted caret in window coordinates, including vertical scrolling.
    pub fn cursor_window_layout(&self) -> Option<(gpui::Bounds<gpui::Pixels>, gpui::Pixels)> {
        let (mut bounds, height) = self.cursor_layout()?;
        bounds.origin.y += self.scroll_handle.offset().y;
        Some((bounds, height))
    }
}

impl<M: InputModeKind> InputBaseState<M> {
    pub(super) fn vim_accepts_text(&self) -> bool {
        self.vim
            .as_ref()
            .is_none_or(|vim| vim.mode() == VimMode::Insert)
    }

    fn commit_vim_insert(&mut self) {
        if self.vim_insert_transaction {
            self.undo_manager.commit_transaction();
            self.vim_insert_transaction = false;
        }
    }

    pub(super) fn intercept_vim(
        &mut self,
        event: &KeystrokeEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.disabled || self.vim.is_none() || !self.focus_handle.is_focused(window) {
            return;
        }
        let stroke = &event.keystroke;
        // Alternate/platform shortcuts keep their normal host meaning.
        if stroke.modifiers.platform || stroke.modifiers.alt {
            return;
        }
        let key = if stroke.modifiers.control {
            format!("ctrl-{}", stroke.key)
        } else if let Some(character) = &stroke.key_char {
            character.to_string()
        } else if stroke.key == "space" {
            " ".into()
        } else if stroke.modifiers.shift && stroke.key.len() == 1 {
            stroke.key.to_uppercase()
        } else {
            stroke.key.clone()
        };
        self.modal_key(&key, window, cx);
    }

    fn modal_cursor(&self) -> usize {
        if self.vim.as_ref().is_some_and(|vim| {
            matches!(
                vim.mode(),
                VimMode::Visual | VimMode::VisualLine | VimMode::VisualBlock
            )
        }) {
            self.vim_cursor.unwrap_or(self.cursor())
        } else {
            self.cursor()
        }
    }

    // Native edits and history replay advance the revision in push_history.
    fn modal_text_snapshot(&mut self) -> Rc<String> {
        let revision = self.document_revision;
        let snapshot = self
            .vim_text_snapshot
            .get_or_insert_with(|| (revision, Rc::new(self.text.to_string())));
        if snapshot.0 != revision {
            *snapshot = (revision, Rc::new(self.text.to_string()));
        }
        snapshot.1.clone()
    }

    fn modal_key(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) {
        if self.vim.as_ref().is_none_or(|vim| !vim.handles_key(key)) {
            return;
        }
        let before = self.modal_text_snapshot();
        let cursor = self.modal_cursor();
        let editable = self.is_editable();
        let tab_size = self.mode.tab_size().tab_size;
        let Some(vim) = &mut self.vim else {
            return;
        };
        let old_mode = vim.mode();
        let outcome = vim.key(key, &before, cursor, editable, tab_size);
        let new_mode = vim.mode();
        if !outcome.handled {
            return;
        }
        // Stop both keybinding dispatch and subsequent platform text input.
        cx.stop_propagation();
        window.prevent_default();
        if old_mode != VimMode::Insert && new_mode == VimMode::Insert {
            self.undo_manager.break_transaction_coalescing();
            self.undo_manager.begin_transaction();
            self.vim_insert_transaction = true;
        }
        self.apply_modal_outcome(outcome, &before, window, cx);
        if new_mode != VimMode::Insert {
            if self.ime_marked_range.take().is_some() {
                self.undo_manager.commit_transaction();
            }
            self.commit_vim_insert();
        }
        if old_mode != new_mode || matches!(key, "escape" | "ctrl-[") {
            M::clear_inline_completion(self, cx);
            M::reset_language_features(self);
        }
        cx.emit(InputEvent::VimChanged);
        self.pause_blink_cursor(cx);
        cx.notify();
    }

    pub(super) fn blur_modal_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.vim.is_none() {
            return;
        }
        let before = self.modal_text_snapshot();
        let cursor = self.modal_cursor();
        let tab_size = self.mode.tab_size().tab_size;
        let Some(vim) = &mut self.vim else {
            return;
        };
        let outcome = vim.blur(&before, cursor, tab_size);
        self.apply_modal_outcome(outcome, &before, window, cx);
        if self.ime_marked_range.take().is_some() {
            self.undo_manager.commit_transaction();
        }
        self.commit_vim_insert();
        cx.emit(InputEvent::VimChanged);
        cx.notify();
    }

    pub(super) fn finish_modal_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.vim.is_none() {
            return;
        }
        let before = self.modal_text_snapshot();
        let cursor = self.modal_cursor();
        let editable = self.is_editable();
        let tab_size = self.mode.tab_size().tab_size;
        let outcome = self
            .vim
            .as_mut()
            .expect("checked modal engine")
            .key("escape", &before, cursor, editable, tab_size);
        self.apply_modal_outcome(outcome, &before, window, cx);
        if self.ime_marked_range.take().is_some() {
            self.undo_manager.commit_transaction();
        }
        self.commit_vim_insert();
        cx.emit(InputEvent::VimChanged);
        cx.notify();
    }

    fn apply_modal_outcome(
        &mut self,
        outcome: VimOutcome,
        before: &Rc<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.vim_message = outcome.message;
        if outcome.native_paste {
            // Preserve the native selection for clipboard replacement.
        } else if let Some((undo, count)) = outcome.history {
            self.commit_vim_insert();
            for _ in 0..count {
                self.replay_modal_history(undo, window, cx);
            }
            let cursor = self.active_selection().start;
            self.set_cursor_to(cursor);
            self.selections.remove_all_but_active();
            self.vim_cursor = Some(cursor);
        } else {
            if !Rc::ptr_eq(before, &outcome.text)
                && before.as_str() != outcome.text.as_str()
                && self.is_editable()
            {
                let (range, replacement) = text_difference(before, &outcome.text);
                self.undo_manager.set_pending_intent(EditIntent::Atomic);
                self.replace_text_in_ranges(&[(range, replacement.to_string())], window, cx);
            }
            self.vim_cursor = Some(outcome.cursor.min(self.text.len()));
            if outcome.selections.is_empty() {
                self.selections.remove_all_but_active();
                self.set_cursor_to(outcome.cursor.min(self.text.len()));
            } else {
                let mut selections: Vec<CursorSelection> = outcome
                    .selections
                    .into_iter()
                    .map(|range| {
                        let id = self.selections.generate_id();
                        let mut selection = CursorSelection::new(id, range.start, range.end);
                        selection.reversed = outcome.cursor <= selection.start;
                        selection
                    })
                    .collect();
                if let Some(index) = selections.iter().position(|selection| {
                    outcome.cursor >= selection.start && outcome.cursor <= selection.end
                }) {
                    selections.swap(0, index);
                }
                self.selections.replace_all(selections);
            }
            self.cursor_line_end_affinity = false;
            self.scroll_to(outcome.cursor.min(self.text.len()), None, cx);
        }
        if outcome.native_paste {
            // Keep the existing native clipboard insertion, including its
            // selection replacement, asynchronous target guard and LSP updates.
            let block_insertion = self.vim.as_ref().is_some_and(VimEngine::block_insertion);
            if !block_insertion {
                self.commit_vim_insert();
            }
            self.undo_manager.break_transaction_coalescing();
            self.paste(&Paste, window, cx);
            if !block_insertion {
                self.undo_manager.begin_transaction();
                self.vim_insert_transaction = true;
            }
            self.vim_cursor = None;
        }
        if let Some(scroll) = outcome.scroll {
            self.scroll_modal(scroll, outcome.scroll_count, window, cx);
        }
        if let Some(command) = outcome.host {
            cx.emit(InputEvent::VimCommand(command));
        }
    }

    fn replay_modal_history(&mut self, undo: bool, window: &mut Window, cx: &mut Context<Self>) {
        if undo {
            self.undo(&crate::input::Undo, window, cx);
        } else {
            self.redo(&crate::input::Redo, window, cx);
        }
    }

    fn scroll_modal(
        &mut self,
        scroll: VimScroll,
        repetitions: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(layout) = &self.last_layout else {
            return;
        };
        let line_height = layout.line_height;
        let rows = (self.input_bounds.size.height / line_height)
            .floor()
            .max(1.) as usize;
        let delta = match scroll {
            VimScroll::HalfPageDown => Some((true, rows / 2)),
            VimScroll::HalfPageUp => Some((false, rows / 2)),
            VimScroll::PageDown => Some((true, rows.saturating_sub(2))),
            VimScroll::PageUp => Some((false, rows.saturating_sub(2))),
            _ => None,
        };
        if let Some((down, count)) = delta {
            let count = count
                .saturating_mul(repetitions)
                .min(self.text.lines_len().max(1))
                .max(1);
            for digit in count.to_string().chars() {
                self.modal_key(&digit.to_string(), window, cx);
            }
            self.modal_key(if down { "j" } else { "k" }, window, cx);
            let mut offset = self.scroll_handle.offset();
            offset.y += line_height * count as f32 * if down { -1. } else { 1. };
            self.update_scroll_offset(Some(offset), cx);
            self.deferred_scroll_offset = Some(self.scroll_handle.offset());
        } else {
            let row = self
                .text
                .offset_to_point(self.vim_cursor.unwrap_or(self.cursor()))
                .row;
            let display_row = self.display_map.buffer_line_to_display_row(row);
            let clearance = match scroll {
                VimScroll::Center => self.input_bounds.size.height / 2. - line_height / 2.,
                VimScroll::Top => px(0.),
                VimScroll::Bottom => self.input_bounds.size.height - line_height,
                _ => px(0.),
            };
            let mut offset = self.scroll_handle.offset();
            offset.y = clearance - line_height * display_row;
            self.update_scroll_offset(Some(offset), cx);
            self.deferred_scroll_offset = Some(self.scroll_handle.offset());
        }
    }
}

/// A single minimal replacement, always on UTF-8 boundaries.
fn text_difference<'a>(before: &str, after: &'a str) -> (std::ops::Range<usize>, &'a str) {
    let mut prefix = before
        .bytes()
        .zip(after.bytes())
        .take_while(|(a, b)| a == b)
        .count();
    while !before.is_char_boundary(prefix) || !after.is_char_boundary(prefix) {
        prefix -= 1;
    }
    let mut suffix = before[prefix..]
        .bytes()
        .rev()
        .zip(after[prefix..].bytes().rev())
        .take_while(|(a, b)| a == b)
        .count();
    while !before.is_char_boundary(before.len() - suffix)
        || !after.is_char_boundary(after.len() - suffix)
    {
        suffix -= 1;
    }
    (
        prefix..before.len() - suffix,
        &after[prefix..after.len() - suffix],
    )
}

#[cfg(test)]
mod tests {
    use super::text_difference;
    #[test]
    fn replacement_never_splits_unicode_or_overlaps_prefix() {
        for (before, after) in [
            ("a界z", "a語z"),
            ("abc", "ababc"),
            ("", "界"),
            ("界", ""),
            ("same", "same"),
        ] {
            let (range, value) = text_difference(before, after);
            let mut text = before.to_string();
            text.replace_range(range, value);
            assert_eq!(text, after);
        }
    }
}
