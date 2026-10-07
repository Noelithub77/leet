//! Fuzzy ranking for universal search: nucleo (from the Helix editor) over prebuilt haystacks,
//! with a typo-tolerant fallback when subsequence matching finds little.

use nucleo_matcher::pattern::{AtomKind, CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32String};

/// Searchable text, prepared once. Rebuild only when the items change.
pub struct Index {
    haystacks: Vec<Utf32String>,
    /// Lowercase words for the typo fallback.
    words: Vec<Box<[String]>>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    pub index: usize,
    pub score: u32,
}

impl Index {
    pub fn new<S: AsRef<str>>(items: impl IntoIterator<Item = S>) -> Self {
        let (haystacks, words) = items
            .into_iter()
            .map(|s| {
                let s = s.as_ref();
                let words = s
                    .split(|c: char| !c.is_alphanumeric())
                    .filter(|w| w.len() > 1)
                    .map(str::to_lowercase)
                    .collect();
                (Utf32String::from(s), words)
            })
            .unzip();
        Self { haystacks, words }
    }

    pub fn len(&self) -> usize {
        self.haystacks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.haystacks.is_empty()
    }
}

pub struct Searcher {
    matcher: Matcher,
}

impl Default for Searcher {
    fn default() -> Self {
        Self { matcher: Matcher::new(Config::DEFAULT) }
    }
}

/// Queries this long or longer may fall back to typo matching.
const TYPO_MIN_LEN: usize = 4;
const TYPO_THRESHOLD: f64 = 0.86;

impl Searcher {
    /// Best matches for `query` among items accepted by `keep`, best first.
    /// Empty queries return accepted items in order. Ties keep item order.
    pub fn rank(&mut self, index: &Index, query: &str, limit: usize, keep: impl Fn(usize) -> bool) -> Vec<Hit> {
        let query = query.trim();
        if query.is_empty() {
            return (0..index.len()).filter(|&i| keep(i)).take(limit).map(|i| Hit { index: i, score: 0 }).collect();
        }
        let pattern = Pattern::new(query, CaseMatching::Ignore, Normalization::Smart, AtomKind::Fuzzy);
        let mut hits: Vec<Hit> = index
            .haystacks
            .iter()
            .enumerate()
            .filter(|(i, _)| keep(*i))
            .filter_map(|(i, h)| {
                let score = pattern.score(h.slice(..), &mut self.matcher)?;
                Some(Hit { index: i, score })
            })
            .collect();
        if hits.len() < 8 && query.len() >= TYPO_MIN_LEN {
            let seen: std::collections::HashSet<usize> = hits.iter().map(|h| h.index).collect();
            hits.extend(typo_hits(index, query, &keep).into_iter().filter(|h| !seen.contains(&h.index)));
        }
        hits.sort_unstable_by(|a, b| b.score.cmp(&a.score).then(a.index.cmp(&b.index)));
        hits.truncate(limit);
        hits
    }
}

/// Items where every query word closely resembles some item word (Jaro-Winkler).
fn typo_hits(index: &Index, query: &str, keep: &impl Fn(usize) -> bool) -> Vec<Hit> {
    let terms: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    index
        .words
        .iter()
        .enumerate()
        .filter(|(i, _)| keep(*i))
        .filter_map(|(i, words)| {
            let mut total = 0.0;
            for term in &terms {
                let best = words.iter().map(|w| strsim::jaro_winkler(term, w)).fold(0.0, f64::max);
                if best < TYPO_THRESHOLD {
                    return None;
                }
                total += best;
            }
            // Below any real fuzzy match, ordered by similarity.
            Some(Hit { index: i, score: (total / terms.len() as f64 * 10.0) as u32 })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn index() -> Index {
        Index::new(["1 Two Sum two-sum", "15 3Sum 3sum", "49 Group Anagrams group-anagrams", "242 Valid Anagram valid-anagram", "146 LRU Cache lru-cache"])
    }

    #[test]
    fn empty_query_keeps_order_and_filter() {
        let mut s = Searcher::default();
        let hits: Vec<usize> = s.rank(&index(), "", 10, |i| i != 1).iter().map(|h| h.index).collect();
        assert_eq!(hits, [0, 2, 3, 4]);
    }

    #[test]
    fn matches_case_insensitively_by_words_ids_and_slugs() {
        let mut s = Searcher::default();
        let idx = index();
        assert_eq!(s.rank(&idx, "two sum", 10, |_| true)[0].index, 0);
        assert_eq!(s.rank(&idx, "lru", 10, |_| true)[0].index, 4);
        assert_eq!(s.rank(&idx, "VALID", 10, |_| true)[0].index, 3);
        assert_eq!(s.rank(&idx, "242", 10, |_| true)[0].index, 3);
        assert_eq!(s.rank(&idx, "group-anagrams", 10, |_| true)[0].index, 2);
        assert_eq!(s.rank(&idx, "anag", 10, |_| true).len(), 2);
    }

    #[test]
    fn tolerates_typos() {
        let mut s = Searcher::default();
        let hits = s.rank(&index(), "anagarm", 10, |_| true);
        assert!(hits.iter().any(|h| h.index == 3), "{hits:?}");
        assert!(s.rank(&index(), "zzzz", 10, |_| true).is_empty());
    }
}
