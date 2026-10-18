//! Versioned, data-only starter bundles from built-in assets or local directories.
//! Git repositories are resolved to the same directory format by [`repository`].

pub mod repository;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path};

use serde::{Deserialize, Serialize};

use crate::{Error, scaffold};

const MAX_FILES: usize = 4096;
const MAX_BYTES: usize = 32 * 1024 * 1024;

fn invalid(message: impl std::fmt::Display) -> Error {
    Error::Invalid(format!("invalid argument: {message}"))
}

/// A portable, relative template path. Internal state and VCS directories cannot be copied.
pub(crate) fn safe_path(path: &str) -> Result<(), Error> {
    if path.is_empty()
        || path.contains(['\\', ':'])
        || path.chars().any(char::is_control)
        || path
            .split('/')
            .any(|p| p.is_empty() || p == "." || p == ".." || p.starts_with('.'))
        || !Path::new(path)
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
    {
        return Err(invalid(format!("unsafe template path {path:?}")));
    }
    Ok(())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Catalog {
    format_version: u32,
    templates: Vec<Entry>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    path: String,
}

fn entries(text: &str) -> Result<Vec<String>, Error> {
    let catalog: Catalog = serde_saphyr::from_str(text).map_err(invalid)?;
    if catalog.format_version != 1 {
        return Err(invalid("unsupported catalog format_version; expected 1"));
    }
    let mut paths = BTreeSet::new();
    catalog
        .templates
        .into_iter()
        .map(|entry| {
            safe_path(&entry.path)?;
            if !paths.insert(entry.path.clone()) {
                return Err(invalid("duplicate template catalog path"));
            }
            Ok(entry.path)
        })
        .collect()
}

/// Discoverable metadata shared by built-in, local and Git templates.
#[derive(Debug, Serialize)]
pub struct Summary {
    /// Stable identifier used for selection within a catalog.
    pub id: String,
    /// Default bundle title.
    pub title: String,
    /// What the template helps with.
    pub description: String,
    /// Intended audiences, never access-control roles.
    pub roles: Vec<String>,
    /// Document types present in the bundle.
    pub types: Vec<String>,
}

/// Public v1 template manifest. Unknown fields and unsupported versions are rejected.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    format_version: u32,
    id: String,
    version: String,
    title: String,
    description: String,
    #[serde(default)]
    roles: Vec<String>,
    bundle: String,
    #[serde(default)]
    questions: Vec<Question>,
}

/// A starter bundle whose files remain ordinary UTF-8 files, without executable hooks.
#[derive(Debug)]
pub struct Template {
    manifest: Manifest,
    files: BTreeMap<String, String>,
    /// Optional grounded questions for authors and acceptance checks.
    pub questions: Vec<Question>,
    /// Source identity recorded in preview, creation output and the bundle log.
    pub source: String,
}

/// A trial question with supporting evidence in a bundle-relative document ID.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Question {
    /// Natural-language question.
    pub question: String,
    /// Document ID, without the `.md` suffix.
    pub source: String,
    /// Literal supporting passage.
    pub evidence: String,
}

fn manifest(text: &str) -> Result<Manifest, Error> {
    let m: Manifest = serde_saphyr::from_str(text).map_err(invalid)?;
    if m.format_version != 1 {
        return Err(invalid("unsupported template format_version; expected 1"));
    }
    if m.id.is_empty()
        || !m
            .id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    {
        return Err(invalid(
            "template id must contain lowercase ASCII letters, digits or hyphens",
        ));
    }
    if m.version.trim().is_empty() || m.title.trim().is_empty() || m.description.trim().is_empty() {
        return Err(invalid(
            "template version, title and description must be non-empty",
        ));
    }
    safe_path(&m.bundle)?;
    Ok(m)
}

