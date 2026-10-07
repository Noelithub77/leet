//! Native renderers for `practice::viz` structures, shared by AI scenes and the debugger.
//! Each structure owns its renderer; playback and drawing styles are shared.

mod array;
mod drawing;
mod graph;
mod grid;
mod heap;
mod intervals;
mod linked_list;
mod map;
mod queue;
mod set;
mod stack;
mod tree;
mod vars;
pub mod player;
pub(crate) mod style;

use gpui_kit::component::{ActiveTheme as _, Theme, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use practice::viz::{Frame, Structure};

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
        Structure::Array { items, pointers, spans, .. } => ("array", array::render(&id, items, pointers, spans, window, cx)),
        Structure::Grid { rows, row_labels, col_labels, pointers, .. } => ("grid", grid::render(rows, row_labels, col_labels, pointers, &theme)),
        Structure::Tree { root, nodes, .. } => ("tree", tree::render(&id, root.as_deref(), nodes, window, cx)),
        Structure::Heap { items, min, .. } => (if *min { "min-heap" } else { "max-heap" }, heap::render(&id, items, window, cx)),
        Structure::Graph { nodes, edges, directed, .. } => ("graph", graph::render(&id, nodes, edges, *directed, window, cx)),
        Structure::LinkedList { nodes, pointers, cycle_to, .. } => ("linked list", linked_list::render(&id, nodes, pointers, *cycle_to, window, cx)),
        Structure::Stack { items, .. } => ("stack", stack::render(items, &theme)),
        Structure::Queue { items, .. } => ("queue", queue::render(items, &theme)),
        Structure::Map { entries, .. } => ("map", map::render(entries, &theme)),
        Structure::Set { items, .. } => ("set", set::render(items, &theme)),
        Structure::Intervals { items, .. } => ("intervals", intervals::render(items, &theme)),
        Structure::Vars { vars, .. } => ("", vars::render(vars, &theme)),
    };
    v_flex().gap_1p5().min_w_0()
        .when(!kind.is_empty(), |el| el.child(caption(structure.label(), kind, &theme)))
        .child(div().id(SharedString::from(format!("{id}-scroll"))).overflow_x_scroll().pb_1().child(body))
        .into_any_element()
}
