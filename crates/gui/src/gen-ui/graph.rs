use std::collections::HashMap;
use gpui_kit::base::spring;
use gpui_kit::component::ActiveTheme as _;
use practice::viz::{Edge, GraphNode, Tone};
use super::drawing::{arrow_head, line};
use super::style::{NODE, motion, tone_colors};
use gpui_kit::*;

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

pub(super) fn render(id: &str, nodes: &[GraphNode], edges: &[Edge], directed: bool, window: &mut Window, cx: &mut App) -> AnyElement {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[::core::prelude::v1::test]
    fn large_graphs_layer_by_breadth_first_distance() {
        let nodes: Vec<GraphNode> = (0..12).map(|i| GraphNode { id: i.to_string(), label: i.to_string(), tone: Tone::Default }).collect();
        let edges: Vec<Edge> = (1..12).map(|i| Edge { from: "0".into(), to: i.to_string(), label: String::new(), tone: Tone::Default }).collect();
        let (pos, _, _) = graph_layout(&nodes, &edges);
        assert_eq!(pos[0].1, 0.);
        assert!(pos[1..].iter().all(|p| p.1 > 0.));
    }
}