fn assemble(
    m: Manifest,
    files: BTreeMap<String, String>,
    source: String,
) -> Result<Template, Error> {
    for required in ["index.md", "okbase.toml", "_meta/vocabulary.md"] {
        if !files.contains_key(required) {
            return Err(invalid(format!("template bundle is missing {required}")));
        }
    }
    if files.len() > MAX_FILES || files.values().map(String::len).sum::<usize>() > MAX_BYTES {
        return Err(invalid("template exceeds 4096 files or 32 MiB"));
    }
    for path in files.keys() {
        safe_path(path)?;
    }
    for q in &m.questions {
        safe_path(&q.source)?;
        if q.question.trim().is_empty()
            || q.evidence.trim().is_empty()
            || !files
                .get(&format!("{}.md", q.source))
                .is_some_and(|text| text.contains(&q.evidence))
        {
            return Err(invalid(format!(
                "trial question has no matching evidence: {}",
                q.question
            )));
        }
    }
    let questions = m.questions.clone();
    Ok(Template {
        manifest: m,
        files,
        questions,
        source,
    })
}

fn embedded(path: &str) -> Result<Template, Error> {
    let find = |name: &str| {
        okbase_templates::FILES
            .iter()
            .find(|(p, _)| *p == name)
            .map(|(_, text)| *text)
    };
    let m = manifest(
        find(&format!("{path}/template.yaml"))
            .ok_or_else(|| invalid("missing embedded manifest"))?,
    )?;
    let prefix = format!("{path}/{}/", m.bundle);
    let files = okbase_templates::FILES
        .iter()
        .filter_map(|(p, text)| {
            p.strip_prefix(&prefix)
                .map(|p| (p.to_owned(), (*text).to_owned()))
        })
        .collect();
    let source = format!("builtin:{}@{}", m.id, m.version);
    assemble(m, files, source)
}

/// Lists shipped templates in catalog order, offline and without writing bundle state.
pub fn catalog() -> Result<Vec<Summary>, Error> {
    unique(
        entries(okbase_templates::CATALOG)?
            .iter()
            .map(|path| embedded(path)),
    )
}

fn unique(templates: impl Iterator<Item = Result<Template, Error>>) -> Result<Vec<Summary>, Error> {
    let mut ids = BTreeSet::new();
    templates
        .map(|t| {
            let summary = t?.summary();
            if !ids.insert(summary.id.clone()) {
                return Err(invalid("duplicate template id in catalog"));
            }
            Ok(summary)
        })
        .collect()
}

/// Loads a built-in ID; IDs are never interpreted as paths or URLs.
pub fn load(id: &str) -> Result<Template, Error> {
    for path in entries(okbase_templates::CATALOG)? {
        let t = embedded(&path)?;
        if t.manifest.id == id {
            return Ok(t);
        }
    }
    Err(invalid(format!(
        "unknown template {id:?}; run `okbase templates`"
    )))
}

fn read(path: &Path) -> Result<String, Error> {
    std::fs::read_to_string(path).map_err(|source| Error::Io {
        path: path.to_owned(),
        source,
    })
}

fn regular(path: &Path) -> Result<std::fs::Metadata, Error> {
    let meta = std::fs::symlink_metadata(path).map_err(|source| Error::Io {
        path: path.to_owned(),
        source,
    })?;
    if meta.is_symlink() || (!meta.is_dir() && !meta.is_file()) {
        return Err(invalid(format!(
            "template path must be a regular file or directory: {}",
            path.display()
        )));
    }
    Ok(meta)
}

fn child(root: &Path, path: &str) -> Result<std::path::PathBuf, Error> {
    safe_path(path)?;
    let mut current = root.to_owned();
    for part in path.split('/') {
        current.push(part);
        regular(&current)?;
    }
    Ok(current)
}

