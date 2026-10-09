//! Small semantic examples for the bundled prefixes, independent of language syntax.
pub struct Guide {
    pub input: &'static str,
    pub output: &'static str,
    pub example: &'static str,
}

pub fn guide(prefix: &str) -> Option<Guide> {
    let (input, output, example) = match prefix {
        "bfs" => ("Unweighted graph and start vertex", "Shortest edge-count distances", "0—1—2, start 0 → [0, 1, 2]"),
        "dfs" => ("Graph and start vertex", "Reachable vertices in traversal order", "0—1—2, start 0 → visit all three"),
        "dijkstra" => ("Graph with nonnegative edge weights", "Shortest weighted distances", "0→1 (2), 1→2 (3) → [0, 2, 5]"),
        "bfs01" => ("Graph with weights 0 or 1", "Shortest weighted distances", "0→1 (0), 1→2 (1) → [0, 0, 1]"),
        "topo" => ("Directed acyclic graph", "An order respecting every edge", "0→1→2 → [0, 1, 2]"),
        "floyd" => ("Distance matrix; no negative cycles", "Shortest distance for every vertex pair", "0→1 (2), 1→2 (3) → distance(0,2)=5"),
        "gridbfs" => ("Grid, start cell, and blocked cells", "Four-neighbor shortest distances", "Open 2×2 grid from (0,0) → [[0,1],[1,2]]"),
        "graph" => ("Vertex count and undirected edges", "Adjacency lists", "Edge 0—1 → [[1], [0]]"),
        "gcd" => ("Two nonnegative integers", "Greatest common divisor and least common multiple", "12, 18 → gcd 6; lcm 36"),
        "modpow" => ("Base, nonnegative exponent, modulus", "Power reduced modulo the modulus", "2⁵ mod 7 → 4"),
        "modinv" => ("Invertible value and modulus", "Multiplicative inverse", "3 mod 7 → 5, since 3×5 mod 7 = 1"),
        "mod" => ("A contest prime modulus", "A reusable modulus constant", "1,000,000,007 → use in modular arithmetic"),
        "ncr" => ("n, r and a prime modulus; n below modulus", "Number of ways to choose r from n", "n=5, r=2 → 10"),
        "sieve" => ("Inclusive upper bound", "Primes or primality flags up to that bound", "Limit 10 → 2, 3, 5, 7 are prime"),
        "factor" => ("Positive integer", "Prime factors with multiplicities", "12 → 2² × 3"),
        "dsu" => ("Elements and union/find operations", "Connected-component membership", "Union 0 and 1 → same component"),
        "bit" => ("Values, point additions, [left,right) queries", "Range sums", "[2,3,5], query [0,2) → 5"),
        "segtree" => ("Values, associative operation and identity", "Updated range aggregates", "Sum on [2,3,5], query [0,2) → 5"),
        "sparse" => ("Static values and range queries", "Range minima", "[4,2,7], query first two → 2"),
        "monostack" => ("Array of values", "Next strictly greater element indices", "[2,1,3] → [2,2,-1]"),
        "trie" => ("Words to insert and look up", "Word membership", "Insert cat → cat found; car absent"),
        "bs" => ("Search bounds and monotone predicate", "First position where the predicate is true", "x² ≥ 10 on [0,5] → 4"),
        "bounds" => ("Sorted values and a target", "Lower and upper insertion positions", "[1,2,2,4], target 2 → positions 1 and 3"),
        "memo" => ("A recurrence with hashable arguments", "Cached results for repeated calls", "Compute f(5) once → reuse its result"),
        "dp2" => ("Row count, column count, initial value", "Independent rows in a 2D table", "2×3 zeros → [[0,0,0],[0,0,0]]"),
        "pref" => ("Array and [left,right) range", "Range sum from padded prefix sums", "[2,3,5], range [1,3) → 8"),
        "pref2" => ("Matrix and rectangular range", "Rectangle sum from 2D prefix sums", "[[1,2],[3,4]], whole grid → 10"),
        "twop" => ("Sorted values and target sum", "A matching pair of indices, if present", "[1,2,4], target 6 → indices 1 and 2"),
        "window" => ("Nonnegative values and a sum limit", "Longest valid contiguous window length", "[1,2,3], limit 3 → length 2"),
        "kmp" => ("String", "Length of each prefix's longest proper border", "ababa → [0,0,1,2,3]"),
        "zfunc" => ("String", "Prefix-match length at each later position", "ababa → positions 1…4: [0,3,0,1]"),
        "hash" => ("String and [left,right) range", "Substring fingerprint; collisions are possible", "banana → hashes of its two ana ranges agree"),
        "oset" => ("Ordered unique values and rank queries", "Count below a key or value at a rank", "{2,5,9} → count below 5 = 1"),
        "pq" => ("Values inserted into a min heap", "Smallest value first", "Push 4,1,3 → pop 1"),
        "umap" => ("Keys and associated values", "Lookup by key", "Store count[7]=2 → lookup 7 returns 2"),
        "cp" => ("Problem input on standard input", "Answers on standard output", "Add solving code inside solve/main"),
        "cpfull" | "cpt" | "tests" => ("Test count followed by test cases", "One result for each case", "t=2 → solve the first and second cases"),
        "cprec" => ("A deep recursive traversal", "A larger recursion stack allowance", "Long graph chain → allow deeper DFS calls"),
        "fori" | "forn" => ("A nonnegative iteration count", "One iteration per index", "n=3 → indices 0, 1, 2"),
        "forr" => ("A nonnegative iteration count", "Indices in reverse order", "n=3 → indices 2, 1, 0"),
        "fore" => ("A container of values", "An iteration for each value", "[4,7] → visit 4 then 7"),
        "while" => ("A condition or input sentinel", "Repeated work while the condition holds", "Condition false → no iterations"),
        "readarr" => ("Array length and integer tokens", "An integer array", "n=3; 4 7 9 → [4,7,9]"),
        "readgrid" => ("Dimensions and character rows", "A character grid", "2×2; ab, cd → rows ab and cd"),
        "printarr" => ("Array or container", "Space-separated output", "[4,7,9] → 4 7 9"),
        "yn" | "yes" | "no" => ("A true/false verdict", "YES or NO", "True → YES; false → NO"),
        "out" => ("Answers or verdicts", "Buffered or newline-separated output", "A result → write it on standard output"),
        "ll" => ("An integer declaration", "A signed 64-bit alias", "ll answer = 10000000000"),
        "vi" => ("Integer values", "An integer vector alias", "vi values = {1,2,3}"),
        "vvi" => ("Rows of integers", "An integer matrix alias", "vvi grid = {{1,2},{3,4}}"),
        "pii" => ("Two integers", "An integer pair alias", "pii edge = {0,1}"),
        "vpii" => ("Pairs of integers", "A vector of integer pairs", "vpii edges = {{0,1},{1,2}}"),
        "all" | "rall" => ("A container", "Forward and reverse iterator ranges", "sort(all(values)) → ascending values"),
        "dirs" => ("A grid coordinate", "Four orthogonal neighbor offsets", "(1,1) → (0,1), (2,1), (1,0), (1,2)"),
        "bits" => ("An integer bit mask", "Bit operations and utilities", "Mask 5 = binary 101 → two set bits"),
        "inf" => ("A distance or minimum-cost initialization", "A large finite sentinel", "Unvisited distance → initialize to infinity"),
        "debug" => ("Values to inspect locally", "Debug information on standard error", "Print state without changing judge output"),
        "rng" => ("A random-generation request", "Pseudo-random values", "Generate values for local stress tests"),
        "util" => ("Contest helpers, values, or coordinates", "Reusable constants and utility functions", "Four orthogonal offsets → grid neighbors"),
        _ => return None,
    };
    Some(Guide { input, output, example })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_builtin_prefix_has_a_compact_usage_guide() {
        for snippet in crate::snippets::builtin() {
            for prefix in &snippet.prefixes {
                let help = guide(prefix).unwrap_or_else(|| panic!("Missing guide for {prefix}"));
                assert!(!help.input.is_empty() && !help.output.is_empty() && !help.example.is_empty());
                assert!(help.example.chars().count() <= 80, "{prefix}");
            }
        }
    }
}
