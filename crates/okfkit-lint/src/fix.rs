//! `--fix-safe`: fixes that cannot lose information.
//!
//! - Creates a missing `index.md` from the directory's documents (never overwrites).
//! - Replaces tag synonyms with their canonical vocabulary tag (only the `tags` key is edited).

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use okfkit_core::{Concept, Value};
use okfkit_standard::meta;
use serde::Serialize;

use crate::{Error, LintConfig, lint};

/// What [`fix_safe`] changed.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct FixReport {
    /// Files created (bundle-relative).
    pub created: Vec<String>,
    /// Files edited (bundle-relative).
    pub updated: Vec<String>,
}

/// Applies safe fixes to the bundle at `root`. **Writes files.**
pub fn fix_safe(root: &Path, config: &LintConfig) -> Result<FixReport, Error> {
    let report = lint(
        root,
        &LintConfig {
            target: okfkit_standard::Level::L2,
            ..config.clone()
        },
    )?;
    let mut out = FixReport::default();

    let docs: Vec<Concept> = okfkit_core::discover(root)?
        .iter()
        .filter_map(|p| Concept::read(root, p).ok())
        .collect();
    for d in report
        .diagnostics
        .iter()
        .filter(|d| d.rule == "missing-index")
    {
        let dir = d
            .path
            .strip_suffix("index.md")
            .unwrap_or("")
            .trim_end_matches('/');
        let text = generate_index(&docs, dir);
        let path = root.join(&d.path);
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut f) => {
                std::io::Write::write_all(&mut f, text.as_bytes()).map_err(|source| {
                    Error::Write {
                        path: d.path.clone(),
                        source,
                    }
                })?;
                out.created.push(d.path.clone());
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(source) => {
                return Err(Error::Write {
                    path: d.path.clone(),
                    source,
                });
            }
        }
    }

    if let Some(vocab) = okfkit_standard::Vocabulary::load(root, &config.vocabulary)
        .ok()
        .flatten()
    {
        let mut paths: Vec<&str> = report
            .diagnostics
            .iter()
            .filter(|d| d.rule == "non-canonical-tag")
            .map(|d| d.path.as_str())
            .collect();
        paths.dedup();
        for path in paths {
            let mut doc = Concept::read(root, Path::new(path))?;
            let Some(Value::Array(tags)) = doc.frontmatter.get("tags").cloned() else {
                continue;
            };
            let mut fixed: Vec<Value> = Vec::new();
            for t in tags {
                let v = match t.as_str().and_then(|s| vocab.canonical(s)) {
                    Some(c) => Value::String(c.to_owned()),
                    None => t,
                };
                if !fixed.contains(&v) {
                    fixed.push(v);
                }
            }
            if doc.frontmatter.set("tags", Value::Array(fixed)).is_ok() {
                write_atomic(root, path, &doc.render())?;
                out.updated.push(path.to_owned());
            }
        }
    }
    Ok(out)
}

fn write_atomic(root: &Path, rel: &str, text: &str) -> Result<(), Error> {
    let path = root.join(rel);
    let tmp = path.with_extension("md.okfkit-tmp");
    let err = |source| Error::Write {
        path: rel.to_owned(),
        source,
    };
    std::fs::write(&tmp, text).map_err(err)?;
    std::fs::rename(&tmp, &path).map_err(err)
}

/// An OKF `index.md` for `dir`: subdirectories, then documents with their descriptions.
fn generate_index(docs: &[Concept], dir: &str) -> String {
    let prefix = if dir.is_empty() {
        String::new()
    } else {
        format!("{dir}/")
    };
    let mut subdirs: BTreeMap<String, Option<String>> = BTreeMap::new();
    let mut here = Vec::new();
    for d in docs {
        let Some(rest) = d.id.as_str().strip_prefix(&prefix) else {
            continue;
        };
        match rest.split_once('/') {
            Some((sub, _)) if !sub.starts_with('_') => {
                subdirs.entry(sub.to_owned()).or_default();
            }
            Some(_) => {}
            None if !d.is_reserved() => here.push(d),
            None => {}
        }
    }
    let mut out = String::new();
    if !subdirs.is_empty() {
        out.push_str("# Subdirectories\n\n");
        for sub in subdirs.keys() {
            let _ = writeln!(out, "* [{sub}]({sub}/index.md)");
        }
    }
    if !here.is_empty() {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str("# Documents\n\n");
        for d in here {
            let m = meta(&d.frontmatter);
            let title = m.title.map_or_else(|| d.id.name().to_owned(), |t| t.value);
            match m.description {
                Some(desc) => {
                    let _ = writeln!(out, "* [{title}]({}.md) - {}", d.id.name(), desc.value);
                }
                None => {
                    let _ = writeln!(out, "* [{title}]({}.md)", d.id.name());
                }
            }
        }
    }
    out
}
