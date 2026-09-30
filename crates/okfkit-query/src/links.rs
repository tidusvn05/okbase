//! `links`: outgoing links and backlinks of a document.

use serde::Serialize;

use crate::get::normalize_id;
use crate::{Error, Scope};

/// A link.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LinkRow {
    /// The other document (target for outgoing links, source for backlinks), if resolved.
    pub id: Option<String>,
    /// The target as written.
    pub raw: String,
    /// Link text.
    pub text: String,
    /// `markdown` or `wiki`.
    pub kind: String,
    /// Line in the source body.
    pub line: usize,
    /// Whether the target document exists (and is visible).
    pub exists: bool,
}

/// Links of one document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LinksResult {
    /// The document.
    pub id: String,
    /// Links from this document to others (external URLs excluded).
    pub outgoing: Vec<LinkRow>,
    /// Links from other visible documents to this one.
    pub backlinks: Vec<LinkRow>,
}

pub(crate) fn links(
    index: &okfkit_index::Index,
    id: &str,
    scope: &Scope,
) -> Result<LinksResult, Error> {
    let id = normalize_id(id);
    let visible: std::collections::HashSet<String> = crate::docs::load(
        index.connection(),
        scope,
        crate::docs::Load {
            reserved: true,
            ..Default::default()
        },
    )?
    .into_iter()
    .map(|d| d.id)
    .collect();
    if !visible.contains(&id) {
        return Err(Error::NotFound(id));
    }
    let conn = index.connection();
    let read = |sql: &str, other_is_target: bool| -> Result<Vec<LinkRow>, Error> {
        let mut st = conn.prepare_cached(sql)?;
        let rows = st.query_map([&id], |r| {
            Ok((
                r.get::<_, Option<String>>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, i64>(4)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (other, raw, text, kind, line) = row?;
            let exists = other.as_ref().is_some_and(|o| visible.contains(o));
            if !other_is_target && !exists {
                continue; // hidden or unknown sources are not revealed
            }
            if other_is_target && other.is_none() && kind == "markdown" {
                continue; // external URL or non-markdown asset
            }
            let id = if other_is_target
                && !exists
                && other.as_ref().is_some_and(|o| !scope.permits_path(o))
            {
                None
            } else {
                other
            };
            out.push(LinkRow {
                id,
                raw,
                text,
                kind,
                line: usize::try_from(line).unwrap_or(0),
                exists,
            });
        }
        Ok(out)
    };
    let outgoing = read(
        "SELECT target, raw, text, kind, line FROM links WHERE src = ?1 ORDER BY line, raw",
        true,
    )?;
    let backlinks = read(
        "SELECT src, raw, text, kind, line FROM links WHERE target = ?1 ORDER BY src, line",
        false,
    )?;
    Ok(LinksResult {
        id,
        outgoing,
        backlinks,
    })
}
