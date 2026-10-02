//! OKF v0.2 conformance checks (okfkit level L0).

use std::sync::LazyLock;

use regex::Regex;

use crate::{Concept, FrontmatterState};

/// How serious an issue is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// Allowed, but not recommended by the spec.
    Warning,
    /// Makes the bundle non-conformant with OKF v0.2.
    Error,
}

/// A conformance problem in one document.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Issue {
    /// Stable machine-readable code, such as `missing-type`.
    pub code: &'static str,
    /// Severity.
    pub severity: Severity,
    /// Human-readable message.
    pub message: String,
}

impl Issue {
    fn error(code: &'static str, message: impl Into<String>) -> Self {
        Issue {
            code,
            severity: Severity::Error,
            message: message.into(),
        }
    }
    fn warning(code: &'static str, message: impl Into<String>) -> Self {
        Issue {
            code,
            severity: Severity::Warning,
            message: message.into(),
        }
    }
}

static DATE_HEADING: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^##[ \t]+(.*?)[ \t#]*$").expect("valid regex"));
static ISO_DATE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\d{4}-\d{2}-\d{2}$").expect("valid regex"));

/// Checks one document against OKF v0.2 §11.
///
/// Concepts need a parseable frontmatter mapping with a non-empty string `type`.
/// `index.md` has no frontmatter, except `okf_version` in the bundle-root index.
/// `log.md` `##` headings are ISO dates (a warning, since the spec leaves the rest as prose).
pub fn validate(doc: &Concept) -> Vec<Issue> {
    validate_profile(doc, false)
}

/// Like [`validate`]; with `content_index` (docs-site and vault profiles) `index.md` is checked as
/// a concept like any other page.
pub fn validate_profile(doc: &Concept, content_index: bool) -> Vec<Issue> {
    let fm = &doc.frontmatter;
    let mut issues = Vec::new();
    let name = match doc.id.name() {
        "index" if content_index => "",
        n => n,
    };
    match name {
        "index" => {
            let root = doc.id.dir().is_empty();
            if fm.is_present()
                && !(root && fm.keys().all(|k| k == "okf_version") && fm.error().is_none())
            {
                let msg = if root {
                    "the bundle-root index.md may only carry `okf_version` in its frontmatter"
                } else {
                    "index.md must not have frontmatter"
                };
                issues.push(Issue::error("index-frontmatter", msg));
            }
        }
        "log" => {
            let mut fenced = false;
            for line in doc.body.lines() {
                if line.trim_start().starts_with("```") || line.trim_start().starts_with("~~~") {
                    fenced = !fenced;
                }
                if let Some(c) = DATE_HEADING.captures(line).filter(|_| !fenced)
                    && !ISO_DATE.is_match(&c[1])
                {
                    issues.push(Issue::warning(
                        "log-date-heading",
                        format!(
                            "log heading `{}` is not an ISO 8601 date (YYYY-MM-DD)",
                            &c[1]
                        ),
                    ));
                }
            }
        }
        _ => match fm.state() {
            FrontmatterState::Absent => issues.push(Issue::error(
                "missing-frontmatter",
                "no YAML frontmatter block (OKF requires `type`)",
            )),
            FrontmatterState::Unterminated | FrontmatterState::Invalid => {
                issues.push(Issue::error(
                    "invalid-frontmatter",
                    format!(
                        "frontmatter cannot be parsed: {}",
                        // serde-saphyr appends a source snippet; the first line says it all.
                        fm.error()
                            .and_then(|e| e.lines().next())
                            .unwrap_or_default()
                            .trim_start_matches("error: ")
                    ),
                ))
            }
            FrontmatterState::Valid => match fm.get("type") {
                None => issues.push(Issue::error("missing-type", "frontmatter has no `type`")),
                Some(v) if v.as_str().is_none_or(|s| s.trim().is_empty()) => issues.push(
                    Issue::error("invalid-type", "`type` must be a non-empty string"),
                ),
                Some(_) => {}
            },
        },
    }
    issues
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn codes(path: &str, text: &str) -> Vec<&'static str> {
        validate(&Concept::parse(Path::new(path), text).unwrap())
            .iter()
            .map(|i| i.code)
            .collect()
    }

    #[test]
    fn concepts() {
        assert!(codes("a.md", "---\ntype: X\n---\n").is_empty());
        assert_eq!(codes("a.md", "# x\n"), ["missing-frontmatter"]);
        assert_eq!(codes("a.md", "---\ntype: X\n"), ["invalid-frontmatter"]);
        assert_eq!(codes("a.md", "---\ntitle: X\n---\n"), ["missing-type"]);
        assert_eq!(codes("a.md", "---\ntype: ''\n---\n"), ["invalid-type"]);
        assert_eq!(codes("a.md", "---\ntype: [a]\n---\n"), ["invalid-type"]);
    }

    #[test]
    fn reserved() {
        assert!(codes("index.md", "---\nokf_version: \"0.2\"\n---\n# T\n").is_empty());
        assert_eq!(
            codes("sub/index.md", "---\nokf_version: \"0.2\"\n---\n"),
            ["index-frontmatter"]
        );
        assert_eq!(
            codes("index.md", "---\ntype: X\n---\n"),
            ["index-frontmatter"]
        );
        assert!(
            codes(
                "log.md",
                "# Log\n\n## 2026-05-22\n* x\n\n```\n## not a heading\n```\n"
            )
            .is_empty()
        );
        assert_eq!(codes("log.md", "## May 22\n"), ["log-date-heading"]);
    }
}
