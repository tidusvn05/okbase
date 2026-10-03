//! Tag vocabulary: usage of the current vocabulary, and suggestions built from the
//! tags documents already use.
//!
//! Suggestions group spelling variants of the same tag (case, accents, `_`/`-`,
//! English plurals). Translations (`refund` / `hoàn tiền`) need a person or an
//! agent: add them as synonyms by hand.

use std::collections::BTreeMap;
use std::path::Path;

use okbase_analyze::Analyzer;
use okbase_core::{Concept, Frontmatter, discover};
pub use okbase_standard::VOCABULARY_PATH;
use okbase_standard::{Vocabulary, meta, normalize_tag};
use serde::Serialize;
use serde_json::json;

use crate::Error;

/// A tag as used in the bundle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TagUse {
    /// Suggested canonical spelling (the most used variant).
    pub tag: String,
    /// Other spellings found.
    pub variants: Vec<String>,
    /// Documents using the tag (any variant).
    pub documents: usize,
    /// The vocabulary term it maps to, if any.
    pub canonical: Option<String>,
}

/// Tag usage of a bundle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VocabReport {
    /// Whether `_meta/vocabulary.md` exists.
    pub has_vocabulary: bool,
    /// Tags, most used first.
    pub tags: Vec<TagUse>,
}

/// Collects every tag (`tags`, or `categories` when mapped) and groups variants.
pub fn report(root: &Path) -> Result<VocabReport, Error> {
    let vocab = Vocabulary::load(root, VOCABULARY_PATH).ok().flatten();
    let analyzer = Analyzer::new();
    // key → (spelling → count)
    let mut groups: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();
    for rel in discover(root)? {
        let Ok(doc) = Concept::read(root, &rel) else {
            continue;
        };
        if doc.is_reserved() || doc.path.starts_with("_meta/") {
            continue;
        }
        let Some(tags) = meta(&doc.frontmatter).tags else {
            continue;
        };
        let mut seen = std::collections::HashSet::new();
        for t in tags.value {
            let key = analyzer.terms(&normalize_tag(&t)).join("-");
            if key.is_empty() || !seen.insert(key.clone()) {
                continue;
            }
            *groups
                .entry(key)
                .or_default()
                .entry(t.trim().to_owned())
                .or_default() += 1;
        }
    }
    let mut tags: Vec<TagUse> = groups
        .into_values()
        .map(|spellings| {
            let mut v: Vec<(String, usize)> = spellings.into_iter().collect();
            // Most used first; on a tie prefer spellings without `_`, then shorter, then lowercase.
            let rank = |s: &str| {
                (
                    s.contains('_'),
                    s.chars().count(),
                    s.chars().any(char::is_uppercase),
                    s.to_owned(),
                )
            };
            v.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| rank(&a.0).cmp(&rank(&b.0))));
            let documents = v.iter().map(|x| x.1).sum();
            let tag = v[0].0.clone();
            let canonical = vocab.as_ref().and_then(|voc| {
                v.iter()
                    .find_map(|(s, _)| voc.canonical(s).map(str::to_owned))
            });
            TagUse {
                tag,
                variants: v.into_iter().skip(1).map(|x| x.0).collect(),
                documents,
                canonical,
            }
        })
        .collect();
    tags.sort_by(|a, b| b.documents.cmp(&a.documents).then(a.tag.cmp(&b.tag)));
    Ok(VocabReport {
        has_vocabulary: vocab.is_some(),
        tags,
    })
}

/// A `_meta/vocabulary.md` proposing one term per tag group (variants as synonyms).
pub fn suggest(report: &VocabReport, actor: &str, today: &str) -> Result<String, Error> {
    let (mut fm, _) = Frontmatter::split("");
    let mut terms = serde_json::Map::new();
    for t in &report.tags {
        let def = if t.variants.is_empty() {
            json!({})
        } else {
            json!({ "synonyms": t.variants })
        };
        terms.insert(t.tag.clone(), def);
    }
    let set =
        |fm: &mut Frontmatter, k: &str, v: serde_json::Value| fm.set(k, v).map_err(Error::Core);
    set(&mut fm, "type", json!("Vocabulary"))?;
    set(&mut fm, "title", json!("Tag vocabulary"))?;
    set(
        &mut fm,
        "description",
        json!(
            "Canonical tags of this bundle, with spelling variants and translations as synonyms."
        ),
    )?;
    set(
        &mut fm,
        "generated",
        json!({"by": actor, "at": today, "fields": ["terms"]}),
    )?;
    set(&mut fm, "terms", serde_json::Value::Object(terms))?;
    Ok(format!(
        "{}\n# Tag vocabulary\n\nSuggested by `okbase vocab --suggest` from the tags in use ({} tags). Review it: merge tags that mean \
         the same, add translations as `synonyms`, and set a `facet` (such as `topic` or `department`) where it helps filtering.\n",
        fm.render(),
        report.tags.len()
    ))
}

impl VocabReport {
    /// One line per tag: `tag (N documents) [variants] -> canonical`.
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        if !self.has_vocabulary {
            out.push_str(&format!(
                "no {VOCABULARY_PATH}; `okbase vocab --suggest` proposes one\n"
            ));
        }
        for t in &self.tags {
            let variants = if t.variants.is_empty() {
                String::new()
            } else {
                format!(" (also: {})", t.variants.join(", "))
            };
            let mapped = match (&t.canonical, self.has_vocabulary) {
                (Some(c), _) if *c != t.tag => format!(" -> {c}"),
                (Some(_), _) => String::new(),
                (None, true) => " [not in vocabulary]".into(),
                (None, false) => String::new(),
            };
            out.push_str(&format!("{} {}{variants}{mapped}\n", t.documents, t.tag));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_variants_and_suggests_a_valid_vocabulary() {
        let tmp = tempfile::tempdir().unwrap();
        let w = |p: &str, tags: &str| {
            let f = tmp.path().join(p);
            std::fs::create_dir_all(f.parent().unwrap()).unwrap();
            std::fs::write(f, format!("---\ntype: T\ntags: {tags}\n---\nx\n")).unwrap();
        };
        w("a.md", "[Refunds, shipping]");
        w("b.md", "[refund, Hoàn tiền]");
        w("c.md", "[refund, hoan_tien]");
        w("d.md", "[shipping]");
        let r = report(tmp.path()).unwrap();
        let got: Vec<_> = r
            .tags
            .iter()
            .map(|t| (t.tag.as_str(), t.documents, t.variants.clone()))
            .collect();
        assert_eq!(
            got,
            [
                ("refund", 3, vec!["Refunds".to_owned()]),
                ("Hoàn tiền", 2, vec!["hoan_tien".to_owned()]),
                ("shipping", 2, vec![]),
            ]
        );
        let text = suggest(&r, "okbase-vocab/test", "2026-10-01").unwrap();
        let doc = Concept::parse(Path::new(VOCABULARY_PATH), &text).unwrap();
        let v = Vocabulary::from_concept(&doc).unwrap();
        assert_eq!(v.canonical("Refunds"), Some("refund"));
        assert_eq!(v.canonical("hoan tien"), Some("Hoàn tiền"));
        assert!(okbase_core::validate(&doc).is_empty());
    }
}
