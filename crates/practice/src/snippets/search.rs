//! Prepared snippet search data shared by every editor tab.
use std::{collections::HashSet, ops::Deref};
use crate::{language::Language, search::{Index, Searcher}};
use super::{Snippet, Origin, body, guide};

pub struct Library {
    snippets: Vec<Snippet>,
    rows: Vec<(usize, usize)>,
    full: Index,
    prefixes: Index,
    prefix_lower: Vec<String>,
    names: Index,
    descriptions: Vec<String>,
    previews: Vec<String>,
}
impl Deref for Library {
    type Target = [Snippet];
    fn deref(&self) -> &Self::Target { &self.snippets }
}
impl Library {
    pub fn new(snippets: Vec<Snippet>) -> Self {
        let rows: Vec<_> = snippets.iter().enumerate().flat_map(|(i, s)| (0..s.prefixes.len()).map(move |p| (i, p))).collect();
        let full = Index::new(rows.iter().map(|&(i,p)| format!("{} {} {}", snippets[i].prefixes[p], snippets[i].name, snippets[i].description)));
        let prefixes = Index::new(rows.iter().map(|&(i,p)| &snippets[i].prefixes[p]));
        let prefix_lower = rows.iter().map(|&(i,p)| snippets[i].prefixes[p].to_lowercase()).collect();
        let names = Index::new(rows.iter().map(|&(i,_)| &snippets[i].name));
        let previews: Vec<String> = snippets.iter().map(|s| body::preview(&s.body).text).collect();
        let descriptions = rows.iter().map(|&(i,p)| {
            let snippet = &snippets[i];
            let mut description = snippet.description.clone();
            if snippet.origin == Origin::Builtin {
                if let Some(help) = guide::guide(&snippet.prefixes[p]) {
                    description.push_str(&format!("\nInput · {}\nOutput · {}\nExample · {}", help.input, help.output, help.example));
                }
            }
            description
        }).collect();
        Self { snippets, rows, full, prefixes, prefix_lower, names, descriptions, previews }
    }
    pub fn preview(&self, prefix: &str, body: &str, language: Language) -> Option<&str> {
        let index = self.snippets.iter().position(|s| s.applies_to(language) && s.prefixes.iter().any(|p| p == prefix) && s.body == body)?;
        Some(&self.previews[index])
    }
    pub fn description(&self, row: usize) -> &str { &self.descriptions[row] }
    pub fn row(&self, row: usize) -> (&Snippet, &str) {
        let (i,p) = self.rows[row];
        (&self.snippets[i], &self.snippets[i].prefixes[p])
    }
    pub fn rank(&self, searcher: &mut Searcher, query: &str, language: Language, limit: usize) -> Vec<usize> {
        let keep = |row: usize| self.snippets[self.rows[row].0].applies_to(language);
        let mut hits = searcher.rank(&self.full, query, self.rows.len(), keep);
        if !query.is_empty() {
            let mut bonuses = vec![0; self.rows.len()];
            for hit in searcher.rank(&self.names, query, self.rows.len(), keep) { bonuses[hit.index] = 100_000 + hit.score; }
            for hit in searcher.rank(&self.prefixes, query, self.rows.len(), keep) { bonuses[hit.index] = 200_000 + hit.score; }
            let lower = query.to_lowercase();
            for hit in &mut hits {
                let prefix = &self.prefix_lower[hit.index];
                hit.score += if prefix == &lower { 1_000_000 }
                    else if prefix.starts_with(&lower) { 500_000 }
                    else { bonuses[hit.index] };
            }
            hits.sort_unstable_by(|a,b| b.score.cmp(&a.score).then(a.index.cmp(&b.index)));
        }
        let mut seen = HashSet::new();
        hits.into_iter().filter(|h| seen.insert(self.rows[h.index].0)).take(limit).map(|h| h.index).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn indexed_snippets_match_alias_name_description_typo_and_language() {
        let library = Library::new(super::super::builtin());
        let mut searcher = Searcher::default();
        for query in ["bfs", "brfs", "breadth first", "reachable", "dijktsra"] {
            let hits = library.rank(&mut searcher, query, Language::Python, 64);
            assert!(!hits.is_empty(), "{query}");
            assert!(hits.iter().all(|&row| library.row(row).0.scope == Some(Language::Python)));
        }
        assert_eq!(library.row(library.rank(&mut searcher, "bfs", Language::Python, 64)[0]).1, "bfs");
        let hits = library.rank(&mut searcher, "", Language::Python, 64);
        let mut names = HashSet::new();
        assert!(hits.iter().all(|&row| names.insert(library.row(row).0.name.clone())));
    }
}
