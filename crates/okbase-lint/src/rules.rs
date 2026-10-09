//! The built-in rules.

use std::collections::{BTreeSet, HashMap, HashSet};

use okbase_core::{
    Concept, ConceptId, LinkKind, Severity as CoreSeverity, Value, resolve_link, resolve_wikilink,
    validate_profile,
};
use okbase_standard::{FieldProblem, Level, assess_profile, is_iso_date};

use crate::{Context, Diagnostic, LintRule, RuleInfo, Severity};

/// Longest description, in characters, still considered one sentence.
pub const MAX_DESCRIPTION_CHARS: usize = 300;
/// Shortest useful description, in characters (CJK characters count triple).
pub const MIN_DESCRIPTION_CHARS: usize = 20;
/// Documents above this many (estimated) tokens get a "consider splitting" warning.
pub const MAX_DOC_TOKENS: usize = 8_000;

macro_rules! info {
    ($id:literal, $level:ident, $sev:ident, $desc:literal) => {
        RuleInfo {
            id: $id,
            level: Level::$level,
            severity: Severity::$sev,
            description: $desc,
        }
    };
}

/// Every diagnostic code with its level and default severity.
pub static CATALOG: &[RuleInfo] = &[
    info!(
        "empty-bundle",
        L0, Error, "The bundle has no markdown documents; run `okbase onboard`."
    ),
    info!(
        "missing-frontmatter",
        L0, Error, "A concept has no YAML frontmatter block."
    ),
    info!(
        "invalid-frontmatter",
        L0, Error, "The frontmatter cannot be parsed as a YAML mapping."
    ),
    info!("missing-type", L0, Error, "The frontmatter has no `type`."),
    info!(
        "invalid-type",
        L0, Error, "`type` is not a non-empty string."
    ),
    info!(
        "index-frontmatter",
        L0, Error, "An index.md has frontmatter other than `okf_version` in the root index."
    ),
    info!(
        "unreadable",
        L0, Error, "A file cannot be read or is not UTF-8."
    ),
    info!(
        "log-date-heading",
        L0, Warning, "A log.md `##` heading is not an ISO date."
    ),
    info!(
        "doc-too-long",
        L0, Warning, "A document is very long; consider splitting it."
    ),
    info!("missing-title", L1, Error, "A concept has no `title`."),
    info!(
        "missing-description",
        L1, Error, "A concept has no `description`."
    ),
    info!(
        "description-repeats-title",
        L1, Error, "`description` only repeats the title."
    ),
    info!(
        "description-too-short",
        L1, Warning, "`description` is too short to tell documents apart."
    ),
    info!(
        "description-too-long",
        L1, Warning, "`description` is longer than one sentence."
    ),
    info!("missing-index", L1, Error, "A directory has no index.md."),
    info!(
        "stale-index",
        L1, Warning, "An index.md does not list every document or subdirectory."
    ),
    info!(
        "unreviewed-generated",
        L1, Warning, "A title, description or type was filled in by a tool and not yet reviewed."
    ),
    info!("missing-tags", L2, Error, "A concept has no `tags`."),
    info!(
        "no-vocabulary",
        L2, Error, "The bundle has no `_meta/vocabulary.md`."
    ),
    info!(
        "invalid-vocabulary",
        L2, Error, "The vocabulary cannot be parsed."
    ),
    info!("unknown-tag", L2, Error, "A tag is not in the vocabulary."),
    info!(
        "non-canonical-tag",
        L2, Error, "A tag is a synonym; the canonical tag should be used."
    ),
    info!("missing-lang", L2, Error, "A concept has no `lang`."),
    info!("missing-status", L2, Error, "A concept has no `status`."),
    info!(
        "invalid-status",
        L2, Error, "`status` is not stable, deprecated or draft."
    ),
    info!(
        "missing-updated",
        L2, Error, "A concept has no `updated` date."
    ),
    info!(
        "invalid-updated",
        L2, Error, "`updated` is not an ISO date."
    ),
    info!(
        "invalid-supersedes",
        L2, Error, "`supersedes` is not an ID or a list of IDs."
    ),
    info!(
        "unknown-supersedes",
        L2, Error, "`supersedes` points to a document that does not exist."
    ),
    info!(
        "superseded-still-stable",
        L2, Error, "A superseded document is still `stable`."
    ),
    info!(
        "invalid-effective-range",
        L2, Error, "`effective_from`/`effective_to` are not dates, or the range is inverted."
    ),
    info!(
        "invalid-schema",
        L2, Error, "A `_meta/types` schema cannot be parsed."
    ),
    info!(
        "schema-missing-field",
        L2, Error, "A field required by the type schema is missing."
    ),
    info!(
        "schema-wrong-type",
        L2, Error, "A field has the wrong type for the type schema."
    ),
    info!(
        "schema-not-allowed",
        L2, Error, "A field value is not one of the values the type schema allows."
    ),
    info!(
        "broken-link",
        L3, Warning, "A link points to a document that does not exist."
    ),
];

