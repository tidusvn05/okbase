//! okbase quality levels L0–L2 (PLAN §2). L3 checks belong to `okbase-lint`.
//!
//! | Level | Requires |
//! |---|---|
//! | L0 | OKF v0.2: every concept has parseable frontmatter with a non-empty `type` |
//! | L1 | L0 + every concept has `title` and a `description` that differs from the title; an `index.md` in every directory |
//! | L2 | L1 + `tags` from the vocabulary, `lang`, `status` (stable/deprecated/draft), `updated`; `supersedes` points to existing concepts |
//!
//! Only native OKF fields count: a page with `summary` but no `description` is
//! readable through [`crate::meta`] but does not meet L1. Documents under `_meta/`
//! only need L0.

use std::collections::{BTreeSet, HashSet};
use std::fmt;
use std::path::Path;

use okbase_core::{
    Concept, ConceptId, FrontmatterState, Severity, Value, discover, validate_profile,
};
use serde::Serialize;

use crate::mapping::meta;
use crate::{Error, META_DIR, Vocabulary};

/// An okbase quality level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub enum Level {
    /// OKF v0.2 conformant.
    L0,
    /// Navigable: titles, descriptions, `index.md` everywhere.
    L1,
    /// Structured: vocabulary tags, language, lifecycle.
    L2,
    /// Curated (checked by `okbase-lint`).
    L3,
}

impl fmt::Display for Level {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

/// Allowed `status` values.
pub const STATUSES: [&str; 3] = ["stable", "deprecated", "draft"];

/// Something that keeps a document or the bundle from reaching a level.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Finding {
    /// Bundle-relative path of the document (or of the missing file).
    pub path: String,
    /// The level this finding blocks.
    pub level: Level,
    /// Stable machine-readable code, such as `missing-description`.
    pub code: &'static str,
    /// Human-readable message, with a hint when one helps.
    pub message: String,
}

/// The result of checking a bundle against the levels.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Assessment {
    /// Highest level fully met, or `None` if the bundle is not OKF-conformant.
    pub level: Option<Level>,
    /// Number of concept documents checked (reserved files excluded).
    pub concepts: usize,
    /// All findings, sorted by level then path.
    pub findings: Vec<Finding>,
}

impl Assessment {
    /// Findings that block `level`.
    pub fn blocking(&self, level: Level) -> impl Iterator<Item = &Finding> {
        self.findings.iter().filter(move |f| f.level == level)
    }
}

/// Reads every document of the bundle at `root` and assesses it.
///
/// The vocabulary is read from `vocabulary_path` (usually [`crate::VOCABULARY_PATH`]).
/// Unreadable documents become L0 findings rather than errors.
pub fn assess_bundle(root: &Path, vocabulary_path: &str) -> Result<Assessment, Error> {
    let mut docs = Vec::new();
    let mut unreadable = Vec::new();
    for rel in discover(root)? {
        match Concept::read(root, &rel) {
            Ok(doc) => docs.push(doc),
            Err(e) => unreadable.push(Finding {
                path: rel.to_string_lossy().replace('\\', "/"),
                level: Level::L0,
                code: "unreadable",
                message: e.to_string(),
            }),
        }
    }
    let (vocab, vocab_error) = match Vocabulary::load(root, vocabulary_path) {
        Ok(v) => (v, None),
        Err(e) => (None, Some(e.to_string())),
    };
    let content_index = okbase_core::site::profile(root).content_index();
    let mut a = assess_profile(&docs, vocab.as_ref(), content_index);
    if let Some(message) = vocab_error {
        a.findings.retain(|f| f.code != "no-vocabulary");
        a.findings.push(Finding {
            path: vocabulary_path.into(),
            level: Level::L2,
            code: "invalid-vocabulary",
            message,
        });
    } else if vocab.is_none() {
        for f in a.findings.iter_mut().filter(|f| f.code == "no-vocabulary") {
            f.path = vocabulary_path.into();
        }
    }
    a.findings.extend(unreadable);
    Ok(finish(a))
}

/// Assesses already-parsed documents. `vocab` is the bundle's tag vocabulary, if any.
pub fn assess(docs: &[Concept], vocab: Option<&Vocabulary>) -> Assessment {
    assess_profile(docs, vocab, false)
}

