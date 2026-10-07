//! Native data-structure visuals shared by AI scenes and debugger frames.
//! Values are display strings; `Tone` names a meaning the UI maps onto theme accents.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Tone {
    #[default]
    Default,
    /// Being read or compared right now.
    Active,
    /// Seen earlier in the walk.
    Visited,
    /// Final, settled, or part of the answer.
    Done,
    /// Written on this step.
    Changed,
    /// Out of the current window or pruned.
    Muted,
    Warn,
    Error,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Cell {
    pub value: String,
    #[serde(default)]
    pub tone: Tone,
}

/// A named marker under an index, such as `i`, `l`, `r`, or `mid`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Pointer {
    pub name: String,
    pub index: usize,
    #[serde(default)]
    pub tone: Tone,
}

/// An inclusive index range, such as a sliding window.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Span {
    pub from: usize,
    pub to: usize,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub tone: Tone,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct GridPointer {
    pub name: String,
    pub row: usize,
    pub col: usize,
    #[serde(default)]
    pub tone: Tone,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TreeNode {
    pub id: String,
    pub value: String,
    #[serde(default)]
    pub left: Option<String>,
    #[serde(default)]
    pub right: Option<String>,
    #[serde(default)]
    pub tone: Tone,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct GraphNode {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub tone: Tone,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Edge {
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub tone: Tone,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Entry {
    pub key: String,
    pub value: String,
    #[serde(default)]
    pub tone: Tone,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Interval {
    pub start: f64,
    pub end: f64,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub tone: Tone,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Var {
    pub name: String,
    pub value: String,
    #[serde(default)]
    pub tone: Tone,
}

/// One drawable structure. Every variant carries a short `label`, usually the variable name.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Structure {
    Array { label: String, items: Vec<Cell>, #[serde(default)] pointers: Vec<Pointer>, #[serde(default)] spans: Vec<Span> },
    /// Matrices and DP tables. Labels are optional headers.
    Grid { label: String, rows: Vec<Vec<Cell>>, #[serde(default)] row_labels: Vec<String>, #[serde(default)] col_labels: Vec<String>, #[serde(default)] pointers: Vec<GridPointer> },
    Tree { label: String, root: Option<String>, nodes: Vec<TreeNode> },
    Graph { label: String, nodes: Vec<GraphNode>, edges: Vec<Edge>, #[serde(default)] directed: bool },
    /// Nodes in `next` order; `cycle_to` is the index the last node links back to.
    LinkedList { label: String, nodes: Vec<Cell>, #[serde(default)] pointers: Vec<Pointer>, #[serde(default)] cycle_to: Option<usize> },
    /// The top is the last item.
    Stack { label: String, items: Vec<Cell> },
    /// The front is the first item.
    Queue { label: String, items: Vec<Cell> },
    /// Array-backed binary heap in heap order.
    Heap { label: String, items: Vec<Cell>, #[serde(default = "yes")] min: bool },
    Map { label: String, entries: Vec<Entry> },
    Set { label: String, items: Vec<Cell> },
    Intervals { label: String, items: Vec<Interval> },
    /// Scalars and anything without a richer shape.
    Vars { label: String, vars: Vec<Var> },
}

fn yes() -> bool { true }

impl Structure {
    pub fn label(&self) -> &str {
        match self {
            Structure::Array { label, .. } | Structure::Grid { label, .. } | Structure::Tree { label, .. }
            | Structure::Graph { label, .. } | Structure::LinkedList { label, .. } | Structure::Stack { label, .. }
            | Structure::Queue { label, .. } | Structure::Heap { label, .. } | Structure::Map { label, .. }
            | Structure::Set { label, .. } | Structure::Intervals { label, .. } | Structure::Vars { label, .. } => label,
        }
    }
}

/// One moment of a walkthrough.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Frame {
    /// One short sentence about what happens on this step.
    pub caption: String,
    /// 1-based line in the user's solution, when the step maps to code.
    #[serde(default)]
    pub line: Option<u32>,
    pub structures: Vec<Structure>,
}

/// An animated walkthrough of an algorithm on one input.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Scene {
    pub title: String,
    /// The input being walked, in the problem's input format.
    #[serde(default)]
    pub input: String,
    pub frames: Vec<Frame>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structures_round_trip_with_defaults() {
        let json = r#"{"kind":"array","label":"nums","items":[{"value":"2"},{"value":"7","tone":"active"}],"pointers":[{"name":"i","index":1}]}"#;
        let structure: Structure = serde_json::from_str(json).unwrap();
        let Structure::Array { items, pointers, spans, .. } = &structure else { panic!("array expected") };
        assert_eq!(items[1].tone, Tone::Active);
        assert_eq!(pointers[0].tone, Tone::Default);
        assert!(spans.is_empty());
        assert_eq!(structure.label(), "nums");
        let heap: Structure = serde_json::from_str(r#"{"kind":"heap","label":"h","items":[]}"#).unwrap();
        assert!(matches!(heap, Structure::Heap { min: true, .. }));
    }

    #[test]
    fn scene_schema_names_every_structure_kind() {
        let schema = serde_json::to_string(&schemars::schema_for!(Scene)).unwrap();
        for kind in ["array", "grid", "tree", "graph", "linked_list", "stack", "queue", "heap", "map", "set", "intervals", "vars"] {
            assert!(schema.contains(&format!("\"{kind}\"")), "{kind} missing from schema");
        }
    }
}
