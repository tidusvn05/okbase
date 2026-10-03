//! Tabular datasets for okbase bundles: CSV, TSV and XLSX files become SQLite
//! tables that agents query with read-only SQL (spike S5: without SQL, agents
//! give up on sheets of ~10k rows).
//!
//! - Every `*.csv`, `*.tsv`, `*.xlsx`, `*.xlsm`, `*.xlsb`, `*.xls` and `*.ods` file in the bundle (hidden directories
//!   excluded) becomes a table named after the file (one table per XLSX sheet).
//!   Column names are folded to `snake_case`; the original headers are kept in
//!   the `_schema` table.
//! - Column types are inferred: `INTEGER`, `REAL` or `TEXT` (values with leading
//!   zeros stay text).
//! - Queries run on a read-only connection with an authorizer that allows only
//!   reads of visible tables, a row limit and a timeout.

mod import;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use rusqlite::hooks::{AuthAction, AuthContext, Authorization};
use rusqlite::types::ValueRef;
use rusqlite::{Connection, OpenFlags, params};
use serde::Serialize;
use serde_json::Value;

/// File name of the dataset database inside the state directory.
pub const DB_FILE: &str = "datasets.sqlite";

/// Table describing every column of every dataset.
pub const SCHEMA_TABLE: &str = "_schema";

const SOURCES_TABLE: &str = "_okbase_sources";

/// Errors returned by `okbase-data`.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// A file could not be read.
    #[error("{path}: {message}")]
    Read {
        /// Bundle-relative path.
        path: String,
        /// What went wrong.
        message: String,
    },
    /// The SQL is invalid or not allowed (only one read-only SELECT is accepted).
    #[error("SQL error: {0}")]
    Sql(String),
    /// The query ran longer than the time limit.
    #[error("query interrupted after {0:?}; use aggregates or add WHERE/LIMIT")]
    Timeout(Duration),
    /// The dataset database failed.
    #[error("dataset database: {0}")]
    Sqlite(#[from] rusqlite::Error),
}

/// Query limits.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    /// Maximum rows returned.
    pub max_rows: usize,
    /// Maximum run time.
    pub timeout: Duration,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            max_rows: 100,
            timeout: Duration::from_secs(5),
        }
    }
}

/// A column of a dataset table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Column {
    /// SQL column name (folded `snake_case`).
    pub name: String,
    /// Original header.
    pub label: String,
    /// `INTEGER`, `REAL` or `TEXT`.
    #[serde(rename = "type")]
    pub sql_type: String,
}

/// A dataset table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Table {
    /// SQL table name.
    pub name: String,
    /// Bundle-relative path of the source file.
    pub source: String,
    /// Sheet name, for XLSX files.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sheet: Option<String>,
    /// Number of rows.
    pub rows: usize,
    /// Columns.
    pub columns: Vec<Column>,
}

/// The visible tables.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TablesResult {
    /// Tables, sorted by name.
    pub tables: Vec<Table>,
}

