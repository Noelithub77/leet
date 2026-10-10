//! Compact completion rows and previews anchored to the source caret.
use gpui_kit::*;
use gpui_kit::assets::IconName;
use gpui_kit::component::{ActiveTheme as _, Icon, h_flex};
use gpui_kit::component::highlighter::SyntaxHighlighter;
use gpui_kit::base::input::CompletionMenuState;
use gpui_kit::component::text::TextView;
use gpui_kit::prelude::FluentBuilder as _;
use lsp_types::{CompletionItemKind, CompletionTextEdit, Documentation};
use practice::language::Language;
use super::expansion::EditorPane;

pub(super) struct Menu {
    pub content: CompletionMenuState,
    pub offset: usize,
    pub selected: usize,
    manually_selected: bool,
    pub scroll: UniformListScrollHandle,
    preview: Option<(String, SyntaxHighlighter)>,
    preview_key: Option<(Language, String)>,
}
impl Menu {
    pub fn new(content: CompletionMenuState, offset: usize) -> Self {
        Self { content, offset, selected: 0, manually_selected: false, scroll: UniformListScrollHandle::new(), preview: None, preview_key: None }
    }
    pub fn update(&mut self, content: CompletionMenuState) {
        self.manually_selected &= self.content.query == content.query;
        let selected = self.manually_selected.then(|| self.content.items.get(self.selected)).flatten();
        let index = selected.and_then(|item| content.items.iter().position(|candidate| candidate.label == item.label && candidate.kind == item.kind && candidate.insert_text == item.insert_text));
        self.manually_selected &= index.is_some();
        self.selected = index.unwrap_or(0);
        self.content = content;
        if !self.content.items.is_empty() { self.scroll.scroll_to_item(self.selected, ScrollStrategy::Nearest); }
    }
    pub fn step(&mut self, delta: isize) {
        let len = self.content.items.len();
        if len > 0 {
            self.manually_selected = true;
            self.selected = (self.selected as isize + delta).rem_euclid(len as isize) as usize;
            self.scroll.scroll_to_item(self.selected, ScrollStrategy::Nearest);
        }
    }
    pub fn prepare(&mut self, language: Language, library: &practice::snippets::search::Library) {
        let code = self.content.items.get(self.selected).filter(|item| item.kind == Some(CompletionItemKind::SNIPPET))
            .and_then(|item| match &item.text_edit {
                Some(CompletionTextEdit::Edit(edit)) => Some(edit.new_text.clone()),
                Some(CompletionTextEdit::InsertAndReplace(edit)) => Some(edit.new_text.clone()),
                None => item.insert_text.clone(),
            });
        if let Some(code) = code {
            if self.preview_key.as_ref().is_some_and(|(old_language, old)| *old_language == language && old == &code) { return; }
            self.preview_key = Some((language, code.clone()));
            let code = library.preview(&self.content.items[self.selected].label, &code, language)
                .map(str::to_owned).unwrap_or_else(|| practice::snippets::body::preview(&code).text);
            if self.preview.as_ref().is_none_or(|(old, _)| old != &code) {
                let mut highlighter = SyntaxHighlighter::new(language.id());
                highlighter.update(None, &ropey::Rope::from_str(&code), None);
                self.preview = Some((code, highlighter));
            }
        } else { self.preview = None; self.preview_key = None; }
    }
}

fn kind_icon(kind: Option<CompletionItemKind>) -> (IconName, &'static str) {
    match kind {
        Some(CompletionItemKind::SNIPPET) => (IconName::Code, "Snippet"),
        Some(CompletionItemKind::FUNCTION | CompletionItemKind::METHOD | CompletionItemKind::CONSTRUCTOR) => (IconName::SquareFunction, "Function"),
        Some(CompletionItemKind::VARIABLE | CompletionItemKind::FIELD | CompletionItemKind::PROPERTY) => (IconName::CircleDot, "Variable"),
        Some(CompletionItemKind::CLASS | CompletionItemKind::STRUCT | CompletionItemKind::INTERFACE | CompletionItemKind::ENUM) => (IconName::Box, "Type"),
        Some(CompletionItemKind::MODULE) => (IconName::Folder, "Module"),
        Some(CompletionItemKind::CONSTANT | CompletionItemKind::ENUM_MEMBER) => (IconName::Hash, "Constant"),
        Some(CompletionItemKind::KEYWORD) => (IconName::Key, "Keyword"),
        _ => (IconName::Code, "Completion"),
    }
}

