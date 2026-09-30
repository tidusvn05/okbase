/// Errors returned by `okfkit-standard`.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// Reading or parsing a document failed.
    #[error(transparent)]
    Core(#[from] okfkit_core::Error),
    /// The vocabulary document is malformed.
    #[error("invalid vocabulary in {path}: {message}")]
    InvalidVocabulary {
        /// Bundle-relative path of the vocabulary document.
        path: String,
        /// What is wrong.
        message: String,
    },
}
