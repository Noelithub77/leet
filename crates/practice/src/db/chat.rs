//! Transactional private conversation storage. Public bundle imports never copy these tables.

use super::*;
use crate::chat::history::{Message, SavedTurn, Thread, Undo};

diesel::table! {
    chat_threads (id) {
        id -> BigInt,
        problem -> Nullable<Text>,
        title -> Text,
        updated_at -> BigInt,
        draft -> Text,
        fork_of -> Nullable<BigInt>,
    }
}
diesel::table! {
    chat_messages (id) {
        id -> BigInt,
        thread_id -> BigInt,
        body -> Text,
    }
}

#[derive(Queryable, Selectable)]
#[diesel(table_name = chat_threads)]
struct ThreadRow { id: i64, problem: Option<String>, title: String, updated_at: i64, draft: String, fork_of: Option<i64> }
impl From<ThreadRow> for Thread {
    fn from(row: ThreadRow) -> Self { Self { id: row.id, problem: row.problem, title: row.title, updated_at: row.updated_at, draft: row.draft, fork_of: row.fork_of } }
}

fn thread(conn: &mut SqliteConnection, id: i64) -> Result<Thread> {
    Ok(chat_threads::table.find(id).select(ThreadRow::as_select()).first::<ThreadRow>(conn)?.into())
}
fn messages(conn: &mut SqliteConnection, id: i64) -> Result<Vec<Message>> {
    let rows: Vec<(i64, String)> = chat_messages::table.filter(chat_messages::thread_id.eq(id)).order(chat_messages::id.asc()).select((chat_messages::id, chat_messages::body)).load(conn)?;
    rows.into_iter().map(|(message_id, body)| Ok(Message { id: message_id, thread_id: id, turn: serde_json::from_str(&body)? })).collect()
}
fn insert_thread(conn: &mut SqliteConnection, problem: Option<&str>, title: &str, fork: Option<i64>) -> Result<Thread> {
    let row = diesel::insert_into(chat_threads::table).values((chat_threads::problem.eq(problem), chat_threads::title.eq(title), chat_threads::updated_at.eq(now()), chat_threads::fork_of.eq(fork)))
        .returning(ThreadRow::as_returning()).get_result::<ThreadRow>(conn)?;
    Ok(row.into())
}
fn insert_message(conn: &mut SqliteConnection, thread_id: i64, turn: &SavedTurn) -> Result<i64> {
    Ok(diesel::insert_into(chat_messages::table).values((chat_messages::thread_id.eq(thread_id), chat_messages::body.eq(serde_json::to_string(turn)?)))
        .returning(chat_messages::id).get_result(conn)?)
}
fn remove_thread(conn: &mut SqliteConnection, id: i64) -> Result<()> {
    diesel::delete(chat_messages::table.filter(chat_messages::thread_id.eq(id))).execute(conn)?;
    diesel::delete(chat_threads::table.find(id)).execute(conn)?;
    Ok(())
}
fn set_undo(conn: &mut SqliteConnection, undo: Undo) -> Result<()> {
    diesel::replace_into(kv::table).values((kv::key.eq("chat:undo"), kv::value.eq(serde_json::to_string(&undo)?))).execute(conn)?;
    Ok(())
}

impl Db {
    /// Convert all previous per-problem histories atomically, including legacy answer-only turns.
    pub fn migrate_chats(&self) -> Result<()> {
        self.conn().transaction::<_, anyhow::Error, _>(|conn| {
            let legacy: Vec<(String, String)> = kv::table.filter(kv::key.like("assist:%")).select((kv::key, kv::value)).load(conn)?;
            for (key, body) in legacy {
                let mut turns: Vec<SavedTurn> = serde_json::from_str(&body).map_err(|error| anyhow!("Cannot migrate {key}: {error}"))?;
                if !turns.is_empty() {
                    let slug = key.strip_prefix("assist:").ok_or_else(|| anyhow!("Invalid conversation key"))?;
                    let thread = insert_thread(conn, Some(slug), "Previous conversation", None)?;
                    turns.reverse();
                    for mut turn in turns {
                        turn.problem = slug.to_owned();
                        insert_message(conn, thread.id, &turn)?;
                    }
                }
                diesel::delete(kv::table.find(&key)).execute(conn)?;
            }
            Ok(())
        })
    }

