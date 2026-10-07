//! Public Codeforces contests and retry-safe private SQLite snapshots.
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use crate::{codeforces::{self, Problem}, db::Db};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Contest {
    pub id: u32,
    pub name: String,
    pub phase: String,
    pub duration_seconds: u64,
    pub start_time_seconds: Option<u64>,
}

impl Contest {
    pub fn past(&self) -> bool { self.phase == "FINISHED" }
    pub fn timing(&self, now: u64) -> String {
        if self.past() { return "Finished".into(); }
        if self.phase != "BEFORE" { return "Live".into(); }
        let Some(start) = self.start_time_seconds else { return "Scheduled".into(); };
        let minutes = start.saturating_sub(now).div_ceil(60);
        if minutes == 0 { "Starting".into() }
        else if minutes < 60 { format!("In {minutes}m") }
        else if minutes < 1440 { format!("In {}h {}m", minutes / 60, minutes % 60) }
        else { format!("In {}d {}h", minutes / 1440, minutes % 1440 / 60) }
    }
}

pub fn cached_list(db: &Db) -> Result<Vec<Contest>> {
    db.get("cf-contests:v1")?.map(|text| serde_json::from_str(&text).map_err(Into::into)).unwrap_or(Ok(vec![]))
}

pub fn refresh_list(db: &Db) -> Result<Vec<Contest>> {
    save_list(db, codeforces::request("contest.list", &[("gym", "false")]))
}

fn save_list(db: &Db, result: Result<Vec<Contest>>) -> Result<Vec<Contest>> {
    let mut contests = result?;
    if contests.is_empty() || contests.iter().any(|contest| contest.id == 0 || contest.name.is_empty()) { bail!("Codeforces returned an incomplete contest list"); }
    contests.sort_by_key(|contest| (contest.past(), if contest.past() { u64::MAX - contest.start_time_seconds.unwrap_or(0) } else { contest.start_time_seconds.unwrap_or(u64::MAX) }));
    db.set("cf-contests:v1", &serde_json::to_string(&contests)?)?;
    Ok(contests)
}

pub fn cached_problems(db: &Db, id: u32) -> Result<Vec<Problem>> {
    db.get(&format!("cf-contest-problems:v1:{id}"))?.map(|text| serde_json::from_str(&text).map_err(Into::into)).unwrap_or(Ok(vec![]))
}

pub fn refresh_problems(db: &Db, id: u32) -> Result<Vec<Problem>> {
    #[derive(Deserialize)]
    struct Standings { problems: Vec<Problem> }
    // Regular public standings require exactly contestId; don't request user rows/filters.
    let standings: Standings = codeforces::request("contest.standings", &[("contestId", &id.to_string())])?;
    save_problems(db, id, Ok(standings.problems))
}

fn save_problems(db: &Db, id: u32, result: Result<Vec<Problem>>) -> Result<Vec<Problem>> {
    let mut problems = result?;
    if problems.is_empty() { bail!("Contest problems are not published yet"); }
    if problems.iter().any(|problem| problem.contest_id != Some(id) || problem.slug().is_none_or(|slug| codeforces::problem_id(&slug).is_err())) { bail!("Codeforces returned problems from another contest"); }
    problems.sort_by(|a, b| a.index.cmp(&b.index));
    db.set(&format!("cf-contest-problems:v1:{id}"), &serde_json::to_string(&problems)?)?;
    Ok(problems)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cached_contests_survive_reopen_and_failed_refresh() {
        let dir = tempfile::tempdir().unwrap(); let path = dir.path().join("contests.db");
        let db = Db::open(&path).unwrap();
        let contest = |id, phase: &str, start| Contest { id, name: format!("Round {id}"), phase: phase.into(), duration_seconds: 7200, start_time_seconds: Some(start) };
        save_list(&db, Ok(vec![contest(1, "FINISHED", 1), contest(3, "BEFORE", 300), contest(2, "BEFORE", 200)])).unwrap();
        assert!(save_list(&db, Err(anyhow::anyhow!("offline"))).is_err());
        assert!(save_list(&db, Ok(vec![])).is_err());
        drop(db); let db = Db::open(&path).unwrap();
        let contests = cached_list(&db).unwrap(); assert_eq!(contests.iter().map(|contest| contest.id).collect::<Vec<_>>(), [2, 3, 1]);
        assert_eq!(contests[0].timing(140), "In 1m"); assert_eq!(contests[2].timing(140), "Finished");
        assert!(cached_problems(&db, 2).unwrap().is_empty());
    }
    #[test]
    fn contest_problem_refresh_preserves_cache_on_errors_and_rejects_other_contests() {
        let dir = tempfile::tempdir().unwrap(); let path = dir.path().join("problems.db"); let db = Db::open(&path).unwrap();
        let problem = |id| Problem { contest_id: Some(id), index: "A".into(), name: "Test".into(), rating: None, tags: vec![] };
        save_problems(&db, 1, Ok(vec![problem(1)])).unwrap();
        assert!(save_problems(&db, 1, Err(anyhow::anyhow!("offline"))).is_err());
        assert!(save_problems(&db, 1, Ok(vec![problem(2)])).is_err());
        assert!(save_problems(&db, 1, Ok(vec![])).is_err());
        drop(db); let db = Db::open(&path).unwrap();
        assert_eq!(cached_problems(&db, 1).unwrap()[0].contest_id, Some(1));
        assert!(cached_problems(&db, 2).unwrap().is_empty());
    }

    #[test]
    #[ignore = "downloads public Codeforces contests and problems"]
    fn live_contests_and_contest_cache() {
        let dir = tempfile::tempdir().unwrap(); let db = Db::open(&dir.path().join("contests.db")).unwrap();
        assert!(!refresh_list(&db).unwrap().is_empty());
        let problems = refresh_problems(&db, 1).unwrap(); assert!(!problems.is_empty());
        assert_eq!(cached_problems(&db, 1).unwrap().len(), problems.len());
    }
}
