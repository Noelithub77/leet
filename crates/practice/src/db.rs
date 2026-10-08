//! SQLite cache (Diesel). The UI keeps the catalog in memory; this persists it between launches.

use std::path::Path;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Result, anyhow};
use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};

use crate::leetcode::{CatalogItem, Question};

mod chat;

const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

mod schema {
    diesel::table! {
        problems (slug) {
            slug -> Text,
            frontend_id -> Integer,
            title -> Text,
            level -> Integer,
            paid_only -> Bool,
            ac_rate -> Float,
        }
    }
    diesel::table! {
        questions (slug) {
            slug -> Text,
            body -> Text,
            fetched_at -> BigInt,
        }
    }
    diesel::table! {
        progress (slug) {
            slug -> Text,
            solved -> Bool,
            updated_at -> BigInt,
        }
    }
    diesel::table! {
        custom_tests (id) {
            id -> Integer,
            slug -> Text,
            input -> Text,
            expected -> Text,
        }
    }
    diesel::table! {
        kv (key) {
            key -> Text,
            value -> Text,
        }
    }
}

use schema::*;

#[derive(Queryable, Insertable, Selectable)]
#[diesel(table_name = problems)]
struct ProblemRow {
    slug: String,
    frontend_id: i32,
    title: String,
    level: i32,
    paid_only: bool,
    ac_rate: f32,
}

#[derive(Clone, Debug, Queryable, Selectable)]
#[diesel(table_name = custom_tests)]
pub struct CustomTest {
    pub id: i32,
    pub slug: String,
    pub input: String,
    pub expected: String,
}

pub struct Db(Mutex<SqliteConnection>);

fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

impl Db {
    pub fn open(path: &Path) -> Result<Self> { Self::open_internal(path, true) }

    /// Snapshot builders never import the already bundled data into their output.
    pub fn open_unseeded(path: &Path) -> Result<Self> { Self::open_internal(path, false) }

