//! Precomputed roadmap membership and progress, so rendering never scans the dataset.

use std::collections::{HashMap, HashSet};

use practice::leetcode::CatalogItem;
use practice::roadmap::{ENTRIES, List, TOPIC_ORDER};

pub struct TopicInfo {
    pub name: &'static str,
    /// Indices into `roadmap::ENTRIES`, in NeetCode order.
    pub entries: Vec<usize>,
    pub solved: usize,
}

#[derive(Default)]
pub struct Library {
    pub topics: Vec<TopicInfo>,
    by_name: HashMap<&'static str, usize>,
    pub solved: usize,
    pub total: usize,
}

impl Library {
    /// Roadmap entries in `list`, minus premium problems once the catalog is known.
    pub fn build(list: List, catalog: &[CatalogItem], by_slug: &HashMap<String, usize>, solved: &HashSet<String>) -> Self {
        let free = |slug: &str| catalog.is_empty() || by_slug.get(slug).is_some_and(|&i| !catalog[i].paid_only);
        let topics: Vec<TopicInfo> = TOPIC_ORDER
            .iter()
            .map(|t| {
                let entries: Vec<usize> = ENTRIES
                    .iter()
                    .enumerate()
                    .filter(|(_, e)| e.topic == t.name && e.in_list(list) && free(&e.slug))
                    .map(|(i, _)| i)
                    .collect();
                let solved = entries.iter().filter(|&&i| solved.contains(&ENTRIES[i].slug)).count();
                TopicInfo { name: t.name, entries, solved }
            })
            .collect();
        let by_name = topics.iter().enumerate().map(|(i, t)| (t.name, i)).collect();
        let solved = topics.iter().map(|t| t.solved).sum();
        let total = topics.iter().map(|t| t.entries.len()).sum();
        Self { topics, by_name, solved, total }
    }

    pub fn topic(&self, name: &str) -> Option<&TopicInfo> {
        self.by_name.get(name).map(|&i| &self.topics[i])
    }

    pub fn progress(&self, name: &str) -> (usize, usize) {
        self.topic(name).map_or((0, 0), |t| (t.solved, t.entries.len()))
    }

    /// Every roadmap problem in reading order, for next/previous.
    pub fn ordered(&self) -> impl Iterator<Item = usize> + '_ {
        self.topics.iter().flat_map(|t| t.entries.iter().copied())
    }
}
