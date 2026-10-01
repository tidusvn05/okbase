//! Lint for okfkit bundles: a rule engine with the okfkit standard rules
//! (levels L0–L3, PLAN §9), text/JSON/SARIF output and safe automatic fixes.
//!
//! Linting only reads the bundle. [`fix_safe`] writes, and only when called.
//!
//! ```no_run
//! use okfkit_lint::{LintConfig, lint};
//! use okfkit_standard::Level;
//!
//! let report = lint("path/to/bundle".as_ref(), &LintConfig::level(Level::L2))?;
//! print!("{}", report.to_text());
//! # Ok::<(), okfkit_lint::Error>(())
//! ```

mod fix;
pub mod rules;
mod sarif;

use std::collections::BTreeMap;
use std::path::Path;

use okfkit_core::{Concept, discover};
use okfkit_standard::{TypeSchema, VOCABULARY_PATH, Vocabulary, load_type_schemas};
use serde::Serialize;

pub use fix::{FixReport, fix_safe};
pub use okfkit_standard::Level;
pub use rules::{CATALOG, rule_info};

/// Errors returned by `okfkit-lint`.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// Reading the bundle failed.
    #[error(transparent)]
    Core(#[from] okfkit_core::Error),
    /// Writing a fix failed.
    #[error("cannot write {path}: {source}")]
    Write {
        /// The file.
        path: String,
        /// The underlying error.
        source: std::io::Error,
    },
}

/// How serious a diagnostic is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// Worth a look; never fails a lint run.
    Warning,
    /// Keeps the bundle from reaching the target level.
    Error,
}

/// Static description of a diagnostic code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RuleInfo {
    /// Stable code, such as `missing-description`.
    pub id: &'static str,
    /// The level the rule belongs to.
    pub level: Level,
    /// Default severity.
    pub severity: Severity,
    /// One-line description.
    pub description: &'static str,
}

/// A problem found by a rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Diagnostic {
    /// Code (see [`CATALOG`]).
    pub rule: &'static str,
    /// Level of the rule.
    pub level: Level,
    /// Severity.
    pub severity: Severity,
    /// Bundle-relative path.
    pub path: String,
    /// 1-based line in the file, when known.
    pub line: Option<usize>,
    /// The frontmatter field involved, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    /// Message, with a hint when one helps.
    pub message: String,
}

/// Lint settings.
#[derive(Debug, Clone)]
pub struct LintConfig {
    /// Rules above this level are not run.
    pub target: Level,
    /// Codes to skip.
    pub disabled: Vec<String>,
    /// Bundle-relative path of the vocabulary.
    pub vocabulary: String,
    /// Token threshold of `doc-too-long`.
    pub max_doc_tokens: usize,
}

impl LintConfig {
    /// Default settings for a target level.
    pub fn level(target: Level) -> Self {
        LintConfig {
            target,
            ..Default::default()
        }
    }
}

impl Default for LintConfig {
    fn default() -> Self {
        LintConfig {
            target: Level::L2,
            disabled: Vec::new(),
            vocabulary: VOCABULARY_PATH.to_owned(),
            max_doc_tokens: rules::MAX_DOC_TOKENS,
        }
    }
}

/// Everything a rule can look at.
pub struct Context<'a> {
    /// Bundle root.
    pub root: &'a Path,
    /// Every readable markdown document, reserved files included, sorted by path.
    pub docs: &'a [Concept],
    /// The tag vocabulary, if present and valid.
    pub vocabulary: Option<&'a Vocabulary>,
    /// Why the vocabulary could not be read, if it exists but is invalid.
    pub vocabulary_error: Option<&'a str>,
    /// Type schemas by type name.
    pub schemas: &'a BTreeMap<String, TypeSchema>,
    /// Schema documents that could not be parsed: (path, message).
    pub schema_errors: &'a [(String, String)],
    /// Files that could not be read: (path, message).
    pub unreadable: &'a [(String, String)],
    /// Settings.
    pub config: &'a LintConfig,
}

impl Context<'_> {
    /// Concept documents outside `_meta/` (reserved files excluded).
    pub fn concepts(&self) -> impl Iterator<Item = &Concept> {
        self.docs
            .iter()
            .filter(|d| !d.is_reserved() && !d.path.starts_with("_meta/"))
    }
}

/// A lint rule. Implement it to add project-specific checks.
pub trait LintRule: Send + Sync {
    /// Short name of the rule group.
    fn name(&self) -> &'static str;
    /// Diagnostics for the bundle. Their `level` decides whether they run for a target.
    fn check(&self, cx: &Context) -> Vec<Diagnostic>;
}

/// The built-in rules.
pub fn default_rules() -> Vec<Box<dyn LintRule>> {
    vec![
        Box::new(rules::StandardLevels),
        Box::new(rules::General),
        Box::new(rules::DescriptionQuality),
        Box::new(rules::StaleIndex),
        Box::new(rules::UnreviewedGenerated),
        Box::new(rules::Lifecycle),
        Box::new(rules::TypeSchemas),
        Box::new(rules::BrokenLinks),
    ]
}