/// Like [`assess`]; with `content_index` (docs-site and vault profiles) `index.md` files are
/// content pages and directories need no listing file.
pub fn assess_profile(
    docs: &[Concept],
    vocab: Option<&Vocabulary>,
    content_index: bool,
) -> Assessment {
    let mut out = Vec::new();
    let ids: HashSet<&str> = docs.iter().map(|d| d.id.as_str()).collect();
    let mut dirs = BTreeSet::new();
    let mut concepts = 0;
    for doc in docs {
        for issue in validate_profile(doc, content_index)
            .into_iter()
            .filter(|i| i.severity == Severity::Error)
        {
            push(&mut out, doc, Level::L0, issue.code, issue.message);
        }
        if doc.is_reserved_in(content_index) {
            continue;
        }
        concepts += 1;
        // Without a usable frontmatter block, L1/L2 findings would only repeat the L0 one.
        if is_meta(&doc.id) || doc.frontmatter.state() != FrontmatterState::Valid {
            continue;
        }
        let mut dir = doc.id.dir();
        loop {
            dirs.insert(dir.to_owned());
            match dir.rsplit_once('/') {
                Some((parent, _)) => dir = parent,
                None if !dir.is_empty() => dir = "",
                None => break,
            }
        }
        check_l1(&mut out, doc);
        check_l2(&mut out, doc, vocab, &ids);
    }
    for dir in dirs.into_iter().filter(|_| !content_index) {
        let index = if dir.is_empty() {
            "index".to_owned()
        } else {
            format!("{dir}/index")
        };
        if !ids.contains(index.as_str()) {
            out.push(Finding {
                path: format!("{index}.md"),
                level: Level::L1,
                code: "missing-index",
                message: format!(
                    "directory `{}` has no index.md (okbase can generate one)",
                    if dir.is_empty() { "." } else { &dir }
                ),
            });
        }
    }
    if vocab.is_none() && concepts > 0 {
        out.push(Finding {
            path: crate::VOCABULARY_PATH.into(),
            level: Level::L2,
            code: "no-vocabulary",
            message: "the bundle has no tag vocabulary".into(),
        });
    }
    finish(Assessment {
        level: None,
        concepts,
        findings: out,
    })
}

fn finish(mut a: Assessment) -> Assessment {
    a.findings
        .sort_by(|x, y| (x.level, &x.path, x.code).cmp(&(y.level, &y.path, y.code)));
    let lowest = a.findings.iter().map(|f| f.level).min();
    a.level = match lowest {
        None => Some(Level::L2),
        Some(Level::L0) => None,
        Some(Level::L1) => Some(Level::L0),
        Some(_) => Some(Level::L1),
    };
    a
}

fn is_meta(id: &ConceptId) -> bool {
    id.as_str().split('/').next() == Some(META_DIR)
}

fn push(
    out: &mut Vec<Finding>,
    doc: &Concept,
    level: Level,
    code: &'static str,
    message: impl Into<String>,
) {
    out.push(Finding {
        path: doc.path.clone(),
        level,
        code,
        message: message.into(),
    });
}

fn check_l1(out: &mut Vec<Finding>, doc: &Concept) {
    let m = meta(&doc.frontmatter);
    let hint = |mapped: &Option<crate::Mapped<String>>, okf: &str| match mapped {
        Some(v) if !v.is_native(okf) => format!(" (found `{}`; rename it to `{okf}`)", v.from),
        _ => String::new(),
    };
    let title = m.title.as_ref().filter(|t| t.is_native("title"));
    let description = m
        .description
        .as_ref()
        .filter(|d| d.is_native("description"));
    if title.is_none() {
        push(
            out,
            doc,
            Level::L1,
            "missing-title",
            format!("no `title`{}", hint(&m.title, "title")),
        );
    }
    match description {
        None => push(
            out,
            doc,
            Level::L1,
            "missing-description",
            format!("no `description`{}", hint(&m.description, "description")),
        ),
        Some(d) if title.is_some_and(|t| t.value.eq_ignore_ascii_case(&d.value)) => push(
            out,
            doc,
            Level::L1,
            "description-repeats-title",
            "`description` only repeats the title",
        ),
        Some(_) => {}
    }
}

fn check_l2(
    out: &mut Vec<Finding>,
    doc: &Concept,
    vocab: Option<&Vocabulary>,
    ids: &HashSet<&str>,
) {
    let fm = &doc.frontmatter;
    match meta(fm).tags.filter(|t| t.is_native("tags")) {
        None => push(out, doc, Level::L2, "missing-tags", "no `tags`"),
        Some(tags) => {
            for tag in vocab
                .iter()
                .flat_map(|v| tags.value.iter().map(move |t| (v, t)))
                .map(|(v, t)| (v.canonical(t), t))
            {
                match tag {
                    (None, t) => push(
                        out,
                        doc,
                        Level::L2,
                        "unknown-tag",
                        format!("tag `{t}` is not in the vocabulary"),
                    ),
                    (Some(c), t) if crate::normalize_tag(c) != crate::normalize_tag(t) => push(
                        out,
                        doc,
                        Level::L2,
                        "non-canonical-tag",
                        format!("tag `{t}` is a synonym; use `{c}`"),
                    ),
                    _ => {}
                }
            }
        }
    }
    if fm.get_str("lang").is_none_or(|l| l.trim().is_empty()) {
        push(
            out,
            doc,
            Level::L2,
            "missing-lang",
            "no `lang` (for example `en`, `vi`, `ja`)",
        );
    }
    match fm.get("status") {
        None => push(
            out,
            doc,
            Level::L2,
            "missing-status",
            "no `status` (stable, deprecated or draft)",
        ),
        Some(Value::String(s)) if STATUSES.contains(&s.as_str()) => {}
        Some(v) => push(
            out,
            doc,
            Level::L2,
            "invalid-status",
            format!("`status` is {v}; expected stable, deprecated or draft"),
        ),
    }
    match fm.get("updated") {
        None => push(out, doc, Level::L2, "missing-updated", "no `updated` date"),
        Some(Value::String(s)) if is_iso_date(s) => {}
        Some(v) => push(
            out,
            doc,
            Level::L2,
            "invalid-updated",
            format!("`updated` is {v}; expected an ISO date (YYYY-MM-DD)"),
        ),
    }
    if let Some(v) = fm.get("supersedes") {
        let targets: Option<Vec<&str>> = match v {
            Value::String(s) => Some(vec![s.as_str()]),
            Value::Array(items) => items.iter().map(Value::as_str).collect(),
            _ => None,
        };
        match targets {
            None => push(
                out,
                doc,
                Level::L2,
                "invalid-supersedes",
                "`supersedes` must be a concept ID or a list of IDs",
            ),
            Some(ts) => {
                for t in ts {
                    let id = t.trim().trim_start_matches('/');
                    let id = id.strip_suffix(".md").unwrap_or(id);
                    if !ids.contains(id) {
                        push(
                            out,
                            doc,
                            Level::L2,
                            "unknown-supersedes",
                            format!("`supersedes` points to `{t}`, which does not exist"),
                        );
                    }
                }
            }
        }
    }
}

