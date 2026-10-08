CREATE TABLE chat_threads (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    problem TEXT,
    title TEXT NOT NULL,
    updated_at BIGINT NOT NULL,
    draft TEXT NOT NULL DEFAULT '',
    fork_of BIGINT
);
CREATE INDEX chat_threads_scope ON chat_threads (problem, updated_at DESC);
CREATE TABLE chat_messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    thread_id BIGINT NOT NULL,
    body TEXT NOT NULL
);
CREATE INDEX chat_messages_thread ON chat_messages (thread_id, id);
