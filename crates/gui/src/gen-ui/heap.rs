use practice::viz::{Cell, TreeNode};
use gpui_kit::*;

pub(super) fn render(id: &str, items: &[Cell], window: &mut Window, cx: &mut App) -> AnyElement {
    super::tree::render(id, Some("0"), &heap_nodes(items), window, cx)
}

/// Array-backed heap as tree nodes with index ids.
fn heap_nodes(items: &[Cell]) -> Vec<TreeNode> {
    let child = |i: usize| (i < items.len()).then(|| i.to_string());
    items.iter().enumerate().map(|(i, c)| TreeNode { id: i.to_string(), value: c.value.clone(), left: child(2 * i + 1), right: child(2 * i + 2), tone: c.tone }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use practice::viz::Tone;

    #[::core::prelude::v1::test]
    fn heap_children_follow_array_indices() {
        let items: Vec<Cell> = ["1", "3", "2", "7"].iter().map(|v| Cell { value: (*v).into(), tone: Tone::Default }).collect();
        let nodes = heap_nodes(&items);
        assert_eq!(nodes[0].left.as_deref(), Some("1"));
        assert_eq!(nodes[1].left.as_deref(), Some("3"));
        assert_eq!(nodes[1].right, None);
    }
}
