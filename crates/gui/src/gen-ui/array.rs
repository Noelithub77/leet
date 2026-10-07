use std::collections::HashMap;
use std::time::Duration;
use gpui_kit::base::{Spring, spring};
use gpui_kit::component::{ActiveTheme as _, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use practice::viz::{Cell, Pointer, Span, Tone};
use super::style::{cell_w, chip, tone_colors};
use gpui_kit::*;

const CELL_H: f32 = 42.;
const GAP: f32 = 6.;
const POINTER_W: f32 = 64.;

fn motion() -> Spring { Spring::new(Duration::from_millis(480)) }

pub(super) fn render(id: &str, items: &[Cell], pointers: &[Pointer], spans: &[Span], window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    let widths: Vec<f32> = items.iter().map(|c| (cell_w(&c.value) + 12.).max(48.)).collect();
    let mut offsets = Vec::with_capacity(widths.len());
    let mut x = 12.;
    for w in &widths { offsets.push(x); x += w + GAP; }
    let total = (x - GAP).max(12.) + 12.;
    let center = |i: usize| offsets.get(i).map_or(total, |o| o + widths[i] / 2.);
    let active: Vec<usize> = pointers.iter().map(|p| p.index).collect();
    // Pointers that share a cell stack downward instead of overlapping.
    let mut rows: HashMap<usize, usize> = HashMap::new();
    let pointer_rows: Vec<usize> = pointers.iter().map(|p| { let row = rows.entry(p.index).or_default(); *row += 1; *row - 1 }).collect();
    let depth = pointer_rows.iter().max().map_or(0., |r| 70. + *r as f32 * 26.);
    let span_h = if spans.iter().any(|span| !span.label.is_empty()) { 24. } else if spans.is_empty() { 0. } else { 10. };
    let mut el = div().relative().w(px(total)).h(px(span_h + CELL_H + 14. + depth))
        .child(div().absolute().left(px(12.)).top(px(span_h)).flex().gap(px(GAP)).children(items.iter().enumerate().map(|(i, c)| {
            let tone = if c.tone == Tone::Default && active.contains(&i) { Tone::Active } else { c.tone };
            chip(&c.value, tone, widths[i], &theme).h(px(CELL_H)).text_size(px(14.))
        })))
        .child(div().absolute().left(px(12.)).top(px(span_h + CELL_H + 2.)).flex().gap(px(GAP)).children(widths.iter().enumerate().map(|(i, w)| {
            div().w(px(*w)).text_center().text_size(px(11.)).text_color(theme.muted_foreground.opacity(0.8)).child(i.to_string())
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
            .when(!span.label.is_empty(), |el| el.child(div().absolute().top(px(-18.)).left(px(4.)).h(px(16.)).line_height(px(16.)).text_size(px(11.)).text_color(color).child(span.label.clone()))));
    }
    // Paint longer stems first so shorter pointer labels remain readable.
    for (n, pointer) in pointers.iter().enumerate().rev() {
        let target = center(pointer.index) - POINTER_W / 2.;
        let x = spring(SharedString::from(format!("{id}-ptr-{}", pointer.name)), target, motion(), window, cx);
        let color = if pointer.tone == Tone::Default { theme.info } else { tone_colors(pointer.tone, &theme).2 };
        let tail = spring(SharedString::from(format!("{id}-ptr-{}-tail", pointer.name)), 24. + pointer_rows[n] as f32 * 26., motion(), window, cx);
        el = el.child(v_flex().absolute().left(px(x)).top(px(span_h + CELL_H + 17.)).w(px(POINTER_W)).h(px(tail + 30.)).items_center()
            .text_size(px(12.)).font_weight(FontWeight::SEMIBOLD).text_color(color)
            .child(div().h(px(10.)).flex_shrink_0().text_size(px(14.)).line_height(px(10.)).child("▲"))
            .child(div().w(px(2.)).h(px(tail)).flex_shrink_0().bg(color).rounded_full())
            .child(div().h(px(20.)).flex_shrink_0().px_1().rounded_sm().bg(theme.background).child(pointer.name.clone())));
    }
    el.into_any_element()
}