/// Looks up a code in the catalog.
pub fn rule_info(code: &str) -> Option<&'static RuleInfo> {
    CATALOG.iter().find(|r| r.id == code)
}

fn diag(
    code: &'static str,
    path: &str,
    field: Option<&str>,
    message: impl Into<String>,
) -> Diagnostic {
    let info = rule_info(code).expect("code in catalog");
    Diagnostic {
        rule: code,
        level: info.level,
        severity: info.severity,
        path: path.to_owned(),
        line: None,
        field: field.map(str::to_owned),
        message: message.into(),
    }
}

/// The okbase level checks (L0–L2) from `okbase-standard`.
pub struct StandardLevels;

impl LintRule for StandardLevels {
    fn name(&self) -> &'static str {
        "standard-levels"
    }

    fn check(&self, cx: &Context) -> Vec<Diagnostic> {
        let a = assess_profile(cx.docs, cx.vocabulary, cx.content_index);
        let mut out: Vec<Diagnostic> = cx
            .unreadable
            .iter()
            .map(|(p, m)| diag("unreadable", p, None, m.clone()))
            .collect();
        let found = a.findings.into_iter().map(|mut f| {
            if f.code == "no-vocabulary"
                && let Some(e) = cx.vocabulary_error
            {
                f.code = "invalid-vocabulary";
                f.message = e.to_owned();
            }
            if f.code == "no-vocabulary" || f.code == "invalid-vocabulary" {
                f.path = cx.config.vocabulary.clone();
            }
            f
        });
        out.extend(found.filter_map(|f| {
            let code = rule_info(f.code)?.id;
            let field = match code {
                "invalid-status" => Some("status"),
                "invalid-updated" => Some("updated"),
                "unknown-tag" | "non-canonical-tag" | "missing-tags" => Some("tags"),
                "invalid-supersedes" | "unknown-supersedes" => Some("supersedes"),
                "invalid-type" => Some("type"),
                "description-repeats-title" => Some("description"),
                _ => None,
            };
            Some(diag(code, &f.path, field, f.message))
        }));
        out
    }
}

/// OKF warnings from `okbase-core` (log headings) and document length.
pub struct General;

impl LintRule for General {
    fn name(&self) -> &'static str {
        "general"
    }

    fn check(&self, cx: &Context) -> Vec<Diagnostic> {
        let mut out = Vec::new();
        for doc in cx.docs {
            for issue in validate_profile(doc, cx.content_index)
                .into_iter()
                .filter(|i| i.severity == CoreSeverity::Warning)
            {
                if let Some(info) = rule_info(issue.code) {
                    out.push(diag(info.id, &doc.path, None, issue.message));
                }
            }
            if !doc.is_reserved_in(cx.content_index) {
                let tokens = okbase_analyze::estimate_tokens(&doc.body);
                if tokens > cx.config.max_doc_tokens {
                    out.push(diag(
                        "doc-too-long",
                        &doc.path,
                        None,
                        format!("about {tokens} tokens; consider splitting it into several concepts linked from an index"),
                    ));
                }
            }
        }
        out
    }
}

/// Description length (L1).
pub struct DescriptionQuality;