fn walk(
    root: &Path,
    dir: &Path,
    files: &mut BTreeMap<String, String>,
    bytes: &mut usize,
) -> Result<(), Error> {
    let entries = std::fs::read_dir(dir).map_err(|source| Error::Io {
        path: dir.to_owned(),
        source,
    })?;
    for entry in entries {
        let path = entry
            .map_err(|source| Error::Io {
                path: dir.to_owned(),
                source,
            })?
            .path();
        let meta = regular(&path)?;
        let relative = path
            .strip_prefix(root)
            .map_err(invalid)?
            .to_str()
            .ok_or_else(|| invalid("template paths must be UTF-8"))?
            .replace('\\', "/");
        safe_path(&relative)?;
        if meta.is_dir() {
            walk(root, &path, files, bytes)?;
        } else {
            if files.len() >= MAX_FILES || meta.len() > (MAX_BYTES.saturating_sub(*bytes)) as u64 {
                return Err(invalid("template exceeds 4096 files or 32 MiB"));
            }
            let text = read(&path)?;
            *bytes += text.len();
            files.insert(relative, text);
        }
    }
    Ok(())
}

/// Loads a directory containing `template.yaml`, rejecting symlinks and unsafe bundle paths.
pub fn load_from(root: &Path) -> Result<Template, Error> {
    regular(root)?;
    let m = manifest(&read(&child(root, "template.yaml")?)?)?;
    let bundle = child(root, &m.bundle)?;
    let mut files = BTreeMap::new();
    walk(&bundle, &bundle, &mut files, &mut 0)?;
    let source = format!("local:{}", root.canonicalize().map_err(invalid)?.display());
    assemble(m, files, source)
}

/// Lists a directory's catalog, or its single root template. Duplicate IDs are invalid.
pub fn catalog_from(root: &Path) -> Result<Vec<Summary>, Error> {
    regular(root)?;
    if root.join("catalog.yaml").exists() {
        let paths = entries(&read(&child(root, "catalog.yaml")?)?)?;
        unique(paths.iter().map(|p| load_from(&child(root, p)?)))
    } else {
        unique(std::iter::once(load_from(root)))
    }
}

/// Selects an ID from a catalog; `None` selects a single root template.
pub fn select_from(root: &Path, id: Option<&str>) -> Result<Template, Error> {
    let summaries = catalog_from(root)?;
    let wanted = match id {
        Some(id) => id,
        None if summaries.len() == 1 => &summaries[0].id,
        None => return Err(invalid("select a catalog entry with --template NAME")),
    };
    if root.join("catalog.yaml").exists() {
        for path in entries(&read(&child(root, "catalog.yaml")?)?)? {
            let t = load_from(&child(root, &path)?)?;
            if t.manifest.id == wanted {
                return Ok(t);
            }
        }
    } else {
        let t = load_from(root)?;
        if t.manifest.id == wanted {
            return Ok(t);
        }
    }
    Err(invalid(format!(
        "unknown template {wanted:?} in selected source"
    )))
}

impl Template {
    /// Metadata derived from the manifest and actual type schema files.
    pub fn summary(&self) -> Summary {
        Summary {
            id: self.manifest.id.clone(),
            title: self.manifest.title.clone(),
            description: self.manifest.description.clone(),
            roles: self.manifest.roles.clone(),
            types: self
                .files
                .keys()
                .filter_map(|p| {
                    p.strip_prefix("_meta/types/")
                        .and_then(|p| p.strip_suffix(".md"))
                })
                .map(str::to_owned)
                .collect(),
        }
    }

