//! `catalog`: a prompt-ready overview of the bundle, meant for the system prompt.
//!
//! Small bundles get a flat list of every document (the okf-scale spike's format);
//! larger ones get the root `index.md`. Both end with the tag vocabulary and
//! facet counts. There are no per-tag document lists (spike S5).

use std::collections::BTreeMap;
use std::fmt::Write as _;

use okbase_standard::{VOCABULARY_PATH, Vocabulary};
use serde::{Deserialize, Serialize};

use crate::{Error, Scope};

/// Default token budget for a flat catalog. The spike used a flat catalog at
/// about 9k tokens and switched to the root index at about 44k.
pub const DEFAULT_CATALOG_TOKENS: usize = 12_000;

/// Catalog options.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CatalogOptions {
    /// Largest flat catalog, in estimated tokens.
    pub max_tokens: usize,
}

impl Default for CatalogOptions {
    fn default() -> Self {
        CatalogOptions {
            max_tokens: DEFAULT_CATALOG_TOKENS,
        }
    }
}

/// A vocabulary term with its usage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CatalogTerm {
    /// Canonical tag.
    pub tag: String,
    /// Synonyms.
    pub synonyms: Vec<String>,
    /// Facet.
    pub facet: Option<String>,
    /// Visible documents with this tag (or a synonym).
    pub count: usize,
}

/// The catalog.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CatalogResult {
    /// `flat` (every document) or `root_index` (the root listing).
    pub mode: &'static str,
    /// Visible concept documents.
    pub docs: usize,
    /// The catalog text, ready for a system prompt.
    pub content: String,
    /// Estimated tokens of `content`.
    pub tokens: usize,
    /// Vocabulary terms (empty without `_meta/vocabulary.md`).
    pub vocabulary: Vec<CatalogTerm>,
    /// Facet → (value, count): `type`, `lang`, `status`, and vocabulary facets.
    pub facets: BTreeMap<String, Vec<(String, usize)>>,
}

pub(crate) fn catalog(
    index: &okbase_index::Index,
    opts: &CatalogOptions,
    scope: &Scope,
) -> Result<CatalogResult, Error> {
    let all = crate::docs::load(
        index.connection(),
        scope,
        crate::docs::Load {
            reserved: true,
            tags: true,
            prefilter: None,
        },
    )?;
    let docs: Vec<_> = all.iter().filter(|d| !d.reserved).collect();

    let mut flat = String::from("# Knowledge catalog (all documents)\n\n");
    for d in &docs {
        match &d.description {
            Some(desc) => {
                let _ = writeln!(flat, "- [{}] {} — {desc}", d.id, d.title);
            }
            None => {
                let _ = writeln!(flat, "- [{}] {}", d.id, d.title);
            }
        }
    }
    let (mode, mut content) = if okbase_index::estimate_tokens(&flat) <= opts.max_tokens {
        ("flat", flat)
    } else {
        let root = scope
            .is_unrestricted()
            .then(|| {
                index
                    .connection()
                    .query_row("SELECT body FROM docs WHERE id = 'index'", [], |r| {
                        r.get::<_, String>(0)
                    })
                    .ok()
            })
            .flatten()
            .unwrap_or_else(|| crate::list::generate(&all, ""));
        (
            "root_index",
            format!("Knowledge bundle root index (use kb_list to browse directories):\n{root}"),
        )
    };

    let vocab = Vocabulary::load(index.root(), VOCABULARY_PATH)
        .ok()
        .flatten()
        .unwrap_or_default();
    let mut vocabulary = Vec::new();
    let mut facets: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();
    for d in &docs {
        for (name, v) in [
            ("type", &d.concept_type),
            ("lang", &d.lang),
            ("status", &d.status),
        ] {
            if let Some(v) = v {
                *facets
                    .entry(name.into())
                    .or_default()
                    .entry(v.clone())
                    .or_default() += 1;
            }
        }
    }
    for term in vocab.terms() {
        let key = okbase_analyze::fold(&okbase_standard::normalize_tag(&term.tag));
        let count = docs.iter().filter(|d| d.tag_keys().contains(&key)).count();
        if let Some(f) = &term.facet
            && count > 0
        {
            facets
                .entry(f.clone())
                .or_default()
                .insert(term.tag.clone(), count);
        }
        vocabulary.push(CatalogTerm {
            tag: term.tag.clone(),
            synonyms: term.synonyms.clone(),
            facet: term.facet.clone(),
            count,
        });
    }
    let facets: BTreeMap<String, Vec<(String, usize)>> = facets
        .into_iter()
        .map(|(k, m)| {
            let mut v: Vec<_> = m.into_iter().collect();
            v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
            (k, v)
        })
        .collect();

    if !vocabulary.is_empty() {
        content.push_str("\n## Tag vocabulary (use these tags in kb_query)\n\n");
        for t in &vocabulary {
            let syn = if t.synonyms.is_empty() {
                String::new()
            } else {
                format!(" (also: {})", t.synonyms.join(", "))
            };
            let facet = t
                .facet
                .as_ref()
                .map(|f| format!(" [{f}]"))
                .unwrap_or_default();
            let _ = writeln!(content, "- {}{facet}{syn}: {} documents", t.tag, t.count);
        }
    }
    if !facets.is_empty() {
        content.push_str("\n## Facets (document counts)\n\n");
        for (k, v) in &facets {
            let _ = writeln!(
                content,
                "- {k}: {}",
                v.iter()
                    .map(|(x, n)| format!("{x}={n}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
    }
    Ok(CatalogResult {
        mode,
        docs: docs.len(),
        tokens: okbase_index::estimate_tokens(&content),
        content,
        vocabulary,
        facets,
    })
}
