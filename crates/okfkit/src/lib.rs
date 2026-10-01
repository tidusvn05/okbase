//! okfkit: make markdown knowledge bundles in the [Open Knowledge Format][okf]
//! work well for AI agents.
//!
//! [`Bundle`] is the entry point for hosts (CLIs, MCP servers, applications).
//! It is lexical and model-free, and read-only unless a write API is called.
//! Every read takes a [`Scope`] from the host.
//!
//! ```no_run
//! use okfkit::{Bundle, OpenOptions, Scope, GrepRequest};
//!
//! let bundle = Bundle::open("path/to/bundle".as_ref(), OpenOptions::default())?;
//! bundle.sync()?;
//! let scope = Scope::all();
//! let hits = bundle.grep(&GrepRequest { pattern: "refund|đổi trả".into(), ..Default::default() }, &scope)?;
//! println!("{}", hits.to_text(false));
//! # Ok::<(), okfkit::Error>(())
//! ```
//!
//! [okf]: https://github.com/GoogleCloudPlatform/open-knowledge-format

pub mod advise;
pub mod config;
mod embed;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

pub use advise::{Advice, AdviseOptions, Audience};
pub use okfkit_analyze as analyze;
pub use okfkit_core as core;
pub use okfkit_data::{
    Limits as DataLimits, QueryResult as DataQueryResult, Table as DataTable,
    TablesResult as DataTables,
};
pub use okfkit_embed::{
    Embedder, Error as EmbedError, ModelInfo, accept_license, find_model, license_accepted,
    models as embedding_models,
};
pub use okfkit_index::{IndexOptions, StateDir, SyncStats};
pub use okfkit_lint::{Level, LintConfig, Report as LintReport};
pub use okfkit_query::{
    CatalogOptions, CatalogResult, Filter, GetRequest, GetResult, GrepRequest, GrepResult,
    LinksResult, ListResult, MetaFilter, Mode, QueryRequest, QueryResult, Range, Scope, Stats,
};
pub use okfkit_search::{
    DEFAULT_BUDGET, EmbedStatus, Hit, RetrieveResult, SearchRequest, SearchResult,
};

/// Errors returned by [`Bundle`].
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// Opening or updating the index failed.
    #[error(transparent)]
    Index(#[from] okfkit_index::Error),
    /// A read failed (including "not found" and invalid arguments).
    #[error(transparent)]
    Query(#[from] okfkit_query::Error),
    /// Linting failed.
    #[error(transparent)]
    Lint(#[from] okfkit_lint::Error),
    /// A dataset operation failed (including SQL errors and timeouts).
    #[error(transparent)]
    Data(#[from] okfkit_data::Error),
    /// The bundle has no datasets, or the data module is off.
    #[error("no datasets: the bundle has no CSV, TSV or XLSX files (or the data module is off)")]
    NoDatasets,
    /// `okfkit.toml` is invalid.
    #[error("config: {0}")]
    Config(String),
    /// Loading the embedder failed (model download, license, API key, missing build feature).
    #[error("embeddings: {0}")]
    Embedding(String),
    /// Search or embedding of the index failed.
    #[error(transparent)]
    Search(#[from] okfkit_search::Error),
    /// Embeddings are not enabled for this bundle.
    #[error(
        "embeddings are off for this bundle; enable them with `okfkit embed enable` (okfkit-full build)"
    )]
    NoEmbedder,
    /// The bundle directory does not exist.
    #[error("bundle not found: {0} is not a directory")]
    NotADirectory(PathBuf),
}

/// Optional modules compiled into this build.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct BuildFeatures {
    /// Local embedding models (ONNX Runtime).
    pub embed_local: bool,
    /// Embeddings through an OpenAI-compatible API.
    pub embed_api: bool,
    /// The Japanese dictionary is embedded (no download needed).
    pub ja_embedded: bool,
}

/// The optional modules of this build.
pub fn build_features() -> BuildFeatures {
    BuildFeatures {
        embed_local: cfg!(feature = "embed-local"),
        embed_api: cfg!(feature = "embed-api"),
        ja_embedded: okfkit_analyze::dict::embedded(),
    }
}

/// Default sets of modules.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Profile {
    /// Core only.
    Minimal,
    /// Core plus the data module when present (v0.2). The default.
    #[default]
    Standard,
    /// Every module, including embeddings (v0.3).
    Full,
}

