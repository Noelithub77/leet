mod code_action_menu;
mod completion_menu;
mod diagnostic_popover;
mod hover_popover;

pub(crate) use code_action_menu::*;
pub(crate) use completion_menu::*;
pub(crate) use diagnostic_popover::*;
pub(crate) use hover_popover::*;

use std::rc::Rc;

use gpui::{
    App, BoxShadow, Div, ElementId, Global, InteractiveElement as _, Pixels, SharedString,
    Stateful, StyleRefinement, Styled, Window, div, hsla, px, rems,
};

use crate::{
    ActiveTheme,
    text::{TextView, TextViewStyle},
};

/// Extends the Markdown of editor hovers, diagnostics and completion docs, such
/// as with an app's own block renderers.
#[derive(Clone)]
pub struct PopoverMarkdown(pub Rc<dyn Fn(TextView) -> TextView>);

impl Global for PopoverMarkdown {}

pub(super) fn render_markdown(
    id: impl Into<ElementId>,
    markdown: impl Into<SharedString>,
    _: &mut Window,
    cx: &mut App,
) -> TextView {
    let view = TextView::markdown(id, markdown)
        .style(
            TextViewStyle::default()
                .paragraph_gap(rems(0.6))
                .heading_font_size(|level, rem_size| match level {
                    1..=3 => rem_size * 1,
                    4 => rem_size * 0.9,
                    _ => rem_size * 0.8,
                })
                .code_block(
                    StyleRefinement::default()
                        .bg(cx.theme().transparent)
                        .p_0()
                        .text_size(px(12.)),
                ),
        )
        .selectable(true);
    match cx.try_global::<PopoverMarkdown>() {
        Some(extend) => (extend.0.clone())(view),
        None => view,
    }
}

/// The rounded, translucent card editor popovers sit on, lifted by a soft
/// shadow and edged by a faint hairline.
pub(super) fn popover_card<E: Styled>(element: E, radius: Pixels, cx: &App) -> E {
    let theme = cx.theme();
    // A theme with square corners keeps them.
    let radius = if theme.radius > px(0.) { radius } else { px(0.) };
    element
        .bg(theme.popover.opacity(0.94))
        .text_color(theme.popover_foreground)
        .border_1()
        .border_color(theme.foreground.opacity(0.08))
        .rounded(radius)
        .shadow(vec![
            BoxShadow::new(px(0.), px(12.), hsla(0., 0., 0., 0.42))
                .blur_radius(px(14.))
                .spread_radius(px(-6.)),
            BoxShadow::new(px(0.), px(2.), hsla(0., 0., 0., 0.3))
                .blur_radius(px(3.))
                .spread_radius(px(-1.)),
        ])
}

pub(super) fn editor_popover(id: impl Into<ElementId>, cx: &App) -> Stateful<Div> {
    popover_card(div().id(id).flex_none().occlude(), px(10.), cx)
        .text_xs()
        .p_1()
}
