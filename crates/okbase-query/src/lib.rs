//! Read APIs for okbase bundles: `grep`, `get`, `list`, `query`, `catalog`,
//! `links`, `stats` and `recommend_mode`.
//!
//! Every function takes a [`Scope`] from the host and treats hidden documents as
//! if they did not exist. Results are plain serializable structs (the `--json`
//! and MCP structured output); `to_text()` renders the compact text form that the
//! spikes validated with agents.

mod catalog;
mod docs;
mod error;
mod get;
mod grep;
mod links;
mod list;
mod query;
mod scope;
mod stats;

use okbase_index::Index;

pub use catalog::{CatalogOptions, CatalogResult, CatalogTerm, DEFAULT_CATALOG_TOKENS};
pub use error::Error;
pub use get::{DEFAULT_GET_TOKENS, GetRequest, GetResult};
pub use grep::{GrepDoc, GrepLine, GrepRequest, GrepResult};
pub use links::{LinkRow, LinksResult};
pub use list::ListResult;
pub use query::{DEFAULT_QUERY_LIMIT, Filter, QueryRequest, QueryResult, QueryRow, Range, Sum};
pub use scope::{MetaFilter, PathGlob, Scope};
pub use stats::{FULL_MODE_MAX_TOKENS, Mode, Stats};

/// Regex search over whole documents, case- and accent-insensitive.
pub fn grep(index: &Index, req: &GrepRequest, scope: &Scope) -> Result<GrepResult, Error> {
    grep::grep(index, req, scope)
}

/// Reads a document, a section or a line range within a token budget.
pub fn get(index: &Index, req: &GetRequest, scope: &Scope) -> Result<GetResult, Error> {
    get::get(index, req, scope)
}

/// Lists a directory: its `index.md`, or a generated listing.
pub fn list(index: &Index, dir: &str, scope: &Scope) -> Result<ListResult, Error> {
    list::list(index, dir, scope)
}

/// Filters documents by metadata, with facets and sums.
pub fn query(index: &Index, req: &QueryRequest, scope: &Scope) -> Result<QueryResult, Error> {
    query::query(index, req, scope)
}

/// A prompt-ready catalog with vocabulary and facets.
pub fn catalog(
    index: &Index,
    opts: &CatalogOptions,
    scope: &Scope,
) -> Result<CatalogResult, Error> {
    catalog::catalog(index, opts, scope)
}

/// Outgoing links and backlinks of a document.
pub fn links(index: &Index, id: &str, scope: &Scope) -> Result<LinksResult, Error> {
    links::links(index, id, scope)
}

/// Counts, tokens and the recommended mode for the visible part of the bundle.
pub fn stats(index: &Index, scope: &Scope) -> Result<Stats, Error> {
    stats::stats(index, scope)
}

/// Estimated tokens of the visible concepts per language (`lang` field, else detected).
pub fn content_langs(
    index: &Index,
    scope: &Scope,
) -> Result<std::collections::BTreeMap<String, usize>, Error> {
    stats::content_langs(index, scope)
}

/// `Full` when the visible concepts fit in about 30k tokens, else `Lexical`.
/// (`Retrieval` needs the embed module.)
pub fn recommend_mode(index: &Index, scope: &Scope) -> Result<Mode, Error> {
    let tokens = docs::load(index.connection(), scope, docs::Load::default())?
        .iter()
        .map(|d| d.tokens)
        .sum();
    Ok(stats::mode_for(tokens))
}

/// IDs of the concept documents visible in `scope` (and matching `filter`, if given).
/// Used by modules such as semantic search to apply the same rules as the other reads.
pub fn visible_ids(
    index: &Index,
    scope: &Scope,
    filter: Option<&Filter>,
) -> Result<std::collections::HashSet<String>, Error> {
    let tags = filter.is_some_and(Filter::uses_tags);
    let docs = docs::load(
        index.connection(),
        scope,
        docs::Load {
            reserved: false,
            tags,
            prefilter: filter,
        },
    )?;
    let docs = match filter {
        Some(f) => query::filter_docs(docs, f)?,
        None => docs,
    };
    Ok(docs.into_iter().map(|d| d.id).collect())
}
