//! `okfkit adopt`: turn a folder of plain markdown (Obsidian, Docusaurus, Hugo,
//! Mintlify, MkDocs or nothing in particular) into an okfkit/OKF bundle, without
//! a language model.
//!
//! [`plan`] reads the folder and computes every change; it never writes.
//! [`apply_to`] writes the result to a new directory, [`apply_in_place`] edits
//! the folder itself. Existing frontmatter values are never overwritten: adopt
//! only fills what is missing, through `okfkit-core`'s round-trip editing, and
//! records machine-made values in `generated` (OKF v0.2) so people know what to
//! verify.

pub mod heuristics;
pub mod scaffold;
pub mod vocab;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use okfkit_core::{Concept, ConceptId, FrontmatterState, Value, discover};
use okfkit_standard::{
    Assessment, Level, VOCABULARY_PATH, Vocabulary, assess_profile, meta, normalize_tag,
};
use serde::Serialize;
use serde_json::json;

pub use heuristics::{Site, detect_site};

/// Errors returned by `okfkit-adopt`.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// Reading the folder failed.
    #[error(transparent)]
    Core(#[from] okfkit_core::Error),
    /// Writing failed.
    #[error("{path}: {source}")]
    Io {
        /// The path.
        path: PathBuf,
        /// The underlying error.
        source: std::io::Error,
    },
    /// The target directory already exists and is not empty.
    #[error("{0} already exists and is not empty; choose a new directory")]
    OutExists(PathBuf),
    /// The request does not fit the folder (it already has documents, a file exists…).
    #[error("{0}")]
    Invalid(String),
}

/// Adopt settings.
#[derive(Debug, Clone)]
pub struct AdoptOptions {
    /// Target level: `L1` (titles, descriptions, index.md) or `L2` (adds lang, status, updated, tags).
    pub level: Level,
    /// Today's date (`YYYY-MM-DD`), for `generated.at`, `updated` fallbacks and the log entry.
    pub today: String,
    /// Actor recorded in `generated.by`.
    pub actor: String,
    /// Only change these documents (bundle-relative paths, or directory prefixes ending in `/`);
    /// empty: every document. With a selection, no index.md or log.md is created.
    pub only: Vec<String>,
}

impl AdoptOptions {
    /// L1 adoption dated `today`.
    pub fn new(today: &str) -> Self {
        AdoptOptions {
            level: Level::L1,
            today: today.to_owned(),
            actor: format!("okfkit-adopt/{}", env!("CARGO_PKG_VERSION")),
            only: Vec::new(),
        }
    }

    fn selected(&self, path: &str) -> bool {
        self.only.is_empty()
            || self
                .only
                .iter()
                .map(|p| p.trim_start_matches("./"))
                .any(|p| p == path || (p.ends_with('/') && path.starts_with(p)))
    }
}

/// What happens to one file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "action", rename_all = "lowercase")]
pub enum ChangeKind {
    /// A new file.
    Create,
    /// Frontmatter or content edited.
    Modify,
    /// Moved from another path (and possibly edited).
    Rename {
        /// The old bundle-relative path.
        from: String,
    },
}

/// A change to one file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Change {
    /// Bundle-relative path after the change.
    pub path: String,
    /// What happens.
    #[serde(flatten)]
    pub kind: ChangeKind,
    /// Human-readable notes, such as `+ description (from summary)`.
    pub notes: Vec<String>,
    /// The new file content.
    #[serde(skip)]
    pub content: String,
}

/// The adoption plan.
#[derive(Debug, Clone, Serialize)]
pub struct Plan {
    /// The detected site kind.
    pub site: Site,
    /// Markdown documents found.
    pub documents: usize,
    /// Level before adoption (`None`: below L0).
    pub level_before: Option<Level>,
    /// Level the result will reach.
    pub level_after: Option<Level>,
    /// Changes, by path.
    pub changes: Vec<Change>,
    /// Descriptions taken from the body because nothing better existed; worth a human (or agent) look.
    pub weak_descriptions: Vec<String>,
    /// Files left alone: (path, reason).
    pub skipped: Vec<(String, String)>,
    /// What still blocks the target level after adoption (code → count).
    pub remaining: BTreeMap<String, usize>,
}

