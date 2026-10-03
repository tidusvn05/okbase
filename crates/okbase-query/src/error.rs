/// Errors returned by `okbase-query`.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// The document or directory does not exist or is not visible in the scope.
    #[error("not found: {0}")]
    NotFound(String),
    /// A request argument is invalid.
    #[error("invalid argument: {0}")]
    InvalidArgument(String),
    /// Reading the index failed.
    #[error("index database: {0}")]
    Sqlite(#[from] rusqlite::Error),
}
