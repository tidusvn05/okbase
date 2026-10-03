//! Incremental SQLite index for okbase bundles: typed metadata, tags, aliases,
//! links and full-text chunks.
//!
//! Files are the source of truth; the index can always be rebuilt. [`Index::sync`]
//! re-reads only files whose size or modification time changed, and re-indexes
//! only those whose content hash changed.
//!
//! ```no_run
//! use okbase_index::{Index, IndexOptions};
//!
//! let mut index = Index::open("path/to/bundle".as_ref(), &IndexOptions::default())?;
//! let stats = index.sync()?;
//! println!("{} documents indexed", stats.added + stats.updated);
//! # Ok::<(), okbase_index::Error>(())
//! ```

pub mod chunk;
mod error;
pub mod schema;
mod sources;
mod state;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use okbase_analyze::{Analyzer, fold};
use okbase_core::{
    Concept, ConceptId, FrontmatterState, LinkKind, Value, discover, resolve_link, resolve_wikilink,
};
use okbase_standard::{VOCABULARY_PATH, Vocabulary, meta, normalize_tag};
use rusqlite::{Connection, OptionalExtension, Transaction, params};

pub use chunk::{Chunk, chunk_body};
pub use error::Error;
pub use okbase_analyze::estimate_tokens;
pub use schema::SCHEMA_VERSION;
pub use sources::{extensions as source_extensions, is_source, load_ocr, ocr_dir};
pub use state::{BUNDLE_STATE_DIR, StateDir, cache_dir};

/// File name of the index database inside the state directory.
pub const DB_FILE: &str = "index.sqlite";

/// Identifies the analyzer; a change forces a rebuild of the full-text index.
const ANALYZER_ID: &str = concat!("okbase-analyze/", env!("CARGO_PKG_VERSION"), "+stem+tok2");

/// Schema version, analyzer and Japanese tokenization mode (`ipadic` or `bigram`).
fn analyzer_identity(content_index: bool) -> String {
    // The profile decides whether index.md files are content: a change re-reads everything.
    format!(
        "{SCHEMA_VERSION}/{ANALYZER_ID}/{}{}",
        okbase_analyze::cjk_mode(),
        if content_index { "/content-index" } else { "" }
    )
}

/// Options for [`Index::open`].
#[derive(Debug, Clone)]
pub struct IndexOptions {
    /// Where the index lives.
    pub state_dir: StateDir,
    /// Bundle-relative path of the tag vocabulary.
    pub vocabulary: String,
}

impl Default for IndexOptions {
    fn default() -> Self {
        IndexOptions {
            state_dir: StateDir::Auto,
            vocabulary: VOCABULARY_PATH.to_owned(),
        }
    }
}

/// What a [`Index::sync`] did.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct SyncStats {
    /// New documents.
    pub added: usize,
    /// Documents whose content changed.
    pub updated: usize,
    /// Documents that no longer exist.
    pub removed: usize,
    /// Documents skipped because they did not change.
    pub unchanged: usize,
    /// Files that could not be read (path, reason). They are left out of the index.
    pub skipped: Vec<(String, String)>,
}

/// A source document with pages that have no text yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingOcr {
    /// Bundle-relative path of the source.
    pub path: String,
    /// 1-based pages still needing a transcription.
    pub pages: Vec<u32>,
    /// Pages in the document, when known.
    pub page_count: Option<u32>,
}

/// An open index for one bundle.
#[derive(Debug)]
pub struct Index {
    conn: Connection,
    root: PathBuf,
    db_path: Option<PathBuf>,
    vocabulary: String,
    /// `index.md` files are content pages (docs-site and vault profiles).
    content_index: bool,
}

