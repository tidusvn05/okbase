//! SQLite schema v1.

/// Bumped whenever the schema or the meaning of stored data changes; a mismatch triggers a full rebuild.
pub const SCHEMA_VERSION: i64 = 1;

pub(crate) const CREATE: &str = r#"
CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);

-- One row per markdown file, reserved files (index.md, log.md) included.
CREATE TABLE docs (
    id                TEXT PRIMARY KEY,
    path              TEXT NOT NULL,
    hash              TEXT NOT NULL,       -- blake3 of the file bytes
    size              INTEGER NOT NULL,
    mtime_ns          INTEGER NOT NULL,
    reserved          INTEGER NOT NULL,    -- 1 for index.md / log.md
    fm_state          TEXT NOT NULL,       -- absent | unterminated | invalid | valid
    type              TEXT,
    title             TEXT NOT NULL,       -- mapped title, else first H1, else file name
    title_from        TEXT NOT NULL,       -- frontmatter key, 'h1' or 'filename'
    description       TEXT,
    description_from  TEXT,
    lang              TEXT,
    status            TEXT,
    updated           TEXT,
    tokens            INTEGER NOT NULL,    -- estimated tokens of the whole file
    frontmatter       TEXT NOT NULL,       -- parsed frontmatter as ordered JSON ('{}' if none)
    fm_raw            TEXT NOT NULL,       -- frontmatter block as written, delimiters included
    body              TEXT NOT NULL
);

-- Top-level frontmatter fields, one row per scalar (list items get idx 0..n).
CREATE TABLE doc_fields (
    doc_id      TEXT NOT NULL REFERENCES docs(id) ON DELETE CASCADE,
    key         TEXT NOT NULL,
    idx         INTEGER NOT NULL,
    value_type  TEXT NOT NULL,             -- string | number | bool | null | json
    value_text  TEXT,                      -- strings, bools, numbers as text, JSON for objects
    value_num   REAL                       -- numbers only
);
CREATE INDEX doc_fields_key ON doc_fields(key, value_text);
CREATE INDEX doc_fields_doc ON doc_fields(doc_id);

CREATE TABLE doc_tags (
    doc_id     TEXT NOT NULL REFERENCES docs(id) ON DELETE CASCADE,
    tag        TEXT NOT NULL,              -- as written (tags, or categories when mapped)
    norm       TEXT NOT NULL,              -- normalize_tag(tag)
    folded     TEXT NOT NULL,              -- fold(norm): accent-insensitive
    canonical  TEXT                        -- vocabulary tag, if the tag or a synonym is known
);
CREATE INDEX doc_tags_doc ON doc_tags(doc_id);
CREATE INDEX doc_tags_folded ON doc_tags(folded);
CREATE INDEX doc_tags_canonical ON doc_tags(canonical);

CREATE TABLE aliases (
    doc_id  TEXT NOT NULL REFERENCES docs(id) ON DELETE CASCADE,
    alias   TEXT NOT NULL,
    folded  TEXT NOT NULL
);
CREATE INDEX aliases_doc ON aliases(doc_id);
CREATE INDEX aliases_folded ON aliases(folded);

CREATE TABLE links (
    src     TEXT NOT NULL REFERENCES docs(id) ON DELETE CASCADE,
    raw     TEXT NOT NULL,                 -- target as written
    target  TEXT,                          -- resolved concept ID (may not exist: broken link)
    kind    TEXT NOT NULL,                 -- markdown | wiki
    text    TEXT NOT NULL,
    line    INTEGER NOT NULL
);
CREATE INDEX links_src ON links(src);
CREATE INDEX links_target ON links(target);

CREATE TABLE chunks (
    id          INTEGER PRIMARY KEY,
    doc_id      TEXT NOT NULL REFERENCES docs(id) ON DELETE CASCADE,
    ord         INTEGER NOT NULL,
    heading     TEXT NOT NULL,
    text        TEXT NOT NULL,
    start_line  INTEGER NOT NULL,
    end_line    INTEGER NOT NULL,
    tokens      INTEGER NOT NULL
);
CREATE INDEX chunks_doc ON chunks(doc_id, ord);

-- Analyzed terms of `title > heading` plus the chunk text; rowid = chunks.id.
CREATE VIRTUAL TABLE chunks_fts USING fts5(terms, tokenize = 'unicode61 remove_diacritics 0');
"#;
