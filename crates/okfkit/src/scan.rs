//! `scan`: what kind of folder this is, and which folders in it should be bundles
//! (docs/PLAN-usecases.md §2). Lexical and read-only; it never writes an index.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::{Error, Level};
use okfkit_core::{Concept, FrontmatterState, Profile};

/// What the scanned folder is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FolderKind {
    /// No markdown and no documents okfkit could import.
    Empty,
    /// No markdown, only source documents (PDF, Word, PowerPoint, HTML…), read directly.
    NonMarkdown,
    /// A software repository; knowledge lives in some of its folders.
    SoftwareRepo,
    /// A folder of markdown (OKF or not), itself the bundle candidate.
    Markdown,
}

/// What a candidate bundle looks like.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CandidateKind {
    /// ≥ 80% of the documents have OKF frontmatter.
    Okf,
    /// 20–80%: an OKF bundle with some files to fix.
    PartialOkf,
    /// Markdown that also builds a documentation site (index.md is content).
    DocsSite,
    /// A notes vault.
    Vault,
    /// Plain markdown.
    Markdown,
    /// Source documents only (PDF, Word, PowerPoint, HTML…), read directly.
    Sources,
}

/// A folder that could be a bundle.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Candidate {
    /// Path relative to the scanned folder (`.` for itself).
    pub path: String,
    /// What it looks like.
    pub kind: CandidateKind,
    /// Markdown documents (index and log files excluded).
    pub docs: usize,
    /// Documents with a valid frontmatter `type`.
    pub conformant: usize,
    /// Source documents read directly (PDF, Word, PowerPoint, HTML…).
    pub sources: usize,
    /// Documents that need fixing (missing or invalid frontmatter), first 20.
    pub to_fix: Vec<String>,
    /// okfkit level.
    pub level: Option<Level>,
    /// Bundle profile (okf, docs-site, vault).
    pub profile: &'static str,
    /// Why it is a candidate.
    pub reason: String,
}

/// The scan.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Scan {
    /// The scanned folder.
    pub root: PathBuf,
    /// What it is.
    pub kind: FolderKind,
    /// What was recognized (git, Cargo.toml, mkdocs.yml…).
    pub signals: Vec<String>,
    /// Markdown files (after ignore rules).
    pub markdown: usize,
    /// Other document files by extension (pdf, docx, html…), which import will handle.
    pub other_documents: BTreeMap<String, usize>,
    /// Spreadsheets (csv, tsv, xlsx), served by the data module.
    pub spreadsheets: usize,
    /// Candidate bundles, best first.
    pub candidates: Vec<Candidate>,
    /// Index into `candidates` of the recommended one, when the choice is clear.
    pub recommended: Option<usize>,
}

/// Manifests that make a folder a software project.
const MANIFESTS: &[&str] = &[
    "Cargo.toml",
    "package.json",
    "pyproject.toml",
    "setup.py",
    "go.mod",
    "pom.xml",
    "build.gradle",
    "build.gradle.kts",
    "Gemfile",
    "composer.json",
    "mix.exs",
    "Package.swift",
    "CMakeLists.txt",
    "Makefile",
];
/// Usual homes of documentation in a repository.
const DOC_DIRS: &[&str] = &[
    "docs",
    "doc",
    "documentation",
    "website/docs",
    "wiki",
    "handbook",
    "knowledge",
    "kb",
    "content",
];
const CODE_EXTS: &[&str] = &[
    "rs", "py", "js", "ts", "tsx", "go", "java", "kt", "rb", "php", "cs", "swift", "c", "cpp",
];

fn is_listing_name(p: &Path) -> bool {
    matches!(
        p.file_name().and_then(|n| n.to_str()),
        Some("index.md" | "log.md")
    )
}

