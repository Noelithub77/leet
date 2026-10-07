use std::collections::HashMap;
use gpui_kit::base::spring;
use gpui_kit::component::{ActiveTheme as _, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use practice::viz::{Cell, Pointer, Span, Tone};
use super::style::{CELL_H, GAP, cell_w, chip, motion, tone_colors};
use gpui_kit::*;

pub(super) fn render(id: &str, items: &[Cell], pointers: &[Pointer], spans: &[Span], window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    let widths: Vec<f32> = items.iter().map(|c| cell_w(&c.value)).collect();
    let mut offsets = Vec::with_capacity(widths.len());
    let mut x = 0.;
    for w in &widths { offsets.push(x); x += w + GAP; }
    let total = (x - GAP).max(0.);
    let center = |i: usize| offsets.get(i).map_or(total, |o| o + widths[i] / 2.);
    let active: Vec<usize> = pointers.iter().map(|p| p.index).collect();
    // Pointers that share a cell stack downward instead of overlapping.
    let mut rows: HashMap<usize, usize> = HashMap::new();
    let pointer_rows: Vec<usize> = pointers.iter().map(|p| { let row = rows.entry(p.index).or_default(); *row += 1; *row - 1 }).collect();
    let depth = pointer_rows.iter().max().map_or(0., |r| (*r + 1) as f32 * 16. + 6.);
    let span_h = if spans.is_empty() { 0. } else { 10. };
    let mut el = div().relative().w(px(total)).h(px(span_h + CELL_H + 14. + depth))
        .child(div().absolute().top(px(span_h)).flex().gap(px(GAP)).children(items.iter().enumerate().map(|(i, c)| {
            let tone = if c.tone == Tone::Default && active.contains(&i) { Tone::Active } else { c.tone };
            chip(&c.value, tone, widths[i], &theme)
        })))
        .child(div().absolute().top(px(span_h + CELL_H + 1.)).flex().gap(px(GAP)).children(widths.iter().enumerate().map(|(i, w)| {
            div().w(px(*w)).text_center().text_size(px(9.)).text_color(theme.muted_foreground.opacity(0.7)).child(i.to_string())
        })));
    for (n, span) in spans.iter().enumerate() {
        let (from, to) = (span.from.min(span.to), span.from.max(span.to).min(items.len().saturating_sub(1)));
        let left = offsets.get(from).copied().unwrap_or(0.);
        let right = offsets.get(to).map_or(total, |o| o + widths[to]);
        let l = spring(SharedString::from(format!("{id}-span-{n}-l")), left, motion(), window, cx);
        let r = spring(SharedString::from(format!("{id}-span-{n}-r")), right, motion(), window, cx);
        let color = if span.tone == Tone::Default { theme.info } else { tone_colors(span.tone, &theme).2 };
        el = el.child(div().absolute().left(px(l - 2.)).top(px(span_h - 4.)).w(px((r - l + 4.).max(0.))).h(px(CELL_H + 8.)).rounded_lg()
            .border_1().border_color(color.opacity(0.6)).bg(color.opacity(0.06))
            .when(!span.label.is_empty(), |el| el.child(div().absolute().top(px(-12.)).left(px(4.)).text_size(px(9.)).text_color(color).child(span.label.clone()))));
    }
    for (n, pointer) in pointers.iter().enumerate() {
        let target = center(pointer.index) - 18.;
        let x = spring(SharedString::from(format!("{id}-ptr-{}", pointer.name)), target, motion(), window, cx);
        let color = if pointer.tone == Tone::Default { theme.info } else { tone_colors(pointer.tone, &theme).2 };
        el = el.child(v_flex().absolute().left(px(x)).top(px(span_h + CELL_H + 13. + pointer_rows[n] as f32 * 16.)).w(px(36.)).items_center()
            .text_size(px(10.)).font_weight(FontWeight::SEMIBOLD).text_color(color)
            .child(format!("▲ {}", pointer.name)));
    }
    el.into_any_element()
}
