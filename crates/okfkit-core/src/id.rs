//! Concept IDs and bundle paths.

use std::fmt;
use std::path::{Component, Path};

use crate::Error;

/// Reserved filenames that are never concept documents (OKF §3.1).
pub const RESERVED_FILES: [&str; 2] = ["index.md", "log.md"];

/// Instruction files for agent CLIs at the bundle root (Codex, Claude Code, Gemini CLI). They
/// tell agents how to work, they are not knowledge, and `okfkit agent install` may write them,
/// so okfkit never indexes or lints them.
pub const AGENT_FILES: [&str; 3] = ["AGENTS.md", "CLAUDE.md", "GEMINI.md"];

/// A concept ID: the bundle-relative path of the file with `/` separators and without `.md`.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(transparent)]
pub struct ConceptId(String);

impl ConceptId {
    /// Builds an ID from a bundle-relative path to a `.md` file.
    pub fn from_path(rel: &Path) -> Result<Self, Error> {
        let path =
            normalize_rel(rel).ok_or_else(|| Error::InvalidPath(rel.display().to_string()))?;
        path.strip_suffix(".md")
            .filter(|s| !s.is_empty() && !s.ends_with('/'))
            .map(|s| ConceptId(s.to_owned()))
            .ok_or_else(|| Error::InvalidPath(rel.display().to_string()))
    }

    /// Builds an ID from a normalized bundle-relative path without `.md` (`a/b`).
    pub fn new(id: impl Into<String>) -> Result<Self, Error> {
        let id = id.into();
        let ok = !id.is_empty()
            && id
                .split('/')
                .all(|s| !s.is_empty() && s != "." && s != "..")
            && !id.contains('\\');
        if ok {
            Ok(ConceptId(id))
        } else {
            Err(Error::InvalidPath(id))
        }
    }

    /// The ID as a string.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The bundle-relative path of the file (`a/b.md`).
    pub fn path(&self) -> String {
        format!("{}.md", self.0)
    }

    /// The directory part of the ID (`a` for `a/b`, empty at the root).
    pub fn dir(&self) -> &str {
        self.0.rsplit_once('/').map_or("", |(d, _)| d)
    }

    /// The last path segment (`b` for `a/b`).
    pub fn name(&self) -> &str {
        self.0.rsplit_once('/').map_or(&self.0, |(_, n)| n)
    }

    /// Whether this ID names a reserved file (`index`, `log`) at any level.
    pub fn is_reserved(&self) -> bool {
        RESERVED_FILES
            .iter()
            .any(|f| f.strip_suffix(".md") == Some(self.name()))
    }
}

impl fmt::Display for ConceptId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for ConceptId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Normalizes a relative path to `/`-separated form. Returns `None` if it is absolute or escapes the root.
pub fn normalize_rel(rel: &Path) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    for c in rel.components() {
        match c {
            Component::Normal(s) => parts.push(s.to_str()?.to_owned()),
            Component::CurDir => {}
            Component::ParentDir => {
                parts.pop()?;
            }
            Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids() {
        let id = ConceptId::from_path(Path::new("tables/./orders.md")).unwrap();
        assert_eq!(id.as_str(), "tables/orders");
        assert_eq!(
            (id.dir(), id.name(), id.path().as_str()),
            ("tables", "orders", "tables/orders.md")
        );
        assert!(
            ConceptId::from_path(Path::new("x/index.md"))
                .unwrap()
                .is_reserved()
        );
        assert!(ConceptId::from_path(Path::new("../x.md")).is_err());
        assert!(ConceptId::from_path(Path::new("/x.md")).is_err());
        assert!(ConceptId::from_path(Path::new("x.txt")).is_err());
        assert!(ConceptId::new("a/../b").is_err());
    }
}
