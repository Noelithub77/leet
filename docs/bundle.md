# Offline question bundle

The tracked `crates/practice/data/base.sqlite` is embedded in leet. Diesel applies the regular migrations and imports missing public catalog, question, reference, and article records on first launch or after the embedded bundle changes. Existing question cache, progress, custom tests, account data, and solution files keep their values. Account submission caches never enter the bundle.

New users default to NeetCode 150. Their statements and available reference code/articles are present before any network request. The base also contains only catalog metadata for the rest of LeetCode and title/ID/rating/tag metadata from the approved Open-R1 Codeforces snapshot. CodeChef includes 100 public statements and samples: 50 popular beginner problems (rating 0–999) and 50 intermediate problems (1000–1800), selected by successful submissions. Codeforces statements fetch on demand and are then cached in the user SQLite; no Codeforces statements or editorials are bundled. Aliases preserve both contest identifiers. Catalog refresh remains live; an upstream error retains the bundled/cached data.

The compacted bundle is 11,567,104 bytes (11.0 MiB), down from 97,742,848 bytes (93.2 MiB) with Codeforces content. It contains 150 NeetCode statements, 100 CodeChef statements, 4,073 LeetCode catalog records and 11,911 Codeforces catalog identities. NeetCode 150 includes 600 language-specific articles and 564 available reference translations.

## Refresh the tracked database

Run from the repository root. The NeetCode and CodeChef commands resume missing statement content; LeetCode and Codeforces refresh catalog metadata only. Each command reports actual failures as JSON with a nonzero exit code. Unpublished language translations are reported separately as `upstream_omissions`.

```sh
./ops snapshot --source neetcode150 --json
./ops snapshot --source leetcode --json
./ops snapshot --source codeforces --json
./ops snapshot --source codechef --json
```

The Codeforces download requires the approved [DuckDB CLI](https://duckdb.org/docs/stable/clients/cli/overview.html) and its `httpfs` extension. It projects only titles, IDs, aliases, ratings, and tags from the dataset's Parquet files; it omits large hidden/generated test collections. To import an already projected file, use `--import /path/to/open-r1-codeforces.jsonl`. `--output` selects a different database; production/user data is never the default target.

Refresh the on-demand statement lookup index with `./ops snapshot --source codeforces-index --output crates/practice/data/codeforces-snapshot-index.json --json`. This projects only identities/aliases and row positions, separately for train/test. Runtime fetches a single Dataset Viewer row, checks its canonical identity, rejects truncated statement fields, and caches the parsed statement in the user's SQLite. Newer problems absent from this historical snapshot require a live download or browser sample import. The app never executes DuckDB.

The builder compacts SQLite and checkpoints its WAL before completing. Commit the `.sqlite` plus its source reports, not transient `-wal` or `-shm` sidecars. Rebuild leet after changing the bundle.

## Sources and coverage limits

- [CodeChef](https://www.codechef.com/): public practice catalog (`/api/list/problems`) and statement HTML/structured sample cases (`/api/contests/PRACTICE/problems/CODE`). Only code, title, difficulty, public statement, sample pairs, provenance, and empty stdin starters enter the bundle. This is a curated 100-problem collection, not the full CodeChef catalog. Original provider URLs remain available; no editorial answers, hidden tests, paid content, account state, or personal submissions are scraped.
- [LeetCode](https://leetcode.com/): public catalog metadata; GraphQL statements/starters load on demand outside NeetCode 150. LeetCode Premium statements outside the public NeetCode alternatives remain unavailable in an anonymous snapshot; their catalog metadata does not imply an offline statement.
- [NeetCode solution articles](https://neetcode.io/solutions/minimum-path-sum): all approaches, selected-language code, complexity notes, prerequisites, pitfalls, image references, and video URLs. Markdown displays article content; videos open in the browser and remote images still require network access. Seven NeetCode 150 premium alternatives use NeetCode's description and Python signatures with empty bodies, never prefilled answers.
- [NeetCode reference code](https://github.com/neetcode-gh/leetcode): MIT, with the bundled attribution/license. Some C/Go translations are not published upstream; the Python and C++ references cover all 150.
- [Open-R1 Codeforces](https://huggingface.co/datasets/open-r1/codeforces): approved historical snapshot through early 2025, licensed [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/). Attribution: the Open-R1 authors/Hugging Face, with problem content originating from Codeforces. leet bundles only catalog metadata, preserves aliases, and omits statements, editorials, hidden tests, and checkers. On-demand statements try Codeforces and fall back to indexed Open-R1 rows when the live request fails; successful results persist in the private cache. This is not proof that every current Codeforces problem is bundled. Original provider links remain available in the app.

The snapshot contains no progress marks, account sessions, submissions, personal Git history, or custom tests. Use the adjacent JSON reports and `SELECT COUNT(*)` on the bundle to audit actual coverage.
