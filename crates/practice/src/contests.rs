//! Public provider contests and retry-safe private SQLite snapshots.
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use crate::{codeforces::{self, Problem}, db::Db};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ContestId { Codeforces(u32), LeetCode(String) }

impl ContestId {
    pub fn source(&self) -> crate::language::Source {
        match self { Self::Codeforces(_) => crate::language::Source::Codeforces, Self::LeetCode(_) => crate::language::Source::LeetCode }
    }
    pub fn url(&self) -> String {
        match self { Self::Codeforces(id) => format!("https://codeforces.com/contest/{id}"), Self::LeetCode(slug) => format!("https://leetcode.com/contest/{slug}/") }
    }
    pub fn key(&self) -> String { match self { Self::Codeforces(id) => format!("cf:{id}"), Self::LeetCode(slug) => format!("lc:{slug}") } }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LeetCodeContest {
    pub title: String,
    pub title_slug: String,
    pub start_time: u64,
    pub duration: u64,
}

pub fn valid_leetcode_slug(slug: &str) -> bool {
    ["weekly-contest-", "biweekly-contest-"].iter().any(|prefix| slug.strip_prefix(prefix).is_some_and(|number| number.bytes().all(|b| b.is_ascii_digit()) && number.parse::<u32>().is_ok_and(|id| id > 0)))
}

#[derive(Clone, Debug)]
pub enum ContestEntry { Codeforces(Contest), LeetCode(LeetCodeContest) }

impl ContestEntry {
    pub fn id(&self) -> ContestId { match self { Self::Codeforces(c) => ContestId::Codeforces(c.id), Self::LeetCode(c) => ContestId::LeetCode(c.title_slug.clone()) } }
    pub fn name(&self) -> &str { match self { Self::Codeforces(c) => &c.name, Self::LeetCode(c) => &c.title } }
    pub fn start(&self) -> u64 { match self { Self::Codeforces(c) => c.start_time_seconds.unwrap_or(u64::MAX), Self::LeetCode(c) => c.start_time } }
    pub fn past(&self, now: u64) -> bool { match self { Self::Codeforces(c) => c.past(), Self::LeetCode(c) => now >= c.start_time.saturating_add(c.duration) } }
    pub fn timing(&self, now: u64) -> String {
        match self {
            Self::Codeforces(c) => c.timing(now),
            Self::LeetCode(_) if self.past(now) => "Finished".into(),
            Self::LeetCode(c) if now >= c.start_time => "Live".into(),
            Self::LeetCode(c) => until_start(Some(c.start_time), now),
        }
    }
}

pub fn cached_leetcode_list(db: &Db) -> Result<Vec<LeetCodeContest>> {
    db.get("lc-contests:v1")?.map(|text| serde_json::from_str(&text).map_err(Into::into)).unwrap_or(Ok(vec![]))
}

pub fn refresh_leetcode_list(db: &Db) -> Result<Vec<LeetCodeContest>> {
    save_leetcode_list(db, crate::leetcode::Client::new(None).contests())
}

fn save_leetcode_list(db: &Db, result: Result<Vec<LeetCodeContest>>) -> Result<Vec<LeetCodeContest>> {
    let contests = result?;
    if contests.is_empty() || contests.iter().any(|c| c.title.is_empty() || !valid_leetcode_slug(&c.title_slug) || c.start_time == 0 || c.duration == 0) { bail!("LeetCode returned an incomplete contest list"); }
    db.set("lc-contests:v1", &serde_json::to_string(&contests)?)?;
    Ok(contests)
}

pub fn cached_entries(db: &Db) -> Result<Vec<ContestEntry>> {
    Ok(cached_list(db)?.into_iter().map(ContestEntry::Codeforces).chain(cached_leetcode_list(db)?.into_iter().map(ContestEntry::LeetCode)).collect())
}

pub fn refresh_entries(db: &Db) -> Result<(Vec<ContestEntry>, Vec<String>)> {
    let mut entries = cached_entries(db)?;
    let mut errors = Vec::new();
    match refresh_list(db) {
        Ok(list) => { entries.retain(|c| !matches!(c, ContestEntry::Codeforces(_))); entries.extend(list.into_iter().map(ContestEntry::Codeforces)); }
        Err(error) => errors.push(format!("Codeforces: {error}")),
    }
    match refresh_leetcode_list(db) {
        Ok(list) => { entries.retain(|c| !matches!(c, ContestEntry::LeetCode(_))); entries.extend(list.into_iter().map(ContestEntry::LeetCode)); }
        Err(error) => errors.push(format!("LeetCode: {error}")),
    }
    Ok((entries, errors))
}

pub enum ContestProblems { Codeforces(Vec<Problem>), LeetCode(Vec<crate::leetcode::CatalogItem>) }

pub fn cached_contest_problems(db: &Db, id: &ContestId) -> Result<ContestProblems> {
    match id {
        ContestId::Codeforces(id) => Ok(ContestProblems::Codeforces(cached_problems(db, *id)?)),
        ContestId::LeetCode(slug) => {
            if !valid_leetcode_slug(slug) { bail!("Invalid LeetCode contest slug"); }
            let list = db.get(&format!("lc-contest-problems:v1:{slug}"))?.map(|text| serde_json::from_str(&text)).transpose()?.unwrap_or_default();
            Ok(ContestProblems::LeetCode(list))
        }
    }
}

pub fn refresh_contest_problems(db: &Db, id: &ContestId, client: &crate::leetcode::Client) -> Result<ContestProblems> {
    match id {
        ContestId::Codeforces(id) => Ok(ContestProblems::Codeforces(refresh_problems(db, *id)?)),
        ContestId::LeetCode(slug) => {
            let slugs = client.contest_problem_slugs(slug)?;
            if slugs.is_empty() { bail!("Contest problems are not published yet"); }
            let questions = client.question_batch(&slugs)?.into_iter().collect::<Result<Vec<_>>>()?;
            let mut items = Vec::new();
            for (question, slug) in questions.into_iter().zip(slugs) {
                if question.slug != slug || question.title.is_empty() || question.content.is_empty() { bail!("LeetCode returned an incomplete contest problem"); }
                let frontend_id = question.frontend_id.parse()?;
                let level = match question.difficulty.as_str() { "Easy" => 1, "Medium" => 2, "Hard" => 3, _ => bail!("LeetCode returned an unknown difficulty") };
                db.save_question(&question)?;
                items.push(crate::leetcode::CatalogItem { slug: question.slug, frontend_id, title: question.title, level, paid_only: question.paid_only, ac_rate: 0., status: None });
            }
            db.set(&format!("lc-contest-problems:v1:{slug}"), &serde_json::to_string(&items)?)?;
            Ok(ContestProblems::LeetCode(items))
        }
    }
}

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
        until_start(self.start_time_seconds, now)
    }
}