/// The id a content page named `index.md` moves to.
fn overview_path(dir: &str, taken: &BTreeSet<String>) -> String {
    let prefix = if dir.is_empty() {
        String::new()
    } else {
        format!("{dir}/")
    };
    let mut n = 1;
    loop {
        let name = if n == 1 {
            "overview".to_owned()
        } else {
            format!("overview-{n}")
        };
        let p = format!("{prefix}{name}.md");
        if !taken.contains(&p) {
            return p;
        }
        n += 1;
    }
}

/// Whether an `index.md` is a listing (OKF §8) rather than a content page.
fn is_listing(doc: &Concept) -> bool {
    let fm_ok = !doc.frontmatter.is_present()
        || (doc.id.dir().is_empty() && doc.frontmatter.keys().all(|k| k == "okf_version"));
    if !fm_ok {
        return false;
    }
    let lines: Vec<&str> = doc
        .body
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();
    let links = lines
        .iter()
        .filter(|l| (l.starts_with("* [") || l.starts_with("- [")) && l.contains("]("))
        .count();
    !lines.is_empty() && links * 2 >= lines.len()
}

/// Computes every change needed to bring the folder at `root` to `opts.level`. Writes nothing.
pub fn plan(root: &Path, opts: &AdoptOptions) -> Result<Plan, Error> {
    let mut site = detect_site(root);
    if site == Site::Plain {
        // Markers of the site may sit in a parent (mkdocs.yml next to docs/).
        site = match okfkit_core::site::find_site(root) {
            Some(okfkit_core::SiteKind::Obsidian) => Site::Obsidian,
            Some(okfkit_core::SiteKind::Docusaurus) => Site::Docusaurus,
            Some(okfkit_core::SiteKind::Mintlify) => Site::Mintlify,
            Some(okfkit_core::SiteKind::Mkdocs) => Site::Mkdocs,
            Some(okfkit_core::SiteKind::Hugo) => Site::Hugo,
            None => Site::Plain,
        };
    }
    // Sites and vaults use index.md as a content page: keep it, add no listings or log.
    let content_index = okfkit_core::site::profile(root).content_index();
    let mut originals = Vec::new();
    let mut skipped = Vec::new();
    for rel in discover(root)? {
        match Concept::read(root, &rel) {
            Ok(doc) => originals.push(doc),
            Err(e) => skipped.push((rel.to_string_lossy().replace('\\', "/"), e.to_string())),
        }
    }
    let vocab = Vocabulary::load(root, VOCABULARY_PATH).ok().flatten();
    let level_before = assess_profile(&originals, vocab.as_ref(), content_index).level;

    let mut taken: BTreeSet<String> = originals.iter().map(|d| d.path.clone()).collect();
    let mut changes: BTreeMap<String, Change> = BTreeMap::new();
    let mut result: Vec<Concept> = Vec::new();
    let mut weak = Vec::new();
    let mut edited = 0;

    for original in &originals {
        let mut doc = original.clone();
        let mut kind = ChangeKind::Modify;
        let mut notes = Vec::new();
        if !opts.selected(&original.path) {
            result.push(doc);
            continue;
        }
        if doc.id.name() == "index" && !content_index && !is_listing(&doc) {
            let to = overview_path(doc.id.dir(), &taken);
            taken.insert(to.clone());
            let id = ConceptId::from_path(Path::new(&to))?;
            notes.push(format!(
                "content page moved out of the reserved name {} (links to it may need updating)",
                original.path
            ));
            kind = ChangeKind::Rename {
                from: original.path.clone(),
            };
            doc = Concept {
                id,
                path: to,
                frontmatter: doc.frontmatter,
                body: doc.body,
            };
        }
        if doc.is_reserved_in(content_index) {
            result.push(doc);
            continue;
        }
        match doc.frontmatter.state() {
            FrontmatterState::Invalid | FrontmatterState::Unterminated => {
                skipped.push((
                    doc.path.clone(),
                    "frontmatter cannot be parsed; fix it by hand".into(),
                ));
                result.push(doc);
                continue;
            }
            FrontmatterState::Absent | FrontmatterState::Valid => {}
        }
        let mut generated = Vec::new();
        fill_fields(
            &mut doc,
            opts,
            root,
            &original.path,
            &mut notes,
            &mut generated,
            &mut weak,
        );
        if !generated.is_empty() && doc.frontmatter.get("generated").is_none() {
            let value = json!({"by": opts.actor, "at": opts.today, "fields": generated});
            if doc.frontmatter.set("generated", value).is_ok() {
                notes.push("+ generated (records the fields okfkit filled in)".into());
            }
        }
        if !notes.is_empty() {
            edited += 1;
            changes.insert(
                doc.path.clone(),
                Change {
                    path: doc.path.clone(),
                    kind,
                    notes,
                    content: doc.render(),
                },
            );
        }
        result.push(doc);
    }

    // index.md for every directory that holds concepts, and its ancestors.
    let mut dirs: BTreeSet<String> = BTreeSet::new();
    for d in result
        .iter()
        .filter(|d| !d.is_reserved() && !d.path.starts_with("_meta/"))
    {
        let mut dir = d.id.dir().to_owned();
        loop {
            dirs.insert(dir.clone());
            match dir.rsplit_once('/') {
                Some((p, _)) => dir = p.to_owned(),
                None if !dir.is_empty() => dir.clear(),
                None => break,
            }
        }
    }
    let mut created_indexes = 0;
    let listings = !content_index && opts.only.is_empty();
    for dir in dirs.iter().filter(|_| listings) {
        let path = if dir.is_empty() {
            "index.md".to_owned()
        } else {
            format!("{dir}/index.md")
        };
        if result.iter().any(|d| d.path == path) {
            continue;
        }
        let content = generate_index(&result, dir);
        let doc = Concept::parse(Path::new(&path), &content)?;
        result.push(doc);
        created_indexes += 1;
        changes.insert(
            path.clone(),
            Change {
                path,
                kind: ChangeKind::Create,
                notes: vec!["directory listing (OKF §8)".into()],
                content,
            },
        );
    }

    if !changes.is_empty() && listings {
        let renamed = changes
            .values()
            .filter(|c| matches!(c.kind, ChangeKind::Rename { .. }))
            .count();
        let mut entry = format!(
            "## {}\n* **Adoption**: `okfkit adopt --level {}` filled in frontmatter for {edited} documents and created {created_indexes} index.md files",
            opts.today, opts.level
        );
        if renamed > 0 {
            let _ = write!(
                entry,
                "; moved {renamed} content pages out of the reserved name index.md"
            );
        }
        entry.push_str(
            ". Values okfkit made up are listed under `generated.fields` and should be reviewed.\n",
        );
        let existing = originals.iter().find(|d| d.path == "log.md");
        let content = match existing {
            Some(log) => insert_log_entry(&log.render(), &entry),
            None => format!("# Update log\n\n{entry}"),
        };
        let kind = if existing.is_some() {
            ChangeKind::Modify
        } else {
            ChangeKind::Create
        };
        if let Some(d) = result.iter_mut().find(|d| d.path == "log.md") {
            *d = Concept::parse(Path::new("log.md"), &content)?;
        } else {
            result.push(Concept::parse(Path::new("log.md"), &content)?);
        }
        changes.insert(
            "log.md".into(),
            Change {
                path: "log.md".into(),
                kind,
                notes: vec!["adoption entry".into()],
                content,
            },
        );
    }

    let after: Assessment = assess_profile(&result, vocab.as_ref(), content_index);
    let mut remaining = BTreeMap::new();
    for f in after.findings.iter().filter(|f| f.level <= opts.level) {
        *remaining.entry(f.code.to_owned()).or_insert(0) += 1;
    }
    weak.sort();
    skipped.sort();
    Ok(Plan {
        site,
        documents: originals.len(),
        level_before,
        level_after: after.level,
        changes: changes.into_values().collect(),
        weak_descriptions: weak,
        skipped,
        remaining,
    })
}

