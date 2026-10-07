use gpui_kit::component::{Theme, h_flex};
use gpui_kit::prelude::FluentBuilder as _;
use practice::viz::Cell;
use super::style::{GAP, cell_w, chip};
use gpui_kit::*;

pub(super) fn render(items: &[Cell], theme: &Theme) -> AnyElement {
    h_flex().flex_wrap().gap(px(GAP)).max_w(px(560.))
        .when(items.is_empty(), |el| el.child(div().text_xs().text_color(theme.muted_foreground).child("∅")))
        .children(items.iter().map(|c| chip(&c.value, c.tone, cell_w(&c.value), theme).rounded_full()))
        .into_any_element()
}