    fn open_internal(path: &Path, seed: bool) -> Result<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let mut conn = SqliteConnection::establish(&path.to_string_lossy())?;
        diesel::sql_query("PRAGMA journal_mode = WAL").execute(&mut conn)?;
        diesel::sql_query("PRAGMA synchronous = NORMAL").execute(&mut conn)?;
        diesel::sql_query("PRAGMA busy_timeout = 5000").execute(&mut conn)?;
        conn.run_pending_migrations(MIGRATIONS).map_err(|e| anyhow!("migrate: {e}"))?;
        if seed { Self::seed(path, &mut conn)?; }
        Ok(Self(Mutex::new(conn)))
    }

    fn seed(path: &Path, conn: &mut SqliteConnection) -> Result<()> {
        use std::hash::{Hash, Hasher};
        const BASE: &[u8] = include_bytes!("../data/base.sqlite");
        let mut hash = std::collections::hash_map::DefaultHasher::new(); BASE.hash(&mut hash);
        let version = format!("{:016x}", hash.finish());
        let previous: Option<String> = kv::table.find("bundle-version").select(kv::value).first(conn).optional()?;
        if previous.as_deref() == Some(&version) { return Ok(()); }
        let bundle = path.with_file_name(format!("base-{version}.sqlite"));
        if !bundle.exists() {
            let temp = bundle.with_extension(format!("{}.tmp", std::process::id()));
            std::fs::write(&temp, BASE)?; std::fs::rename(temp, &bundle)?;
        }
        diesel::sql_query("ATTACH DATABASE ? AS bundled").bind::<diesel::sql_types::Text, _>(bundle.to_string_lossy()).execute(conn)?;
        let imported = conn.transaction(|conn| {
            diesel::sql_query("INSERT OR IGNORE INTO questions SELECT * FROM bundled.questions").execute(conn)?;
            diesel::sql_query("INSERT OR IGNORE INTO problems SELECT * FROM bundled.problems").execute(conn)?;
            diesel::sql_query("INSERT OR IGNORE INTO kv SELECT * FROM bundled.kv WHERE key IN ('cf-catalog','cc-catalog','reference-index') OR key LIKE 'article:%' OR key LIKE 'reference:%' OR key LIKE 'editorial:%'").execute(conn)?;
            diesel::replace_into(kv::table).values((kv::key.eq("bundle-version"), kv::value.eq(&version))).execute(conn)?;
            diesel::QueryResult::Ok(())
        });
        diesel::sql_query("DETACH DATABASE bundled").execute(conn)?;
        imported?; Ok(())
    }

    /// Only snapshot tooling calls this; user caches are never pruned on bundle import.
    pub fn remove_codeforces_snapshot_content(&self) -> Result<()> {
        let mut conn = self.conn();
        conn.transaction::<_, anyhow::Error, _>(|conn| {
            diesel::delete(questions::table.filter(questions::slug.like("cf:%"))).execute(conn)?;
            diesel::delete(kv::table.filter(kv::key.like("editorial:cf:%"))).execute(conn)?;
            Ok(())
        })
    }

    pub fn retain_snapshot_questions(&self, slugs: &[String]) -> Result<()> {
        diesel::delete(questions::table.filter(questions::slug.ne_all(slugs))).execute(&mut *self.conn())?;
        Ok(())
    }

    pub fn compact_snapshot(&self) -> Result<()> {
        diesel::sql_query("VACUUM").execute(&mut *self.conn())?;
        self.checkpoint()
    }

    pub fn checkpoint(&self) -> Result<()> {
        diesel::sql_query("PRAGMA wal_checkpoint(TRUNCATE)").execute(&mut *self.conn())?;
        Ok(())
    }

    fn conn(&self) -> std::sync::MutexGuard<'_, SqliteConnection> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn catalog(&self) -> Result<Vec<CatalogItem>> {
        let rows: Vec<ProblemRow> = problems::table
            .order(problems::frontend_id.asc())
            .select(ProblemRow::as_select())
            .load(&mut *self.conn())?;
        Ok(rows
            .into_iter()
            .map(|r| CatalogItem {
                slug: r.slug,
                frontend_id: r.frontend_id as u32,
                title: r.title,
                level: r.level as u8,
                paid_only: r.paid_only,
                ac_rate: r.ac_rate,
                status: None,
            })
            .collect())
    }

    /// Replaces the catalog and, for a signed-in fetch, merges judge-solved marks.
    pub fn replace_catalog(&self, items: &[CatalogItem]) -> Result<()> {
        let rows: Vec<ProblemRow> = items
            .iter()
            .map(|i| ProblemRow {
                slug: i.slug.clone(),
                frontend_id: i.frontend_id as i32,
                title: i.title.clone(),
                level: i.level as i32,
                paid_only: i.paid_only,
                ac_rate: i.ac_rate,
            })
            .collect();
        let solved: Vec<&str> = items
            .iter()
            .filter(|i| i.status.as_deref() == Some("ac"))
            .map(|i| i.slug.as_str())
            .collect();
        self.conn().transaction(|conn| {
            diesel::delete(problems::table).execute(conn)?;
            for chunk in rows.chunks(500) {
                diesel::insert_into(problems::table).values(chunk).execute(conn)?;
            }
            for slug in solved {
                diesel::insert_into(progress::table)
                    .values((progress::slug.eq(slug), progress::solved.eq(true), progress::updated_at.eq(now())))
                    .on_conflict(progress::slug)
                    .do_update()
                    .set((progress::solved.eq(true), progress::updated_at.eq(now())))
                    .execute(conn)?;
            }
            diesel::QueryResult::Ok(())
        })?;
        Ok(())
    }

    pub fn question_slugs(&self) -> Result<Vec<String>> { Ok(questions::table.select(questions::slug).load(&mut *self.conn())?) }

    pub fn question(&self, slug: &str) -> Result<Option<Question>> {
        let body: Option<String> = questions::table
            .find(slug)
            .select(questions::body)
            .first(&mut *self.conn())
            .optional()?;
        Ok(body.and_then(|b| serde_json::from_str(&b).ok()))
    }

    pub fn save_question(&self, q: &Question) -> Result<()> {
        let body = serde_json::to_string(q)?;
        diesel::replace_into(questions::table)
            .values((questions::slug.eq(&q.slug), questions::body.eq(body), questions::fetched_at.eq(now())))
            .execute(&mut *self.conn())?;
        Ok(())
    }

    pub fn solved(&self) -> Result<Vec<String>> {
        Ok(progress::table
            .filter(progress::solved.eq(true))
            .select(progress::slug)
            .load(&mut *self.conn())?)
    }

    pub fn set_solved(&self, slug: &str, solved: bool) -> Result<()> {
        diesel::replace_into(progress::table)
            .values((progress::slug.eq(slug), progress::solved.eq(solved), progress::updated_at.eq(now())))
            .execute(&mut *self.conn())?;
        Ok(())
    }

    pub fn custom_tests(&self, slug: &str) -> Result<Vec<CustomTest>> {
        Ok(custom_tests::table
            .filter(custom_tests::slug.eq(slug))
            .order(custom_tests::id.asc())
            .select(CustomTest::as_select())
            .load(&mut *self.conn())?)
    }

    pub fn add_custom_test(&self, slug: &str, input: &str, expected: &str) -> Result<()> {
        diesel::insert_into(custom_tests::table)
            .values((custom_tests::slug.eq(slug), custom_tests::input.eq(input), custom_tests::expected.eq(expected)))
            .execute(&mut *self.conn())?;
        Ok(())
    }

    pub fn delete_custom_test(&self, id: i32) -> Result<()> {
        diesel::delete(custom_tests::table.find(id)).execute(&mut *self.conn())?;
        Ok(())
    }

    pub fn test_cases(&self, slug: &str) -> Result<Option<Vec<crate::runner::Case>>> {
        self.get(&format!("test-cases:v1:{slug}"))?.map(|value| Ok(serde_json::from_str(&value)?)).transpose()
    }

    pub fn save_test_cases(&self, slug: &str, cases: &[crate::runner::Case]) -> Result<()> {
        self.set(&format!("test-cases:v1:{slug}"), &serde_json::to_string(cases)?)
    }

    pub fn statement_sections(&self) -> Result<[bool; 3]> {
        Ok(self.get("statement-sections:v1")?.and_then(|value| serde_json::from_str(&value).ok()).unwrap_or([true, false, false]))
    }

    pub fn save_statement_sections(&self, sections: [bool; 3]) -> Result<()> {
        self.set("statement-sections:v1", &serde_json::to_string(&sections)?)
    }

    pub fn get(&self, key: &str) -> Result<Option<String>> {
        Ok(kv::table.find(key).select(kv::value).first(&mut *self.conn()).optional()?)
    }

    pub fn set(&self, key: &str, value: &str) -> Result<()> {
        diesel::replace_into(kv::table)
            .values((kv::key.eq(key), kv::value.eq(value)))
            .execute(&mut *self.conn())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(slug: &str, id: u32, status: Option<&str>) -> CatalogItem {
        CatalogItem {
            slug: slug.into(),
            frontend_id: id,
            title: slug.into(),
            level: 1,
            paid_only: false,
            ac_rate: 50.0,
            status: status.map(Into::into),
        }
    }

    #[test]
    fn bundle_seeding_is_offline_idempotent_and_keeps_personal_state() {
        let dir = tempfile::tempdir().unwrap(); let path = dir.path().join("leet.db");
        {
            let db = Db::open_unseeded(&path).unwrap(); db.set("last", "my-question").unwrap(); db.set_solved("my-question", true).unwrap();
            db.add_custom_test("my-question", "1", "2").unwrap();
        }
        let db = Db::open(&path).unwrap();
        for entry in crate::roadmap::ENTRIES.iter().filter(|entry| entry.in_list(crate::roadmap::List::NeetCode150)) {
            let question = db.question(&entry.slug).unwrap().expect("bundled NeetCode 150 statement");
            assert!(!question.content.is_empty()); assert!(!question.python.is_empty(), "{} starter", entry.slug);
            assert!(db.get(&format!("article:neetcode:v1:{}:python", entry.slug)).unwrap().is_some());
        }
        assert!(db.question("cf:4:A").unwrap().is_none());
        let catalog: Vec<crate::codeforces::Problem> = serde_json::from_str(&db.get("cf-catalog").unwrap().unwrap()).unwrap();
        assert!(catalog.iter().any(|problem| problem.slug().as_deref() == Some("cf:4:A")));
        assert_eq!(db.get("last").unwrap().as_deref(), Some("my-question")); assert_eq!(db.solved().unwrap(), ["my-question"]);
        assert_eq!(db.custom_tests("my-question").unwrap().len(), 1);
        let mut cached = db.question("two-sum").unwrap().unwrap();
        cached.slug = "cf:4:A".into(); cached.content = "Existing user cache".into();
        db.save_question(&cached).unwrap(); db.set("bundle-version", "previous-build").unwrap();
        drop(db); let db = Db::open(&path).unwrap(); assert_eq!(db.get("last").unwrap().as_deref(), Some("my-question"));
        assert_eq!(db.question("cf:4:A").unwrap().unwrap().content, "Existing user cache");
        let codechef: Vec<crate::codechef::Problem> = serde_json::from_str(&db.get("cc-catalog").unwrap().unwrap()).unwrap();
        assert_eq!(codechef.len(), 100);
        for problem in codechef {
            let q = db.question(&problem.slug().unwrap()).unwrap().expect("Bundled CodeChef statement");
            assert!(!q.content.is_empty() && !q.examples.is_empty());
            assert_eq!(q.examples.len(), q.outputs.len());
            for language in crate::language::Language::ALL { assert!(q.starter(language).is_some()); }
        }
        assert_eq!(db.question_slugs().unwrap().len(), 251);
    }

    #[test]
    fn catalog_replace_keeps_order_and_merges_solved() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open(&dir.path().join("leet.db")).unwrap();
        db.set_solved("old", true).unwrap();
        db.replace_catalog(&[item("b", 2, Some("ac")), item("a", 1, None)]).unwrap();
        let slugs: Vec<_> = db.catalog().unwrap().into_iter().map(|i| i.slug).collect();
        assert_eq!(slugs, ["a", "b"]);
        let mut solved = db.solved().unwrap();
        solved.sort();
        assert_eq!(solved, ["b", "old"]);
    }

    #[test]
    fn edited_added_and_reset_cases_persist_without_changing_question_data() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cases.db");
        let original = crate::runner::Case { id: 0, input: "[2,7]\n9".into(), expected: Some("[0,1]".into()), custom: false };
        let db = Db::open(&path).unwrap();
        assert!(db.test_cases("two-sum").unwrap().is_none());
        let mut edited = original.clone(); edited.input = "[3,2,4]\n6".into(); edited.expected = Some("[1,2]".into());
        let added = crate::runner::Case { id: 1, input: "[1,3]\n4".into(), expected: None, custom: true };
        db.save_test_cases("two-sum", &[edited, added]).unwrap();
        drop(db);
        let db = Db::open(&path).unwrap();
        let saved = db.test_cases("two-sum").unwrap().unwrap();
        assert_eq!(saved.len(), 2); assert_eq!(saved[0].expected.as_deref(), Some("[1,2]"));
        assert_eq!(saved[1].input, "[1,3]\n4"); assert!(saved[1].custom); assert!(saved[1].expected.is_none());
        assert!(db.test_cases("valid-anagram").unwrap().is_none());
        db.save_test_cases("two-sum", &[original]).unwrap(); drop(db);
        let db = Db::open(&path).unwrap();
        let reset = db.test_cases("two-sum").unwrap().unwrap();
        assert_eq!(reset.len(), 1); assert_eq!(reset[0].input, "[2,7]\n9"); assert!(!reset[0].custom);
        assert_eq!(db.question("two-sum").unwrap().unwrap().examples[0], "[2,7,11,15]\n9");
    }

    #[test]
    fn statement_expansion_is_shared_and_survives_reopening() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sections.db");
        let db = Db::open(&path).unwrap();
        assert_eq!(db.statement_sections().unwrap(), [true, false, false]);
        db.save_statement_sections([false, true, true]).unwrap();
        assert!(db.question("two-sum").unwrap().is_some());
        assert!(db.question("valid-anagram").unwrap().is_some());
        assert_eq!(db.statement_sections().unwrap(), [false, true, true]);
        drop(db);
        let db = Db::open(&path).unwrap();
        assert_eq!(db.statement_sections().unwrap(), [false, true, true]);
        db.set("statement-sections:v1", "[true]").unwrap();
        assert_eq!(db.statement_sections().unwrap(), [true, false, false]);
    }

    #[test]
    fn custom_tests_and_kv_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open(&dir.path().join("leet.db")).unwrap();
        db.add_custom_test("two-sum", "[1,2]\n3", "[0,1]").unwrap();
        let tests = db.custom_tests("two-sum").unwrap();
        assert_eq!(tests.len(), 1);
        db.delete_custom_test(tests[0].id).unwrap();
        assert!(db.custom_tests("two-sum").unwrap().is_empty());
        db.set("last", "two-sum").unwrap();
        assert_eq!(db.get("last").unwrap().as_deref(), Some("two-sum"));
    }
}