fn set(doc: &mut Concept, key: &str, value: Value, note: String, notes: &mut Vec<String>) -> bool {
    match doc.frontmatter.set(key, value) {
        Ok(()) => {
            notes.push(note);
            true
        }
        Err(e) => {
            notes.push(format!("! could not set {key}: {e}"));
            false
        }
    }
}

fn fill_fields(
    doc: &mut Concept,
    opts: &AdoptOptions,
    root: &Path,
    original_path: &str,
    notes: &mut Vec<String>,
    generated: &mut Vec<&'static str>,
    weak: &mut Vec<String>,
) {
    let fm = doc.frontmatter.clone(); // values before adoption
    let has = |k: &str| {
        fm.get(k)
            .is_some_and(|v| v.as_str().is_none_or(|s| !s.trim().is_empty()) && !v.is_null())
    };
    let m = meta(&fm);
    let (has_type, has_title, has_desc) = (has("type"), has("title"), has("description"));
    let body = doc.body.clone();

    if !has_type {
        let ty = heuristics::guess_type(doc.id.as_str());
        if set(
            doc,
            "type",
            json!(ty),
            format!("+ type: {ty} (from the path)"),
            notes,
        ) {
            generated.push("type");
        }
    }
    let title = if has_title {
        doc.frontmatter
            .get_str("title")
            .unwrap_or_default()
            .to_owned()
    } else {
        let (t, from) = match m.title.filter(|t| !t.is_native("title")) {
            Some(t) => (t.value, t.from.to_owned()),
            None => match heuristics::first_h1(&body) {
                Some(h) => (h, "the first heading".to_owned()),
                None => (
                    heuristics::title_from_name(doc.id.name()),
                    "the file name".to_owned(),
                ),
            },
        };
        set(
            doc,
            "title",
            json!(t),
            format!("+ title (from {from})"),
            notes,
        );
        t
    };
    if !has_desc {
        let mapped = m
            .description
            .filter(|d| !d.is_native("description") && !d.value.eq_ignore_ascii_case(&title));
        let (desc, from, made_up) = match mapped {
            Some(d) => (d.value, format!("from {}", d.from), false),
            None => match heuristics::first_sentence(&body)
                .filter(|s| !s.eq_ignore_ascii_case(&title))
            {
                Some(s) => (s, "first sentence of the body".to_owned(), true),
                None => (
                    format!(
                        "{} ({}).",
                        title,
                        doc.frontmatter.get_str("type").unwrap_or("Document")
                    ),
                    "title and type; no prose found".to_owned(),
                    true,
                ),
            },
        };
        if set(
            doc,
            "description",
            json!(desc),
            format!("+ description ({from})"),
            notes,
        ) && made_up
        {
            generated.push("description");
            weak.push(doc.path.clone());
        }
    }
    if opts.level < Level::L2 {
        return;
    }
    if !has("lang")
        && let Some(lang) = okfkit_analyze::detect_lang(&format!(
            "{title}\n{}",
            body.chars().take(2000).collect::<String>()
        ))
        && set(
            doc,
            "lang",
            json!(lang.code()),
            format!("+ lang: {} (detected)", lang.code()),
            notes,
        )
    {
        generated.push("lang");
    }
    if !has("status")
        && set(
            doc,
            "status",
            json!("stable"),
            "+ status: stable (assumed)".into(),
            notes,
        )
    {
        generated.push("status");
    }
    if !has("updated") {
        let (date, from) = git_date(root, original_path)
            .map_or_else(|| (opts.today.clone(), "today"), |d| (d, "git history"));
        if set(
            doc,
            "updated",
            json!(date),
            format!("+ updated (from {from})"),
            notes,
        ) {
            generated.push("updated");
        }
    }
    if m.tags.as_ref().is_none_or(|t| !t.is_native("tags")) {
        let tags: Vec<String> = match &m.tags {
            Some(t) => t.value.clone(),
            None => doc
                .id
                .as_str()
                .split('/')
                .rev()
                .skip(1)
                .take(2)
                .map(normalize_tag)
                .filter(|t| !t.is_empty())
                .collect(),
        };
        if !tags.is_empty()
            && set(
                doc,
                "tags",
                json!(tags),
                "+ tags (from categories or directories)".into(),
                notes,
            )
        {
            generated.push("tags");
        }
    }
}

