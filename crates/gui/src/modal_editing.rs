//! Workspace ownership of modal preferences, shared editing state and host commands.
use gpui_kit::base::input::VimHostCommand;
use gpui_kit::component::input::EditorState;
use gpui_kit::component::notification::Notification;
use gpui_kit::component::{ActiveTheme as _, WindowExt as _, v_flex};
use gpui_kit::*;

use crate::workspace::Workspace;

impl Workspace {
    pub(crate) fn configure_vim_editor(&self, editor: &Entity<EditorState>, cx: &mut App) {
        editor.update(cx, |editor, cx| {
            editor.set_vim_shared(self.vim_shared.clone());
            editor.set_vim_enabled(self.config.vim_mode, cx);
        });
    }

    /// Applies to active, parked and recently closed editors without replacing their history.
    pub(crate) fn apply_vim_mode(&self, window: &mut Window, cx: &mut App) {
        for editor in std::iter::once(self.editor.clone()).chain(self.tab_editors()) {
            editor.update(cx, |editor, cx| {
                if !self.config.vim_mode { editor.finish_vim(window, cx); }
                editor.set_vim_shared(self.vim_shared.clone());
                editor.set_vim_enabled(self.config.vim_mode, cx);
            });
        }
        if let Some(editor) = &self.snippet_editor {
            editor.update(cx, |editor, cx| editor.configure_vim(self.config.vim_mode, self.vim_shared.clone(), window, cx));
        }
    }

    pub fn toggle_vim(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.config.vim_mode = !self.config.vim_mode;
        self.apply_vim_mode(window, cx);
        self.save_config(window, cx);
        self.flash(if self.config.vim_mode { "Vim keybindings on" } else { "Vim keybindings off" }, cx);
    }

    pub(crate) fn vim_command(&mut self, command: VimHostCommand, window: &mut Window, cx: &mut Context<Self>) {
        match command {
            VimHostCommand::Save => {
                if self.session.as_ref().is_none_or(|session| session.question.is_none()) {
                    self.toast(Notification::warning("No solution file to save"), window, cx);
                    return;
                }
                match self.save_solution_now(cx) {
                    Ok(()) => self.flash("Saved", cx),
                    Err(error) => self.toast(Notification::error(format!("Solution could not be saved: {error}")), window, cx),
                }
            }
            VimHostCommand::Close | VimHostCommand::SaveClose | VimHostCommand::SaveIfDirtyClose => self.close_problem(window, cx),
        }
    }
}

pub(crate) fn status(editor: &EditorState) -> Option<String> {
    let mode = editor.vim_mode()?;
    let mut text = mode.label().to_owned();
    let pending = editor.vim_pending();
    if !pending.is_empty() { text.push_str(" · "); text.push_str(&pending); }
    if let Some(message) = editor.vim_message() { text.push_str(" · "); text.push_str(message); }
    Some(text)
}

pub(crate) fn open_help(window: &mut Window, cx: &mut App) {
    window.open_dialog(cx, |dialog, _, cx| {
        dialog.title("Vim command help").w(px(720.)).child(
            v_flex().gap_3().text_sm()
                .child("Enable Settings > Editor > Vim keybindings, or use Toggle Vim keybindings in the palette. Escape or Ctrl+[ returns to Normal.")
                .child("Modes: i/I/a/A/o/O enter Insert; v selects characters; V selects lines; Ctrl+V selects a rectangle. Ctrl+V in a rectangle returns to Normal.")
                .child("Insert Ctrl+V uses the native clipboard and stays in Insert. Ctrl+W deletes a word; Ctrl+U deletes to the start of the line. Native completion, brackets and snippets stay available.")
                .child("Move: h/j/k/l, arrows, w/W/b/B/e/E/ge/gE, 0/^/$, gg/G, {/}, %, f/F/t/T, ;/, . Prefix motions and operators with counts, such as 2d3w.")
                .child("Edit: d/c/y with motions or inner/around text objects; dd/cc/yy; D/C/Y; x/X; s/S; r; J; >>/<<. Text objects: words, WORDs, quotes, parentheses, brackets and braces.")
                .child("Visual: motions, text objects, o/O, gv, yank/delete/change/replace, indentation and paste. Rectangles support blockwise put and I/A across selected rows.")
                .child("History: p/P, u, Ctrl+R and dot repeat. The unnamed register, last change and search are shared across code tabs.")
                .child("Search: / or ? for literal text; n/N; whole-word */#. Scroll: Ctrl+D/U/F/B and zz/zt/zb.")
                .child(":w saves; :q closes through the existing save flow; :wq and :x close only after saving succeeds. :<number> and :$ go to a line. Read-only code allows navigation, selection, yank and search.")
                .child(div().text_xs().text_color(cx.theme().muted_foreground).child("Not supported: continuous Replace, named/numbered registers, macros, marks, jump lists, substitutions, regex search, mappings, vimrc, plugins and window commands."))
        )
    });
}