/// Measures one folder as a bundle.
fn measure(root: &Path, dir: &Path, reason: &str) -> Result<Option<Candidate>, Error> {
    let files = okfkit_core::walk(dir, &["md"]).map_err(|e| Error::Io(e.to_string()))?;
    let profile = okfkit_core::site::profile(dir);
    let content_index = profile.content_index();
    let (mut docs, mut conformant, mut to_fix) = (0, 0, Vec::new());
    for rel in &files {
        if is_listing_name(rel) && !(content_index && rel.ends_with("index.md")) {
            continue;
        }
        if okfkit_core::id::AGENT_FILES.contains(&rel.to_string_lossy().as_ref()) {
            continue;
        }
        docs += 1;
        let ok = Concept::read(dir, rel).is_ok_and(|c| {
            c.frontmatter.state() == FrontmatterState::Valid && c.concept_type().is_some()
        });
        if ok {
            conformant += 1;
        } else if to_fix.len() < 20 {
            to_fix.push(rel.to_string_lossy().replace('\\', "/"));
        }
    }
    let sources = okfkit_core::walk(dir, okfkit_index::source_extensions())
        .map(|v| v.len())
        .unwrap_or(0);
    if docs == 0 && sources == 0 {
        return Ok(None);
    }
    let share = conformant as f64 / docs.max(1) as f64;
    let kind = match profile {
        _ if docs == 0 => CandidateKind::Sources,
        Profile::Vault => CandidateKind::Vault,
        Profile::DocsSite(_) => CandidateKind::DocsSite,
        Profile::Okf if share >= 0.8 => CandidateKind::Okf,
        Profile::Okf if share >= 0.2 => CandidateKind::PartialOkf,
        Profile::Okf => CandidateKind::Markdown,
    };
    let level = okfkit_standard::assess_bundle(dir, okfkit_standard::VOCABULARY_PATH)
        .ok()
        .and_then(|a| a.level);
    let rel = dir.strip_prefix(root).unwrap_or(dir);
    let path = if rel.as_os_str().is_empty() {
        ".".to_owned()
    } else {
        rel.to_string_lossy().replace('\\', "/")
    };
    Ok(Some(Candidate {
        path,
        kind,
        docs,
        conformant,
        sources,
        to_fix,
        level,
        profile: profile.name(),
        reason: reason.to_owned(),
    }))
}

/// Directories (depth ≤ 3) that hold an OKF bundle: a root `index.md` with `okf_version`.
fn nested_okf(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(md) = okfkit_core::walk(root, &["md"]) else {
        return out;
    };
    for rel in md {
        if rel.file_name().and_then(|n| n.to_str()) != Some("index.md") {
            continue;
        }
        let Some(dir) = rel.parent().filter(|p| !p.as_os_str().is_empty()) else {
            continue;
        };
        if dir.components().count() > 3 {
            continue;
        }
        if std::fs::read_to_string(root.join(&rel)).is_ok_and(|t| t.contains("okf_version"))
            && !out.iter().any(|o: &PathBuf| dir.starts_with(o))
        {
            out.push(dir.to_owned());
        }
    }
    out
}