impl LintRule for DescriptionQuality {
    fn name(&self) -> &'static str {
        "description-quality"
    }

    fn check(&self, cx: &Context) -> Vec<Diagnostic> {
        let mut out = Vec::new();
        for doc in cx.concepts() {
            let Some(d) = doc
                .frontmatter
                .get_str("description")
                .map(str::trim)
                .filter(|d| !d.is_empty())
            else {
                continue;
            };
            let weight: usize = d
                .chars()
                .map(|c| if okbase_analyze::is_cjk(c) { 3 } else { 1 })
                .sum();
            if weight < MIN_DESCRIPTION_CHARS {
                out.push(diag(
                    "description-too-short",
                    &doc.path,
                    Some("description"),
                    format!("`description` is only {} characters; say what the document covers in one sentence", d.chars().count()),
                ));
            } else if d.chars().count() > MAX_DESCRIPTION_CHARS {
                out.push(diag(
                    "description-too-long",
                    &doc.path,
                    Some("description"),
                    format!("`description` is {} characters; keep it to one sentence (≤ {MAX_DESCRIPTION_CHARS})", d.chars().count()),
                ));
            }
        }
        out
    }
}

/// Fields listed in `generated.fields` (written by `okbase adopt`) without a `verified` entry (L1).
pub struct UnreviewedGenerated;

impl LintRule for UnreviewedGenerated {
    fn name(&self) -> &'static str {
        "unreviewed-generated"
    }

    fn check(&self, cx: &Context) -> Vec<Diagnostic> {
        let mut out = Vec::new();
        for doc in cx.concepts() {
            let fm = &doc.frontmatter;
            if fm.get("verified").is_some() {
                continue;
            }
            let Some(Value::Array(fields)) = fm.get("generated").and_then(|g| g.get("fields"))
            else {
                continue;
            };
            let key: Vec<&str> = fields
                .iter()
                .filter_map(Value::as_str)
                .filter(|f| ["title", "description", "type"].contains(f))
                .collect();
            if !key.is_empty() {
                out.push(diag(
                    "unreviewed-generated",
                    &doc.path,
                    Some(key[0]),
                    format!(
                        "{} filled in by a tool; review, then drop it from generated.fields",
                        key.join(", ")
                    ),
                ));
            }
        }
        out
    }
}

/// index.md files that miss documents or subdirectories of their directory (L1).
pub struct StaleIndex;

impl LintRule for StaleIndex {
    fn name(&self) -> &'static str {
        "stale-index"
    }

    fn check(&self, cx: &Context) -> Vec<Diagnostic> {
        let mut out = Vec::new();
        // Under the docs-site and vault profiles, index.md is a content page, not a listing.
        for index in cx
            .docs
            .iter()
            .filter(|d| d.id.name() == "index" && !cx.content_index)
        {
            let dir = index.id.dir();
            if is_meta(dir) {
                continue;
            }
            let prefix = if dir.is_empty() {
                String::new()
            } else {
                format!("{dir}/")
            };
            let listed: HashSet<String> = index
                .links()
                .iter()
                .filter(|l| l.kind == LinkKind::Markdown)
                .filter_map(|l| resolve_link(&index.id, &l.target))
                .map(|id| id.as_str().to_owned())
                .collect();
            let mut missing = BTreeSet::new();
            for doc in cx.concepts() {
                let Some(rest) = doc.id.as_str().strip_prefix(&prefix) else {
                    continue;
                };
                let entry = match rest.split_once('/') {
                    Some((sub, _)) if !is_meta(&format!("{prefix}{sub}")) => {
                        format!("{prefix}{sub}/index")
                    }
                    Some(_) => continue,
                    None => doc.id.as_str().to_owned(),
                };
                if !listed.contains(&entry) {
                    missing.insert(
                        entry
                            .strip_suffix("/index")
                            .map_or_else(|| entry.clone(), |d| format!("{d}/")),
                    );
                }
            }
            if !missing.is_empty() {
                let list: Vec<_> = missing.into_iter().collect();
                out.push(diag(
                    "stale-index",
                    &index.path,
                    None,
                    format!("does not list: {}", list.join(", ")),
                ));
            }
        }
        out
    }
}

/// `supersedes` consistency and effective dates (L2).
pub struct Lifecycle;

