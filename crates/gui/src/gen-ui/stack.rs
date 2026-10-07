use gpui_kit::component::{Theme, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use practice::viz::Cell;
use super::style::{GAP, cell_w, chip};
use gpui_kit::*;

pub(super) fn render(items: &[Cell], theme: &Theme) -> AnyElement {
    let w = items.iter().map(|c| cell_w(&c.value)).fold(56f32, f32::max);
    v_flex().gap(px(GAP)).items_start()
        .when(items.is_empty(), |el| el.child(div().text_xs().text_color(theme.muted_foreground).child("empty")))
        .children(items.iter().enumerate().rev().map(|(i, c)| {
            h_flex().gap_2().child(chip(&c.value, c.tone, w, theme))
                .when(i + 1 == items.len(), |el| el.child(div().text_size(px(10.)).text_color(theme.info).child("← top")))
        }))
        .into_any_element()
}