/// Options for [`Bundle::open`].
#[derive(Debug, Clone, Default)]
pub struct OpenOptions {
    /// Module profile.
    pub profile: Profile,
    /// Where the index lives.
    pub state_dir: StateDir,
    /// An embedder supplied by the host (shared between bundles); overrides `okfkit.toml`.
    pub embedder: Option<std::sync::Arc<dyn Embedder>>,
    /// Vector cache file (default `<user cache>/okfkit/emb/vectors.sqlite`).
    pub vector_cache: Option<PathBuf>,
}

impl From<okfkit_embed::Error> for Error {
    fn from(e: okfkit_embed::Error) -> Self {
        Error::Embedding(e.to_string())
    }
}

impl OpenOptions {
    /// Sets the profile.
    pub fn profile(mut self, profile: Profile) -> Self {
        self.profile = profile;
        self
    }

    /// Uses this embedder instead of the one configured in `okfkit.toml`.
    pub fn embedder(mut self, embedder: std::sync::Arc<dyn Embedder>) -> Self {
        self.embedder = Some(embedder);
        self
    }

    /// Uses this vector cache file.
    pub fn vector_cache(mut self, path: PathBuf) -> Self {
        self.vector_cache = Some(path);
        self
    }

    /// Sets the state directory.
    pub fn state_dir(mut self, state_dir: StateDir) -> Self {
        self.state_dir = state_dir;
        self
    }
}

/// A capability a module provides. Tools, commands and skills are shown only when present.
pub mod capability {
    /// Regex search.
    pub const GREP: &str = "read.grep";
    /// Read documents and sections.
    pub const GET: &str = "read.get";
    /// Directory listings.
    pub const LIST: &str = "read.list";
    /// Metadata queries.
    pub const QUERY: &str = "read.query";
    /// Prompt-ready catalog.
    pub const CATALOG: &str = "read.catalog";
    /// Links and backlinks.
    pub const LINKS: &str = "read.links";
    /// Lint.
    pub const LINT: &str = "maint.lint";
    /// Read-only SQL over datasets (module `data`).
    pub const DATA_SQL: &str = "data.sql";
    /// Semantic search and retrieval (module `embed`).
    pub const EMBED_SEARCH: &str = "embed.search";
}

/// The capabilities available for a bundle.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Capabilities(BTreeSet<&'static str>);

impl Capabilities {
    /// Whether a capability is present.
    pub fn has(&self, cap: &str) -> bool {
        self.0.contains(cap)
    }

    /// All capabilities, sorted.
    pub fn iter(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.0.iter().copied()
    }
}

struct Inner {
    root: PathBuf,
    index: Mutex<okfkit_index::Index>,
    profile: Profile,
    /// Dataset module, when the profile allows it and the bundle has CSV/TSV/XLSX files.
    data: Option<okfkit_data::Data>,
    /// Keeps the temporary state directory of an in-memory bundle alive.
    _tmp: Option<tempfile::TempDir>,
    embed: embed::EmbedState,
}

/// An open knowledge bundle. Cheap to clone and safe to share between threads.
#[derive(Clone)]
pub struct Bundle(Arc<Inner>);

impl std::fmt::Debug for Bundle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Bundle")
            .field("root", &self.0.root)
            .field("profile", &self.0.profile)
            .finish()
    }
}

