//! The tag vocabulary: canonical tags, their synonyms (across languages) and facets.
//!
//! The vocabulary lives in `_meta/vocabulary.md`, an ordinary OKF document whose
//! frontmatter carries the terms. The body is free prose for humans.
//!
//! ```markdown
//! ---
//! type: Vocabulary
//! title: Tag vocabulary
//! terms:
//!   refund:
//!     synonyms: [hoàn tiền, đổi trả, 返金, returns]
//!     facet: topic
//!     description: Refunds, returns and exchanges.
//!   shipping: [giao hàng, 配送]   # shorthand: a list of synonyms
//!   hr: {}                        # a tag without synonyms
//! ---
//! ```

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use okfkit_core::{Concept, Value};
use serde::Serialize;

use crate::Error;

/// A canonical tag.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Term {
    /// The canonical tag, as written in the vocabulary.
    pub tag: String,
    /// Other spellings, translations and aliases that mean the same tag.
    pub synonyms: Vec<String>,
    /// The facet (dimension) this tag belongs to, such as `topic` or `department`.
    pub facet: Option<String>,
    /// What the tag means.
    pub description: Option<String>,
}

/// A parsed tag vocabulary.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Vocabulary {
    terms: Vec<Term>,
    /// Normalized tag or synonym → index into `terms`.
    lookup: HashMap<String, usize>,
}

/// Normalizes a tag for comparison: trimmed, lowercased, with runs of whitespace,
/// `_` and `-` collapsed to a single `-`.
///
/// Accent folding (`hoàn tiền` ~ `hoan tien`) is left to `okfkit-analyze`.
pub fn normalize_tag(tag: &str) -> String {
    let mut out = String::with_capacity(tag.len());
    let mut sep = false;
    for c in tag.trim().chars() {
        if c.is_whitespace() || c == '_' || c == '-' {
            sep = !out.is_empty();
        } else {
            if sep {
                out.push('-');
                sep = false;
            }
            out.extend(c.to_lowercase());
        }
    }
    out
}

impl Vocabulary {
    /// Loads the vocabulary at `root/rel_path`. Returns `Ok(None)` if the file does not exist.
    pub fn load(root: &Path, rel_path: &str) -> Result<Option<Self>, Error> {
        if !root.join(rel_path).is_file() {
            return Ok(None);
        }
        let doc = Concept::read(root, Path::new(rel_path))?;
        Vocabulary::from_concept(&doc).map(Some)
    }

    /// Parses the `terms` of a vocabulary document.
    pub fn from_concept(doc: &Concept) -> Result<Self, Error> {
        let invalid = |message: String| Error::InvalidVocabulary {
            path: doc.path.clone(),
            message,
        };
        let fm = &doc.frontmatter;
        if let Some(e) = fm.error() {
            return Err(invalid(e.to_owned()));
        }
        let terms = match fm.get("terms") {
            None | Some(Value::Null) => return Ok(Vocabulary::default()),
            Some(Value::Object(map)) => map,
            Some(_) => {
                return Err(invalid(
                    "`terms` must be a mapping of tag to definition".into(),
                ));
            }
        };
        let mut vocab = Vocabulary::default();
        for (tag, def) in terms {
            let term = parse_term(tag, def).map_err(|m| invalid(format!("term `{tag}`: {m}")))?;
            let idx = vocab.terms.len();
            for name in std::iter::once(&term.tag).chain(&term.synonyms) {
                let key = normalize_tag(name);
                match vocab.lookup.get(&key) {
                    Some(&other) if other != idx => {
                        return Err(invalid(format!(
                            "`{name}` is used by both `{}` and `{tag}`",
                            vocab.terms[other].tag
                        )));
                    }
                    _ => {
                        vocab.lookup.insert(key, idx);
                    }
                }
            }
            vocab.terms.push(term);
        }
        Ok(vocab)
    }

    /// All terms, in vocabulary order.
    pub fn terms(&self) -> &[Term] {
        &self.terms
    }

    /// Whether the vocabulary has no terms.
    pub fn is_empty(&self) -> bool {
        self.terms.is_empty()
    }

