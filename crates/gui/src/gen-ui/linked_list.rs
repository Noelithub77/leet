use std::collections::HashMap;
use gpui_kit::base::spring;
use gpui_kit::component::{ActiveTheme as _, h_flex};
use practice::viz::{Cell, Pointer, Tone};
use super::drawing::arrow_head;
use super::style::{CELL_H, cell_w, chip, motion};
use gpui_kit::*;

pub(super) fn render(id: &str, nodes: &[Cell], pointers: &[Pointer], cycle_to: Option<usize>, window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    let link = 18.;
    let widths: Vec<f32> = nodes.iter().map(|c| cell_w(&c.value).max(34.)).collect();
    let mut offsets = Vec::new();
    let mut x = 0.;
    for w in &widths { offsets.push(x); x += w + link; }
    let total = (x - link).max(0.) + 26.;
    let active: Vec<usize> = pointers.iter().map(|p| p.index).collect();
    let cycle = cycle_to.filter(|i| *i < nodes.len()).map(|i| (offsets[i] + widths[i] / 2., offsets.last().copied().unwrap_or(0.) + widths.last().copied().unwrap_or(0.) / 2.));
    let top = if cycle.is_some() { 16. } else { 0. };
    let mut rows: HashMap<usize, usize> = HashMap::new();
    let depth = pointers.iter().map(|p| { let r = rows.entry(p.index).or_default(); *r += 1; *r }).max().unwrap_or(0) as f32 * 16.;
    let mut el = div().relative().w(px(total)).h(px(top + CELL_H + 6. + depth))
        .child(h_flex().absolute().top(px(top)).children(nodes.iter().enumerate().map(|(i, c)| {
            let tone = if c.tone == Tone::Default && active.contains(&i) { Tone::Active } else { c.tone };
            h_flex().child(chip(&c.value, tone, widths[i], &theme))
                .child(div().w(px(link)).text_center().text_color(theme.muted_foreground).child("→"))
        })).child(div().text_xs().text_color(theme.muted_foreground).child(if cycle.is_some() { "" } else { "∅" })));
    if let Some((to, from)) = cycle {
        let color = theme.warning;
        el = el.child(canvas(|_, _, _| (), move |bounds, _, window, _| {
            let o = bounds.origin;
            let mut path = PathBuilder::stroke(px(1.4));
            path.move_to(point(o.x + px(from), o.y + px(top)));
            path.cubic_bezier_to(point(o.x + px(to), o.y + px(top)), point(o.x + px(from), o.y), point(o.x + px(to), o.y));
            if let Ok(path) = path.build() { window.paint_path(path, color); }
            arrow_head(window, o, (to + 0.1, top - 6.), (to, top), color);
        }).absolute().size_full());
    }
    let mut rows: HashMap<usize, usize> = HashMap::new();
    for pointer in pointers {
        let row = { let r = rows.entry(pointer.index).or_default(); *r += 1; *r - 1 };
        let target = offsets.get(pointer.index).map_or(total - 20., |o| o + widths[pointer.index] / 2.) - 20.;
        let x = spring(SharedString::from(format!("{id}-lptr-{}", pointer.name)), target, motion(), window, cx);
        el = el.child(div().absolute().left(px(x)).top(px(top + CELL_H + 3. + row as f32 * 16.)).w(px(40.)).text_center()
            .text_size(px(10.)).font_weight(FontWeight::SEMIBOLD).text_color(theme.info).child(format!("▲ {}", pointer.name)));
    }
    el.into_any_element()
}
