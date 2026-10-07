use gpui_kit::component::{Theme, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use practice::viz::{Entry, Tone};
use super::style::{GAP, cell_w, chip};
use gpui_kit::*;

pub(super) fn render(entries: &[Entry], theme: &Theme) -> AnyElement {
    let key_w = entries.iter().map(|e| cell_w(&e.key)).fold(30f32, f32::max);
    v_flex().gap(px(GAP))
        .when(entries.is_empty(), |el| el.child(div().text_xs().text_color(theme.muted_foreground).child("{ }")))
        .children(entries.iter().map(|e| {
            h_flex().gap_1p5().items_center()
                .child(chip(&e.key, if e.tone == Tone::Default { Tone::Default } else { e.tone }, key_w, theme))
                .child(div().text_xs().text_color(theme.muted_foreground).child("→"))
                .child(chip(&e.value, e.tone, cell_w(&e.value), theme))
        }))
        .into_any_element()
}