impl LintRule for Lifecycle {
    fn name(&self) -> &'static str {
        "lifecycle"
    }

    fn check(&self, cx: &Context) -> Vec<Diagnostic> {
        let mut out = Vec::new();
        let by_id: HashMap<&str, &Concept> = cx.docs.iter().map(|d| (d.id.as_str(), d)).collect();
        for doc in cx.concepts() {
            let fm = &doc.frontmatter;
            let targets: Vec<&str> = match fm.get("supersedes") {
                Some(Value::String(s)) => vec![s.as_str()],
                Some(Value::Array(items)) => items.iter().filter_map(Value::as_str).collect(),
                _ => Vec::new(),
            };
            for t in targets {
                let id = t.trim().trim_start_matches('/');
                let id = id.strip_suffix(".md").unwrap_or(id);
                if let Some(old) = by_id.get(id)
                    && old.frontmatter.get_str("status") == Some("stable")
                {
                    out.push(diag(
                        "superseded-still-stable",
                        &old.path,
                        Some("status"),
                        format!(
                            "superseded by `{}` but still `status: stable`; set it to `deprecated`",
                            doc.id
                        ),
                    ));
                }
            }
            let date = |k: &str| {
                fm.get(k).map(|v| {
                    v.as_str()
                        .filter(|s| is_iso_date(s))
                        .map(|s| s[..10].to_owned())
                })
            };
            let (from, to) = (date("effective_from"), date("effective_to"));
            for (key, v) in [("effective_from", &from), ("effective_to", &to)] {
                if matches!(v, Some(None)) {
                    out.push(diag(
                        "invalid-effective-range",
                        &doc.path,
                        Some(key),
                        format!("`{key}` is not an ISO date (YYYY-MM-DD)"),
                    ));
                }
            }
            if let (Some(Some(f)), Some(Some(t))) = (&from, &to)
                && f > t
            {
                out.push(diag(
                    "invalid-effective-range",
                    &doc.path,
                    Some("effective_to"),
                    format!("`effective_to` ({t}) is before `effective_from` ({f})"),
                ));
            }
        }
        out
    }
}

/// Fields checked against `_meta/types/<Type>.md` (L2).
pub struct TypeSchemas;

impl LintRule for TypeSchemas {
    fn name(&self) -> &'static str {
        "type-schemas"
    }

    fn check(&self, cx: &Context) -> Vec<Diagnostic> {
        let mut out: Vec<Diagnostic> = cx
            .schema_errors
            .iter()
            .map(|(path, msg)| diag("invalid-schema", path, None, msg.clone()))
            .collect();
        for doc in cx.concepts() {
            let Some(schema) = doc.concept_type().and_then(|t| cx.schemas.get(t)) else {
                continue;
            };
            for (field, problem) in schema.check(&doc.frontmatter) {
                let (code, msg) = match problem {
                    FieldProblem::Missing => (
                        "schema-missing-field",
                        format!("`{field}` is required for type `{}`", schema.applies_to),
                    ),
                    FieldProblem::WrongType(t) => (
                        "schema-wrong-type",
                        format!("`{field}` must be a {t} (see {})", schema.path),
                    ),
                    FieldProblem::NotAllowed(allowed) => (
                        "schema-not-allowed",
                        format!("`{field}` must be one of: {}", allowed.join(", ")),
                    ),
                };
                out.push(diag(code, &doc.path, Some(&field), msg));
            }
        }
        out
    }
}

/// Links to documents that do not exist (L3).
pub struct BrokenLinks;

impl LintRule for BrokenLinks {
    fn name(&self) -> &'static str {
        "broken-links"
    }

    fn check(&self, cx: &Context) -> Vec<Diagnostic> {
        let ids: Vec<ConceptId> = cx.docs.iter().map(|d| d.id.clone()).collect();
        let known: HashSet<&str> = ids.iter().map(ConceptId::as_str).collect();
        let mut out = Vec::new();
        for doc in cx.docs {
            let body_start = doc.frontmatter.render().lines().count();
            for link in doc.links() {
                let broken = match link.kind {
                    LinkKind::Markdown => {
                        let is_md = link
                            .target
                            .split(['#', '?'])
                            .next()
                            .is_some_and(|t| t.ends_with(".md") || t.ends_with('/'));
                        is_md
                            && resolve_link(&doc.id, &link.target)
                                .is_some_and(|t| !known.contains(t.as_str()))
                    }
                    LinkKind::Wiki => resolve_wikilink(&link.target, &ids).is_none(),
                };
                if broken {
                    let mut d = diag(
                        "broken-link",
                        &doc.path,
                        None,
                        format!("link to `{}` does not resolve to a document", link.target),
                    );
                    d.line = Some(body_start + link.line);
                    out.push(d);
                }
            }
        }
        out
    }
}

fn is_meta(dir: &str) -> bool {
    dir == okbase_standard::META_DIR || dir.starts_with("_meta/")
}
