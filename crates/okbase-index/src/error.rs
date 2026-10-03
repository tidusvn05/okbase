use std::path::PathBuf;

/// Errors returned by `okbase-index`.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// A filesystem operation failed.
    #[error("{path}: {source}")]
    Io {
        /// The path involved.
        path: PathBuf,
        /// The underlying error.
        source: std::io::Error,
    },
    /// A database operation failed.
    #[error("index database: {0}")]
    Sqlite(#[from] rusqlite::Error),
    /// Reading the bundle failed.
    #[error(transparent)]
    Core(#[from] okbase_core::Error),
}
