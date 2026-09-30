use std::path::PathBuf;

/// Errors returned by `okfkit-core`.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// Reading a file failed.
    #[error("cannot read {path}: {source}")]
    Io {
        /// The file that could not be read.
        path: PathBuf,
        /// The underlying error.
        source: std::io::Error,
    },
    /// A file is not valid UTF-8.
    #[error("{0} is not valid UTF-8")]
    NotUtf8(PathBuf),
    /// A path cannot be used as a concept path (absolute, escapes the bundle, or not `.md`).
    #[error("invalid concept path {0:?}: expected a relative `.md` path inside the bundle")]
    InvalidPath(String),
    /// The frontmatter cannot be parsed, so it cannot be edited.
    #[error("invalid frontmatter: {0}")]
    InvalidFrontmatter(String),
    /// An edit was refused because it could not be applied without touching other keys.
    #[error("refused to edit frontmatter: {0}")]
    UnsafeEdit(String),
}
