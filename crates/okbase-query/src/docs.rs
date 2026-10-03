//! Loading document metadata from the index, with the scope applied.
//!
//! Frontmatter JSON is parsed lazily and tags are loaded only on request: most
//! queries filter on indexed columns first and touch few documents' fields.

use std::cell::OnceCell;
use std::collections::HashMap;

use rusqlite::Connection;
use serde_json::{Map, Value};

use crate::{Error, Scope};

/// Metadata of one document, as stored in the index.
#[derive(Debug, Clone)]
pub(crate) struct DocMeta {
    pub id: String,
    pub reserved: bool,
    pub title: String,
    pub description: Option<String>,
    pub concept_type: Option<String>,
    pub status: Option<String>,
    pub lang: Option<String>,
    pub updated: Option<String>,
    pub tokens: usize,
    fm_raw: String,
    fm: OnceCell<Map<String, Value>>,
    /// Tags as written and their folded keys (with canonical vocabulary forms); `None` until loaded.
    tags: Option<(Vec<String>, Vec<String>)>,
}

impl DocMeta {
    /// Parsed frontmatter.
    pub fn frontmatter(&self) -> &Map<String, Value> {
        self.fm
            .get_or_init(|| match serde_json::from_str::<Value>(&self.fm_raw) {
                Ok(Value::Object(m)) => m,
                _ => Map::new(),
            })
    }

    /// Tags as written. Empty unless loaded.
    pub fn tags(&self) -> &[String] {
        self.tags.as_ref().map_or(&[], |t| &t.0)
    }

    /// Accent- and case-folded tags plus their canonical forms. Empty unless loaded.
    pub fn tag_keys(&self) -> &[String] {
        self.tags.as_ref().map_or(&[], |t| &t.1)
    }
}

/// Which documents and extra data to load.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Load<'a> {
    /// Include index.md / log.md.
    pub reserved: bool,
    /// Load tags for every document.
    pub tags: bool,
    /// Apply the filter's `type`/`status`/`lang` lists in SQL (the full filter still runs afterwards).
    pub prefilter: Option<&'a crate::query::Filter>,
}

/// Loads visible documents, sorted by ID.
pub(crate) fn load(conn: &Connection, scope: &Scope, what: Load) -> Result<Vec<DocMeta>, Error> {
    let mut sql = String::from(
        "SELECT id, reserved, title, description, type, status, lang, updated, tokens, frontmatter \
         FROM docs WHERE (reserved = 0 OR ?1)",
    );
    let mut args: Vec<String> = Vec::new();
    if let Some(f) = what.prefilter {
        for (col, values) in [("type", &f.types), ("status", &f.status), ("lang", &f.lang)] {
            if !values.is_empty() {
                let marks: Vec<String> = values
                    .iter()
                    .map(|v| {
                        args.push(v.to_lowercase());
                        format!("?{}", args.len() + 1)
                    })
                    .collect();
                sql.push_str(&format!(" AND lower({col}) IN ({})", marks.join(", ")));
            }
        }
    }
    sql.push_str(" ORDER BY id");
    let mut st = conn.prepare_cached(&sql)?;
    let mut params: Vec<&dyn rusqlite::ToSql> = vec![&what.reserved];
    params.extend(args.iter().map(|a| a as &dyn rusqlite::ToSql));
    let rows = st.query_map(params.as_slice(), |r| {
        Ok(DocMeta {
            id: r.get(0)?,
            reserved: r.get(1)?,
            title: r.get(2)?,
            description: r.get(3)?,
            concept_type: r.get(4)?,
            status: r.get(5)?,
            lang: r.get(6)?,
            updated: r.get(7)?,
            tokens: usize::try_from(r.get::<_, i64>(8)?).unwrap_or(0),
            fm_raw: r.get(9)?,
            fm: OnceCell::new(),
            tags: None,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        let d = row?;
        if !scope.permits_path(&d.id) {
            continue;
        }
        // Reserved files carry no metadata of their own; they are visible whenever their path is.
        if scope.has_filters() && !d.reserved && !scope.permits(&d.id, d.frontmatter()) {
            continue;
        }
        out.push(d);
    }
    if what.tags {
        load_all_tags(conn, &mut out)?;
    }
    Ok(out)
}

fn tag_entry(
    tag: String,
    folded: String,
    canonical: Option<String>,
    e: &mut (Vec<String>, Vec<String>),
) {
    e.0.push(tag);
    e.1.push(folded);
    if let Some(c) = canonical {
        e.1.push(okbase_analyze::fold(&okbase_standard::normalize_tag(&c)));
    }
}

fn load_all_tags(conn: &Connection, docs: &mut [DocMeta]) -> Result<(), Error> {
    let mut map: HashMap<String, (Vec<String>, Vec<String>)> = HashMap::new();
    let mut st =
        conn.prepare_cached("SELECT doc_id, tag, folded, canonical FROM doc_tags ORDER BY rowid")?;
    let rows = st.query_map([], |r| {
        Ok((r.get::<_, String>(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
    })?;
    for row in rows {
        let (doc, tag, folded, canonical) = row?;
        tag_entry(tag, folded, canonical, map.entry(doc).or_default());
    }
    for d in docs {
        d.tags = Some(map.remove(&d.id).unwrap_or_default());
    }
    Ok(())
}

/// Loads tags for a few documents (such as the rows shown by a query).
pub(crate) fn load_tags(conn: &Connection, docs: &mut [DocMeta]) -> Result<(), Error> {
    let mut st = conn.prepare_cached(
        "SELECT tag, folded, canonical FROM doc_tags WHERE doc_id = ?1 ORDER BY rowid",
    )?;
    for d in docs.iter_mut().filter(|d| d.tags.is_none()) {
        let mut e = (Vec::new(), Vec::new());
        let rows = st.query_map([&d.id], |r| {
            Ok((r.get::<_, String>(0)?, r.get(1)?, r.get(2)?))
        })?;
        for row in rows {
            let (tag, folded, canonical) = row?;
            tag_entry(tag, folded, canonical, &mut e);
        }
        d.tags = Some(e);
    }
    Ok(())
}