    pub fn chat_threads(&self, problem: Option<&str>) -> Result<Vec<Thread>> {
        let mut query = chat_threads::table.into_boxed();
        query = if let Some(problem) = problem { query.filter(chat_threads::problem.eq(problem)) } else { query.filter(chat_threads::problem.is_null()) };
        Ok(query.order((chat_threads::updated_at.desc(), chat_threads::id.desc())).select(ThreadRow::as_select()).load::<ThreadRow>(&mut *self.conn())?.into_iter().map(Into::into).collect())
    }
    /// Only chats belonging to the open problem are visible; older Root records stay private.
    pub fn visible_chat_threads(&self, problem: Option<&str>, query: &str) -> Result<Vec<Thread>> {
        match problem { Some(problem) => self.search_chat_threads(Some(problem), query), None => Ok(vec![]) }
    }
    pub fn search_chat_threads(&self, problem: Option<&str>, query: &str) -> Result<Vec<Thread>> {
        if query.trim().is_empty() { return self.chat_threads(problem); }
        #[derive(QueryableByName)]
        struct SearchRow {
            #[diesel(sql_type = diesel::sql_types::BigInt)] id: i64,
            #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Text>)] problem: Option<String>,
            #[diesel(sql_type = diesel::sql_types::Text)] title: String,
            #[diesel(sql_type = diesel::sql_types::BigInt)] updated_at: i64,
            #[diesel(sql_type = diesel::sql_types::Text)] draft: String,
            #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::BigInt>)] fork_of: Option<i64>,
        }
        let escaped = query.trim().replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_");
        let pattern = format!("%{escaped}%");
        let rows = diesel::sql_query(r"SELECT t.* FROM chat_threads t WHERE t.problem IS ? AND
            (t.title LIKE ? ESCAPE '\' OR EXISTS (SELECT 1 FROM chat_messages m WHERE m.thread_id = t.id AND
                (json_extract(m.body, '$.instructions') LIKE ? ESCAPE '\'
                OR json_extract(m.body, '$.request_prompt') LIKE ? ESCAPE '\'
                OR json_extract(m.body, '$.problem_title') LIKE ? ESCAPE '\'
                OR EXISTS (SELECT 1 FROM json_tree(m.body, '$.answer.answer') a WHERE a.type = 'text' AND a.atom LIKE ? ESCAPE '\'))))
            ORDER BY t.updated_at DESC, t.id DESC")
            .bind::<diesel::sql_types::Nullable<diesel::sql_types::Text>, _>(problem)
            .bind::<diesel::sql_types::Text, _>(&pattern).bind::<diesel::sql_types::Text, _>(&pattern)
            .bind::<diesel::sql_types::Text, _>(&pattern).bind::<diesel::sql_types::Text, _>(&pattern)
            .bind::<diesel::sql_types::Text, _>(&pattern)
            .load::<SearchRow>(&mut *self.conn())?;
        Ok(rows.into_iter().map(|row| Thread { id: row.id, problem: row.problem, title: row.title, updated_at: row.updated_at, draft: row.draft, fork_of: row.fork_of }).collect())
    }
    /// Latest native review for a problem, independent of which thread is selected.
    pub fn latest_chat_review(&self, problem: &str) -> Result<Option<Message>> {
        #[derive(QueryableByName)]
        struct ReviewRow {
            #[diesel(sql_type = diesel::sql_types::BigInt)] id: i64,
            #[diesel(sql_type = diesel::sql_types::BigInt)] thread_id: i64,
            #[diesel(sql_type = diesel::sql_types::Text)] body: String,
        }
        let row = diesel::sql_query("SELECT id, thread_id, body FROM chat_messages WHERE json_extract(body, '$.problem') = ? AND json_extract(body, '$.answer.action') = 'review' ORDER BY id DESC LIMIT 1")
            .bind::<diesel::sql_types::Text, _>(problem).get_result::<ReviewRow>(&mut *self.conn()).optional()?;
        row.map(|row| Ok(Message { id: row.id, thread_id: row.thread_id, turn: serde_json::from_str(&row.body)? })).transpose()
    }
    pub fn chat_thread(&self, id: i64) -> Result<Thread> { thread(&mut self.conn(), id) }
    pub fn chat_messages(&self, id: i64) -> Result<Vec<Message>> { messages(&mut self.conn(), id) }
    pub fn create_chat(&self, problem: Option<&str>, title: &str) -> Result<Thread> { insert_thread(&mut self.conn(), problem, title, None) }
    pub fn save_chat_draft(&self, id: i64, draft: &str) -> Result<()> {
        diesel::update(chat_threads::table.find(id)).set(chat_threads::draft.eq(draft)).execute(&mut *self.conn())?; Ok(())
    }
    pub fn rename_chat(&self, id: i64, title: &str) -> Result<()> {
        diesel::update(chat_threads::table.find(id)).set(chat_threads::title.eq(title)).execute(&mut *self.conn())?; Ok(())
    }
    pub fn append_chat(&self, id: i64, turn: &SavedTurn) -> Result<i64> {
        self.conn().transaction::<_, anyhow::Error, _>(|conn| {
            thread(conn, id)?;
            let message = insert_message(conn, id, turn)?;
            diesel::update(chat_threads::table.find(id)).set(chat_threads::updated_at.eq(now())).execute(conn)?;
            Ok(message)
        })
    }
    pub fn save_chat_message(&self, id: i64, turn: &SavedTurn) -> Result<()> {
        let affected = diesel::update(chat_messages::table.find(id)).set(chat_messages::body.eq(serde_json::to_string(turn)?)).execute(&mut *self.conn())?;
        if affected == 0 { return Err(anyhow!("Conversation message no longer exists")); }
        Ok(())
    }
    /// Fork through a message; editing excludes that message and all subsequent replies.
    pub fn fork_chat(&self, id: i64, message: i64, include: bool) -> Result<Thread> {
        self.conn().transaction::<_, anyhow::Error, _>(|conn| {
            let source = thread(conn, id)?;
            let turns = messages(conn, id)?;
            let index = turns.iter().position(|turn| turn.id == message).ok_or_else(|| anyhow!("Message no longer exists"))?;
            let fork = insert_thread(conn, source.problem.as_deref(), &format!("{} · branch", source.title), Some(id))?;
            for turn in turns.into_iter().take(index + usize::from(include)) { insert_message(conn, fork.id, &turn.turn)?; }
            set_undo(conn, Undo::RemoveThread { id: fork.id, previous: Some(id) })?;
            Ok(fork)
        })
    }
    pub fn delete_chat(&self, id: i64) -> Result<()> {
        self.conn().transaction::<_, anyhow::Error, _>(|conn| {
            let undo = Undo::RestoreThread { thread: thread(conn, id)?, messages: messages(conn, id)? };
            remove_thread(conn, id)?;
            set_undo(conn, undo)
        })
    }
    pub fn can_undo_chat(&self) -> bool { self.get("chat:undo").ok().flatten().is_some() }
    pub fn undo_chat(&self) -> Result<Option<i64>> {
        self.conn().transaction::<_, anyhow::Error, _>(|conn| {
            let saved: Option<String> = kv::table.find("chat:undo").select(kv::value).first(conn).optional()?;
            let Some(saved) = saved else { return Ok(None) };
            let selected = match serde_json::from_str::<Undo>(&saved)? {
                Undo::RemoveThread { id, previous } => { remove_thread(conn, id)?; previous }
                Undo::RestoreThread { thread, messages } => {
                    diesel::insert_into(chat_threads::table).values((chat_threads::id.eq(thread.id), chat_threads::problem.eq(thread.problem), chat_threads::title.eq(thread.title), chat_threads::updated_at.eq(thread.updated_at), chat_threads::draft.eq(thread.draft), chat_threads::fork_of.eq(thread.fork_of))).execute(conn)?;
                    for message in messages {
                        diesel::insert_into(chat_messages::table).values((chat_messages::id.eq(message.id), chat_messages::thread_id.eq(thread.id), chat_messages::body.eq(serde_json::to_string(&message.turn)?))).execute(conn)?;
                    }
                    Some(thread.id)
                }
            };
            diesel::delete(kv::table.find("chat:undo")).execute(conn)?;
            Ok(selected)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assist::{Action, Answer};
    fn turn(problem: &str, text: &str) -> SavedTurn {
        SavedTurn { model: "model".into(), agent: None, answer: Some(Answer::Chat(text.into())), instructions: text.into(), request_prompt: String::new(), action: Some(Action::Ask), phase: Default::default(), problem: problem.into(), problem_title: problem.into() }
    }
    #[test]
    fn migration_is_atomic_idempotent_and_preserves_old_answers() {
        let dir = tempfile::tempdir().unwrap(); let db = Db::open_unseeded(&dir.path().join("test.db")).unwrap();
        db.set("assist:two-sum", r#"[{"model":"old","agent":null,"answer":{"action":"chat","answer":"Old answer"}}]"#).unwrap();
        db.set("assist:broken", "invalid").unwrap();
        assert!(db.migrate_chats().is_err()); assert!(db.chat_threads(Some("two-sum")).unwrap().is_empty());
        db.set("assist:broken", "[]").unwrap(); db.migrate_chats().unwrap(); db.migrate_chats().unwrap();
        let threads = db.chat_threads(Some("two-sum")).unwrap(); assert_eq!(threads.len(), 1);
        let saved = db.chat_messages(threads[0].id).unwrap(); assert_eq!(saved[0].turn.action(), Action::Ask); assert_eq!(saved[0].turn.problem, "two-sum");
        assert!(db.get("assist:two-sum").unwrap().is_none());
    }
    #[test]
    fn chat_search_finds_message_content_and_treats_wildcards_literally() {
        let dir = tempfile::tempdir().unwrap(); let db = Db::open_unseeded(&dir.path().join("test.db")).unwrap();
        let local = db.create_chat(Some("two-sum"), "Lookup").unwrap(); let root = db.create_chat(None, "Shared notes").unwrap();
        db.append_chat(local.id, &turn("two-sum", "Use a complement map with 100% certainty")).unwrap();
        db.append_chat(root.id, &turn("two-sum", "Global complement")).unwrap();
        assert_eq!(db.search_chat_threads(Some("two-sum"), "complement").unwrap()[0].id, local.id);
        assert_eq!(db.search_chat_threads(None, "complement").unwrap()[0].id, root.id);
        let visible = db.visible_chat_threads(Some("two-sum"), "complement").unwrap();
        assert_eq!(visible.iter().map(|thread| thread.id).collect::<Vec<_>>(), vec![local.id]);
        assert!(db.visible_chat_threads(Some("other"), "complement").unwrap().is_empty());
        assert!(db.visible_chat_threads(None, "complement").unwrap().is_empty());
        assert_eq!(db.search_chat_threads(Some("two-sum"), "%").unwrap().len(), 1);
        assert!(db.search_chat_threads(Some("two-sum"), "_").unwrap().is_empty());
        assert!(db.search_chat_threads(Some("other"), "complement").unwrap().is_empty());
    }
    #[test]
    fn scopes_forks_deletion_and_undo_survive_reopening() {
        let dir = tempfile::tempdir().unwrap(); let path = dir.path().join("test.db"); let db = Db::open_unseeded(&path).unwrap();
        let local = db.create_chat(Some("two-sum"), "Lookup").unwrap(); let root = db.create_chat(None, "Patterns").unwrap();
        db.append_chat(root.id, &turn("two-sum", "global")).unwrap();
        let first = db.append_chat(local.id, &turn("two-sum", "first")).unwrap(); let second = db.append_chat(local.id, &turn("two-sum", "second")).unwrap();
        let fork = db.fork_chat(local.id, second, false).unwrap(); assert_eq!(db.chat_messages(fork.id).unwrap().len(), 1);
        assert_eq!(db.chat_messages(local.id).unwrap().len(), 2); assert_eq!(db.undo_chat().unwrap(), Some(local.id)); assert!(db.chat_thread(fork.id).is_err());
        let fork = db.fork_chat(local.id, first, true).unwrap(); assert_eq!(db.chat_messages(fork.id).unwrap().len(), 1);
        db.save_chat_draft(local.id, "draft").unwrap(); db.delete_chat(local.id).unwrap(); drop(db);
        let db = Db::open_unseeded(&path).unwrap(); assert!(db.chat_thread(local.id).is_err()); assert_eq!(db.undo_chat().unwrap(), Some(local.id));
        assert_eq!(db.chat_thread(local.id).unwrap().draft, "draft"); assert_eq!(db.chat_messages(local.id).unwrap().len(), 2);
        assert_eq!(db.chat_threads(None).unwrap().len(), 1); assert_eq!(db.chat_threads(Some("other")).unwrap().len(), 0);
        assert_eq!(db.chat_messages(root.id).unwrap()[0].turn.instructions, "global");
    }
}