fn until_start(start: Option<u64>, now: u64) -> String {
    let Some(start) = start else { return "Scheduled".into(); };
    let minutes = start.saturating_sub(now).div_ceil(60);
    if minutes == 0 { "Starting".into() }
    else if minutes < 60 { format!("In {minutes}m") }
    else if minutes < 1440 { format!("In {}h {}m", minutes / 60, minutes % 60) }
    else { format!("In {}d {}h", minutes / 1440, minutes % 1440 / 60) }
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
    fn leetcode_cache_survives_failed_or_invalid_refresh_and_tracks_time() {
        let dir = tempfile::tempdir().unwrap(); let path = dir.path().join("contests.db");
        let db = Db::open(&path).unwrap();
        let contest = LeetCodeContest { title: "Weekly Contest 1".into(), title_slug: "weekly-contest-1".into(), start_time: 100, duration: 90 };
        save_leetcode_list(&db, Ok(vec![contest.clone()])).unwrap();
        assert!(save_leetcode_list(&db, Err(anyhow::anyhow!("offline"))).is_err());
        assert!(save_leetcode_list(&db, Ok(vec![])).is_err());
        assert!(save_leetcode_list(&db, Ok(vec![LeetCodeContest { title_slug: "weekly-contest-1/../../".into(), ..contest.clone() }])).is_err());
        drop(db); let db = Db::open(&path).unwrap();
        let entries = cached_entries(&db).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].timing(40), "In 1m");
        assert_eq!(entries[0].timing(100), "Live");
        assert_eq!(entries[0].timing(189), "Live");
        assert_eq!(entries[0].timing(190), "Finished");
        assert_ne!(entries[0].id().key(), ContestId::Codeforces(1).key());
        assert_eq!(entries[0].id().url(), "https://leetcode.com/contest/weekly-contest-1/");
        assert!(!valid_leetcode_slug("weekly-contest-0"));
        assert!(!valid_leetcode_slug("weekly-contest-+1"));
    }

    #[test]
    #[ignore = "downloads public LeetCode contests and problems"]
    fn live_leetcode_contests_and_problem_cache() {
        let dir = tempfile::tempdir().unwrap(); let db = Db::open(&dir.path().join("contests.db")).unwrap();
        let contests = refresh_leetcode_list(&db).unwrap();
        assert!(contests.iter().any(|c| c.title_slug.starts_with("weekly-contest-")));
        assert!(contests.iter().any(|c| c.title_slug.starts_with("biweekly-contest-")));
        let id = ContestId::LeetCode("weekly-contest-522".into());
        let ContestProblems::LeetCode(items) = refresh_contest_problems(&db, &id, &crate::leetcode::Client::new(None)).unwrap() else { panic!("wrong provider"); };
        assert_eq!(items.len(), 4);
        for item in &items { assert!(db.question(&item.slug).unwrap().is_some()); }
        let ContestProblems::LeetCode(cached) = cached_contest_problems(&db, &id).unwrap() else { panic!("wrong provider"); };
        assert_eq!(cached, items);
        let other = ContestId::LeetCode("weekly-contest-521".into());
        let ContestProblems::LeetCode(cached) = cached_contest_problems(&db, &other).unwrap() else { panic!("wrong provider"); };
        assert!(cached.is_empty());
    }

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
