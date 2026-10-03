//! `stats` and `recommend_mode`.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::{Error, Scope};

/// Bundles up to this many (estimated) tokens fit in the context window: the
/// okf-scale spike found full-context best at ≤ ~30k tokens.
pub const FULL_MODE_MAX_TOKENS: usize = 30_000;

/// How an agent should work with the bundle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// Put the whole bundle in context.
    Full,
    /// Pre-retrieve with semantic search (needs the embed module).
    Retrieval,
    /// Catalog plus grep/query/get tools.
    Lexical,
}

/// Bundle statistics.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Stats {
    /// Visible concept documents.
    pub docs: usize,
    /// Visible reserved files (index.md, log.md).
    pub reserved: usize,
    /// Estimated tokens of the visible concepts.
    pub tokens: usize,
    /// Chunks of the visible concepts.
    pub chunks: usize,
    /// Concepts per `type`.
    pub types: BTreeMap<String, usize>,
    /// Concepts per `lang`.
    pub langs: BTreeMap<String, usize>,
    /// Links from visible concepts whose target does not exist.
    pub broken_links: usize,
    /// Recommended mode.
    pub mode: Mode,
}

pub(crate) fn stats(index: &okbase_index::Index, scope: &Scope) -> Result<Stats, Error> {
    let all = crate::docs::load(
        index.connection(),
        scope,
        crate::docs::Load {
            reserved: true,
            ..Default::default()
        },
    )?;
    let conn = index.connection();
    let (mut docs, mut reserved, mut tokens, mut chunks, mut broken) = (0, 0, 0, 0, 0);
    let mut types = BTreeMap::new();
    let mut langs = BTreeMap::new();
    let mut st_chunks = conn.prepare_cached("SELECT COUNT(*) FROM chunks WHERE doc_id = ?1")?;
    let mut st_broken = conn.prepare_cached(
        "SELECT COUNT(*) FROM links WHERE src = ?1 AND (target IS NULL AND kind = 'wiki' OR target IS NOT NULL AND target NOT IN (SELECT id FROM docs))",
    )?;
    for d in &all {
        if d.reserved {
            reserved += 1;
            continue;
        }
        docs += 1;
        tokens += d.tokens;
        chunks += st_chunks.query_row([&d.id], |r| r.get::<_, i64>(0))? as usize;
        broken += st_broken.query_row([&d.id], |r| r.get::<_, i64>(0))? as usize;
        *types
            .entry(d.concept_type.clone().unwrap_or_else(|| "(none)".into()))
            .or_default() += 1;
        *langs
            .entry(d.lang.clone().unwrap_or_else(|| "(none)".into()))
            .or_default() += 1;
    }
    Ok(Stats {
        docs,
        reserved,
        tokens,
        chunks,
        types,
        langs,
        broken_links: broken,
        mode: mode_for(tokens),
    })
}

pub(crate) fn mode_for(tokens: usize) -> Mode {
    if tokens <= FULL_MODE_MAX_TOKENS {
        Mode::Full
    } else {
        Mode::Lexical
    }
}

/// Characters of a body that language detection looks at.
const DETECT_CHARS: usize = 2_000;

/// Estimated tokens of the visible concepts per language: the `lang` field when set, else
/// detected from the start of the body (`vi`, `en`, `ja`; `other` when undetectable).
pub(crate) fn content_langs(
    index: &okbase_index::Index,
    scope: &Scope,
) -> Result<BTreeMap<String, usize>, Error> {
    let conn = index.connection();
    let mut st = conn.prepare_cached("SELECT body FROM docs WHERE id = ?1")?;
    let mut out = BTreeMap::new();
    // _meta/ holds the vocabulary and schemas: about the bundle, not part of its content.
    for d in crate::docs::load(conn, scope, crate::docs::Load::default())?
        .into_iter()
        .filter(|d| !d.id.starts_with("_meta/"))
    {
        let lang = match d.lang.as_deref().map(str::to_lowercase) {
            Some(l) if !l.is_empty() => l.split(['-', '_']).next().unwrap_or("other").to_owned(),
            _ => {
                let body: String = st.query_row([&d.id], |r| r.get(0))?;
                let head: String = body.chars().take(DETECT_CHARS).collect();
                okbase_analyze::detect_lang(&head)
                    .map_or("other", |l| l.code())
                    .to_owned()
            }
        };
        *out.entry(lang).or_default() += d.tokens.max(1);
    }
    Ok(out)
}
