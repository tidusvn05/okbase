//! Where the index lives.

use std::fs;
use std::path::{Path, PathBuf};

/// Where to keep the index and other okfkit state for a bundle.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum StateDir {
    /// `<bundle>/.okfkit/` if it exists or can be created, otherwise the user cache
    /// (`~/.cache/okfkit/bundles/<hash>` on Linux).
    #[default]
    Auto,
    /// Always the user cache, never inside the bundle.
    Cache,
    /// An explicit directory.
    Path(PathBuf),
}

/// Name of the in-bundle state directory.
pub const BUNDLE_STATE_DIR: &str = ".okfkit";

impl StateDir {
    /// Resolves the directory for the bundle at `root`, creating it if needed.
    pub fn resolve(&self, root: &Path) -> std::io::Result<PathBuf> {
        let dir = match self {
            StateDir::Path(p) => p.clone(),
            StateDir::Cache => cache_dir(root)?,
            StateDir::Auto => {
                let local = root.join(BUNDLE_STATE_DIR);
                if local.is_dir() || create_local(&local).is_ok() {
                    local
                } else {
                    cache_dir(root)?
                }
            }
        };
        fs::create_dir_all(&dir)?;
        Ok(dir)
    }
}

/// Creates `.okfkit/` with a `.gitignore` so the index never ends up in version control.
fn create_local(dir: &Path) -> std::io::Result<()> {
    fs::create_dir(dir)?;
    fs::write(
        dir.join(".gitignore"),
        "# Created by okfkit: rebuildable index and cache.\n*\n",
    )
}

/// The per-bundle directory in the user cache, keyed by the bundle's canonical path.
pub fn cache_dir(root: &Path) -> std::io::Result<PathBuf> {
    let canonical = root.canonicalize()?;
    let hash = blake3::hash(canonical.to_string_lossy().as_bytes()).to_hex();
    let base = user_cache_dir().ok_or_else(|| {
        std::io::Error::other("no user cache directory (set HOME or XDG_CACHE_HOME)")
    })?;
    Ok(base.join("okfkit").join("bundles").join(&hash[..16]))
}

fn user_cache_dir() -> Option<PathBuf> {
    let env = |k: &str| {
        std::env::var_os(k)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    if cfg!(windows) {
        env("LOCALAPPDATA")
    } else if cfg!(target_os = "macos") {
        env("HOME").map(|h| h.join("Library/Caches"))
    } else {
        env("XDG_CACHE_HOME").or_else(|| env("HOME").map(|h| h.join(".cache")))
    }
}
