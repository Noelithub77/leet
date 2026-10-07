//! Native renderers for `practice::viz` structures, shared by AI scenes and the debugger.
//! Pointers and nodes spring between positions so stepping reads as motion.

pub mod player;

use std::collections::HashMap;
use std::time::Duration;

use gpui_kit::base::{Spring, spring};
use gpui_kit::component::{ActiveTheme as _, Theme, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use practice::viz::{Cell, Edge, Entry, Frame, GraphNode, Interval, Pointer, Span, Structure, Tone, TreeNode, Var};

const CELL_H: f32 = 30.;
const GAP: f32 = 4.;
const NODE: f32 = 30.;

fn motion() -> Spring { Spring::new(Duration::from_millis(360)) }

/// Fill, border, and text colors for a tone.
pub fn tone_colors(tone: Tone, theme: &Theme) -> (Hsla, Hsla, Hsla) {
    let tint = |color: Hsla, alpha: f32| (color.opacity(alpha), color.opacity(0.75), color);
    match tone {
        Tone::Default => (theme.secondary, theme.border, theme.foreground),
        Tone::Active => tint(theme.info, 0.2),
        Tone::Changed => tint(theme.warning, 0.2),
        Tone::Visited => (theme.muted, theme.muted_foreground.opacity(0.35), theme.muted_foreground),
        Tone::Done => tint(theme.success, 0.18),
        Tone::Muted => (theme.secondary.opacity(0.4), theme.border.opacity(0.4), theme.muted_foreground.opacity(0.6)),
        Tone::Warn => tint(theme.warning, 0.28),
        Tone::Error => tint(theme.danger, 0.22),
    }
}

/// Width that fits a value in the mono font.
fn cell_w(value: &str) -> f32 { (value.chars().count() as f32 * 7.6 + 14.).clamp(30., 160.) }

fn chip(value: &str, tone: Tone, width: f32, theme: &Theme) -> Div {
    let (bg, border, fg) = tone_colors(tone, theme);
    div().flex_shrink_0().w(px(width)).h(px(CELL_H)).flex().items_center().justify_center().rounded_md()
        .bg(bg).border_1().border_color(border).text_color(fg)
        .font_family(theme.mono_font_family.clone()).text_xs().truncate().child(value.to_owned())
}

fn caption(label: &str, kind: &str, theme: &Theme) -> Div {
    h_flex().gap_1p5().text_xs()
        .child(div().text_color(theme.foreground).font_weight(FontWeight::MEDIUM).child(label.to_owned()))
        .child(div().text_color(theme.muted_foreground).child(kind.to_owned()))
}

/// A walkthrough frame: its caption, then each structure.
pub fn frame(scope: &str, frame: &Frame, window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    v_flex().gap_3().w_full()
        .when(!frame.caption.is_empty(), |el| el.child(div().text_sm().text_color(theme.foreground).child(frame.caption.clone())))
        .children(frame.structures.iter().map(|s| structure(scope, s, window, cx)))
        .into_any_element()
}

pub fn structure(scope: &str, structure: &Structure, window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    let id = format!("{scope}-{}", structure.label());
    let (kind, body) = match structure {
        Structure::Array { items, pointers, spans, .. } => ("array", array(&id, items, pointers, spans, window, cx)),
        Structure::Grid { rows, row_labels, col_labels, pointers, .. } => ("grid", grid(rows, row_labels, col_labels, pointers, &theme)),
        Structure::Tree { root, nodes, .. } => ("tree", tree(&id, root.as_deref(), nodes, window, cx)),
        Structure::Heap { items, min, .. } => (if *min { "min-heap" } else { "max-heap" }, tree(&id, Some("0"), &heap_nodes(items), window, cx)),
        Structure::Graph { nodes, edges, directed, .. } => ("graph", graph(&id, nodes, edges, *directed, window, cx)),
        Structure::LinkedList { nodes, pointers, cycle_to, .. } => ("linked list", linked(&id, nodes, pointers, *cycle_to, window, cx)),
        Structure::Stack { items, .. } => ("stack", stack(items, &theme)),
        Structure::Queue { items, .. } => ("queue", queue(items, &theme)),
        Structure::Map { entries, .. } => ("map", map(entries, &theme)),
        Structure::Set { items, .. } => ("set", set(items, &theme)),
        Structure::Intervals { items, .. } => ("intervals", intervals(items, &theme)),
        Structure::Vars { vars, .. } => ("", self::vars(vars, &theme)),
    };
    v_flex().gap_1p5().min_w_0()
        .when(!kind.is_empty(), |el| el.child(caption(structure.label(), kind, &theme)))
        .child(div().id(SharedString::from(format!("{id}-scroll"))).overflow_x_scroll().pb_1().child(body))
        .into_any_element()
}

fn array(id: &str, items: &[Cell], pointers: &[Pointer], spans: &[Span], window: &mut Window, cx: &mut App) -> AnyElement {
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

fn grid(rows: &[Vec<Cell>], row_labels: &[String], col_labels: &[String], pointers: &[practice::viz::GridPointer], theme: &Theme) -> AnyElement {
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

/// Array-backed heap as tree nodes with index ids.
fn heap_nodes(items: &[Cell]) -> Vec<TreeNode> {
    let child = |i: usize| (i < items.len()).then(|| i.to_string());
    items.iter().enumerate().map(|(i, c)| TreeNode { id: i.to_string(), value: c.value.clone(), left: child(2 * i + 1), right: child(2 * i + 2), tone: c.tone }).collect()
}

/// In-order x and depth y, so subtrees never overlap.
fn tree_layout(root: Option<&str>, nodes: &[TreeNode]) -> HashMap<String, (f32, f32)> {
    let by_id: HashMap<&str, &TreeNode> = nodes.iter().map(|n| (n.id.as_str(), n)).collect();
    let mut out = HashMap::new();
    let mut seen = std::collections::HashSet::new();
    let mut next = 0.;
    fn walk<'a>(id: &'a str, depth: usize, by_id: &HashMap<&str, &'a TreeNode>, seen: &mut std::collections::HashSet<&'a str>, out: &mut HashMap<String, (f32, f32)>, next: &mut f32) {
        if depth > 64 || !seen.insert(id) { return; }
        let Some(node) = by_id.get(id) else { return };
        if let Some(left) = &node.left { walk(left, depth + 1, by_id, seen, out, next); }
        out.insert(id.to_owned(), (*next, depth as f32));
        *next += 1.;
        if let Some(right) = &node.right { walk(right, depth + 1, by_id, seen, out, next); }
    }
    if let Some(root) = root { walk(root, 0, &by_id, &mut seen, &mut out, &mut next); }
    out
}

fn tree(id: &str, root: Option<&str>, nodes: &[TreeNode], window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    if root.is_none() || nodes.is_empty() { return div().text_xs().text_color(theme.muted_foreground).child("empty").into_any_element(); }
    let layout = tree_layout(root, nodes);
    let (sx, sy) = (NODE + 8., NODE + 18.);
    let width = layout.values().map(|p| p.0).fold(0., f32::max) * sx + NODE;
    let height = layout.values().map(|p| p.1).fold(0., f32::max) * sy + NODE;
    let mut placed = HashMap::new();
    let mut el = div().relative().w(px(width)).h(px(height));
    let mut children = Vec::new();
    for node in nodes {
        let Some(&(x, y)) = layout.get(&node.id) else { continue };
        let x = spring(SharedString::from(format!("{id}-node-{}-x", node.id)), x * sx, motion(), window, cx);
        let y = spring(SharedString::from(format!("{id}-node-{}-y", node.id)), y * sy, motion(), window, cx);
        placed.insert(node.id.clone(), (x, y));
        let (bg, border, fg) = tone_colors(node.tone, &theme);
        children.push(div().absolute().left(px(x)).top(px(y)).size(px(NODE)).rounded_full().flex().items_center().justify_center()
            .bg(bg).border_1().border_color(border).text_color(fg).font_family(theme.mono_font_family.clone()).text_size(px(11.))
            .child(node.value.clone()));
    }
    let placed = &placed;
    let edges: Vec<((f32, f32), (f32, f32))> = nodes.iter().flat_map(|n| {
        let from = placed.get(&n.id).copied();
        [&n.left, &n.right].into_iter().flatten().filter_map(move |c| Some((from?, *placed.get(c)?))).collect::<Vec<_>>()
    }).collect();
    let color = theme.muted_foreground.opacity(0.45);
    el = el.child(canvas(|_, _, _| (), move |bounds, _, window, _| {
        for ((x1, y1), (x2, y2)) in &edges {
            let half = NODE / 2.;
            line(window, bounds.origin, (x1 + half, y1 + NODE), (x2 + half, *y2), color);
        }
    }).absolute().size_full());
    el.children(children).into_any_element()
}

fn line(window: &mut Window, origin: Point<Pixels>, from: (f32, f32), to: (f32, f32), color: Hsla) {
    let mut path = PathBuilder::stroke(px(1.4));
    path.move_to(point(origin.x + px(from.0), origin.y + px(from.1)));
    path.line_to(point(origin.x + px(to.0), origin.y + px(to.1)));
    if let Ok(path) = path.build() { window.paint_path(path, color); }
}

fn arrow_head(window: &mut Window, origin: Point<Pixels>, from: (f32, f32), to: (f32, f32), color: Hsla) {
    let (dx, dy) = (to.0 - from.0, to.1 - from.1);
    let len = (dx * dx + dy * dy).sqrt().max(0.001);
    let (ux, uy) = (dx / len, dy / len);
    let base = (to.0 - ux * 7., to.1 - uy * 7.);
    let (px_, py_) = (-uy * 3.5, ux * 3.5);
    let mut path = PathBuilder::fill();
    path.move_to(point(origin.x + px(to.0), origin.y + px(to.1)));
    path.line_to(point(origin.x + px(base.0 + px_), origin.y + px(base.1 + py_)));
    path.line_to(point(origin.x + px(base.0 - px_), origin.y + px(base.1 - py_)));
    path.close();
    if let Ok(path) = path.build() { window.paint_path(path, color); }
}

/// Small graphs sit on a circle; larger ones in BFS layers from the first node.
fn graph_layout(nodes: &[GraphNode], edges: &[Edge]) -> (Vec<(f32, f32)>, f32, f32) {
    let n = nodes.len();
    if n <= 10 {
        let r = (n as f32 * 14.).max(40.);
        let pos = (0..n).map(|i| {
            let a = std::f32::consts::TAU * i as f32 / n.max(1) as f32 - std::f32::consts::FRAC_PI_2;
            (r + r * a.cos(), r + r * a.sin())
        }).collect();
        return (pos, 2. * r + NODE, 2. * r + NODE);
    }
    let index: HashMap<&str, usize> = nodes.iter().enumerate().map(|(i, node)| (node.id.as_str(), i)).collect();
    let mut layer = vec![usize::MAX; n];
    let mut order = Vec::new();
    for start in 0..n {
        if layer[start] != usize::MAX { continue; }
        layer[start] = 0;
        let mut queue = std::collections::VecDeque::from([start]);
        while let Some(u) = queue.pop_front() {
            order.push(u);
            for e in edges.iter().filter(|e| index.get(e.from.as_str()) == Some(&u) || index.get(e.to.as_str()) == Some(&u)) {
                let other = if index.get(e.from.as_str()) == Some(&u) { &e.to } else { &e.from };
                if let Some(&v) = index.get(other.as_str()) && layer[v] == usize::MAX { layer[v] = layer[u] + 1; queue.push_back(v); }
            }
        }
    }
    let mut slot: HashMap<usize, usize> = HashMap::new();
    let mut pos = vec![(0., 0.); n];
    for u in order { let s = slot.entry(layer[u]).or_default(); pos[u] = (*s as f32 * (NODE + 18.), layer[u] as f32 * (NODE + 26.)); *s += 1; }
    let w = pos.iter().map(|p| p.0).fold(0., f32::max) + NODE;
    let h = pos.iter().map(|p| p.1).fold(0., f32::max) + NODE;
    (pos, w, h)
}

fn graph(id: &str, nodes: &[GraphNode], edges: &[Edge], directed: bool, window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    let (pos, w, h) = graph_layout(nodes, edges);
    let mut placed = HashMap::new();
    let mut children = Vec::new();
    for (node, (x, y)) in nodes.iter().zip(pos) {
        let x = spring(SharedString::from(format!("{id}-g-{}-x", node.id)), x, motion(), window, cx);
        let y = spring(SharedString::from(format!("{id}-g-{}-y", node.id)), y, motion(), window, cx);
        placed.insert(node.id.clone(), (x, y));
        let (bg, border, fg) = tone_colors(node.tone, &theme);
        children.push(div().absolute().left(px(x)).top(px(y)).size(px(NODE)).rounded_full().flex().items_center().justify_center()
            .bg(bg).border_1().border_color(border).text_color(fg).font_family(theme.mono_font_family.clone()).text_size(px(11.)).child(node.label.clone()));
    }
    let lines: Vec<_> = edges.iter().filter_map(|e| {
        let (a, b) = (placed.get(&e.from)?, placed.get(&e.to)?);
        let color = if e.tone == Tone::Default { theme.muted_foreground.opacity(0.45) } else { tone_colors(e.tone, &theme).2 };
        Some(((a.0 + NODE / 2., a.1 + NODE / 2.), (b.0 + NODE / 2., b.1 + NODE / 2.), color))
    }).collect();
    div().relative().w(px(w)).h(px(h))
        .child(canvas(|_, _, _| (), move |bounds, _, window, _| {
            for (from, to, color) in &lines {
                let (dx, dy) = (to.0 - from.0, to.1 - from.1);
                let len = (dx * dx + dy * dy).sqrt().max(0.001);
                let r = NODE / 2.;
                let start = (from.0 + dx / len * r, from.1 + dy / len * r);
                let end = (to.0 - dx / len * r, to.1 - dy / len * r);
                line(window, bounds.origin, start, end, *color);
                if directed { arrow_head(window, bounds.origin, start, end, *color); }
            }
        }).absolute().size_full())
        .children(children)
        .into_any_element()
}

fn linked(id: &str, nodes: &[Cell], pointers: &[Pointer], cycle_to: Option<usize>, window: &mut Window, cx: &mut App) -> AnyElement {
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

fn stack(items: &[Cell], theme: &Theme) -> AnyElement {
    let w = items.iter().map(|c| cell_w(&c.value)).fold(56f32, f32::max);
    v_flex().gap(px(GAP)).items_start()
        .when(items.is_empty(), |el| el.child(div().text_xs().text_color(theme.muted_foreground).child("empty")))
        .children(items.iter().enumerate().rev().map(|(i, c)| {
            h_flex().gap_2().child(chip(&c.value, c.tone, w, theme))
                .when(i + 1 == items.len(), |el| el.child(div().text_size(px(10.)).text_color(theme.info).child("← top")))
        }))
        .into_any_element()
}

fn queue(items: &[Cell], theme: &Theme) -> AnyElement {
    h_flex().gap(px(GAP)).items_center()
        .child(div().text_size(px(10.)).text_color(theme.info).child("front →"))
        .when(items.is_empty(), |el| el.child(div().text_xs().text_color(theme.muted_foreground).child("empty")))
        .children(items.iter().map(|c| chip(&c.value, c.tone, cell_w(&c.value), theme)))
        .child(div().text_size(px(10.)).text_color(theme.muted_foreground).child("← back"))
        .into_any_element()
}

fn map(entries: &[Entry], theme: &Theme) -> AnyElement {
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

fn set(items: &[Cell], theme: &Theme) -> AnyElement {
    h_flex().flex_wrap().gap(px(GAP)).max_w(px(560.))
        .when(items.is_empty(), |el| el.child(div().text_xs().text_color(theme.muted_foreground).child("∅")))
        .children(items.iter().map(|c| chip(&c.value, c.tone, cell_w(&c.value), theme).rounded_full()))
        .into_any_element()
}

fn intervals(items: &[Interval], theme: &Theme) -> AnyElement {
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

fn vars(vars: &[Var], theme: &Theme) -> AnyElement {
    h_flex().flex_wrap().gap_1p5().max_w(px(640.))
        .children(vars.iter().map(|v| {
            let (bg, border, fg) = tone_colors(v.tone, theme);
            h_flex().h(px(24.)).px_2().gap_1p5().rounded_md().bg(bg).border_1().border_color(border).text_xs()
                .child(div().text_color(theme.muted_foreground).child(v.name.clone()))
                .child(div().text_color(fg).font_family(theme.mono_font_family.clone()).max_w(px(220.)).truncate().child(v.value.clone()))
        }))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(id: &str, left: Option<&str>, right: Option<&str>) -> TreeNode {
        TreeNode { id: id.into(), value: id.into(), left: left.map(Into::into), right: right.map(Into::into), tone: Tone::Default }
    }

    #[::core::prelude::v1::test]
    fn tree_layout_orders_in_order_and_survives_cycles() {
        let nodes = vec![node("a", Some("b"), Some("c")), node("b", None, Some("a")), node("c", None, None)];
        let layout = tree_layout(Some("a"), &nodes);
        assert!(layout["b"].0 < layout["a"].0 && layout["a"].0 < layout["c"].0);
        assert_eq!(layout["b"].1, 1.);
    }

    #[::core::prelude::v1::test]
    fn heap_children_follow_array_indices() {
        let items: Vec<Cell> = ["1", "3", "2", "7"].iter().map(|v| Cell { value: (*v).into(), tone: Tone::Default }).collect();
        let nodes = heap_nodes(&items);
        assert_eq!(nodes[0].left.as_deref(), Some("1"));
        assert_eq!(nodes[1].left.as_deref(), Some("3"));
        assert_eq!(nodes[1].right, None);
    }

    #[::core::prelude::v1::test]
    fn large_graphs_layer_by_breadth_first_distance() {
        let nodes: Vec<GraphNode> = (0..12).map(|i| GraphNode { id: i.to_string(), label: i.to_string(), tone: Tone::Default }).collect();
        let edges: Vec<Edge> = (1..12).map(|i| Edge { from: "0".into(), to: i.to_string(), label: String::new(), tone: Tone::Default }).collect();
        let (pos, _, _) = graph_layout(&nodes, &edges);
        assert_eq!(pos[0].1, 0.);
        assert!(pos[1..].iter().all(|p| p.1 > 0.));
    }
}