/// Scans `root`.
pub fn scan(root: &Path) -> Result<Scan, Error> {
    if !root.is_dir() {
        return Err(Error::NotADirectory(root.to_owned()));
    }
    let has = |p: &str| root.join(p).exists();
    let mut signals = Vec::new();
    if has(".git") {
        signals.push("git repository".to_owned());
    }
    let manifests: Vec<&str> = MANIFESTS.iter().copied().filter(|m| has(m)).collect();
    signals.extend(manifests.iter().map(|m| (*m).to_owned()));
    if let Some(site) = okfkit_core::site::site_markers(root) {
        signals.push(format!("{site:?} site").to_lowercase());
    }
    let walk = |exts: &[&str]| okfkit_core::walk(root, exts).unwrap_or_default();
    let markdown = walk(&["md"]).len();
    let mut other_documents = BTreeMap::new();
    for f in walk(okfkit_index::source_extensions()) {
        if let Some(ext) = f.extension().and_then(|e| e.to_str()) {
            *other_documents.entry(ext.to_lowercase()).or_default() += 1;
        }
    }
    let spreadsheets = walk(&["csv", "tsv", "xlsx"]).len();
    let code_files = walk(CODE_EXTS).len();
    // A manifest, or a repository where code is not a side note to the markdown.
    let software =
        !manifests.is_empty() || (has(".git") && code_files > 0 && code_files * 3 >= markdown);
    let site_root = okfkit_core::site::site_markers(root).is_some();
    if software && manifests.is_empty() {
        signals.push(format!("{code_files} source files"));
    }

    let mut candidates = Vec::new();
    let mut seen: Vec<PathBuf> = Vec::new();
    let mut add = |dir: PathBuf, reason: &str, out: &mut Vec<Candidate>| -> Result<(), Error> {
        if seen.contains(&dir) {
            return Ok(());
        }
        seen.push(dir.clone());
        if let Some(c) = measure(root, &dir, reason)? {
            out.push(c);
        }
        Ok(())
    };
    for d in nested_okf(root) {
        add(
            root.join(&d),
            "OKF bundle (index.md declares okf_version)",
            &mut candidates,
        )?;
    }
    // A folder that is already a bundle (root index.md, _meta/, okfkit.toml) keeps its
    // subfolders (knowledge/, policies/…) as part of it.
    let root_is_bundle = !software && (has("index.md") || has("_meta") || has("okfkit.toml"));
    for d in DOC_DIRS.iter().filter(|_| !root_is_bundle) {
        let dir = root.join(d);
        if dir.is_dir() {
            add(dir, "documentation / knowledge folder", &mut candidates)?;
        }
    }
    if !(software || site_root) || candidates.is_empty() {
        add(root.to_owned(), "the folder itself", &mut candidates)?;
    }
    // A subfolder that holds most of the folder's documents is the real bundle: the rest is
    // a README, a changelog and the like.
    if let Some(i) = candidates
        .iter()
        .position(|c| c.path == ".")
        .filter(|_| !root_is_bundle)
    {
        let root_docs = candidates[i].docs;
        if candidates
            .iter()
            .any(|c| c.path != "." && c.docs * 10 >= root_docs * 8)
        {
            candidates.remove(i);
        }
    }
    // Best first: OKF, then partial OKF, then by size.
    let rank = |c: &Candidate| match c.kind {
        CandidateKind::Okf => 0,
        CandidateKind::PartialOkf => 1,
        CandidateKind::DocsSite | CandidateKind::Vault => 2,
        CandidateKind::Markdown => 3,
        CandidateKind::Sources => 4,
    };
    candidates.sort_by(|a, b| rank(a).cmp(&rank(b)).then(b.docs.cmp(&a.docs)));
    let recommended = match candidates.len() {
        0 => None,
        1 => Some(0),
        _ if rank(&candidates[0]) < rank(&candidates[1]) => Some(0),
        _ => None,
    };
    let kind = if markdown == 0 {
        if other_documents.is_empty() {
            FolderKind::Empty
        } else {
            FolderKind::NonMarkdown
        }
    } else if software {
        FolderKind::SoftwareRepo
    } else {
        FolderKind::Markdown
    };
    Ok(Scan {
        root: root.to_owned(),
        kind,
        signals,
        markdown,
        other_documents,
        spreadsheets,
        candidates,
        recommended,
    })
}

