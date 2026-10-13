//! The hover card for a noted line: a kind chip, the note, its labelled detail,
//! the line's values, and the shortcuts that act on it.
use std::rc::Rc;
use gpui_kit::*;
use gpui_kit::base::text::{markdown_ast, MarkdownNode, MarkdownParseContext, MarkdownPlugin};
use gpui_kit::component::{ActiveTheme as _, Theme, h_flex, v_flex};
use gpui_kit::component::input::PopoverMarkdown;
use gpui_kit::component::kbd::Kbd;
use practice::notes::Kind;
use serde::{Deserialize, Serialize};
use crate::actions::{QuoteSelection, AskInPlace, NextNoteStep, ResolveNote};

/// The fenced block a card travels in through the editor's Markdown hover.
const FENCE: &str = "leet-note";

#[derive(Clone, Default, Serialize, Deserialize)]
pub(crate) struct Card {
    pub items: Vec<Item>,
    /// The line's cost and debugger values, as label and value.
    pub facts: Vec<(String, String)>,
    pub keys: Vec<Key>,
}

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct Item {
    pub kind: Kind,
    pub step: Option<(u32, u32)>,
    pub summary: String,
    pub detail: String,
    pub outdated: bool,
}

#[derive(Clone, Copy, PartialEq, Serialize, Deserialize)]
pub(crate) enum Key { Resolve, Question, Ask, NextStep }

impl Key {
    fn label(self) -> &'static str {
        match self { Self::Resolve => "Resolve", Self::Question => "Ask in chat", Self::Ask => "Ask AI", Self::NextStep => "Next step" }
    }
    fn action(self) -> Box<dyn Action> {
        match self { Self::Resolve => Box::new(ResolveNote), Self::Question => Box::new(QuoteSelection), Self::Ask => Box::new(AskInPlace), Self::NextStep => Box::new(NextNoteStep) }
    }
}

impl Card {
    pub(crate) fn markdown(&self) -> String {
        format!("```{FENCE}\n{}\n```", serde_json::to_string(self).unwrap_or_default())
    }
    /// Plain text for copying and screen readers.
    fn text(&self) -> String {
        let items = self.items.iter().map(|item| format!("{}: {}\n{}", label(item.kind), item.summary, item.detail).trim_end().to_owned());
        items.chain(self.facts.iter().map(|(label, value)| format!("{label}: {value}"))).collect::<Vec<_>>().join("\n\n")
    }
}

/// Renders note cards inside every editor popover's Markdown.
pub(crate) fn install(cx: &mut App) {
    cx.set_global(PopoverMarkdown(Rc::new(|view| view.plugin(CardPlugin))));
}

struct CardPlugin;
impl MarkdownPlugin for CardPlugin {
    fn name(&self) -> &str { FENCE }
    fn is_block(&self) -> bool { true }
    fn parse(&self, node: &markdown_ast::Node, context: &MarkdownParseContext<'_>) -> Option<MarkdownNode> {
        let markdown_ast::Node::Code(code) = node else { return None };
        if code.lang.as_deref() != Some(FENCE) { return None; }
        let card: Card = serde_json::from_str(&code.value).ok()?;
        let text = card.text();
        Some(MarkdownNode::new(FENCE, card).text(text.clone()).markdown(context.node_source(node).unwrap_or(&text).to_owned()))
    }
    fn render(&self, node: &MarkdownNode, window: &mut Window, cx: &mut App) -> impl IntoElement {
        match node.data::<Card>() { Some(card) => render(card, window, cx).into_any_element(), None => div().into_any_element() }
    }
}

fn label(kind: Kind) -> &'static str {
    match kind {
        Kind::Bug => "Bug", Kind::Risk => "Risk", Kind::Stuck => "Stuck", Kind::Slow => "Slow",
        Kind::Hint => "Hint", Kind::Answer => "Answer", Kind::Ask => "Your question", Kind::Comment => "Note",
    }
}

fn tint(kind: Kind, theme: &Theme) -> Hsla {
    crate::notes::color(kind, theme).unwrap_or(theme.magenta)
}

