use gpui_kit::component::{Theme, h_flex, v_flex};
use practice::viz::{Interval, Tone};
use super::style::tone_colors;
use gpui_kit::*;

pub(super) fn render(items: &[Interval], theme: &Theme) -> AnyElement {
    let lo = items.iter().map(|i| i.start.min(i.end)).fold(f64::INFINITY, f64::min);
    let hi = items.iter().map(|i| i.start.max(i.end)).fold(f64::NEG_INFINITY, f64::max);
    let span = (hi - lo).max(1.);
    let width = 320.;
    v_flex().gap_1().w(px(width + 70.))
        .children(items.iter().map(|i| {
            let left = ((i.start.min(i.end) - lo) / span) as f32 * width;
            let w = (((i.end - i.start).abs() / span) as f32 * width).max(4.);
            let (bg, border, fg) = tone_colors(if i.tone == Tone::Default { Tone::Active } else { i.tone }, theme);
            h_flex().h(px(18.)).items_center()
                .child(div().relative().w(px(width)).h_full()
                    .child(div().absolute().left(px(left)).top(px(4.)).w(px(w)).h(px(10.)).rounded_full().bg(bg).border_1().border_color(border)))
                .child(div().pl_2().text_size(px(10.)).text_color(fg).font_family(theme.mono_font_family.clone())
                    .child(if i.label.is_empty() { format!("[{}, {}]", trim(i.start), trim(i.end)) } else { i.label.clone() }))
        }))
        .into_any_element()
}

fn trim(v: f64) -> String { if v.fract() == 0. { format!("{v:.0}") } else { format!("{v}") } }