impl Index {
    /// Opens (or creates) the index of the bundle at `root`. Does not sync.
    pub fn open(root: &Path, opts: &IndexOptions) -> Result<Self, Error> {
        let dir = opts.state_dir.resolve(root).map_err(|source| Error::Io {
            path: root.to_owned(),
            source,
        })?;
        let db_path = dir.join(DB_FILE);
        let conn = Connection::open(&db_path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        // Several processes (for example parallel MCP servers) may sync the same index.
        conn.busy_timeout(std::time::Duration::from_secs(30))?;
        Index::init(conn, root, Some(db_path), &opts.vocabulary)
    }

    /// Opens a throw-away in-memory index of the bundle at `root`.
    pub fn open_in_memory(root: &Path) -> Result<Self, Error> {
        Index::init(Connection::open_in_memory()?, root, None, VOCABULARY_PATH)
    }

    fn init(
        conn: Connection,
        root: &Path,
        db_path: Option<PathBuf>,
        vocabulary: &str,
    ) -> Result<Self, Error> {
        conn.pragma_update(None, "foreign_keys", true)?;
        let current: Option<String> = conn
            .query_row("SELECT value FROM meta WHERE key = 'schema'", [], |r| {
                r.get(0)
            })
            .optional()
            .or_else(|e| {
                if is_no_such_table(&e) {
                    Ok(None)
                } else {
                    Err(e)
                }
            })?;
        let content_index = okbase_core::site::profile(root).content_index();
        let wanted = analyzer_identity(content_index);
        if current.as_deref() != Some(wanted.as_str()) {
            drop_all(&conn)?;
            conn.execute_batch(schema::CREATE)?;
            conn.execute(
                "INSERT INTO meta(key, value) VALUES ('schema', ?1)",
                [&wanted],
            )?;
        }
        Ok(Index {
            conn,
            root: root.to_owned(),
            db_path,
            vocabulary: vocabulary.to_owned(),
            content_index,
        })
    }

    /// The bundle root.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Path of the database file, or `None` for an in-memory index.
    pub fn db_path(&self) -> Option<&Path> {
        self.db_path.as_deref()
    }

    /// Source documents with pages that still need a transcription.
    pub fn sources_needing_ocr(&self) -> Result<Vec<PendingOcr>, Error> {
        let mut st = self.conn.prepare(
            "SELECT path, needs_ocr, pages FROM sources WHERE status = 'partial' ORDER BY path",
        )?;
        let rows = st.query_map([], |r| {
            let pages: String = r.get(1)?;
            Ok(PendingOcr {
                path: r.get(0)?,
                pages: serde_json::from_str(&pages).unwrap_or_default(),
                page_count: r.get(2)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Forgets the stored content hash of a document so the next sync reads it again (for example
    /// after a page transcription was added for a source).
    pub fn invalidate(&self, id: &str) -> Result<(), Error> {
        self.conn
            .execute("UPDATE docs SET hash = '', size = -1 WHERE id = ?1", [id])?;
        Ok(())
    }

    /// The SQLite connection, for read queries (see `okbase-query`).
    pub fn connection(&self) -> &Connection {
        &self.conn
    }

    /// Brings the index up to date with the files on disk.
    pub fn sync(&mut self) -> Result<SyncStats, Error> {
        let mut stats = SyncStats::default();
        let mut paths = discover(&self.root)?;
        // Source documents (PDF, Word…) are read directly, unless their markdown was imported.
        let materialized: std::collections::HashSet<String> = {
            let mut st = self.conn.prepare(
                "SELECT json_extract(frontmatter, '$.source.path') FROM docs \
                 WHERE path LIKE '%.md' AND json_extract(frontmatter, '$.source.path') IS NOT NULL",
            )?;
            st.query_map([], |r| r.get::<_, String>(0))?
                .collect::<Result<_, _>>()?
        };
        if !sources::extensions().is_empty() {
            paths.extend(
                okbase_core::walk(&self.root, sources::extensions())?
                    .into_iter()
                    .filter(|p| !materialized.contains(&p.to_string_lossy().replace('\\', "/"))),
            );
        }
        let known: HashMap<String, (String, i64, i64)> = {
            let mut st = self
                .conn
                .prepare("SELECT id, hash, size, mtime_ns FROM docs")?;
            st.query_map([], |r| Ok((r.get(0)?, (r.get(1)?, r.get(2)?, r.get(3)?))))?
                .collect::<Result<_, _>>()?
        };

        // Decide what to (re)read without touching the database.
        let mut todo = Vec::new();
        let mut touched = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for rel in &paths {
            let id = if sources::is_source(rel) {
                ConceptId::new(rel.to_string_lossy().replace('\\', "/"))
            } else {
                ConceptId::from_path(rel)
            };
            let Ok(id) = id else {
                continue;
            };
            let full = self.root.join(rel);
            let (size, mtime) = match std::fs::metadata(&full) {
                Ok(m) => (m.len() as i64, mtime_ns(&m)),
                Err(e) => {
                    stats.skipped.push((id.path(), e.to_string()));
                    continue;
                }
            };
            seen.insert(id.as_str().to_owned());
            match known.get(id.as_str()) {
                Some((_, s, m)) if *s == size && *m == mtime => stats.unchanged += 1,
                prev => todo.push((rel.clone(), id, size, mtime, prev.map(|p| p.0.clone()))),
            }
        }

        let state = self
            .db_path
            .as_deref()
            .and_then(Path::parent)
            .map(Path::to_owned);
        let prepared = prepare_all(&self.root, todo, self.content_index, state.as_deref());
        let vocab = Vocabulary::load(&self.root, &self.vocabulary)
            .ok()
            .flatten();
        let tx = self.conn.transaction()?;
        for item in prepared {
            match item {
                Prepared::Failed { path, reason } => {
                    if sources::is_source(Path::new(&path)) {
                        tx.execute(
                            "INSERT OR REPLACE INTO sources (path, hash, status, detail) VALUES (?1, '', 'error', ?2)",
                            params![path, reason],
                        )?;
                    }
                    stats.skipped.push((path, reason))
                }
                Prepared::SameContent { id, size, mtime } => {
                    touched.push(id.clone());
                    tx.execute(
                        "UPDATE docs SET size = ?2, mtime_ns = ?3 WHERE id = ?1",
                        params![id, size, mtime],
                    )?;
                    stats.unchanged += 1;
                }
                Prepared::Doc(doc) => {
                    if doc.is_new {
                        stats.added += 1
                    } else {
                        stats.updated += 1
                    }
                    write_doc(&tx, &doc)?;
                }
            }
        }
        for id in known.keys().filter(|id| !seen.contains(*id)) {
            tx.execute(
                "DELETE FROM chunks_fts WHERE rowid IN (SELECT id FROM chunks WHERE doc_id = ?1)",
                [id],
            )?;
            tx.execute("DELETE FROM docs WHERE id = ?1", [id])?;
            tx.execute("DELETE FROM sources WHERE converted = ?1", [id])?;
            stats.removed += 1;
        }
        // Sources whose markdown was imported in this sync give way right away.
        tx.execute(
            "DELETE FROM chunks_fts WHERE rowid IN (SELECT c.id FROM chunks c JOIN docs d ON d.id = c.doc_id \
             WHERE d.path NOT LIKE '%.md' AND d.path IN (SELECT json_extract(frontmatter, '$.source.path') \
             FROM docs WHERE path LIKE '%.md' AND json_extract(frontmatter, '$.source.path') IS NOT NULL))",
            [],
        )?;
        let dropped = tx.execute(
            "DELETE FROM docs WHERE path NOT LIKE '%.md' AND path IN (SELECT json_extract(frontmatter, '$.source.path') \
             FROM docs WHERE path LIKE '%.md' AND json_extract(frontmatter, '$.source.path') IS NOT NULL)",
            [],
        )?;
        stats.removed += dropped;
        resolve_wikilinks(&tx)?;
        canonicalize_tags(&tx, vocab.as_ref())?;
        // The Japanese dictionary may have been installed while analyzing: documents
        // indexed earlier used bigrams, so rebuild once to keep terms consistent.
        let identity = analyzer_identity(self.content_index);
        let stored: String =
            tx.query_row("SELECT value FROM meta WHERE key = 'schema'", [], |r| {
                r.get(0)
            })?;
        let rebuild = stored != identity && stats.unchanged > 0;
        tx.execute(
            "UPDATE meta SET value = ?1 WHERE key = 'schema'",
            [&identity],
        )?;
        tx.commit()?;
        if rebuild {
            drop_all(&self.conn)?;
            self.conn.execute_batch(schema::CREATE)?;
            self.conn.execute(
                "INSERT INTO meta(key, value) VALUES ('schema', ?1)",
                [&identity],
            )?;
            let again = self.sync()?;
            return Ok(SyncStats {
                added: again.added,
                updated: 0,
                removed: 0,
                unchanged: 0,
                skipped: again.skipped,
            });
        }
        stats.skipped.sort();
        Ok(stats)
    }
}

fn is_no_such_table(e: &rusqlite::Error) -> bool {
    e.to_string().contains("no such table")
}

fn drop_all(conn: &Connection) -> rusqlite::Result<()> {
    let names: Vec<(String, String)> = {
        let mut st = conn.prepare(
            "SELECT type, name FROM sqlite_master WHERE type IN ('table', 'view') AND name NOT LIKE 'sqlite_%' \
             AND name NOT LIKE 'chunks_fts_%'",
        )?;
        st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<Result<_, _>>()?
    };
    conn.pragma_update(None, "foreign_keys", false)?;
    for (kind, name) in names {
        conn.execute_batch(&format!(
            "DROP {} IF EXISTS \"{}\";",
            kind.to_uppercase(),
            name.replace('"', "\"\"")
        ))?;
    }
    conn.pragma_update(None, "foreign_keys", true)
}

fn mtime_ns(m: &std::fs::Metadata) -> i64 {
    m.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| i64::try_from(d.as_nanos()).unwrap_or(i64::MAX))
}

enum Prepared {
    Failed { path: String, reason: String },
    SameContent { id: String, size: i64, mtime: i64 },
    Doc(Box<DocRow>),
}

struct DocRow {
    /// Conversion outcome, for a source document read directly.
    source: Option<sources::SourceInfo>,
    is_new: bool,
    /// A listing or log file rather than a concept (see the bundle profile).
    reserved: bool,
    concept: Concept,
    hash: String,
    size: i64,
    mtime: i64,
    tokens: usize,
    chunks: Vec<(Chunk, String)>,
}

/// Reads, parses, chunks and analyzes documents on all cores.
fn prepare_all(
    root: &Path,
    todo: Vec<(PathBuf, ConceptId, i64, i64, Option<String>)>,
    content_index: bool,
    state: Option<&Path>,
) -> Vec<Prepared> {
    let threads = std::thread::available_parallelism()
        .map_or(1, |n| n.get())
        .min(todo.len().max(1));
    let per = todo.len().div_ceil(threads).max(1);
    let analyzer = Analyzer::new();
    std::thread::scope(|s| {
        let handles: Vec<_> = todo
            .chunks(per)
            .map(|part| {
                s.spawn(|| {
                    part.iter()
                        .map(|t| prepare(root, t, &analyzer, content_index, state))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().expect("index worker panicked"))
            .collect()
    })
}

fn prepare(
    root: &Path,
    (rel, id, size, mtime, prev_hash): &(PathBuf, ConceptId, i64, i64, Option<String>),
    analyzer: &Analyzer,
    content_index: bool,
    state: Option<&Path>,
) -> Prepared {
    let source = sources::is_source(rel);
    let fail = |reason: String| Prepared::Failed {
        path: if source {
            rel.to_string_lossy().replace('\\', "/")
        } else {
            id.path()
        },
        reason,
    };
    let bytes = match std::fs::read(root.join(rel)) {
        Ok(b) => b,
        Err(e) => return fail(e.to_string()),
    };
    let hash = blake3::hash(&bytes).to_hex().to_string();
    if prev_hash.as_deref() == Some(hash.as_str()) {
        return Prepared::SameContent {
            id: id.as_str().to_owned(),
            size: *size,
            mtime: *mtime,
        };
    }
    let (concept, text, source_info) = if source {
        let ocr = sources::load_ocr(state, &hash);
        match sources::convert(rel, id, &bytes, &ocr) {
            Ok((c, info)) => {
                let text = c.body.clone();
                (c, text, Some(info))
            }
            Err(e) => return fail(e),
        }
    } else {
        let Ok(text) = String::from_utf8(bytes) else {
            return fail("not valid UTF-8".into());
        };
        match Concept::parse(rel, &text) {
            Ok(c) => (c, text, None),
            Err(e) => return fail(e.to_string()),
        }
    };
    let reserved = concept.is_reserved_in(content_index);
    let chunks = if reserved {
        Vec::new()
    } else {
        let title = display_title(&concept).0;
        chunk_body(&concept.body)
            .into_iter()
            .map(|c| {
                let prefix = if c.heading.is_empty() {
                    title.clone()
                } else {
                    format!("{title} > {}", c.heading)
                };
                let terms = analyzer.fts_text(&format!("{prefix}\n{}", c.text));
                (c, terms)
            })
            .collect()
    };
    Prepared::Doc(Box::new(DocRow {
        source: source_info,
        reserved,
        is_new: prev_hash.is_none(),
        tokens: estimate_tokens(&text),
        concept,
        hash,
        size: *size,
        mtime: *mtime,
        chunks,
    }))
}

/// The title to display: mapped frontmatter title, else the first `# ` heading, else the file name.
fn display_title(c: &Concept) -> (String, &'static str) {
    if let Some(t) = meta(&c.frontmatter).title {
        return (t.value, t.from);
    }
    let mut fenced = false;
    for line in c.body.lines() {
        let t = line.trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            fenced = !fenced;
        }
        if !fenced && let Some(h) = line.strip_prefix("# ") {
            let h = h.trim().trim_end_matches('#').trim();
            if !h.is_empty() {
                return (h.to_owned(), "h1");
            }
        }
    }
    (c.id.name().to_owned(), "filename")
}

fn fm_state(s: FrontmatterState) -> &'static str {
    match s {
        FrontmatterState::Absent => "absent",
        FrontmatterState::Unterminated => "unterminated",
        FrontmatterState::Invalid => "invalid",
        FrontmatterState::Valid => "valid",
    }
}

fn write_doc(tx: &Transaction, d: &DocRow) -> Result<(), Error> {
    let c = &d.concept;
    let id = c.id.as_str();
    if let Some(info) = &d.source {
        let status = if info.needs_ocr.is_empty() {
            "ok"
        } else {
            "partial"
        };
        tx.execute(
            "INSERT OR REPLACE INTO sources (path, hash, status, format, pages, needs_ocr, columns, detail, converted) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                c.path,
                d.hash,
                status,
                info.format,
                info.pages,
                serde_json::to_string(&info.needs_ocr).unwrap_or_default(),
                serde_json::to_string(&info.columns).unwrap_or_default(),
                format!("{}{}", info.converter, if info.encoding_issues { "; broken font encodings" } else { "" }),
                id
            ],
        )?;
    }
    tx.execute(
        "DELETE FROM chunks_fts WHERE rowid IN (SELECT id FROM chunks WHERE doc_id = ?1)",
        [id],
    )?;
    tx.execute("DELETE FROM docs WHERE id = ?1", [id])?;

    let fm = &c.frontmatter;
    let m = meta(fm);
    let (title, title_from) = display_title(c);
    let str_field = |k: &str| {
        fm.get_str(k)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
    };
    let json = fm
        .mapping()
        .map_or_else(|| "{}".to_owned(), |m| Value::Object(m.clone()).to_string());
    tx.execute(
        "INSERT INTO docs (id, path, hash, size, mtime_ns, reserved, fm_state, type, title, title_from, description, \
         description_from, lang, status, updated, tokens, frontmatter, fm_raw, body) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)",
        params![
            id,
            c.path,
            d.hash,
            d.size,
            d.mtime,
            d.reserved,
            fm_state(fm.state()),
            m.concept_type,
            title,
            title_from,
            m.description.as_ref().map(|x| &x.value),
            m.description.as_ref().map(|x| x.from),
            str_field("lang"),
            str_field("status"),
            fm.get("updated").and_then(scalar_text),
            d.tokens as i64,
            json,
            fm.render(),
            c.body,
        ],
    )?;

    if let Some(map) = fm.mapping() {
        let mut st = tx.prepare_cached(
            "INSERT INTO doc_fields (doc_id, key, idx, value_type, value_text, value_num) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )?;
        for (key, value) in map {
            let items: Vec<&Value> = match value {
                Value::Array(items) => items.iter().collect(),
                v => vec![v],
            };
            for (i, v) in items.into_iter().enumerate() {
                let (ty, text, num) = match v {
                    Value::String(s) => ("string", Some(s.clone()), None),
                    Value::Number(n) => ("number", Some(n.to_string()), n.as_f64()),
                    Value::Bool(b) => ("bool", Some(b.to_string()), None),
                    Value::Null => ("null", None, None),
                    other => ("json", Some(other.to_string()), None),
                };
                st.execute(params![id, key, i as i64, ty, text, num])?;
            }
        }
    }

    if let Some(tags) = &m.tags {
        let mut st = tx.prepare_cached(
            "INSERT INTO doc_tags (doc_id, tag, norm, folded) VALUES (?1, ?2, ?3, ?4)",
        )?;
        for tag in &tags.value {
            let norm = normalize_tag(tag);
            st.execute(params![id, tag, norm, fold(&norm)])?;
        }
    }
    let aliases: Vec<String> = match fm.get("aliases") {
        Some(Value::Array(items)) => items.iter().filter_map(scalar_text).collect(),
        Some(v) => scalar_text(v).into_iter().collect(),
        None => Vec::new(),
    };
    for alias in aliases.iter().filter(|a| !a.trim().is_empty()) {
        tx.prepare_cached("INSERT INTO aliases (doc_id, alias, folded) VALUES (?1, ?2, ?3)")?
            .execute(params![id, alias, fold(alias.trim())])?;
    }

    let mut st = tx.prepare_cached(
        "INSERT INTO links (src, raw, target, kind, text, line) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )?;
    for link in c.links() {
        let (kind, target) = match link.kind {
            LinkKind::Markdown => (
                "markdown",
                resolve_link(&c.id, &link.target).map(|t| t.to_string()),
            ),
            LinkKind::Wiki => ("wiki", None), // resolved after all documents are known
        };
        st.execute(params![
            id,
            link.target,
            target,
            kind,
            link.text,
            link.line as i64
        ])?;
    }

    let mut st = tx.prepare_cached(
        "INSERT INTO chunks (doc_id, ord, heading, text, start_line, end_line, tokens) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    )?;
    let mut fts = tx.prepare_cached("INSERT INTO chunks_fts (rowid, terms) VALUES (?1, ?2)")?;
    for (i, (chunk, terms)) in d.chunks.iter().enumerate() {
        st.execute(params![
            id,
            i as i64,
            chunk.heading,
            chunk.text,
            chunk.start_line as i64,
            chunk.end_line as i64,
            chunk.tokens as i64
        ])?;
        fts.execute(params![tx.last_insert_rowid(), terms])?;
    }
    Ok(())
}

fn scalar_text(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

/// Wikilinks resolve by file name across the bundle, so they are re-resolved after every sync.
fn resolve_wikilinks(tx: &Transaction) -> Result<(), Error> {
    let ids: Vec<ConceptId> = {
        let mut st = tx.prepare("SELECT id FROM docs ORDER BY id")?;
        st.query_map([], |r| r.get::<_, String>(0))?
            .filter_map(|r| r.ok().and_then(|s| ConceptId::new(s).ok()))
            .collect()
    };
    let links: Vec<(i64, String)> = {
        let mut st = tx.prepare("SELECT rowid, raw FROM links WHERE kind = 'wiki'")?;
        st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<Result<_, _>>()?
    };
    let mut st = tx.prepare("UPDATE links SET target = ?2 WHERE rowid = ?1")?;
    for (rowid, raw) in links {
        st.execute(params![
            rowid,
            resolve_wikilink(&raw, &ids).map(|i| i.to_string())
        ])?;
    }
    Ok(())
}

/// Maps every tag to its vocabulary term (or clears the mapping without a vocabulary).
fn canonicalize_tags(tx: &Transaction, vocab: Option<&Vocabulary>) -> Result<(), Error> {
    let norms: Vec<String> = {
        let mut st = tx.prepare("SELECT DISTINCT norm FROM doc_tags")?;
        st.query_map([], |r| r.get(0))?.collect::<Result<_, _>>()?
    };
    let mut st =
        tx.prepare("UPDATE doc_tags SET canonical = ?2 WHERE norm = ?1 AND canonical IS NOT ?2")?;
    for norm in norms {
        st.execute(params![norm, vocab.and_then(|v| v.canonical(&norm))])?;
    }
    Ok(())
}