    /// Validates a bundle and plans creation; destination files are never changed here.
    pub fn files(
        &self,
        root: &Path,
        options: &scaffold::InitOptions,
    ) -> Result<Vec<(String, String)>, Error> {
        // Reuse existing empty-target checks and the generation of additive schemas.
        let mut options = options.clone();
        if options.title.is_empty() {
            options.title.clone_from(&self.manifest.title);
        }
        if options.description.is_empty() {
            options.description.clone_from(&self.manifest.description);
        }
        options.types.extend(self.summary().types);
        options.types.sort();
        options.types.dedup();
        let skeleton = scaffold::init_files(root, &options)?;
        let mut files = self.files.clone();
        for (path, text) in skeleton {
            if path.starts_with("_meta/types/") {
                files.entry(path).or_insert(text);
            }
        }
        let index = files.get_mut("index.md").expect("required at load");
        if options.title.contains(['\r', '\n']) || options.description.contains(['\r', '\n']) {
            return Err(invalid("bundle title and description must be single lines"));
        }
        let mut document = okbase_core::Concept::parse(Path::new("index.md"), index)?;
        let mut found_heading = false;
        let mut found_description = false;
        document.body = document
            .body
            .split_inclusive('\n')
            .map(|line| {
                let (text, eol) = if let Some(text) = line.strip_suffix("\r\n") {
                    (text, "\r\n")
                } else if let Some(text) = line.strip_suffix('\n') {
                    (text, "\n")
                } else {
                    (line, "")
                };
                if !found_heading && text.starts_with("# ") {
                    found_heading = true;
                    format!("# {}{eol}", options.title)
                } else if found_heading && !found_description && text == self.manifest.description {
                    found_description = true;
                    format!("{}{eol}", options.description)
                } else {
                    line.to_owned()
                }
            })
            .collect();
        if !found_heading {
            return Err(invalid("template index.md needs a level-one heading"));
        }
        *index = document.render();
        let schemas: BTreeSet<_> = self
            .summary()
            .types
            .into_iter()
            .chain(options.types.iter().cloned())
            .collect();
        for (path, text) in &files {
            if path.ends_with(".md")
                && path != "index.md"
                && !path.ends_with("/index.md")
                && path != "log.md"
                && !path.starts_with("_meta/")
            {
                let concept = okbase_core::Concept::parse(Path::new(path), text)?;
                if let Some(kind) = concept.frontmatter.get_str("type")
                    && !schemas.contains(kind)
                {
                    return Err(invalid(format!(
                        "template bundle must meet L2: missing schema for {kind} in {path}"
                    )));
                }
            }
        }
        let config = files.get_mut("okbase.toml").expect("required at load");
        let mut parsed = config.parse::<toml_edit::DocumentMut>().map_err(invalid)?;
        let bundle_settings = parsed
            .get("bundle")
            .and_then(toml_edit::Item::as_table_like)
            .ok_or_else(|| invalid("template okbase.toml must contain a [bundle] table"))?;
        if bundle_settings
            .get("profile")
            .is_some_and(|p| p.as_str() != Some("okf"))
        {
            return Err(invalid("template bundle profile must be okf"));
        }

        if !options.langs.is_empty() {
            let langs: toml_edit::Array = options.langs.iter().map(String::as_str).collect();
            parsed["bundle"]["langs"] = toml_edit::value(langs);
            *config = parsed.to_string();
        }
        let log = files
            .entry("log.md".into())
            .or_insert_with(|| "# Update log\n".into());
        log.push_str(&format!("\n## {}\n* **Initialized** from template `{}` version `{}` with okbase {}. Source: {}.\n", options.today, self.manifest.id, self.manifest.version, env!("CARGO_PKG_VERSION"), self.source.replace(['\r', '\n'], " ")));
        // Validate in disposable storage. No destination or index state is created by preview.
        let scratch = tempfile::tempdir().map_err(invalid)?;
        let planned: Vec<_> = files.into_iter().collect();
        scaffold::write_new(scratch.path(), &planned, &[])?;
        let report = okbase_lint::lint(
            scratch.path(),
            &okbase_lint::LintConfig::level(okbase_standard::Level::L2),
        )
        .map_err(invalid)?;
        if report.level < Some(okbase_standard::Level::L2) || report.errors > 0 {
            return Err(invalid(format!(
                "template bundle must meet L2: {}",
                report.to_text()
            )));
        }
        scaffold::check_new(root, &planned, &[])?;
        Ok(planned)
    }
}