impl Bundle {
    /// Opens the bundle at `dir` and its index. Call [`Bundle::sync`] to bring the index up to date.
    pub fn open(dir: &Path, options: OpenOptions) -> Result<Self, Error> {
        if !dir.is_dir() {
            return Err(Error::NotADirectory(dir.to_owned()));
        }
        let index = okfkit_index::Index::open(
            dir,
            &IndexOptions {
                state_dir: options.state_dir,
                ..Default::default()
            },
        )?;
        let data = (options.profile != Profile::Minimal && okfkit_data::Data::has_datasets(dir))
            .then(|| {
                index
                    .db_path()
                    .and_then(Path::parent)
                    .map(|d| okfkit_data::Data::new(dir, &d.join(okfkit_data::DB_FILE)))
            })
            .flatten();
        let embed = embed::EmbedState::new(
            config::load(dir)?.embed,
            options.embedder,
            options.vector_cache,
        );
        Ok(Bundle(Arc::new(Inner {
            root: dir.to_owned(),
            index: Mutex::new(index),
            profile: options.profile,
            data,
            _tmp: None,
            embed,
        })))
    }

    /// Opens the bundle with a throw-away in-memory index (for tests and one-off reads).
    pub fn open_in_memory(dir: &Path) -> Result<Self, Error> {
        if !dir.is_dir() {
            return Err(Error::NotADirectory(dir.to_owned()));
        }
        let index = okfkit_index::Index::open_in_memory(dir)?;
        let (data, tmp) = if okfkit_data::Data::has_datasets(dir) {
            let tmp = tempfile::tempdir().map_err(|e| {
                Error::Data(okfkit_data::Error::Read {
                    path: "temp dir".into(),
                    message: e.to_string(),
                })
            })?;
            (
                Some(okfkit_data::Data::new(
                    dir,
                    &tmp.path().join(okfkit_data::DB_FILE),
                )),
                Some(tmp),
            )
        } else {
            (None, None)
        };
        Ok(Bundle(Arc::new(Inner {
            root: dir.to_owned(),
            index: Mutex::new(index),
            profile: Profile::Standard,
            data,
            _tmp: tmp,
            embed: embed::EmbedState::new(config::EmbedConfig::Off, None, None),
        })))
    }

    /// The bundle root.
    pub fn root(&self) -> &Path {
        &self.0.root
    }

    /// Where the index is stored (`None` for an in-memory index).
    pub fn index_path(&self) -> Option<PathBuf> {
        self.index().db_path().map(Path::to_owned)
    }

    fn index(&self) -> MutexGuard<'_, okfkit_index::Index> {
        self.0.index.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Re-indexes changed files (and re-imports changed datasets).
    pub fn sync(&self) -> Result<SyncStats, Error> {
        let stats = self.index().sync()?;
        if let Some(d) = &self.0.data {
            d.sync()?;
        }
        self.0.embed.invalidate();
        Ok(stats)
    }

    /// The embedding model id, if embeddings are enabled (does not load the model).
    pub fn embedding_model(&self) -> Option<String> {
        self.0.embed.model_id()
    }

    /// How many chunks have vectors for the configured model.
    pub fn embed_status(&self) -> Result<EmbedStatus, Error> {
        let model = self.0.embed.model_id().ok_or(Error::NoEmbedder)?;
        Ok(okfkit_search::status(&self.index(), &model)?)
    }

    /// Embeds the chunks that have no vector yet (loads the model; slow on first run).
    /// `progress(done, total)` reports newly embedded chunks.
    pub fn embed_sync(&self, progress: &mut dyn FnMut(usize, usize)) -> Result<EmbedStatus, Error> {
        let embedder = self.0.embed.embedder()?;
        let mut cache = okfkit_embed::VectorCache::open(&self.0.embed.cache_path()?)?;
        let status =
            okfkit_search::embed_sync(&self.index(), embedder.as_ref(), &mut cache, progress)?;
        self.0.embed.invalidate();
        Ok(status)
    }

    /// Semantic search over the visible chunks (module embed).
    pub fn search(&self, req: &SearchRequest, scope: &Scope) -> Result<SearchResult, Error> {
        let embedder = self.0.embed.embedder()?;
        let index = self.index();
        let store = self.0.embed.store(&index)?;
        Ok(okfkit_search::search(
            &index,
            &store,
            embedder.as_ref(),
            req,
            scope,
        )?)
    }

