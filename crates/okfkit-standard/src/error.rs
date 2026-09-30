/// Errors returned by `okfkit-standard`.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// Reading or parsing a document failed.
    #[error(transparent)]
    Core(#[from] okfkit_core::Error),
    /// A vocabulary or type-schema document is malformed.
    #[error("invalid {path}: {message}")]
    InvalidMeta {
        /// Bundle-relative path of the vocabulary document.
        path: String,
        /// What is wrong.
        message: String,
    },
}