impl Scan {
    /// Text form.
    pub fn to_text(&self) -> String {
        let kind = match self.kind {
            FolderKind::Empty => "empty (no documents)",
            FolderKind::NonMarkdown => "source documents only (read directly)",
            FolderKind::SoftwareRepo => "software repository",
            FolderKind::Markdown => "markdown folder",
        };
        let mut out = format!(
            "{}: {kind}{}\nmarkdown files: {}{}{}\n",
            self.root.display(),
            if self.signals.is_empty() {
                String::new()
            } else {
                format!(" ({})", self.signals.join(", "))
            },
            self.markdown,
            if self.other_documents.is_empty() {
                String::new()
            } else {
                format!(
                    "; other documents: {}",
                    self.other_documents
                        .iter()
                        .map(|(k, v)| format!("{v} {k}"))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            },
            if self.spreadsheets > 0 {
                format!("; spreadsheets: {}", self.spreadsheets)
            } else {
                String::new()
            }
        );
        if !self.candidates.is_empty() {
            out.push_str("\nCandidate bundles\n");
        }
        for (i, c) in self.candidates.iter().enumerate() {
            out.push_str(&format!(
                "  {} {:<24} {:?}, {} docs ({} with OKF frontmatter), level {}, profile {} — {}\n",
                if Some(i) == self.recommended {
                    "→"
                } else {
                    " "
                },
                c.path,
                c.kind,
                c.docs,
                c.conformant,
                c.level.map_or("below L0".into(), |l| l.to_string()),
                c.profile,
                c.reason
            ));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(root: &Path, rel: &str, text: &str) {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }

    const OKF: &str = "---\ntype: Guide\ntitle: T\ndescription: D.\n---\n# T\n";

    #[test]
    fn classifies_real_folders() {
        let tmp = tempfile::tempdir().unwrap();
        // Empty.
        let empty = tmp.path().join("empty");
        std::fs::create_dir_all(&empty).unwrap();
        let s = scan(&empty).unwrap();
        assert_eq!((s.kind, s.candidates.len()), (FolderKind::Empty, 0));
        // PDFs only.
        write(&tmp.path().join("pdfs"), "a.pdf", "%PDF");
        assert_eq!(
            scan(&tmp.path().join("pdfs")).unwrap().kind,
            FolderKind::NonMarkdown
        );
        // A software repository with docs/ and dependencies.
        let repo = tmp.path().join("repo");
        write(&repo, "Cargo.toml", "[package]\n");
        write(&repo, "README.md", "# R\n");
        write(&repo, "node_modules/x/README.md", "# x\n");
        write(&repo, "mkdocs.yml", "site_name: x\n");
        write(&repo, "docs/index.md", "# Home\n");
        write(&repo, "docs/guide.md", "# Guide\n");
        let s = scan(&repo).unwrap();
        assert_eq!(s.kind, FolderKind::SoftwareRepo);
        assert_eq!(s.candidates[0].path, "docs");
        assert_eq!(s.candidates[0].kind, CandidateKind::DocsSite);
        assert_eq!(
            s.candidates[0].docs, 2,
            "index.md is content in a docs site"
        );
        assert_eq!(s.recommended, Some(0));
        // A repository with an OKF bundle for a bot.
        let bot = tmp.path().join("bot");
        write(&bot, ".git/HEAD", "x");
        write(&bot, "app/bot.py", "print(1)\n");
        write(&bot, "README.md", "# Bot\n");
        write(
            &bot,
            "knowledge/index.md",
            "---\nokf_version: \"0.2\"\n---\n# K\n",
        );
        for i in 0..5 {
            write(&bot, &format!("knowledge/p{i}.md"), OKF);
        }
        let s = scan(&bot).unwrap();
        assert_eq!(s.candidates[s.recommended.unwrap()].path, "knowledge");
        assert_eq!(s.candidates[0].kind, CandidateKind::Okf);
        // OKF with a few broken files.
        let partial = tmp.path().join("partial");
        for i in 0..8 {
            write(&partial, &format!("p{i}.md"), OKF);
        }
        write(&partial, "loose.md", "# no frontmatter\n");
        write(&partial, "bad.md", "---\ntitle: a: b\n---\n");
        let s = scan(&partial).unwrap();
        let c = &s.candidates[0];
        assert_eq!(c.path, ".");
        assert_eq!((c.kind, c.docs, c.conformant), (CandidateKind::Okf, 10, 8));
        assert_eq!(c.to_fix, ["bad.md", "loose.md"]);
        assert_eq!(s.kind, FolderKind::Markdown);
        // A bundle laid out as recommended (index.md at the root, documents in knowledge/) is
        // one bundle, not a candidate per subfolder.
        let laid_out = tmp.path().join("laid-out");
        write(&laid_out, "index.md", "# Index\n");
        for i in 0..4 {
            write(&laid_out, &format!("knowledge/p{i}.md"), OKF);
        }
        let s = scan(&laid_out).unwrap();
        assert_eq!(s.candidates.len(), 1);
        assert_eq!(s.candidates[0].path, ".");
    }
}
