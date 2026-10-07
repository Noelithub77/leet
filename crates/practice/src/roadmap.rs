//! NeetCode problem lists and the topic graph from neetcode.io/roadmap.

use std::collections::HashMap;
use std::sync::LazyLock;

use serde::{Deserialize, Serialize};

/// A NeetCode list. Each larger list contains the smaller one.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum List {
    #[default]
    NeetCode150,
    NeetCode250,
    All,
}

impl List {
    pub fn label(self) -> &'static str {
        match self {
            List::NeetCode150 => "NeetCode 150",
            List::NeetCode250 => "NeetCode 250",
            List::All => "NeetCode All",
        }
    }

    pub fn next(self) -> Self {
        match self {
            List::NeetCode150 => List::NeetCode250,
            List::NeetCode250 => List::All,
            List::All => List::NeetCode150,
        }
    }

    fn tag(self) -> Option<&'static str> {
        match self {
            List::NeetCode150 => Some("neetcode150"),
            List::NeetCode250 => Some("neetcode250"),
            List::All => None,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct Entry {
    /// LeetCode title slug.
    pub slug: String,
    pub title: String,
    pub topic: String,
    pub difficulty: String,
    /// neetcode.io problem slug.
    pub nc: String,
    pub video: String,
    pub lists: Vec<String>,
}

impl Entry {
    pub fn in_list(&self, list: List) -> bool {
        list.tag().is_none_or(|tag| self.lists.iter().any(|l| l == tag))
    }

    pub fn neetcode_url(&self) -> String {
        format!("https://neetcode.io/problems/{}/question", self.nc)
    }
}

pub static ENTRIES: LazyLock<Vec<Entry>> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../data/neetcode.json")).expect("bundled NeetCode data")
});

/// A roadmap node: grid row/column for layout, and the topics it unlocks.
pub struct Topic {
    pub name: &'static str,
    pub row: u8,
    pub col: f32,
    pub children: &'static [&'static str],
}

/// The neetcode.io/roadmap graph. Columns are centered positions on a 6-wide grid.
pub const TOPICS: &[Topic] = &[
    Topic { name: "Arrays & Hashing", row: 0, col: 2.5, children: &["Two Pointers", "Stack"] },
    Topic { name: "Two Pointers", row: 1, col: 1.5, children: &["Binary Search", "Sliding Window", "Linked List"] },
    Topic { name: "Stack", row: 1, col: 3.5, children: &[] },
    Topic { name: "Binary Search", row: 2, col: 0.5, children: &["Trees"] },
    Topic { name: "Sliding Window", row: 2, col: 2.5, children: &[] },
    Topic { name: "Linked List", row: 2, col: 4.5, children: &["Trees"] },
    Topic { name: "Trees", row: 3, col: 2.5, children: &["Tries", "Heap / Priority Queue", "Backtracking"] },
    Topic { name: "Tries", row: 4, col: 0.5, children: &[] },
    Topic { name: "Heap / Priority Queue", row: 4, col: 2.0, children: &["Intervals", "Greedy", "Advanced Graphs"] },
    Topic { name: "Backtracking", row: 4, col: 4.0, children: &["Graphs", "1-D Dynamic Programming"] },
    Topic { name: "Intervals", row: 5, col: 0.0, children: &[] },
    Topic { name: "Greedy", row: 5, col: 1.0, children: &[] },
    Topic { name: "Advanced Graphs", row: 6, col: 1.5, children: &[] },
    Topic { name: "Graphs", row: 5, col: 3.0, children: &["Advanced Graphs", "2-D Dynamic Programming", "Math & Geometry"] },
    Topic { name: "1-D Dynamic Programming", row: 5, col: 5.0, children: &["2-D Dynamic Programming", "Bit Manipulation"] },
    Topic { name: "2-D Dynamic Programming", row: 6, col: 3.5, children: &[] },
    Topic { name: "Bit Manipulation", row: 6, col: 5.0, children: &["Math & Geometry"] },
    Topic { name: "Math & Geometry", row: 7, col: 4.0, children: &[] },
];

pub fn topic(name: &str) -> Option<&'static Topic> {
    TOPICS.iter().find(|t| t.name == name)
}

/// Entries of `topic` in `list`, in NeetCode's order.
pub fn problems(topic: &str, list: List) -> impl Iterator<Item = &'static Entry> {
    ENTRIES.iter().filter(move |e| e.topic == topic && e.in_list(list))
}

static BY_SLUG: LazyLock<HashMap<&'static str, usize>> =
    LazyLock::new(|| ENTRIES.iter().enumerate().map(|(i, e)| (e.slug.as_str(), i)).collect());

pub fn entry_index(slug: &str) -> Option<usize> {
    BY_SLUG.get(slug).copied()
}

pub fn entry(slug: &str) -> Option<&'static Entry> {
    entry_index(slug).map(|i| &ENTRIES[i])
}

/// Topics in reading order (row, then column).
pub static TOPIC_ORDER: LazyLock<Vec<&'static Topic>> = LazyLock::new(|| {
    let mut topics: Vec<_> = TOPICS.iter().collect();
    topics.sort_by(|a, b| a.row.cmp(&b.row).then(a.col.total_cmp(&b.col)));
    topics
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_have_expected_sizes() {
        let count = |list| ENTRIES.iter().filter(|e| e.in_list(list)).count();
        assert_eq!(count(List::NeetCode150), 150);
        assert_eq!(count(List::NeetCode250), 250);
        assert!(count(List::All) > 250);
    }

    #[test]
    fn every_entry_topic_and_edge_is_on_the_roadmap() {
        for e in ENTRIES.iter() {
            assert!(topic(&e.topic).is_some(), "{} has unknown topic {}", e.slug, e.topic);
        }
        for t in TOPICS {
            for child in t.children {
                let child = topic(child).expect("edge target exists");
                assert!(child.row > t.row, "{} -> {} must go down", t.name, child.name);
            }
        }
    }
}
