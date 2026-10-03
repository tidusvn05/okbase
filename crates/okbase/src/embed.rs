//! Embedding state of a bundle: the embedder (from the host or `okbase.toml`,
//! created on first use because loading a model takes seconds), the shared
//! vector cache, and the in-memory vector store (refreshed after a sync).

use std::sync::{Arc, Mutex, OnceLock};

use okbase_embed::Embedder;
use okbase_search::VectorStore;

use crate::Error;
use crate::config::EmbedConfig;

pub(crate) struct EmbedState {
    pub config: EmbedConfig,
    host: Option<Arc<dyn Embedder>>,
    loaded: OnceLock<Result<Arc<dyn Embedder>, String>>,
    store: Mutex<Option<Arc<VectorStore>>>,
    cache: Option<std::path::PathBuf>,
}

impl EmbedState {
    pub fn new(
        config: EmbedConfig,
        host: Option<Arc<dyn Embedder>>,
        cache: Option<std::path::PathBuf>,
    ) -> Self {
        EmbedState {
            config,
            host,
            loaded: OnceLock::new(),
            store: Mutex::new(None),
            cache,
        }
    }

    /// The vector cache file: the configured one, else the user-wide default.
    pub fn cache_path(&self) -> Result<std::path::PathBuf, Error> {
        match &self.cache {
            Some(p) => Ok(p.clone()),
            None => cache_path(),
        }
    }

    /// Whether embeddings are configured (host embedder or `okbase.toml`).
    pub fn enabled(&self) -> bool {
        self.host.is_some() || self.config != EmbedConfig::Off
    }

    /// The model id without loading the model.
    pub fn model_id(&self) -> Option<String> {
        if let Some(h) = &self.host {
            return Some(h.model_id().to_owned());
        }
        match &self.config {
            EmbedConfig::Off => None,
            // Custom models are stored under `custom:<name>@<hash>` (read from the manifest), so
            // vectors of replaced weights are never reused; an uninstalled one fails on load.
            EmbedConfig::Local { model } => {
                Some(okbase_embed::resolve_model_id(model).unwrap_or_else(|_| model.clone()))
            }
            EmbedConfig::Api { model, .. } => Some(format!("api:{model}")),
        }
    }

    pub fn embedder(&self) -> Result<Arc<dyn Embedder>, Error> {
        if let Some(h) = &self.host {
            return Ok(h.clone());
        }
        self.loaded
            .get_or_init(|| {
                create(&self.config).map_err(|e| match e {
                    // Kept without its "embeddings:" prefix: it is wrapped again below.
                    Error::Embedding(m) => m,
                    other => other.to_string(),
                })
            })
            .clone()
            .map_err(Error::Embedding)
    }

    pub fn invalidate(&self) {
        *self.store.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }

    pub fn store(&self, index: &okbase_index::Index) -> Result<Arc<VectorStore>, Error> {
        let mut guard = self.store.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(s) = guard.as_ref() {
            return Ok(s.clone());
        }
        let model = self.model_id().ok_or(Error::NoEmbedder)?;
        let s = Arc::new(VectorStore::load(index, &model)?);
        *guard = Some(s.clone());
        Ok(s)
    }
}

/// Loads a local model (built-in id or `custom:<name>`), downloading a built-in one on first use.
pub fn load_local_model(model: &str) -> Result<Arc<dyn Embedder>, Error> {
    create(&EmbedConfig::Local {
        model: model.to_owned(),
    })
}

fn create(config: &EmbedConfig) -> Result<Arc<dyn Embedder>, Error> {
    match config {
        EmbedConfig::Off => Err(Error::NoEmbedder),
        #[cfg(feature = "embed-local")]
        EmbedConfig::Local { model } => {
            Ok(Arc::new(okbase_embed::LocalEmbedder::load(model, None)?))
        }
        #[cfg(not(feature = "embed-local"))]
        EmbedConfig::Local { .. } => {
            Err(okbase_embed::Error::NotBuilt("embed-local", "embed-local").into())
        }
        #[cfg(feature = "embed-api")]
        EmbedConfig::Api {
            base_url,
            model,
            key_env,
        } => Ok(Arc::new(okbase_embed::ApiEmbedder::new(
            okbase_embed::ApiConfig {
                base_url: base_url.clone(),
                model: model.clone(),
                key_env: key_env.clone(),
                query_prefix: String::new(),
            },
        )?)),
        #[cfg(not(feature = "embed-api"))]
        EmbedConfig::Api { .. } => {
            Err(okbase_embed::Error::NotBuilt("embed-api", "embed-api").into())
        }
    }
}

/// `<user cache>/okbase/emb/vectors.sqlite`, or `OKBASE_EMB_CACHE`.
pub(crate) fn cache_path() -> Result<std::path::PathBuf, Error> {
    if let Some(p) = std::env::var_os("OKBASE_EMB_CACHE").filter(|v| !v.is_empty()) {
        return Ok(p.into());
    }
    okbase_analyze::dict::user_cache_dir()
        .map(|c| c.join("okbase").join("emb").join("vectors.sqlite"))
        .ok_or_else(|| {
            Error::Embedding("no user cache directory (set HOME or OKBASE_EMB_CACHE)".into())
        })
}
