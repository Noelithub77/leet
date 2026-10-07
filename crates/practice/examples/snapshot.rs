//! Public-data-only, resumable offline bundle builder. Never reads account credentials.
use std::{collections::BTreeMap, path::PathBuf, sync::{Arc, atomic::{AtomicUsize, Ordering}}, time::Duration};
use anyhow::{Result, bail};
use serde_json::{Value, json};
use practice::{db::Db, language::Language, leetcode::Client, roadmap::{ENTRIES, List}, solutions};
fn main() -> Result<()> {
    let mut output = PathBuf::from("crates/practice/data/base.sqlite");
    let mut source = String::from("neetcode150"); let mut import = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() { match arg.as_str() {
        "--output" => output = args.next().ok_or_else(|| anyhow::anyhow!("--output needs a path"))?.into(),
        "--source" => source = args.next().ok_or_else(|| anyhow::anyhow!("--source needs neetcode150, leetcode, or codeforces"))?,
        "--import" => import = Some(PathBuf::from(args.next().ok_or_else(|| anyhow::anyhow!("--import needs Open-R1 JSONL"))?)),
        "--json" => {},
        "--help" => { println!("snapshot --source <neetcode150|leetcode|codeforces|codeforces-index> [--output PATH] [--import OPEN_R1_JSONL] [--json]\nBundles full NeetCode 150 and metadata-only LeetCode/Codeforces catalogs; no accounts, progress, or credentials. Codeforces downloads only Open-R1 catalog metadata with DuckDB, or imports JSONL; statements load on demand. codeforces-index requires --output PATH.json and projects only lookup metadata. See docs/bundle.md."); return Ok(()); },
        _ => bail!("Unknown argument {arg}"),
    }}
    if source == "codeforces-index" {
        if output.extension().and_then(|value| value.to_str()) != Some("json") { bail!("codeforces-index requires --output PATH.json; SQLite bundles are never overwritten"); }
        return download_codeforces_index(&output);
    }
    let db = Arc::new(Db::open_unseeded(&output)?); let omissions = Arc::new(std::sync::Mutex::new(BTreeMap::<String,String>::new())); let failures = Arc::new(std::sync::Mutex::new(BTreeMap::<String,String>::new()));
    match source.as_str() {
        "neetcode150" | "leetcode" => {
            let client = Client::new(None);
            let catalog = client.catalog()?; db.replace_catalog(&catalog)?;
            let slugs: Vec<_> = ENTRIES.iter().filter(|entry| entry.in_list(List::NeetCode150)).map(|entry| entry.slug.clone()).collect();
            if source == "leetcode" { db.retain_snapshot_questions(&slugs)?; }
            let pending: Vec<_> = if source == "neetcode150" { slugs.iter().filter(|slug| db.question(slug).ok().flatten().is_none_or(|question| question.content.is_empty() || question.python.is_empty())).cloned().collect() } else { vec![] };
            for chunk in pending.chunks(20) {
                match client.question_batch(chunk) {
                    Ok(questions) => for (slug, result) in chunk.iter().zip(questions) { match result { Ok(mut question) => {
                        if question.content.is_empty() && source == "neetcode150" {
                            let article = solutions::article(&db, slug, Language::Python)?;
                            let description = article.markdown.split("## Description").nth(1).and_then(|body| body.split("## Prerequisites").next()).unwrap_or(&article.markdown);
                            question.content = description.trim().into(); question.meta["statementMarkdown"] = json!(true);
                            question.meta["provenance"] = json!("neetcode.io");
                        }
                        if question.python.is_empty() && source == "neetcode150" {
                            let reference = solutions::get(&db, slug, Language::Python)?;
                            question.python = solutions::python_starter(&reference.code);
                            question.snippets.insert("python3".into(), question.python.clone());
                        }
                        if question.content.is_empty() { failures.lock().unwrap().insert(slug.clone(), "No public statement".into()); }
                        else { db.save_question(&question)?; }
                    }, Err(error) => { failures.lock().unwrap().insert(slug.clone(), error.to_string()); } } },
                    Err(error) => { for slug in chunk { failures.lock().unwrap().insert(slug.clone(), error.to_string()); } },
                }
                eprintln!("{source}: fetched batch ending {}", chunk.last().unwrap()); std::thread::sleep(Duration::from_millis(350));
            }
            if source == "neetcode150" {
                let next = AtomicUsize::new(0);
                std::thread::scope(|scope| { for _ in 0..4 { let next = &next; let slugs = &slugs; let db = &db; let failures = &failures; let omissions = &omissions;
                    scope.spawn(move || loop {
                        let index = next.fetch_add(1, Ordering::Relaxed); let Some(slug) = slugs.get(index) else { break; };
                        for language in Language::ALL {
                            if let Err(error) = solutions::article(db, slug, language) { failures.lock().unwrap().insert(format!("article:{slug}:{}", language.id()), error.to_string()); }
                            if let Err(error) = solutions::get(db, slug, language) {
                                let error = error.to_string();
                                if error.starts_with("No ") && error.contains(" reference;") { omissions.lock().unwrap().insert(format!("code:{slug}:{}", language.id()), error); }
                                else { failures.lock().unwrap().insert(format!("code:{slug}:{}", language.id()), error); }
                            }
                        }
                        eprintln!("NeetCode solution {}/{}, {slug}", index+1, slugs.len());
                    });
                }});
            }
        },
        "codeforces" => {
            let path = match import { Some(path) => path, None => download_codeforces()? };
            use std::io::BufRead as _;
            let mut catalog = BTreeMap::new();
            for line in std::io::BufReader::new(std::fs::File::open(path)?).lines() {
                let row: Value = serde_json::from_str(&line?)?;
                let contest: u32 = row["contest_id"].as_str().unwrap_or_default().parse()?;
                let index = row["index"].as_str().unwrap_or_default();
                let mut ids = vec![format!("{contest}_{index}")];
                ids.extend(row["aliases"].as_array().into_iter().flatten().filter_map(|id| id.as_str().map(str::to_owned)));
                let text = |key: &str| row[key].as_str().unwrap_or_default().to_owned();
                for id in ids {
                    let Some((contest, index)) = id.split_once('/').or_else(|| id.split_once('_')) else { continue; };
                    let Ok(contest) = contest.parse::<u32>() else { continue; };
                    let slug = format!("cf:{contest}:{index}"); practice::codeforces::problem_id(&slug)?;
                    let problem = practice::codeforces::Problem { contest_id: Some(contest), index:index.into(), name:text("title"), rating:row["rating"].as_u64().map(|v| v as u32), tags: serde_json::from_value(row["tags"].clone()).unwrap_or_default() };
                    catalog.insert(slug, problem);
                }
            }
            db.remove_codeforces_snapshot_content()?;
            db.set("cf-catalog", &serde_json::to_string(&catalog.into_values().collect::<Vec<_>>())?)?;
        },
        _ => bail!("Unknown source {source}"),
    }
    db.compact_snapshot()?;
    let failures = failures.lock().unwrap();
    let report = json!({"environment":"local-bundle","source":source,"output":output,"bytes":std::fs::metadata(&output)?.len(),"statements":db.question_slugs()?.len(),"leetcode_catalog":db.catalog()?.len(),"content_policy":"full-neetcode150-metadata-elsewhere","failures":*failures,"upstream_omissions":*omissions.lock().unwrap()});
    std::fs::write(output.with_extension(format!("{source}.json")),serde_json::to_vec_pretty(&report)?)?;
    println!("{report}");
    if !failures.is_empty() { bail!("{} records unavailable; bundle preserved for retry",failures.len()); }
    Ok(())
}

