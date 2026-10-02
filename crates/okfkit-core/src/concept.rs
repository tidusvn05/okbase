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

    /// See [`ConceptId::is_reserved_in`].
    pub fn is_reserved_in(&self, content_index: bool) -> bool {
        self.id.is_reserved_in(content_index)
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

/// Directory names never walked, whatever the ignore files say: installed dependencies, which are
/// never the user's knowledge and must never be edited by okfkit. Build output (`target/`,
/// `dist/`, `site/`…) is left to `.gitignore`, because those names can also be real folders of a
/// knowledge base.
pub const DEFAULT_EXCLUDES: &[&str] = &[
    "node_modules",
    "bower_components",
    "jspm_packages",
    "__pycache__",
    "site-packages",
];

/// Per-bundle ignore file, same syntax as `.gitignore` (negations re-include files).
pub const IGNORE_FILE: &str = ".okfkitignore";

/// Lists the `.md` files of a bundle as sorted, `/`-separated relative paths.
///
/// Skips hidden files and directories (such as `.git` and `.okfkit`), agent instruction files at
/// the root ([`crate::id::AGENT_FILES`]), the directories in [`DEFAULT_EXCLUDES`], and anything
/// ignored by `.gitignore` files (of the bundle and its parent directories up to the repository)
/// or by [`IGNORE_FILE`]. Does not follow symbolic links, so the walk never leaves the bundle.
pub fn discover(root: &Path) -> Result<Vec<PathBuf>, Error> {
    let mut out = walk(root, &["md"])?;
    out.retain(|rel| {
        !(rel.parent().is_some_and(|p| p.as_os_str().is_empty())
            && rel
                .to_str()
                .is_some_and(|n| crate::id::AGENT_FILES.contains(&n)))
    });
    Ok(out)
}

/// Lists files with one of `extensions` (lowercase, without the dot) under `root`, with the same
/// skipping rules as [`discover`]. Sorted, `/`-separated relative paths.
pub fn walk(root: &Path, extensions: &[&str]) -> Result<Vec<PathBuf>, Error> {
    if !root.is_dir() {
        return Err(Error::Io {
            path: root.to_owned(),
            source: std::io::Error::new(std::io::ErrorKind::NotFound, "not a directory"),
        });
    }
    let walker = ignore::WalkBuilder::new(root)
        .hidden(true)
        .follow_links(false)
        .git_ignore(true)
        .git_exclude(true)
        .git_global(false)
        .require_git(false)
        .parents(true)
        .add_custom_ignore_filename(IGNORE_FILE)
        .filter_entry(|e| {
            !(e.depth() > 0
                && e.file_type().is_some_and(|t| t.is_dir())
                && e.file_name()
                    .to_str()
                    .is_some_and(|n| DEFAULT_EXCLUDES.contains(&n)))
        })
        .build();
    let mut out = Vec::new();
    for entry in walker {
        let entry = entry.map_err(|e| Error::Io {
            path: root.to_owned(),
            source: std::io::Error::other(e.to_string()),
        })?;
        if !entry.file_type().is_some_and(|t| t.is_file()) {
            continue;
        }
        let path = entry.path();
        let ext_ok = path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| extensions.iter().any(|x| x.eq_ignore_ascii_case(e)));
        if !ext_ok {
            continue;
        }
        if let Ok(rel) = path.strip_prefix(root) {
            out.push(rel.to_owned());
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
    fn discover_skips_dependencies_build_output_and_ignored_files() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path();
        for f in [
            "docs/a.md",
            "docs/drafts/wip.md",
            "docs/drafts/keep.md",
            "docs/node_modules/x/README.md",
            "docs/vendor/contracts.md",
            "docs/generated/api.md",
            "docs/.hidden/n.md",
        ] {
            let p = repo.join(f);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, "x").unwrap();
        }
        // The repository's .gitignore (a parent of the bundle) and the bundle's .okfkitignore apply.
        std::fs::write(repo.join(".gitignore"), "docs/generated/\n").unwrap();
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        std::fs::write(
            repo.join("docs/.okfkitignore"),
            "drafts/\n!drafts/keep.md\n",
        )
        .unwrap();
        let found: Vec<String> = discover(&repo.join("docs"))
            .unwrap()
            .iter()
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .collect();
        // `vendor/` is not a dependency folder by name alone: it may be real knowledge.
        assert_eq!(
            found,
            [
                "a.md",
                "docs/vendor/contracts.md".trim_start_matches("docs/")
            ],
            "{found:?}"
        );
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
