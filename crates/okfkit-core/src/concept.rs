//! Concept documents and bundle discovery.

use std::fs;
use std::path::{Path, PathBuf};

use crate::links::{Link, extract_links};
use crate::{ConceptId, Error, Frontmatter};

/// A markdown document in a bundle: a concept, or a reserved file (`index.md`, `log.md`).
#[derive(Debug, Clone, PartialEq)]
pub struct Concept {
    /// The concept ID (path without `.md`).
    pub id: ConceptId,
    /// The bundle-relative path, `/`-separated.
    pub path: String,
    /// The frontmatter block.
    pub frontmatter: Frontmatter,
    /// Everything after the frontmatter block, unchanged.
    pub body: String,
}

impl Concept {
    /// Parses a document from its bundle-relative path and text. Never fails on content.
    pub fn parse(rel_path: &Path, text: &str) -> Result<Self, Error> {
        let id = ConceptId::from_path(rel_path)?;
        let (frontmatter, body) = Frontmatter::split(text);
        Ok(Concept {
            path: id.path(),
            id,
            frontmatter,
            body: body.to_owned(),
        })
    }

    /// Reads and parses `root/rel_path`.
    pub fn read(root: &Path, rel_path: &Path) -> Result<Self, Error> {
        let full = root.join(rel_path);
        let bytes = fs::read(&full).map_err(|source| Error::Io {
            path: full.clone(),
            source,
        })?;
        let text = String::from_utf8(bytes).map_err(|_| Error::NotUtf8(full))?;
        Concept::parse(rel_path, &text)
    }

    /// The document text. Byte-identical to the input unless the frontmatter was edited.
    pub fn render(&self) -> String {
        let mut s = self.frontmatter.render();
        s.push_str(&self.body);
        s
    }

    /// Whether this is a reserved file (`index.md`, `log.md`) rather than a concept.
    pub fn is_reserved(&self) -> bool {
        self.id.is_reserved()
    }

    /// The `type` frontmatter value, if it is a string.
    pub fn concept_type(&self) -> Option<&str> {
        self.frontmatter.get_str("type")
    }

    /// Links in the body.
    pub fn links(&self) -> Vec<Link> {
        extract_links(&self.body)
    }
}

/// Lists the `.md` files of a bundle as sorted, `/`-separated relative paths.
///
/// Skips hidden files and directories (such as `.git` and `.okfkit`) and agent instruction
/// files at the root ([`crate::id::AGENT_FILES`]), and does not follow symbolic links, so the
/// walk never leaves the bundle.
pub fn discover(root: &Path) -> Result<Vec<PathBuf>, Error> {
    let mut out = Vec::new();
    let mut stack = vec![PathBuf::new()];
    while let Some(rel) = stack.pop() {
        let dir = root.join(&rel);
        let entries = fs::read_dir(&dir).map_err(|source| Error::Io {
            path: dir.clone(),
            source,
        })?;
        for entry in entries {
            let entry = entry.map_err(|source| Error::Io {
                path: dir.clone(),
                source,
            })?;
            let name = entry.file_name();
            let Some(name) = name.to_str() else { continue };
            if name.starts_with('.') {
                continue;
            }
            let ft = entry.file_type().map_err(|source| Error::Io {
                path: entry.path(),
                source,
            })?;
            if ft.is_dir() {
                stack.push(rel.join(name));
            } else if ft.is_file()
                && name.ends_with(".md")
                && !(rel.as_os_str().is_empty() && crate::id::AGENT_FILES.contains(&name))
            {
                out.push(rel.join(name));
            }
        }
    }
    out.sort();
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discover_skips_agent_files_at_the_root() {
        let tmp = tempfile::tempdir().unwrap();
        for f in ["AGENTS.md", "CLAUDE.md", "a.md", "docs/AGENTS.md"] {
            let p = tmp.path().join(f);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, "x").unwrap();
        }
        let found: Vec<String> = discover(tmp.path())
            .unwrap()
            .iter()
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .collect();
        // Only the root ones are agent instructions; a docs page named AGENTS.md is knowledge.
        assert_eq!(found, ["a.md", "docs/AGENTS.md"]);
    }

    #[test]
    fn parse_render() {
        let text = "---\ntype: Metric\n---\n\nSee [x](x.md).\n";
        let c = Concept::parse(Path::new("m/revenue.md"), text).unwrap();
        assert_eq!(
            (c.id.as_str(), c.path.as_str(), c.concept_type()),
            ("m/revenue", "m/revenue.md", Some("Metric"))
        );
        assert_eq!(c.body, "\nSee [x](x.md).\n");
        assert_eq!(c.render(), text);
        assert_eq!(c.links().len(), 1);
    }
}
