//! Opt-in embeddings for okfkit (PLAN §4.3, module `embed-local` / `embed-api`).
//!
//! Nothing here is part of the default `okfkit` build: the core stays lexical and
//! model-free. Models are never bundled; local models are downloaded into the
//! user cache on first use, and models with their own terms (Gemma) need an
//! explicit acceptance first.
//!
//! - [`Embedder`]: query and document embeddings, L2-normalized.
//! - [`models`]: the supported local models (EmbeddingGemma 300M Q4, bge-m3 int8).
//! - [`VectorCache`]: vectors keyed by model and text hash, shared between bundles.

pub mod cache;
pub mod custom;
pub mod models;

#[cfg(feature = "api")]
mod api;
#[cfg(feature = "local")]
mod local;

pub use cache::{VectorCache, embed_documents_cached};
pub use custom::{
    CustomModel, custom_models, find_custom, install_custom, remove_custom, resolve_model_id,
};
pub use models::{ModelInfo, accept_license, find_model, license_accepted, models, models_dir};

#[cfg(feature = "api")]
pub use api::{ApiConfig, ApiEmbedder};
#[cfg(feature = "local")]
pub use local::LocalEmbedder;

/// Errors returned by `okfkit-embed`.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// The model is unknown.
    #[error(
        "unknown embedding model `{0}`; see `okfkit embed models` (built-in ids, custom:<name>, api:<model>)"
    )]
    UnknownModel(String),
    /// The model's license must be accepted before it is downloaded.
    #[error(
        "{model} is released under {license} ({url}); accept it first with `okfkit embed enable --model {model} --accept-license`"
    )]
    LicenseNotAccepted {
        /// Model id.
        model: String,
        /// License name.
        license: String,
        /// License URL.
        url: String,
    },
    /// Downloading or loading a model failed.
    #[error("embedding model: {0}")]
    Model(String),
    /// The embeddings API failed.
    #[error("embeddings API: {0}")]
    Api(String),
    /// The vector cache failed.
    #[error("vector cache: {0}")]
    Cache(#[from] rusqlite::Error),
    /// A module was not compiled in.
    #[error(
        "{0} is not available in this build (rebuild with the `{1}` feature, or use okfkit-full)"
    )]
    NotBuilt(&'static str, &'static str),
}

/// A text embedding model.
pub trait Embedder: Send + Sync {
    /// Stable model id, part of the cache key (`embeddinggemma-300m-q4`, `api:text-embedding-3-small`).
    fn model_id(&self) -> &str;
    /// Embeds search queries (with the model's query prompt). Vectors are L2-normalized.
    fn embed_queries(&self, queries: &[String]) -> Result<Vec<Vec<f32>>, Error>;
    /// Embeds documents given as `(title, text)` (with the model's document prompt). L2-normalized.
    fn embed_documents(&self, docs: &[(String, String)]) -> Result<Vec<Vec<f32>>, Error>;
}

impl std::fmt::Debug for dyn Embedder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Embedder({})", self.model_id())
    }
}

/// Scales `v` to unit length (no-op for the zero vector).
pub fn normalize(mut v: Vec<f32>) -> Vec<f32> {
    let n = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if n > 0.0 {
        v.iter_mut().for_each(|x| *x /= n);
    }
    v
}

/// Dot product (cosine similarity for normalized vectors).
pub fn dot(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalized_dot_is_cosine() {
        let a = normalize(vec![3.0, 4.0]);
        assert!((a[0] - 0.6).abs() < 1e-6 && (a[1] - 0.8).abs() < 1e-6);
        assert!((dot(&a, &a) - 1.0).abs() < 1e-6);
        assert_eq!(normalize(vec![0.0, 0.0]), [0.0, 0.0]);
    }
}