fn render(card: &Card, window: &mut Window, cx: &App) -> impl IntoElement + use<> {
    let theme = cx.theme();
    let hairline = || div().h(px(1.)).bg(theme.foreground.opacity(0.07));
    let mut column = v_flex().min_w(px(280.)).gap(px(12.)).font_family(theme.font_family.clone());
    for (index, item) in card.items.iter().enumerate() {
        if index > 0 { column = column.child(hairline()); }
        column = column.child(render_item(item, index, theme));
    }
    if !card.facts.is_empty() {
        column = column.child(v_flex().gap(px(4.)).children(card.facts.iter().map(|(label, value)| h_flex().gap(px(10.))
            .child(div().w(px(76.)).flex_none().text_size(px(11.5)).text_color(theme.muted_foreground).child(label.clone()))
            .child(div().text_size(px(12.5)).font_family(theme.mono_font_family.clone()).text_color(theme.foreground).child(value.clone())))));
    }
    let keys: Vec<AnyElement> = card.keys.iter().filter_map(|key| {
        let kbd = Kbd::binding_for_action(&*key.action(), None, window)?;
        Some(h_flex().gap(px(6.)).child(kbd).child(div().text_size(px(11.5)).text_color(theme.muted_foreground).child(key.label())).into_any_element())
    }).collect();
    if !keys.is_empty() { column = column.child(h_flex().pt(px(10.)).border_t_1().border_color(theme.foreground.opacity(0.07)).gap(px(16.)).children(keys)); }
    column
}

fn render_item(item: &Item, index: usize, theme: &Theme) -> impl IntoElement + use<> {
    let color = tint(item.kind, theme);
    let chip = chip(label(item.kind), color);
    let aside = match item.step {
        Some((step, of)) => h_flex().gap(px(4.)).children((1..=of).map(|at| div().w(px(18.)).h(px(4.)).rounded_full()
            .bg(if at == step { color } else if at < step { color.opacity(0.35) } else { theme.foreground.opacity(0.1) }))).into_any_element(),
        None if item.outdated => div().text_size(px(11.)).text_color(theme.muted_foreground).child("Code changed").into_any_element(),
        None => div().into_any_element(),
    };
    let mut column = v_flex().gap(px(8.))
        .child(h_flex().justify_between().gap(px(12.)).child(chip).child(aside))
        .child(div().text_size(px(14.5)).line_height(rems(1.35)).font_weight(FontWeight::SEMIBOLD).text_color(theme.foreground).child(item.summary.clone()));
    for (part, paragraph) in item.detail.split("\n\n").map(str::trim).filter(|paragraph| !paragraph.is_empty()).enumerate() {
        let id = format!("note-card-{index}-{part}");
        column = column.child(match section(paragraph) {
            Section::Labelled(name, text) => v_flex().gap(px(2.))
                .child(div().text_size(px(11.)).font_weight(FontWeight::MEDIUM).text_color(if name == "Fix" { theme.success } else { theme.warning }).child(name))
                .child(prose(id, text, theme.foreground.opacity(0.88))).into_any_element(),
            Section::Question(text) => h_flex().items_start().gap(px(10.))
                .child(div().w(px(2.)).h_full().min_h(px(16.)).rounded_full().bg(theme.magenta))
                .child(div().flex_1().min_w_0().italic().child(prose(id, text, theme.foreground.opacity(0.88)))).into_any_element(),
            Section::Plain(text) => prose(id, text, theme.muted_foreground).into_any_element(),
        });
    }
    column
}

/// A tinted kind label with a dot, shared by note cards and chat tags.
pub(crate) fn chip(label: impl Into<SharedString>, color: Hsla) -> Div {
    h_flex().flex_none().gap(px(6.)).px(px(8.)).py(px(2.)).rounded_full().bg(color.opacity(0.14))
        .text_size(px(11.5)).font_weight(FontWeight::MEDIUM).text_color(color)
        .child(div().size(px(6.)).rounded_full().bg(color)).child(label.into())
}

enum Section<'a> { Labelled(&'static str, &'a str), Question(&'a str), Plain(&'a str) }

/// The detail paragraphs notes write: `Fix: …`, `Next step: …`, and an `_italic_` question.
fn section(paragraph: &str) -> Section<'_> {
    for name in ["Fix", "Next step"] {
        if let Some(text) = paragraph.strip_prefix(name).and_then(|rest| rest.strip_prefix(": ")) { return Section::Labelled(name, text); }
    }
    match paragraph.strip_prefix('_').and_then(|rest| rest.strip_suffix('_')) {
        Some(text) if !text.contains('\n') => Section::Question(text),
        _ => Section::Plain(paragraph),
    }
}

fn prose(id: String, text: &str, color: Hsla) -> impl IntoElement + use<> {
    div().text_size(px(12.5)).line_height(rems(1.45)).text_color(color).child(crate::rich_text::markdown(SharedString::from(id), text))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[::core::prelude::v1::test]
    fn detail_paragraphs_become_labelled_sections() {
        assert!(matches!(section("Fix: use `mid + 1`"), Section::Labelled("Fix", "use `mid + 1`")));
        assert!(matches!(section("Next step: try a smaller case"), Section::Labelled("Next step", _)));
        assert!(matches!(section("_What does lo hold?_"), Section::Question("What does lo hold?")));
        assert!(matches!(section("The loop stops early."), Section::Plain(_)));
    }
}