    /// Resolves a tag or synonym to its term.
    pub fn resolve(&self, tag: &str) -> Option<&Term> {
        self.lookup
            .get(&normalize_tag(tag))
            .map(|&i| &self.terms[i])
    }

    /// The canonical form of a tag or synonym.
    pub fn canonical(&self, tag: &str) -> Option<&str> {
        self.resolve(tag).map(|t| t.tag.as_str())
    }

    /// Whether `tag` is written exactly in canonical form (up to [`normalize_tag`]).
    pub fn is_canonical(&self, tag: &str) -> bool {
        self.canonical(tag)
            .is_some_and(|c| normalize_tag(c) == normalize_tag(tag))
    }

    /// Facet name → canonical tags in that facet. Tags without a facet are omitted.
    pub fn facets(&self) -> BTreeMap<&str, Vec<&str>> {
        let mut out: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for t in &self.terms {
            if let Some(f) = &t.facet {
                out.entry(f).or_default().push(&t.tag);
            }
        }
        out
    }
}

fn parse_term(tag: &str, def: &Value) -> Result<Term, String> {
    let mut term = Term {
        tag: tag.to_owned(),
        synonyms: Vec::new(),
        facet: None,
        description: None,
    };
    if normalize_tag(tag).is_empty() {
        return Err("empty tag".into());
    }
    match def {
        Value::Null => {}
        Value::Array(_) => term.synonyms = strings(def)?,
        Value::Object(map) => {
            for (k, v) in map {
                match k.as_str() {
                    "synonyms" => term.synonyms = strings(v)?,
                    "facet" => term.facet = Some(string(v, "facet")?),
                    "description" => term.description = Some(string(v, "description")?),
                    _ => {} // unknown keys are allowed, as in OKF
                }
            }
        }
        _ => return Err("expected a list of synonyms or a mapping".into()),
    }
    Ok(term)
}

fn string(v: &Value, what: &str) -> Result<String, String> {
    v.as_str()
        .map(str::to_owned)
        .ok_or_else(|| format!("`{what}` must be a string"))
}

fn strings(v: &Value) -> Result<Vec<String>, String> {
    match v {
        Value::Array(items) => items.iter().map(|i| string(i, "synonyms")).collect(),
        Value::String(s) => Ok(vec![s.clone()]),
        _ => Err("`synonyms` must be a list of strings".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vocab(yaml: &str) -> Result<Vocabulary, Error> {
        let doc = Concept::parse(
            Path::new("_meta/vocabulary.md"),
            &format!("---\ntype: Vocabulary\n{yaml}---\n"),
        )
        .unwrap();
        Vocabulary::from_concept(&doc)
    }

    #[test]
    fn normalizes() {
        assert_eq!(normalize_tag("  Human_Resources "), "human-resources");
        assert_eq!(normalize_tag("hoàn  tiền"), "hoàn-tiền");
        assert_eq!(normalize_tag("--a--b--"), "a-b");
        assert_eq!(normalize_tag("ＡＢＣ返金"), "ａｂｃ返金");
    }

    #[test]
    fn parses_and_resolves() {
        let v = vocab(
            "terms:\n  refund:\n    synonyms: [hoàn tiền, 返金]\n    facet: topic\n  shipping: [giao hàng]\n  hr: {}\n  it:\n    facet: department\n",
        )
        .unwrap();
        assert_eq!(v.terms().len(), 4);
        assert_eq!(v.canonical("Hoàn Tiền"), Some("refund"));
        assert_eq!(v.canonical("返金"), Some("refund"));
        assert_eq!(v.canonical("giao_hàng"), Some("shipping"));
        assert!(v.is_canonical("Refund"));
        assert!(!v.is_canonical("返金"));
        assert_eq!(v.canonical("unknown"), None);
        assert_eq!(
            v.facets(),
            BTreeMap::from([("department", vec!["it"]), ("topic", vec!["refund"])])
        );
    }

    #[test]
    fn rejects_conflicts_and_bad_shapes() {
        assert!(vocab("terms:\n  a: [x]\n  b: [X]\n").is_err());
        assert!(vocab("terms: [a, b]\n").is_err());
        assert!(vocab("terms:\n  a: 3\n").is_err());
        assert!(vocab("title: none\n").unwrap().is_empty());
    }
}
