use gpui_kit::component::{Theme, h_flex};
use gpui_kit::prelude::FluentBuilder as _;
use practice::viz::Cell;
use super::style::{GAP, cell_w, chip};
use gpui_kit::*;

pub(super) fn render(items: &[Cell], theme: &Theme) -> AnyElement {
    h_flex().gap(px(GAP)).items_center()
        .child(div().text_size(px(10.)).text_color(theme.info).child("front →"))
        .when(items.is_empty(), |el| el.child(div().text_xs().text_color(theme.muted_foreground).child("empty")))
        .children(items.iter().map(|c| chip(&c.value, c.tone, cell_w(&c.value), theme)))
        .child(div().text_size(px(10.)).text_color(theme.muted_foreground).child("← back"))
        .into_any_element()
}