/// Date of the last commit touching `rel`, if the folder is in a git repository.
fn git_date(root: &Path, rel: &str) -> Option<String> {
    let out = std::process::Command::new("git")
        .args(["log", "-1", "--format=%cs", "--", rel])
        .current_dir(root)
        .output()
        .ok()?;
    let s = String::from_utf8(out.stdout).ok()?.trim().to_owned();
    (out.status.success() && s.len() == 10).then_some(s)
}

/// An OKF `index.md` for `dir`.
fn generate_index(docs: &[Concept], dir: &str) -> String {
    let prefix = if dir.is_empty() {
        String::new()
    } else {
        format!("{dir}/")
    };
    let mut subdirs: BTreeMap<String, usize> = BTreeMap::new();
    let mut here: Vec<&Concept> = Vec::new();
    for d in docs.iter().filter(|d| !d.is_reserved()) {
        let Some(rest) = d.id.as_str().strip_prefix(&prefix) else {
            continue;
        };
        match rest.split_once('/') {
            Some((sub, _)) if !sub.starts_with('_') => {
                *subdirs.entry(sub.to_owned()).or_default() += 1
            }
            Some(_) => {}
            None => here.push(d),
        }
    }
    let mut out = String::new();
    if dir.is_empty() {
        out.push_str("---\nokf_version: \"0.2\"\n---\n");
    }
    if !subdirs.is_empty() {
        out.push_str("# Subdirectories\n\n");
        for (sub, n) in &subdirs {
            let _ = writeln!(
                out,
                "* [{sub}]({sub}/index.md) - {n} document{}",
                if *n == 1 { "" } else { "s" }
            );
        }
    }
    if !here.is_empty() {
        if !subdirs.is_empty() {
            out.push('\n');
        }
        out.push_str("# Documents\n\n");
        for d in here {
            let m = meta(&d.frontmatter);
            let title = m.title.map_or_else(|| d.id.name().to_owned(), |t| t.value);
            let desc = m
                .description
                .map(|x| format!(" - {}", x.value))
                .unwrap_or_default();
            let _ = writeln!(out, "* [{title}]({}.md){desc}", d.id.name());
        }
    }
    out
}