/// The outcome of a lint run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Report {
    /// The level linted against.
    pub target: Level,
    /// Highest level fully met (L0–L2), or `None` if the bundle is not OKF-conformant.
    pub level: Option<Level>,
    /// Documents checked.
    pub documents: usize,
    /// Errors.
    pub errors: usize,
    /// Warnings.
    pub warnings: usize,
    /// All diagnostics, sorted by path, line and rule.
    pub diagnostics: Vec<Diagnostic>,
}

/// Lints the bundle at `root` with the built-in rules.
pub fn lint(root: &Path, config: &LintConfig) -> Result<Report, Error> {
    lint_with(root, config, &default_rules())
}

/// Lints the bundle at `root` with the given rules.
pub fn lint_with(
    root: &Path,
    config: &LintConfig,
    rules: &[Box<dyn LintRule>],
) -> Result<Report, Error> {
    let mut docs = Vec::new();
    let mut texts = BTreeMap::new();
    let mut unreadable = Vec::new();
    for rel in discover(root)? {
        let path = rel.to_string_lossy().replace('\\', "/");
        match std::fs::read(root.join(&rel))
            .map_err(|e| e.to_string())
            .and_then(|b| String::from_utf8(b).map_err(|_| "not valid UTF-8".to_owned()))
        {
            Ok(text) => match Concept::parse(&rel, &text) {
                Ok(doc) => {
                    texts.insert(doc.path.clone(), text);
                    docs.push(doc);
                }
                Err(e) => unreadable.push((path, e.to_string())),
            },
            Err(e) => unreadable.push((path, e)),
        }
    }
    let (vocabulary, vocabulary_error) = match Vocabulary::load(root, &config.vocabulary) {
        Ok(v) => (v, None),
        Err(e) => (None, Some(e.to_string())),
    };
    let (schemas, schema_errors) = match load_type_schemas(root) {
        Ok(s) => (s, Vec::new()),
        Err(e) => (
            BTreeMap::new(),
            vec![(okfkit_standard::TYPES_DIR.to_owned(), e.to_string())],
        ),
    };
    let cx = Context {
        root,
        docs: &docs,
        vocabulary: vocabulary.as_ref(),
        vocabulary_error: vocabulary_error.as_deref(),
        schemas: &schemas,
        schema_errors: &schema_errors,
        unreadable: &unreadable,
        config,
    };
    let mut all: Vec<Diagnostic> = rules.iter().flat_map(|r| r.check(&cx)).collect();
    let level = achieved(&all);
    all.retain(|d| d.level <= config.target && !config.disabled.iter().any(|x| x == d.rule));
    for d in &mut all {
        if d.line.is_none() {
            d.line = Some(
                texts
                    .get(&d.path)
                    .map_or(1, |t| line_of(t, d.field.as_deref())),
            );
        }
    }
    all.sort_by(|a, b| {
        (&a.path, a.line, a.rule, &a.message).cmp(&(&b.path, b.line, b.rule, &b.message))
    });
    all.dedup();
    let errors = all.iter().filter(|d| d.severity == Severity::Error).count();
    Ok(Report {
        target: config.target,
        level,
        documents: docs.len(),
        errors,
        warnings: all.len() - errors,
        diagnostics: all,
    })
}

/// Highest level with no errors at or below it (L3 is never claimed by lint).
fn achieved(diags: &[Diagnostic]) -> Option<Level> {
    let lowest = diags
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .map(|d| d.level)
        .min();
    match lowest {
        Some(Level::L0) => None,
        Some(Level::L1) => Some(Level::L0),
        Some(Level::L2) => Some(Level::L1),
        _ => Some(Level::L2),
    }
}

/// Line of a frontmatter key, else 1.
fn line_of(text: &str, field: Option<&str>) -> usize {
    let Some(field) = field else { return 1 };
    let mut in_fm = false;
    for (i, line) in text.lines().enumerate() {
        if line.trim_end() == "---" {
            if in_fm {
                break;
            }
            in_fm = i == 0;
            continue;
        }
        if in_fm
            && line
                .strip_prefix(field)
                .is_some_and(|r| r.trim_start().starts_with(':'))
        {
            return i + 1;
        }
    }
    1
}

impl Report {
    /// Human-readable output: one `path:line: severity[code] message` per diagnostic and a summary.
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        for d in &self.diagnostics {
            let sev = match d.severity {
                Severity::Error => "error",
                Severity::Warning => "warning",
            };
            out.push_str(&format!(
                "{}:{}: {sev}[{}] {}\n",
                d.path,
                d.line.unwrap_or(1),
                d.rule,
                d.message
            ));
        }
        let level = self.level.map_or_else(
            || "below L0 (not OKF-conformant)".to_owned(),
            |l| l.to_string(),
        );
        out.push_str(&format!(
            "{} documents, {} errors, {} warnings (target {}); bundle level: {level}\n",
            self.documents, self.errors, self.warnings, self.target
        ));
        out
    }

    /// SARIF 2.1.0 output, for code-scanning tools.
    pub fn to_sarif(&self) -> serde_json::Value {
        sarif::to_sarif(self)
    }
}

#[cfg(test)]
mod tests {
    use super::line_of;

    #[test]
    fn lines() {
        let t = "---\ntype: A\nstatus: live\n---\nstatus: body\n";
        assert_eq!(line_of(t, Some("status")), 3);
        assert_eq!(line_of(t, Some("tags")), 1);
        assert_eq!(line_of(t, None), 1);
    }
}
