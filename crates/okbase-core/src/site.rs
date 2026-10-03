//! Bundle profiles: plain OKF, or markdown that also feeds a documentation site or a notes vault.
//!
//! In OKF, `index.md` is reserved for directory listings. Documentation sites (MkDocs,
//! Docusaurus, Hugo, Mintlify) and Obsidian vaults use `index.md` as a normal content page (the
//! home of a folder). Under the `docs-site` and `vault` profiles okbase treats `index.md` as
//! content, builds directory listings when reading, and never asks for listing files, so the
//! user's site keeps working. The profile is detected from marker files in the bundle and its
//! parent directories up to the repository root, or set in `okbase.toml`:
//!
//! ```toml
//! [bundle]
//! profile = "okf"        # or "docs-site", "vault"
//! ```

use std::path::Path;

use serde::Serialize;

/// A documentation generator or notes app recognized by its marker files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SiteKind {
    /// Obsidian (`.obsidian/`).
    Obsidian,
    /// Docusaurus (`docusaurus.config.*`).
    Docusaurus,
    /// Mintlify (`mint.json`, `docs.json`).
    Mintlify,
    /// MkDocs (`mkdocs.yml`).
    Mkdocs,
    /// Hugo (`hugo.toml`, or `config.toml` next to `content/`).
    Hugo,
}

/// How okbase reads the bundle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case", tag = "profile", content = "site")]
pub enum Profile {
    /// Plain OKF: `index.md` is a directory listing.
    Okf,
    /// Markdown that also builds a documentation site: `index.md` is content.
    DocsSite(Option<SiteKind>),
    /// A notes vault (Obsidian): `index.md` is content; no listing files are created.
    Vault,
}

impl Profile {
    /// Whether `index.md` files are content pages (not listings).
    pub fn content_index(self) -> bool {
        !matches!(self, Profile::Okf)
    }

    /// Name as in `okbase.toml`.
    pub fn name(self) -> &'static str {
        match self {
            Profile::Okf => "okf",
            Profile::DocsSite(_) => "docs-site",
            Profile::Vault => "vault",
        }
    }
}

/// The site whose marker files are in `dir` itself.
pub fn site_markers(dir: &Path) -> Option<SiteKind> {
    let has = |p: &str| dir.join(p).exists();
    if has(".obsidian") {
        Some(SiteKind::Obsidian)
    } else if [
        "docusaurus.config.js",
        "docusaurus.config.ts",
        "docusaurus.config.mjs",
    ]
    .iter()
    .any(|p| has(p))
    {
        Some(SiteKind::Docusaurus)
    } else if has("mint.json") || has("docs.json") {
        Some(SiteKind::Mintlify)
    } else if has("mkdocs.yml") || has("mkdocs.yaml") {
        Some(SiteKind::Mkdocs)
    } else if has("hugo.toml") || (has("config.toml") && has("content")) {
        Some(SiteKind::Hugo)
    } else {
        None
    }
}

/// The site the bundle belongs to: markers in `root` or a parent directory, stopping at the
/// repository root (a directory with `.git`) and after a few levels.
pub fn find_site(root: &Path) -> Option<SiteKind> {
    let root = root.canonicalize().ok()?;
    let mut dir = Some(root.as_path());
    for _ in 0..6 {
        let d = dir?;
        if let Some(s) = site_markers(d) {
            return Some(s);
        }
        if d.join(".git").exists() {
            return None;
        }
        dir = d.parent();
    }
    None
}

/// The profile set in `<root>/okbase.toml` (`[bundle] profile`), if any.
pub fn configured_profile(root: &Path) -> Option<Profile> {
    let text = std::fs::read_to_string(root.join("okbase.toml")).ok()?;
    let doc: toml_edit::DocumentMut = text.parse().ok()?;
    match doc.get("bundle")?.get("profile")?.as_str()? {
        "okf" => Some(Profile::Okf),
        "docs-site" => Some(Profile::DocsSite(find_site(root))),
        "vault" => Some(Profile::Vault),
        _ => None,
    }
}

/// The bundle's profile: `okbase.toml` if set; else plain OKF when the root `index.md` declares
/// `okf_version`; else detected from site markers; else plain OKF.
pub fn profile(root: &Path) -> Profile {
    if let Some(p) = configured_profile(root) {
        return p;
    }
    if std::fs::read_to_string(root.join("index.md")).is_ok_and(|t| t.contains("okf_version")) {
        return Profile::Okf;
    }
    match find_site(root) {
        Some(SiteKind::Obsidian) => Profile::Vault,
        Some(s) => Profile::DocsSite(Some(s)),
        None => Profile::Okf,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detection_and_overrides() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path();
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        std::fs::create_dir_all(repo.join("docs/guide")).unwrap();
        // Plain folder.
        assert_eq!(profile(&repo.join("docs")), Profile::Okf);
        // mkdocs.yml at the repository root makes docs/ (and its subfolders) a docs site.
        std::fs::write(repo.join("mkdocs.yml"), "site_name: x\n").unwrap();
        assert_eq!(
            profile(&repo.join("docs")),
            Profile::DocsSite(Some(SiteKind::Mkdocs))
        );
        assert!(profile(&repo.join("docs/guide")).content_index());
        // A real OKF bundle inside it stays OKF.
        std::fs::write(
            repo.join("docs/guide/index.md"),
            "---\nokf_version: \"0.2\"\n---\n",
        )
        .unwrap();
        assert_eq!(profile(&repo.join("docs/guide")), Profile::Okf);
        // okbase.toml wins.
        std::fs::write(
            repo.join("docs/okbase.toml"),
            "[bundle]\nprofile = \"okf\"\n",
        )
        .unwrap();
        assert_eq!(profile(&repo.join("docs")), Profile::Okf);
        // Markers above the repository root are not followed.
        let outer = tmp.path().join("outer");
        std::fs::create_dir_all(outer.join("repo/.git")).unwrap();
        std::fs::write(outer.join("mkdocs.yml"), "x").unwrap();
        assert_eq!(profile(&outer.join("repo")), Profile::Okf);
        // Obsidian vault.
        let vault = tmp.path().join("vault");
        std::fs::create_dir_all(vault.join(".obsidian")).unwrap();
        assert_eq!(profile(&vault), Profile::Vault);
    }
}