/// Whether `s` starts with an ISO date `YYYY-MM-DD`, optionally followed by a time.
pub fn is_iso_date(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() >= 10
        && b[..10].iter().enumerate().all(|(i, c)| {
            if i == 4 || i == 7 {
                *c == b'-'
            } else {
                c.is_ascii_digit()
            }
        })
        && (b.len() == 10 || matches!(b[10], b'T' | b't' | b' '))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(path: &str, text: &str) -> Concept {
        Concept::parse(Path::new(path), text).unwrap()
    }

    fn codes(a: &Assessment) -> Vec<(&str, Level, &str)> {
        a.findings
            .iter()
            .map(|f| (f.path.as_str(), f.level, f.code))
            .collect()
    }

    const L2_DOC: &str = "---\ntype: Policy\ntitle: Refunds\ndescription: How refunds work.\ntags: [refund]\nlang: en\nstatus: stable\nupdated: 2026-09-01\n---\n";

    fn vocab() -> Vocabulary {
        let v = doc(
            "_meta/vocabulary.md",
            "---\ntype: Vocabulary\nterms:\n  refund: [hoàn tiền]\n---\n",
        );
        Vocabulary::from_concept(&v).unwrap()
    }

    #[test]
    fn full_l2_bundle() {
        let docs = [
            doc("index.md", "# x\n"),
            doc("refund.md", L2_DOC),
            doc("_meta/vocabulary.md", "---\ntype: Vocabulary\n---\n"),
        ];
        let a = assess(&docs, Some(&vocab()));
        assert_eq!(codes(&a), []);
        assert_eq!((a.level, a.concepts), (Some(Level::L2), 2));
    }

    #[test]
    fn levels_and_findings() {
        let docs = [
            doc(
                "a/x.md",
                "---\ntype: T\nsummary: Foo.\ntitle: X\ntags: [hoàn tiền, nope]\nstatus: live\nupdated: 5\nsupersedes: [a/y, a/gone]\n---\n",
            ),
            doc(
                "a/y.md",
                "---\ntype: T\ntitle: Same\ndescription: same\n---\n",
            ),
        ];
        let a = assess(&docs, Some(&vocab()));
        assert_eq!(a.level, Some(Level::L0));
        let c = codes(&a);
        assert!(c.contains(&("a/x.md", Level::L1, "missing-description")));
        assert!(c.contains(&("a/y.md", Level::L1, "description-repeats-title")));
        assert!(c.contains(&("index.md", Level::L1, "missing-index")));
        assert!(c.contains(&("a/index.md", Level::L1, "missing-index")));
        for code in [
            "non-canonical-tag",
            "unknown-tag",
            "missing-lang",
            "invalid-status",
            "invalid-updated",
            "unknown-supersedes",
        ] {
            assert!(c.contains(&("a/x.md", Level::L2, code)), "{code}");
        }
        assert!(
            a.findings
                .iter()
                .any(|f| f.message.contains("found `summary`"))
        );
    }

    #[test]
    fn l0_failure_and_missing_vocabulary() {
        let a = assess(&[doc("index.md", ""), doc("x.md", "# no fm\n")], None);
        assert_eq!(a.level, None);
        assert!(codes(&a).contains(&("x.md", Level::L0, "missing-frontmatter")));
        let a = assess(&[doc("index.md", ""), doc("refund.md", L2_DOC)], None);
        assert_eq!(
            (a.level, codes(&a)),
            (
                Some(Level::L1),
                vec![("_meta/vocabulary.md", Level::L2, "no-vocabulary")]
            )
        );
    }

    #[test]
    fn iso_dates() {
        assert!(is_iso_date("2026-09-01") && is_iso_date("2026-09-01T10:00:00Z"));
        assert!(!is_iso_date("2026-9-1") && !is_iso_date("2026-09-01x"));
    }
}