pub(super) fn render(pane: &EditorPane, window: &mut Window, cx: &mut Context<EditorPane>) -> Option<AnyElement> {
    let menu = pane.menu.as_ref()?;
    let theme = cx.theme().clone();
    let editor = pane.state.read(cx);
    let (cursor, line_height) = editor.cursor_layout()?;
    let input = editor.input_bounds();
    let pos = editor.scroll_offset() + cursor.origin - input.origin;
    let width = px(340.).min(input.size.width.max(px(160.)));
    let preview_width = px(480.).min((input.size.width - px(8.)).max(px(160.)));
    let height = px(26. * menu.content.items.len().min(8) as f32 + 8.);
    let below = pos.y + line_height + px(4.);
    let available_height = input.size.height.min((window.viewport_size().height - input.origin.y).max(px(0.)));
    let side = width + preview_width + px(16.) <= input.size.width;
    let selected = menu.content.items.get(menu.selected)?;
    let documentation = selected.documentation.as_ref().map(|doc| match doc { Documentation::String(text) => text.clone(), Documentation::MarkupContent(markup) => markup.value.clone() });
    let has_preview = menu.preview.is_some() || documentation.is_some();
    let popup_width = if !has_preview { width } else if side { width + preview_width + px(16.) } else { width.max(preview_width) };
    let left = pos.x.max(px(0.)).min((input.size.width - popup_width).max(px(0.)));
    let popup_height = if has_preview { if side { height.max(px(240.)) } else { height + px(244.) } } else { height };
    let top = if below + popup_height > available_height { (pos.y - popup_height - px(4.)).max(px(0.)) } else { below };
    let preview: Option<AnyElement> = if let Some((code, highlighter)) = &menu.preview {
        Some(div().font_family(theme.mono_font_family.clone()).text_xs()
            .when_some(documentation.as_ref().and_then(|doc| Some(doc.split_once("```").map_or(doc.as_str(), |(header, _)| header).trim().to_owned())).filter(|header| !header.is_empty()), |view, header|
                view.child(div().mb_2().text_color(theme.muted_foreground).child(header)))
            .child(StyledText::new(code.clone()).with_highlights(highlighter.styles(&(0..code.len()), theme.highlight_theme.as_ref()))).into_any_element())
    } else {
        documentation.map(|doc| TextView::markdown("completion-detail", doc).into_any_element())
    };
    let list = uniform_list("source-completion-list", menu.content.items.len(), cx.processor(|pane, range: std::ops::Range<usize>, _, cx| {
        let Some(menu) = &pane.menu else { return vec![]; };
        range.filter_map(|index| menu.content.items.get(index).map(|item| (index, item))).map(|(index, item)| {
            let (icon, label) = kind_icon(item.kind); let theme = cx.theme().clone();
            h_flex().id(index).h(px(26.)).w_full().px_2().gap_2().rounded_sm()
                .font_family(theme.mono_font_family.clone()).text_xs()
                .when(index == menu.selected, |row| row.bg(theme.list_active))
                .hover(|row| row.bg(theme.accent.opacity(0.5)))
                .child(div().id(("completion-kind", index)).tooltip(move |window, cx| gpui_kit::component::tooltip::Tooltip::new(label).build(window, cx))
                    .child(Icon::new(icon).size(px(14.)).text_color(theme.primary.opacity(0.85))))
                .child(div().flex_1().truncate().child(if index == menu.selected {
                    menu.preview.as_ref().and_then(|(code, _)| code.lines().find(|line| !line.trim().is_empty()))
                        .map(str::to_owned).unwrap_or_else(|| item.label.clone())
                } else { item.label.clone() }))
                .when_some(if index == menu.selected && menu.preview.is_some() { Some(item.label.clone()) } else { item.detail.clone() }, |row, detail|
                    row.child(div().max_w(px(145.)).truncate().text_xs().text_color(theme.muted_foreground).child(detail)))
                .on_mouse_down(MouseButton::Left, cx.listener(move |pane, _, window, cx| {
                    if let Some(menu) = &mut pane.menu { menu.selected = index; }
                    pane.accept_completion(window, cx); cx.stop_propagation();
                })).into_any_element()
        }).collect()
    })).track_scroll(&menu.scroll).w(width).h(height);
    Some(deferred(div().id("source-completions").test_support().absolute().left(left).top(top).flex().gap_1().items_start()
        .when(!side, |popup| popup.flex_col())
        .child(div().p_1().bg(theme.popover).border_1().border_color(theme.border).rounded_md().shadow_md().child(list)
            .when_some(crate::copilot::preview_hint(pane, window, cx), |menu, hint| menu.child(hint)))
        .when_some(preview, |popup, preview| popup.child(div().id("source-completion-preview").test_support().relative().overflow_hidden()
            .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
            .w(preview_width).bg(theme.popover).border_1().border_color(theme.border).rounded_md().shadow_md()
            .child(div().id("source-completion-preview-scroll").overflow_y_scroll().max_h(px(240.)).p_2().pb_8().text_xs().child(preview))
            .child(deferred(div().id("completion-preview-fade").test_support().absolute().w_full().left_0().bottom_0().h_8().rounded_b_md()
                .bg(linear_gradient(180., linear_color_stop(theme.popover.opacity(0.), 0.), linear_color_stop(theme.popover, 1.)))))))
        .on_mouse_down_out(cx.listener(|pane, _, window, cx| pane.close_completions(window, cx)))
    ).into_any_element())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[::core::prelude::v1::test]
    fn completion_kinds_have_distinct_small_icons() {
        assert_ne!(kind_icon(Some(CompletionItemKind::SNIPPET)).0, kind_icon(Some(CompletionItemKind::FUNCTION)).0);
        assert_ne!(kind_icon(Some(CompletionItemKind::FUNCTION)).0, kind_icon(Some(CompletionItemKind::VARIABLE)).0);
        assert_eq!(kind_icon(Some(CompletionItemKind::METHOD)).1, "Function");
    }
}
