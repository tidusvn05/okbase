//! What a caller may read. The host decides; okbase only enforces.

use regex::Regex;
use serde_json::{Map, Value};

use crate::Error;

/// A path pattern over concept IDs (`a/b` for `a/b.md`).
///
/// `*` matches within one path segment, `**` across segments. A pattern without
/// `*` is a prefix: `policies` and `policies/` match everything under
/// `policies/` and `policies` itself. A trailing `.md` is ignored.
#[derive(Debug, Clone)]
pub struct PathGlob {
    pattern: String,
    re: Regex,
}

impl PathGlob {
    /// Compiles a pattern.
    pub fn new(pattern: &str) -> Result<Self, Error> {
        let p = pattern
            .trim()
            .trim_start_matches("./")
            .trim_start_matches('/');
        let p = p.strip_suffix(".md").unwrap_or(p);
        let re = if p.is_empty() || p == "." || p == "**" {
            String::from("^")
        } else if !p.contains('*') {
            let p = p.trim_end_matches('/');
            format!("^{}(?:/|$)", regex::escape(p))
        } else {
            let mut re = String::from("^");
            let mut it = p.chars().peekable();
            while let Some(c) = it.next() {
                if c == '*' {
                    if it.peek() == Some(&'*') {
                        it.next();
                        re.push_str(".*");
                    } else {
                        re.push_str("[^/]*");
                    }
                } else {
                    re.push_str(&regex::escape(&c.to_string()));
                }
            }
            re.push('$');
            re
        };
        let re = Regex::new(&re)
            .map_err(|e| Error::InvalidArgument(format!("path pattern {pattern:?}: {e}")))?;
        Ok(PathGlob {
            pattern: pattern.to_owned(),
            re,
        })
    }

    /// Whether the concept ID matches.
    pub fn matches(&self, id: &str) -> bool {
        self.re.is_match(id)
    }

    /// The pattern as given.
    pub fn as_str(&self) -> &str {
        &self.pattern
    }
}

/// A condition on a frontmatter field, used by a host to hide documents.
#[derive(Debug, Clone, PartialEq)]
pub enum MetaFilter {
    /// Keep documents whose field equals (or, for lists, contains) one of the values.
    /// Documents without the field are dropped.
    In(String, Vec<String>),
    /// Drop documents whose field equals (or contains) one of the values.
    /// Documents without the field are kept.
    NotIn(String, Vec<String>),
}

impl MetaFilter {
    /// Keeps only documents whose `audience` is not `value` (for example `private`).
    pub fn not_audience(value: &str) -> Self {
        MetaFilter::NotIn("audience".into(), vec![value.into()])
    }

    fn permits(&self, fm: &Map<String, Value>) -> bool {
        match self {
            MetaFilter::In(field, values) => {
                fm.get(field).is_some_and(|v| value_matches(v, values))
            }
            MetaFilter::NotIn(field, values) => {
                !fm.get(field).is_some_and(|v| value_matches(v, values))
            }
        }
    }
}

/// Whether a frontmatter value equals one of `values` (case-insensitive); lists match if any item does.
pub(crate) fn value_matches(v: &Value, values: &[String]) -> bool {
    match v {
        Value::Array(items) => items.iter().any(|i| value_matches(i, values)),
        Value::String(s) => values.iter().any(|x| x.eq_ignore_ascii_case(s.trim())),
        Value::Number(n) => values.iter().any(|x| x == &n.to_string()),
        Value::Bool(b) => values
            .iter()
            .any(|x| x.eq_ignore_ascii_case(&b.to_string())),
        _ => false,
    }
}

/// The set of documents a caller may see, provided by the host on every read.
///
/// A document is visible when it matches an `allow` pattern (all documents if
/// there are none), matches no `deny` pattern, and passes every filter. Hidden
/// documents behave as if they did not exist.
///
/// ```
/// use okbase_query::{MetaFilter, Scope};
/// let scope = Scope::all().deny("memory/people/**").unwrap().filter(MetaFilter::not_audience("private"));
/// assert!(scope.permits_path("policies/refunds"));
/// assert!(!scope.permits_path("memory/people/alice"));
/// ```
#[derive(Debug, Clone, Default)]
pub struct Scope {
    allow: Vec<PathGlob>,
    deny: Vec<PathGlob>,
    filters: Vec<MetaFilter>,
}

impl Scope {
    /// Everything is visible.
    pub fn all() -> Self {
        Scope::default()
    }

    /// Restricts visibility to documents matching `pattern` (repeatable: any match allows).
    pub fn allow(mut self, pattern: &str) -> Result<Self, Error> {
        self.allow.push(PathGlob::new(pattern)?);
        Ok(self)
    }

    /// Hides documents matching `pattern`.
    pub fn deny(mut self, pattern: &str) -> Result<Self, Error> {
        self.deny.push(PathGlob::new(pattern)?);
        Ok(self)
    }

    /// Hides documents that fail `filter`.
    pub fn filter(mut self, filter: MetaFilter) -> Self {
        self.filters.push(filter);
        self
    }

    /// Whether nothing is hidden.
    pub fn is_unrestricted(&self) -> bool {
        self.allow.is_empty() && self.deny.is_empty() && self.filters.is_empty()
    }

    /// Whether the path rules alone permit the ID.
    pub fn permits_path(&self, id: &str) -> bool {
        (self.allow.is_empty() || self.allow.iter().any(|g| g.matches(id)))
            && !self.deny.iter().any(|g| g.matches(id))
    }

    /// Whether a document with this ID and frontmatter is visible.
    pub fn permits(&self, id: &str, frontmatter: &Map<String, Value>) -> bool {
        self.permits_path(id) && self.filters.iter().all(|f| f.permits(frontmatter))
    }

    /// Whether the scope has metadata filters (which need the frontmatter to evaluate).
    pub fn has_filters(&self) -> bool {
        !self.filters.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn globs() {
        let g = |p: &str, id: &str| PathGlob::new(p).unwrap().matches(id);
        assert!(g("gateway/**", "gateway/a/b"));
        assert!(
            g("gateway/", "gateway/a") && g("gateway", "gateway") && !g("gateway", "gateways/x")
        );
        assert!(g("*/index", "a/index") && !g("*/index", "a/b/index"));
        assert!(g("policies/*.md", "policies/refund"));
        assert!(g(".", "anything") && g("", "x"));
    }

    #[test]
    fn scope_rules() {
        let s = Scope::all()
            .allow("public/**")
            .unwrap()
            .deny("public/drafts/**")
            .unwrap()
            .filter(MetaFilter::not_audience("private"));
        let fm = |v: Value| v.as_object().unwrap().clone();
        assert!(s.permits("public/a", &fm(json!({"audience": "all"}))));
        assert!(s.permits("public/a", &fm(json!({}))));
        assert!(!s.permits("public/a", &fm(json!({"audience": ["staff", "Private"]}))));
        assert!(!s.permits("public/drafts/a", &fm(json!({}))));
        assert!(!s.permits("internal/a", &fm(json!({}))));
        let only = Scope::all().filter(MetaFilter::In("lang".into(), vec!["vi".into()]));
        assert!(
            only.permits("x", &fm(json!({"lang": "vi"}))) && !only.permits("x", &fm(json!({})))
        );
    }
}
