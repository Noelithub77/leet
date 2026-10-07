CREATE TABLE problems (
    slug TEXT PRIMARY KEY NOT NULL,
    frontend_id INTEGER NOT NULL,
    title TEXT NOT NULL,
    level INTEGER NOT NULL,
    paid_only BOOLEAN NOT NULL,
    ac_rate REAL NOT NULL
);

-- Full question JSON, fetched on first open.
CREATE TABLE questions (
    slug TEXT PRIMARY KEY NOT NULL,
    body TEXT NOT NULL,
    fetched_at BIGINT NOT NULL
);

CREATE TABLE progress (
    slug TEXT PRIMARY KEY NOT NULL,
    solved BOOLEAN NOT NULL,
    updated_at BIGINT NOT NULL
);

CREATE TABLE custom_tests (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    slug TEXT NOT NULL,
    input TEXT NOT NULL,
    expected TEXT NOT NULL
);
CREATE INDEX custom_tests_slug ON custom_tests (slug);

-- Small app state: last problem, panel layout, catalog refresh time.
CREATE TABLE kv (
    key TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL
);