/// Inserts a dated entry above the first `## ` heading (newest first), or at the end.
fn insert_log_entry(log: &str, entry: &str) -> String {
    match log.find("\n## ") {
        Some(i) => format!("{}\n{entry}\n{}", &log[..i], &log[i + 1..]),
        None => format!("{}\n\n{entry}", log.trim_end()),
    }
}

/// Writes the adopted bundle to `out` (which must not exist or be empty): every
/// file of `root` is copied (hidden directories except `.okfkit` included), then
/// the plan is applied. `root` is not modified.
pub fn apply_to(root: &Path, plan: &Plan, out: &Path) -> Result<(), Error> {
    if out.exists()
        && std::fs::read_dir(out)
            .map(|mut d| d.next().is_some())
            .unwrap_or(true)
    {
        return Err(Error::OutExists(out.to_owned()));
    }
    let renamed: BTreeSet<String> = plan
        .changes
        .iter()
        .filter_map(|c| match &c.kind {
            ChangeKind::Rename { from } => Some(from.clone()),
            _ => None,
        })
        .collect();
    copy_tree(root, out, Path::new(""), &renamed)?;
    write_changes(out, plan)
}

/// Applies the plan to `root` itself. **Writes files.** Callers should check that
/// the folder is under version control first.
pub fn apply_in_place(root: &Path, plan: &Plan) -> Result<(), Error> {
    write_changes(root, plan)?;
    for c in &plan.changes {
        if let ChangeKind::Rename { from } = &c.kind {
            let old = root.join(from);
            if old.exists() && from != &c.path && !plan.changes.iter().any(|x| &x.path == from) {
                std::fs::remove_file(&old).map_err(|source| Error::Io { path: old, source })?;
            }
        }
    }
    Ok(())
}