fn download_codeforces_index(output: &std::path::Path) -> Result<()> {
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(60))).build().into();
    let files: Value = agent.get("https://datasets-server.huggingface.co/parquet?dataset=open-r1/codeforces").call()?.body_mut().read_json()?;
    let quote = |value: &str| format!("'{}'", value.replace('\'', "''"));
    let mut index = BTreeMap::new();
    for split in ["train", "test"] {
        let mut urls: Vec<String> = files["parquet_files"].as_array().into_iter().flatten()
            .filter(|file| file["config"] == "default" && file["split"] == split)
            .filter_map(|file| file["url"].as_str().map(str::to_owned)).collect();
        urls.sort();
        if urls.is_empty() { bail!("No snapshot files for {split}"); }
        let rows = std::env::temp_dir().join(format!("leet-snapshot-index-{split}.jsonl"));
        let script = rows.with_extension("sql");
        std::fs::write(&script, format!("INSTALL httpfs; LOAD httpfs; SET threads=1; COPY (SELECT row_number() OVER () - 1 AS row_idx, id, aliases FROM read_parquet([{}])) TO {} (FORMAT JSON);",
            urls.iter().map(|url| quote(url)).collect::<Vec<_>>().join(","), quote(&rows.to_string_lossy())))?;
        let status = std::process::Command::new("duckdb").args(["-bail", ":memory:", "-init"]).arg(script).stdin(std::process::Stdio::null()).status()?;
        if !status.success() { bail!("Snapshot index projection failed; output preserved"); }
        for line in std::fs::read_to_string(rows)?.lines() {
            let row: Value = serde_json::from_str(line)?;
            let location = json!([split, row["row_idx"], row["id"]]);
            if let Some(id) = row["id"].as_str() { index.insert(id.to_owned(), location.clone()); }
            for alias in row["aliases"].as_array().into_iter().flatten().filter_map(Value::as_str) {
                index.insert(alias.to_owned(), location.clone());
            }
        }
    }
    let temporary = output.with_extension("json.tmp");
    std::fs::write(&temporary, serde_json::to_vec(&index)?)?;
    std::fs::rename(temporary, output)?;
    println!("{}", json!({"environment":"local-public-index", "output":output, "identities":index.len(), "bytes":std::fs::metadata(output)?.len()}));
    Ok(())
}

fn download_codeforces() -> Result<PathBuf> {
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(60))).build().into();
    let files: Value = agent.get("https://datasets-server.huggingface.co/parquet?dataset=open-r1/codeforces").call()?.body_mut().read_json()?;
    let urls: Vec<_> = files["parquet_files"].as_array().into_iter().flatten().filter(|file| file["config"] == "default").filter_map(|file| file["url"].as_str()).collect();
    if urls.is_empty() { bail!("No Open-R1 Parquet files were returned"); }
    let path = std::env::temp_dir().join("leet-open-r1-codeforces.jsonl");
    let quote = |value: &str| format!("'{}'", value.replace('\'', "''"));
    let query = format!("INSTALL httpfs; LOAD httpfs; COPY (SELECT id, aliases, contest_id, index, title, rating, tags FROM read_parquet([{}])) TO {} (FORMAT JSON);", urls.iter().map(|url| quote(url)).collect::<Vec<_>>().join(","), quote(&path.to_string_lossy()));
    let script = path.with_extension("sql"); std::fs::write(&script, query)?;
    let status = std::process::Command::new("duckdb").args(["-bail",":memory:","-init"]).arg(&script).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::inherit()).status().map_err(|error| anyhow::anyhow!("Install the DuckDB CLI for snapshot tooling: {error}"))?;
    if !status.success() { bail!("DuckDB Codeforces download failed; existing bundle kept"); } Ok(path)
}
