use std::collections::HashMap;
use gpui_kit::base::spring;
use gpui_kit::component::ActiveTheme as _;
use practice::viz::TreeNode;
use super::drawing::line;
use super::style::{NODE, motion, tone_colors};
use gpui_kit::*;

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

pub(super) fn render(id: &str, root: Option<&str>, nodes: &[TreeNode], window: &mut Window, cx: &mut App) -> AnyElement {
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

#[cfg(test)]
mod tests {
    use super::*;
    use practice::viz::Tone;

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
}
