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
use std::{hash::{Hash, Hasher}, time::Duration};
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
    if pane.predictions.as_ref().is_some_and(|predictions| predictions.alt.get()) { return None; }
    let menu = pane.menu.as_ref()?;
    let theme = cx.theme().clone();
    let editor = pane.state.read(cx);
    let (cursor, line_height) = editor.cursor_layout()?;
    let input = editor.input_bounds();
    let pos = editor.scroll_offset() + cursor.origin - input.origin;
    let gap = rems(0.25).to_pixels(window.rem_size());
    let chrome = rems(0.5).to_pixels(window.rem_size()) + px(2.);
    let width = px(240.).min((input.size.width - chrome).max(px(0.)));
    let preview_width = px(300.).min(input.size.width);
    let hint = crate::copilot::preview_hint(pane, window, cx);
    let hint_height = if hint.is_some() { rems(1.75).to_pixels(window.rem_size()) } else { px(0.) };
    let signature_height = super::signature_help::height(pane, window);
    let row_height = rems(1.375).to_pixels(window.rem_size());
    let desired_list = row_height * menu.content.items.len().min(6) as f32;
    let available_height = input.size.height.min((window.viewport_size().height - input.origin.y).max(px(0.)));
    let preferred_left = pos.x + px(12.);
    let side = preferred_left + width + chrome + preview_width + gap <= input.size.width;
    let selected = menu.content.items.get(menu.selected)?;
    let completion_documentation = selected.documentation.as_ref().map(|doc| match doc { Documentation::String(text) => text.clone(), Documentation::MarkupContent(markup) => markup.value.clone() }).filter(|text| !text.trim().is_empty());
    let parameter_detail = completion_documentation.is_none() && menu.preview.is_none();
    let preview_width = if parameter_detail && !side { width + chrome } else { preview_width };
    let documentation = completion_documentation.or_else(|| super::signature_help::documentation(pane).map(|doc| match doc { lsp_wire::Documentation::String(text) => text.clone(), lsp_wire::Documentation::MarkupContent(markup) => markup.value.clone() })).filter(|text| !text.trim().is_empty());
    let has_preview = menu.preview.is_some() || documentation.is_some();
    let desired_menu = desired_list + chrome + hint_height + signature_height;
    let short_parameter_detail = parameter_detail && documentation.as_ref().is_some_and(|text| text.len() <= 160 && text.lines().count() <= 2);
    let desired_preview = if short_parameter_detail { rems(4.).to_pixels(window.rem_size()) } else { px(184.) };
    let desired_height = if !has_preview { desired_menu } else if side { desired_menu.max(desired_preview) } else { desired_menu + gap + desired_preview };
    let above = (pos.y - px(4.)).max(px(0.)).min(available_height);
    let below = (pos.y + line_height + px(4.)).max(px(0.)).min(available_height);
    let below_space = available_height - below;
    let use_below = desired_height <= below_space || (desired_height > above && below_space >= above);
    let budget = if use_below { below_space } else { above };
    // Fit the whole popup on one side of the typing line, including its footer and borders.
    let minimum_menu = row_height + chrome + hint_height + signature_height;
    let stacked = has_preview && !side;
    if budget < minimum_menu + if stacked { gap + px(40.) } else { px(0.) } { return None; }
    let menu_budget = if stacked && desired_menu + gap + desired_preview <= budget { desired_menu }
        else if stacked { desired_menu.min((budget * 0.45).max(minimum_menu)) } else { desired_menu.min(budget) };
    let height = row_height * ((menu_budget - chrome - hint_height - signature_height).as_f32() / row_height.as_f32()).floor();
    let menu_height = height + chrome + hint_height + signature_height;
    let preview_height = if stacked { desired_preview.min((budget - menu_height - gap).max(px(0.))) } else { desired_preview.min(budget) };
    let popup_height = if !has_preview { menu_height } else if side { menu_height.max(preview_height) } else { menu_height + gap + preview_height };
    let popup_width = if !has_preview { width + chrome } else if side { width + chrome + preview_width + gap } else { (width + chrome).max(preview_width) };
    let left = preferred_left.min((input.size.width - popup_width).max(px(0.)));
    let top = if use_below { below } else { above - popup_height };
    let surface = theme.popover.opacity(0.97);
    let preview_text = format!("{}{}", menu.preview.as_ref().map_or("", |(code, _)| code.as_str()), documentation.as_deref().unwrap_or(""));
    let preview: Option<AnyElement> = if let Some((code, highlighter)) = &menu.preview {
        Some(div().font_family(theme.mono_font_family.clone()).text_size(rems(11. / 16.))
            .when_some(documentation.as_ref().and_then(|doc| Some(doc.split_once("```").map_or(doc.as_str(), |(header, _)| header).trim().to_owned())).filter(|header| !header.is_empty()), |view, header|
                view.child(div().mb_2().text_color(theme.muted_foreground).child(header)))
            .child(StyledText::new(code.clone()).with_highlights(highlighter.styles(&(0..code.len()), theme.highlight_theme.as_ref()))).into_any_element())
    } else {
        documentation.map(|doc| TextView::markdown("completion-detail", doc).into_any_element())
    };
    let list = uniform_list("source-completion-list", menu.content.items.len(), cx.processor(move |pane, range: std::ops::Range<usize>, _, cx| {
        let Some(menu) = &pane.menu else { return vec![]; };
        range.filter_map(|index| menu.content.items.get(index).map(|item| (index, item))).map(|(index, item)| {
            let (icon, label) = kind_icon(item.kind); let theme = cx.theme().clone();
            h_flex().id(index).h(row_height).w_full().px_2().gap_2().rounded_sm()
                .font_family(theme.mono_font_family.clone()).text_size(rems(11. / 16.))
                .when(index == menu.selected, |row| row.bg(theme.list_active))
                .hover(|row| row.bg(theme.accent.opacity(0.5)))
                .child(div().id(("completion-kind", index)).tooltip(move |window, cx| gpui_kit::component::tooltip::Tooltip::new(label).build(window, cx))
                    .child(Icon::new(icon).size(px(12.)).text_color(theme.primary.opacity(0.85))))
                .child({
                    let text = if index == menu.selected {
                        menu.preview.as_ref().and_then(|(code, _)| code.lines().find(|line| !line.trim().is_empty()))
                            .map(str::to_owned).unwrap_or_else(|| item.label.clone())
                    } else { item.label.clone() };
                    soften_text(div().flex_1().truncate().child(text.clone()), &text, cx)
                })
                .when_some(if index == menu.selected && menu.preview.is_some() { Some(item.label.clone()) } else { item.detail.clone() }, |row, detail|
                    row.child(div().max_w(px(100.)).truncate().text_size(rems(11. / 16.)).text_color(theme.muted_foreground).child(detail)))
                .on_mouse_down(MouseButton::Left, cx.listener(move |pane, _, window, cx| {
                    if let Some(menu) = &mut pane.menu { menu.selected = index; }
                    pane.accept_completion(window, cx); cx.stop_propagation();
                })).into_any_element()
        }).collect()
    })).track_scroll(&menu.scroll).w(width).h(height).test_support();
    Some(deferred(anchored().position(input.origin + point(left, top)).child(div().id("source-completions").test_support().flex().gap_1().items_start()
        .when(!side, |popup| popup.flex_col())
        .child(div().p_1().bg(surface).border_1().border_color(theme.border).rounded_md().shadow_md()
            .when_some(super::signature_help::render_content(pane, false, cx), |menu, signature| menu.child(div().id("completion-signature").test_support().w(width).h(signature_height).overflow_hidden().border_b_1().border_color(theme.border)
                .child(div().id("completion-signature-scroll").size_full().overflow_y_scroll().on_scroll_wheel(|_, _, cx| cx.stop_propagation()).child(signature))))
            .child(list)
            .when_some(hint, |menu, hint| menu.child(div().h(hint_height).overflow_hidden().child(hint))))
        .when_some(preview, |popup, preview| popup.child(div().id("source-completion-preview").test_support().relative().overflow_hidden()
            .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
            .w(preview_width).h(preview_height).bg(surface).border_1().border_color(theme.border).rounded_md().shadow_md()
            .child(soften_text(div().id("source-completion-preview-scroll").overflow_y_scroll().h((preview_height - px(2.)).max(px(0.))).p_2().when(menu.preview.is_some(), |view| view.pb_8()).text_size(rems(11. / 16.)).child(preview), &preview_text, cx))
            .when(menu.preview.is_some(), |view| view.child(deferred(div().id("completion-preview-fade").test_support().absolute().w_full().left_0().bottom_0().h_8().rounded_b_md()
                .bg(linear_gradient(180., linear_color_stop(theme.popover.opacity(0.), 0.), linear_color_stop(surface, 1.))))))))
        .on_mouse_down_out(cx.listener(|pane, _, window, cx| pane.close_completions(window, cx)))
    )).into_any_element())
}

// Keep text readable on the first frame; only soften the final contrast change.
fn soften_text<E: Styled + IntoElement + 'static>(view: E, text: &str, cx: &App) -> AnyElement {
    if cx.reduce_motion() { return view.into_any_element(); }
    let mut key = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut key);
    view.with_animation(("completion-text", key.finish()), Animation::new(Duration::from_millis(50)),
        |view, progress| view.opacity(0.88 + 0.12 * progress)).into_any_element()
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
