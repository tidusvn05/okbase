//! Embedding state of a bundle: the embedder (from the host or `okfkit.toml`,
//! created on first use because loading a model takes seconds), the shared
//! vector cache, and the in-memory vector store (refreshed after a sync).

use std::sync::{Arc, Mutex, OnceLock};

use okfkit_embed::Embedder;
use okfkit_search::VectorStore;

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

    /// Whether embeddings are configured (host embedder or `okfkit.toml`).
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
            EmbedConfig::Local { model } => Some(model.clone()),
            EmbedConfig::Api { model, .. } => Some(format!("api:{model}")),
        }
    }

    pub fn embedder(&self) -> Result<Arc<dyn Embedder>, Error> {
        if let Some(h) = &self.host {
            return Ok(h.clone());
        }
        self.loaded
            .get_or_init(|| create(&self.config).map_err(|e| e.to_string()))
            .clone()
            .map_err(Error::Embedding)
    }

    pub fn invalidate(&self) {
        *self.store.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }

    pub fn store(&self, index: &okfkit_index::Index) -> Result<Arc<VectorStore>, Error> {
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

fn create(config: &EmbedConfig) -> Result<Arc<dyn Embedder>, Error> {
    match config {
        EmbedConfig::Off => Err(Error::NoEmbedder),
        #[cfg(feature = "embed-local")]
        EmbedConfig::Local { model } => {
            Ok(Arc::new(okfkit_embed::LocalEmbedder::load(model, None)?))
        }
        #[cfg(not(feature = "embed-local"))]
        EmbedConfig::Local { .. } => {
            Err(okfkit_embed::Error::NotBuilt("embed-local", "embed-local").into())
        }
        #[cfg(feature = "embed-api")]
        EmbedConfig::Api {
            base_url,
            model,
            key_env,
        } => Ok(Arc::new(okfkit_embed::ApiEmbedder::new(
            okfkit_embed::ApiConfig {
                base_url: base_url.clone(),
                model: model.clone(),
                key_env: key_env.clone(),
                query_prefix: String::new(),
            },
        )?)),
        #[cfg(not(feature = "embed-api"))]
        EmbedConfig::Api { .. } => {
            Err(okfkit_embed::Error::NotBuilt("embed-api", "embed-api").into())
        }
    }
}

/// `<user cache>/okfkit/emb/vectors.sqlite`, or `OKFKIT_EMB_CACHE`.
pub(crate) fn cache_path() -> Result<std::path::PathBuf, Error> {
    if let Some(p) = std::env::var_os("OKFKIT_EMB_CACHE").filter(|v| !v.is_empty()) {
        return Ok(p.into());
    }
    okfkit_analyze::dict::user_cache_dir()
        .map(|c| c.join("okfkit").join("emb").join("vectors.sqlite"))
        .ok_or_else(|| {
            Error::Embedding("no user cache directory (set HOME or OKFKIT_EMB_CACHE)".into())
        })
}