fn write_changes(root: &Path, plan: &Plan) -> Result<(), Error> {
    for c in &plan.changes {
        let path = root.join(&c.path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| Error::Io {
                path: parent.to_owned(),
                source,
            })?;
        }
        std::fs::write(&path, &c.content).map_err(|source| Error::Io { path, source })?;
    }
    Ok(())
}

fn copy_tree(
    from_root: &Path,
    to_root: &Path,
    rel: &Path,
    skip: &BTreeSet<String>,
) -> Result<(), Error> {
    let dir = from_root.join(rel);
    let io = |path: PathBuf| move |source| Error::Io { path, source };
    std::fs::create_dir_all(to_root.join(rel)).map_err(io(to_root.join(rel)))?;
    for e in std::fs::read_dir(&dir).map_err(io(dir.clone()))? {
        let e = e.map_err(io(dir.clone()))?;
        let name = e.file_name();
        if name == ".git" || name == ".okfkit" {
            continue;
        }
        let child = rel.join(&name);
        let ft = e.file_type().map_err(io(e.path()))?;
        if ft.is_dir() {
            copy_tree(from_root, to_root, &child, skip)?;
        } else if ft.is_file() && !skip.contains(&child.to_string_lossy().replace('\\', "/")) {
            std::fs::copy(e.path(), to_root.join(&child)).map_err(io(to_root.join(&child)))?;
        }
    }
    Ok(())
}

impl Plan {
    /// Human-readable summary of the plan.
    pub fn to_text(&self, verbose: bool) -> String {
        let lvl = |l: Option<Level>| l.map_or_else(|| "below L0".to_owned(), |l| l.to_string());
        let created = self
            .changes
            .iter()
            .filter(|c| c.kind == ChangeKind::Create)
            .count();
        let renamed = self
            .changes
            .iter()
            .filter(|c| matches!(c.kind, ChangeKind::Rename { .. }))
            .count();
        let modified = self.changes.len() - created - renamed;
        let mut out = format!(
            "site: {:?}; {} markdown documents\nlevel: {} -> {}\nchanges: {modified} edited, {created} created, {renamed} renamed\n",
            self.site,
            self.documents,
            lvl(self.level_before),
            lvl(self.level_after)
        );
        if verbose {
            for c in &self.changes {
                let tag = match &c.kind {
                    ChangeKind::Create => "create".to_owned(),
                    ChangeKind::Modify => "edit".to_owned(),
                    ChangeKind::Rename { from } => format!("move {from} ->"),
                };
                let _ = writeln!(out, "  {tag} {}: {}", c.path, c.notes.join("; "));
            }
        }
        if !self.weak_descriptions.is_empty() {
            let _ = writeln!(
                out,
                "{} descriptions were taken from the first sentence of the body; review them (or let an agent with the okfkit-curate skill rewrite them)",
                self.weak_descriptions.len()
            );
        }
        for (p, why) in &self.skipped {
            let _ = writeln!(out, "skipped {p}: {why}");
        }
        if !self.remaining.is_empty() {
            let r: Vec<String> = self
                .remaining
                .iter()
                .map(|(k, n)| format!("{k}={n}"))
                .collect();
            let _ = writeln!(out, "still blocking the target level: {}", r.join(", "));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_entries_go_newest_first() {
        assert_eq!(
            insert_log_entry("# Log\n\n## 2026-01-01\n* old\n", "## 2026-10-01\n* new\n"),
            "# Log\n\n## 2026-10-01\n* new\n\n## 2026-01-01\n* old\n"
        );
        assert_eq!(
            insert_log_entry("# Log", "## 2026-10-01\n* new\n"),
            "# Log\n\n## 2026-10-01\n* new\n"
        );
    }

    #[test]
    fn listing_detection() {
        let listing = Concept::parse(
            Path::new("a/index.md"),
            "# Docs\n\n* [A](a.md) - x\n* [B](b.md)\n",
        )
        .unwrap();
        let page = Concept::parse(
            Path::new("index.md"),
            "---\ntitle: Home\n---\n# Home\n\nWelcome to the docs.\n",
        )
        .unwrap();
        assert!(is_listing(&listing) && !is_listing(&page));
    }
}