impl TablesResult {
    /// Compact text: `name (N rows): col TYPE, …` per table (as in the biz-meta spike).
    pub fn to_text(&self) -> String {
        if self.tables.is_empty() {
            return "no datasets (add CSV, TSV or XLSX files to the bundle)".into();
        }
        self.tables
            .iter()
            .map(|t| {
                let cols: Vec<String> = t
                    .columns
                    .iter()
                    .map(|c| format!("{} {}", c.name, c.sql_type))
                    .collect();
                format!(
                    "{} ({} rows): {}  [source: {}]",
                    t.name,
                    t.rows,
                    cols.join(", "),
                    t.source
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// The result of a query.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct QueryResult {
    /// Column names.
    pub columns: Vec<String>,
    /// Rows (at most `max_rows`).
    pub rows: Vec<Vec<Value>>,
    /// Whether more rows were available.
    pub truncated: bool,
    /// Run time in milliseconds.
    pub elapsed_ms: u64,
}

impl QueryResult {
    /// A markdown table (as in the biz-meta spike).
    pub fn to_text(&self) -> String {
        let cell = |v: &Value| match v {
            Value::Null => String::new(),
            Value::String(s) => s.replace('|', "\\|"),
            other => other.to_string(),
        };
        let mut out = format!("| {} |\n", self.columns.join(" | "));
        for r in &self.rows {
            out.push_str(&format!(
                "| {} |\n",
                r.iter().map(cell).collect::<Vec<_>>().join(" | ")
            ));
        }
        if self.truncated {
            out.push_str(&format!(
                "... truncated at {} rows (use aggregates or LIMIT)\n",
                self.rows.len()
            ));
        }
        out.trim_end().to_owned()
    }
}

/// What a sync did.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct SyncReport {
    /// Source files (re)imported.
    pub imported: Vec<String>,
    /// Source files whose tables were dropped.
    pub removed: Vec<String>,
    /// Files that could not be imported: (path, reason).
    pub skipped: Vec<(String, String)>,
}

/// Datasets of one bundle.
#[derive(Debug)]
pub struct Data {
    root: PathBuf,
    db: PathBuf,
}

impl Data {
    /// Uses `db` (usually `<state dir>/datasets.sqlite`) for the bundle at `root`.
    pub fn new(root: &Path, db: &Path) -> Self {
        Data {
            root: root.to_owned(),
            db: db.to_owned(),
        }
    }

    /// Whether the bundle has any dataset file.
    pub fn has_datasets(root: &Path) -> bool {
        !import::discover(root).is_empty()
    }

    /// Imports new and changed dataset files and drops tables of removed ones.
    pub fn sync(&self) -> Result<SyncReport, Error> {
        let mut conn = Connection::open(&self.db)?;
        conn.busy_timeout(Duration::from_secs(30))?;
        import::sync(&self.root, &mut conn)
    }

    fn read_only(&self) -> Result<Connection, Error> {
        let conn = Connection::open_with_flags(
            &self.db,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        conn.pragma_update(None, "query_only", true)?;
        Ok(conn)
    }

    /// The tables whose source path passes `visible` (a bundle-relative path without extension).
    pub fn tables(&self, visible: &dyn Fn(&str) -> bool) -> Result<TablesResult, Error> {
        let conn = self.read_only()?;
        Ok(TablesResult {
            tables: load_tables(&conn)?
                .into_iter()
                .filter(|t| visible(strip_ext(&t.source)))
                .collect(),
        })
    }

    /// Runs one read-only `SELECT` (or `WITH … SELECT`) over the visible tables.
    pub fn query(
        &self,
        sql: &str,
        limits: &Limits,
        visible: &dyn Fn(&str) -> bool,
    ) -> Result<QueryResult, Error> {
        let sql = sql.trim().trim_end_matches(';').trim();
        if sql.is_empty() {
            return Err(Error::Sql("empty query".into()));
        }
        let conn = self.read_only()?;
        let tables = load_tables(&conn)?;
        let hidden: Vec<String> = tables
            .iter()
            .filter(|t| !visible(strip_ext(&t.source)))
            .map(|t| t.name.clone())
            .collect();
        let allowed: Vec<String> = tables
            .iter()
            .map(|t| t.name.clone())
            .filter(|n| !hidden.contains(n))
            .collect();
        let schema_ok = hidden.is_empty();
        // Names that are real tables; anything else read by a query is a CTE or subquery alias.
        let real: Vec<String> = {
            let mut st =
                conn.prepare("SELECT name FROM sqlite_master WHERE type IN ('table', 'view')")?;
            st.query_map([], |r| r.get(0))?.collect::<Result<_, _>>()?
        };
        conn.authorizer(Some(move |ctx: AuthContext<'_>| -> Authorization {
            match ctx.action {
                AuthAction::Select | AuthAction::Function { .. } | AuthAction::Recursive => {
                    Authorization::Allow
                }
                AuthAction::Read { table_name, .. } => {
                    let cte = !real.iter().any(|t| t.eq_ignore_ascii_case(table_name))
                        && !table_name.to_ascii_lowercase().starts_with("sqlite_");
                    if cte
                        || allowed.iter().any(|t| t == table_name)
                        || (table_name == SCHEMA_TABLE && schema_ok)
                    {
                        Authorization::Allow
                    } else {
                        Authorization::Deny
                    }
                }
                _ => Authorization::Deny,
            }
        }))?;
        let start = Instant::now();
        let timeout = limits.timeout;
        conn.progress_handler(10_000, Some(move || start.elapsed() > timeout))?;

        let mut stmt = conn.prepare(sql).map_err(sql_error)?;
        if !stmt.readonly() {
            return Err(Error::Sql("only SELECT queries are allowed".into()));
        }
        let columns: Vec<String> = stmt.column_names().into_iter().map(str::to_owned).collect();
        let mut rows = Vec::new();
        let mut truncated = false;
        let mut cursor = stmt.query([]).map_err(|e| timeout_or(e, start, timeout))?;
        while let Some(row) = cursor.next().map_err(|e| timeout_or(e, start, timeout))? {
            if rows.len() == limits.max_rows {
                truncated = true;
                break;
            }
            rows.push(
                (0..columns.len())
                    .map(|i| to_json(row.get_ref(i).unwrap_or(ValueRef::Null)))
                    .collect(),
            );
        }
        Ok(QueryResult {
            columns,
            rows,
            truncated,
            elapsed_ms: start.elapsed().as_millis() as u64,
        })
    }
}

fn strip_ext(p: &str) -> &str {
    p.rsplit_once('.').map_or(p, |(a, _)| a)
}

fn sql_error(e: rusqlite::Error) -> Error {
    let msg = e.to_string();
    if msg.contains("not authorized") {
        Error::Sql(
            "not allowed: only reads of the dataset tables (see data_tables) are permitted".into(),
        )
    } else {
        Error::Sql(msg)
    }
}

fn timeout_or(e: rusqlite::Error, start: Instant, timeout: Duration) -> Error {
    if start.elapsed() > timeout || e.to_string().contains("interrupted") {
        Error::Timeout(timeout)
    } else {
        sql_error(e)
    }
}

fn to_json(v: ValueRef<'_>) -> Value {
    match v {
        ValueRef::Null => Value::Null,
        ValueRef::Integer(i) => Value::from(i),
        ValueRef::Real(f) => serde_json::Number::from_f64(f).map_or(Value::Null, Value::Number),
        ValueRef::Text(t) => Value::String(String::from_utf8_lossy(t).into_owned()),
        ValueRef::Blob(b) => Value::String(format!("<{} bytes>", b.len())),
    }
}

fn load_tables(conn: &Connection) -> Result<Vec<Table>, Error> {
    let exists: bool = conn.query_row(
        "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type = 'table' AND name = ?1",
        params![SOURCES_TABLE],
        |r| r.get(0),
    )?;
    if !exists {
        return Ok(Vec::new());
    }
    let mut cols: BTreeMap<String, Vec<Column>> = BTreeMap::new();
    let mut st = conn.prepare(&format!("SELECT table_name, column_name, label, type FROM {SCHEMA_TABLE} ORDER BY table_name, position"))?;
    for row in st.query_map([], |r| {
        Ok((r.get::<_, String>(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
    })? {
        let (t, name, label, sql_type) = row?;
        cols.entry(t).or_default().push(Column {
            name,
            label,
            sql_type,
        });
    }
    let mut st = conn.prepare(&format!(
        "SELECT table_name, source, sheet, rows FROM {SOURCES_TABLE} ORDER BY table_name"
    ))?;
    let tables = st
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, i64>(3)?,
            ))
        })?
        .map(|row| {
            row.map(|(name, source, sheet, rows)| Table {
                columns: cols.remove(&name).unwrap_or_default(),
                name,
                source,
                sheet,
                rows: rows as usize,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(tables)
}
