use gpui_kit::component::{Theme, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use practice::viz::{Cell, Tone};
use super::style::{GAP, cell_w, chip};
use gpui_kit::*;

pub(super) fn render(rows: &[Vec<Cell>], row_labels: &[String], col_labels: &[String], pointers: &[practice::viz::GridPointer], theme: &Theme) -> AnyElement {
    let cols = rows.iter().map(Vec::len).max().unwrap_or(0);
    let widths: Vec<f32> = (0..cols).map(|c| rows.iter().filter_map(|r| r.get(c)).map(|cell| cell_w(&cell.value)).fold(30f32, f32::max)).collect();
    let label = |text: String, w: f32| div().w(px(w)).flex_shrink_0().text_center().text_size(px(9.)).text_color(theme.muted_foreground.opacity(0.75)).child(text);
    let header_w = 22.;
    v_flex().gap(px(GAP))
        .child(h_flex().gap(px(GAP)).child(div().w(px(header_w)))
            .children(widths.iter().enumerate().map(|(c, w)| label(col_labels.get(c).cloned().unwrap_or_else(|| c.to_string()), *w))))
        .children(rows.iter().enumerate().map(|(r, row)| {
            h_flex().gap(px(GAP)).child(label(row_labels.get(r).cloned().unwrap_or_else(|| r.to_string()), header_w))
                .children(row.iter().enumerate().map(|(c, cell)| {
                    let here: Vec<&str> = pointers.iter().filter(|p| p.row == r && p.col == c).map(|p| p.name.as_str()).collect();
                    let tone = if cell.tone == Tone::Default && !here.is_empty() { Tone::Active } else { cell.tone };
                    chip(&cell.value, tone, widths[c], theme).relative()
                        .when(!here.is_empty(), |el| el.child(div().absolute().top(px(-7.)).right(px(-4.)).px_1().rounded_sm().bg(theme.info)
                            .text_size(px(8.)).text_color(theme.background).child(here.join(","))))
                }))
        }))
        .into_any_element()
}