    /// The best visible sections for `query` within `budget` tokens, for a prompt (module embed).
    pub fn retrieve(
        &self,
        query: &str,
        budget: usize,
        scope: &Scope,
    ) -> Result<RetrieveResult, Error> {
        let embedder = self.0.embed.embedder()?;
        let index = self.index();
        let store = self.0.embed.store(&index)?;
        Ok(okfkit_search::retrieve(
            &index,
            &store,
            embedder.as_ref(),
            query,
            budget,
            scope,
        )?)
    }

    fn datasets(&self) -> Result<&okfkit_data::Data, Error> {
        self.0.data.as_ref().ok_or(Error::NoDatasets)
    }

    /// Dataset tables visible in the scope (module `data`).
    pub fn data_tables(&self, scope: &Scope) -> Result<DataTables, Error> {
        Ok(self.datasets()?.tables(&|p| scope.permits_path(p))?)
    }

    /// Runs one read-only SQL `SELECT` over the visible dataset tables (module `data`).
    pub fn data_query(
        &self,
        sql: &str,
        limits: &DataLimits,
        scope: &Scope,
    ) -> Result<DataQueryResult, Error> {
        Ok(self
            .datasets()?
            .query(sql, limits, &|p| scope.permits_path(p))?)
    }

    /// Capabilities of the enabled modules.
    pub fn capabilities(&self) -> Capabilities {
        use capability::*;
        Capabilities(
            [GREP, GET, LIST, QUERY, CATALOG, LINKS, LINT]
                .into_iter()
                .chain(self.0.data.as_ref().map(|_| DATA_SQL))
                .chain(self.0.embed.enabled().then_some(EMBED_SEARCH))
                .collect(),
        )
    }

    /// A prompt-ready catalog.
    pub fn catalog(&self, options: &CatalogOptions, scope: &Scope) -> Result<CatalogResult, Error> {
        Ok(okfkit_query::catalog(&self.index(), options, scope)?)
    }

    /// `Full` if the visible bundle fits in about 30k tokens; else `Retrieval` when every chunk is embedded, else `Lexical`.
    pub fn recommend_mode(&self, scope: &Scope) -> Result<Mode, Error> {
        let mode = okfkit_query::recommend_mode(&self.index(), scope)?;
        // Pre-retrieval only once every chunk has a vector (PLAN §4.7).
        if mode == Mode::Lexical
            && self
                .embed_status()
                .is_ok_and(|s| s.complete() && s.chunks > 0)
        {
            return Ok(Mode::Retrieval);
        }
        Ok(mode)
    }

    /// Regex search.
    pub fn grep(&self, req: &GrepRequest, scope: &Scope) -> Result<GrepResult, Error> {
        Ok(okfkit_query::grep(&self.index(), req, scope)?)
    }

    /// Reads a document, a section or a line range.
    pub fn get(&self, req: &GetRequest, scope: &Scope) -> Result<GetResult, Error> {
        Ok(okfkit_query::get(&self.index(), req, scope)?)
    }

    /// Lists a directory.
    pub fn list(&self, dir: &str, scope: &Scope) -> Result<ListResult, Error> {
        Ok(okfkit_query::list(&self.index(), dir, scope)?)
    }

    /// Metadata query.
    pub fn query(&self, req: &QueryRequest, scope: &Scope) -> Result<QueryResult, Error> {
        Ok(okfkit_query::query(&self.index(), req, scope)?)
    }

    /// Links and backlinks of a document.
    pub fn links(&self, id: &str, scope: &Scope) -> Result<LinksResult, Error> {
        Ok(okfkit_query::links(&self.index(), id, scope)?)
    }

    /// Statistics of the visible bundle.
    pub fn stats(&self, scope: &Scope) -> Result<Stats, Error> {
        Ok(okfkit_query::stats(&self.index(), scope)?)
    }

    /// Lints the files on disk (not the index).
    pub fn lint(&self, config: &LintConfig) -> Result<LintReport, Error> {
        Ok(okfkit_lint::lint(&self.0.root, config)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundle_is_send_sync() {
        fn check<T: Send + Sync + Clone>() {}
        check::<Bundle>();
    }
}
