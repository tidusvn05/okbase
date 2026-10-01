//! `query`: filter documents by metadata, with counts, facets and sums
//! (semantics from the biz-meta spike's `kb_query`).

use std::collections::BTreeMap;
use std::fmt::Write as _;

use okfkit_analyze::fold;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::docs::DocMeta;
use crate::scope::{PathGlob, value_matches};
use crate::{Error, Scope};

/// Frontmatter keys shown in dedicated columns rather than in `extra`.
const STANDARD_KEYS: [&str; 16] = [
    "type",
    "title",
    "description",
    "tags",
    "status",
    "lang",
    "updated",
    "aliases",
    "supersedes",
    "sources",
    "generated",
    "verified",
    "resource",
    "summary",
    "excerpt",
    "categories",
];

/// Metadata filters, combined with AND. Every list matches any of its values, case-insensitively.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Filter {
    /// Document types.
    #[serde(rename = "type")]
    pub types: Vec<String>,
    /// Documents must have all of these tags (synonyms and accents are resolved through the vocabulary).
    pub tags_all: Vec<String>,
    /// Documents must have at least one of these tags.
    pub tags_any: Vec<String>,
    /// Allowed `status` values.
    pub status: Vec<String>,
    /// Excluded `status` values.
    pub status_not: Vec<String>,
    /// Allowed `lang` values.
    pub lang: Vec<String>,
    /// Any other frontmatter field → allowed values (for example `department: [hr]`).
    pub fields: BTreeMap<String, Vec<String>>,
    /// ID prefix or glob (`policies/jp/`, `policies/**/refund*`).
    pub path: Option<String>,
    /// Case- and accent-insensitive substring of the ID, title or description.
    pub text: Option<String>,
    /// `updated` ≥ this date (YYYY-MM-DD).
    pub updated_from: Option<String>,
    /// `updated` ≤ this date.
    pub updated_to: Option<String>,
    /// Field → inclusive range, compared as numbers when both sides are numbers, else as text (ISO dates sort correctly).
    pub ranges: BTreeMap<String, Range>,
    /// Keep documents in force on this date: `status` is `stable` (or absent) and
    /// `effective_from` ≤ date ≤ `effective_to` where those fields exist.
    pub active_on: Option<String>,
}

impl Filter {
    /// Whether the filter needs tags (tag filters require loading them).
    pub fn uses_tags(&self) -> bool {
        !self.tags_all.is_empty() || !self.tags_any.is_empty()
    }
}

/// An inclusive range; either end may be open.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Range {
    /// Lower bound.
    pub from: Option<String>,
    /// Upper bound.
    pub to: Option<String>,
}

/// A query request: a filter plus what to return.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct QueryRequest {
    /// Filters.
    #[serde(flatten)]
    pub filter: Filter,
    /// Sort field, `-` prefix for descending (`-updated`, `effective_to`). Default `id`.
    pub sort: Option<String>,
    /// Maximum rows (default 50).
    pub limit: Option<usize>,
    /// Return only counts, facets and sums.
    pub count_only: bool,
    /// Fields to count over the filtered set (`type`, `tags`, `status`, any field).
    pub facets: Vec<String>,
    /// Numeric field to sum over the filtered set.
    pub sum_field: Option<String>,
}

/// Default number of rows returned by `query`.
pub const DEFAULT_QUERY_LIMIT: usize = 50;

/// A matching document.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct QueryRow {
    /// Concept ID.
    pub id: String,
    /// Title.
    pub title: String,
    /// `type`.
    #[serde(rename = "type")]
    pub concept_type: Option<String>,
    /// `status`.
    pub status: Option<String>,
    /// `lang`.
    pub lang: Option<String>,
    /// `updated`.
    pub updated: Option<String>,
    /// Tags as written.
    pub tags: Vec<String>,
    /// Other scalar frontmatter fields.
    pub extra: BTreeMap<String, Value>,
}

/// A sum over the filtered set.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Sum {
    /// The summed field.
    pub field: String,
    /// The total.
    pub value: f64,
    /// Documents that had a numeric value.
    pub count: usize,
    /// Distribution of `currency` (or `unit`) among the summed documents, so mixed units are visible.
    pub units: BTreeMap<String, usize>,
}

/// The result of a query.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct QueryResult {
    /// Matching documents.
    pub total: usize,
    /// Rows (empty with `count_only`).
    pub docs: Vec<QueryRow>,
    /// Rows not shown because of `limit`.
    pub more: usize,
    /// Facet field → (value, count), most frequent first.
    pub facets: BTreeMap<String, Vec<(String, usize)>>,
    /// Sum, if `sum_field` was given.
    pub sum: Option<Sum>,
}

pub(crate) fn query(
    index: &okfkit_index::Index,
    req: &QueryRequest,
    scope: &Scope,
) -> Result<QueryResult, Error> {
    let tags = req.filter.uses_tags() || req.facets.iter().any(|f| f == "tags");
    let docs = crate::docs::load(
        index.connection(),
        scope,
        crate::docs::Load {
            reserved: false,
            tags,
            prefilter: Some(&req.filter),
        },
    )?;
    let mut docs = filter_docs(docs, &req.filter)?;
    let total = docs.len();

    let sum = req.sum_field.as_ref().map(|field| {
        let mut s = Sum {
            field: field.clone(),
            value: 0.0,
            count: 0,
            units: BTreeMap::new(),
        };
        for d in &docs {
            if let Some(v) = field_value(d, field).and_then(as_number) {
                s.value += v;
                s.count += 1;
                if let Some(u) = d
                    .frontmatter()
                    .get("currency")
                    .or_else(|| d.frontmatter().get("unit"))
                    .and_then(scalar)
                {
                    *s.units.entry(u).or_default() += 1;
                }
            }
        }
        s
    });

    let mut facets = BTreeMap::new();
    for f in &req.facets {
        let mut counts: BTreeMap<String, usize> = BTreeMap::new();
        for d in &docs {
            let values: Vec<String> = match f.as_str() {
                "tags" => d.tags().to_vec(),
                _ => match field_value(d, f) {
                    Some(Value::Array(items)) => items.iter().filter_map(scalar).collect(),
                    Some(v) => scalar(&v).into_iter().collect(),
                    None => Vec::new(),
                },
            };
            for v in values {
                *counts.entry(v).or_default() += 1;
            }
        }
        let mut v: Vec<(String, usize)> = counts.into_iter().collect();
        v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        facets.insert(f.clone(), v);
    }

    let sort = req.sort.as_deref().unwrap_or("id");
    let (desc, key) = match sort.strip_prefix('-') {
        Some(k) => (true, k),
        None => (false, sort),
    };
    docs.sort_by(|a, b| {
        let (x, y) = (sort_key(a, key), sort_key(b, key));
        let o = compare_values(&x, &y).then_with(|| a.id.cmp(&b.id));
        if desc { o.reverse() } else { o }
    });
    let limit = req.limit.unwrap_or(DEFAULT_QUERY_LIMIT);
    let rows: Vec<QueryRow> = if req.count_only {
        Vec::new()
    } else {
        docs.truncate(limit);
        crate::docs::load_tags(index.connection(), &mut docs)?;
        docs.iter().map(row).collect()
    };
    let more = if req.count_only {
        0
    } else {
        total.saturating_sub(rows.len())
    };
    Ok(QueryResult {
        total,
        docs: rows,
        more,
        facets,
        sum,
    })
}

/// Applies a filter.
pub(crate) fn filter_docs(docs: Vec<DocMeta>, f: &Filter) -> Result<Vec<DocMeta>, Error> {
    let path = f
        .path
        .as_deref()
        .filter(|p| !p.trim().is_empty())
        .map(PathGlob::new)
        .transpose()?;
    let text = f
        .text
        .as_deref()
        .map(|t| fold(t.trim()))
        .filter(|t| !t.is_empty());
    let tag_key = |t: &String| fold(&okfkit_standard::normalize_tag(t));
    let tags_all: Vec<String> = f.tags_all.iter().map(tag_key).collect();
    let tags_any: Vec<String> = f.tags_any.iter().map(tag_key).collect();
    let in_list = |v: &Option<String>, list: &[String]| {
        list.is_empty()
            || v.as_deref()
                .is_some_and(|v| list.iter().any(|x| x.eq_ignore_ascii_case(v)))
    };
    Ok(docs
        .into_iter()
        .filter(|d| {
            in_list(&d.concept_type, &f.types)
                && in_list(&d.status, &f.status)
                && !(!f.status_not.is_empty() && in_list(&d.status, &f.status_not))
                && in_list(&d.lang, &f.lang)
                && path.as_ref().is_none_or(|g| g.matches(&d.id))
                && tags_all.iter().all(|t| d.tag_keys().contains(t))
                && (tags_any.is_empty() || tags_any.iter().any(|t| d.tag_keys().contains(t)))
                && f.fields
                    .iter()
                    .all(|(k, vals)| field_value(d, k).is_some_and(|v| value_matches(&v, vals)))
                && text.as_ref().is_none_or(|t| {
                    fold(&format!(
                        "{} {} {}",
                        d.id,
                        d.title,
                        d.description.as_deref().unwrap_or("")
                    ))
                    .contains(t.as_str())
                })
                && in_range(
                    d.updated.as_deref(),
                    f.updated_from.as_deref(),
                    f.updated_to.as_deref(),
                )
                && f.ranges.iter().all(|(k, r)| {
                    field_value(d, k)
                        .and_then(|v| scalar(&v))
                        .is_some_and(|v| in_range(Some(&v), r.from.as_deref(), r.to.as_deref()))
                })
                && f.active_on.as_deref().is_none_or(|day| active_on(d, day))
        })
        .collect())
}

fn active_on(d: &DocMeta, day: &str) -> bool {
    let date = |k: &str| d.frontmatter().get(k).and_then(scalar);
    d.status
        .as_deref()
        .is_none_or(|s| s.eq_ignore_ascii_case("stable"))
        && date("effective_from").is_none_or(|f| f.as_str() <= day)
        && date("effective_to").is_none_or(|t| t.as_str() >= day)
}

fn in_range(v: Option<&str>, from: Option<&str>, to: Option<&str>) -> bool {
    if from.is_none() && to.is_none() {
        return true;
    }
    let Some(v) = v else { return false };
    let cmp = |a: &str, b: &str| match (a.parse::<f64>(), b.parse::<f64>()) {
        (Ok(x), Ok(y)) => x.partial_cmp(&y).unwrap_or(std::cmp::Ordering::Equal),
        _ => a.cmp(b),
    };
    from.is_none_or(|f| cmp(v, f).is_ge()) && to.is_none_or(|t| cmp(v, t).is_le())
}

/// A field by name; `type`, `title`, `description`, `id` come from the index columns.
fn field_value(d: &DocMeta, key: &str) -> Option<Value> {
    match key {
        "id" => Some(Value::String(d.id.clone())),
        "title" => Some(Value::String(d.title.clone())),
        "type" => d.concept_type.clone().map(Value::String),
        "description" => d.description.clone().map(Value::String),
        _ => d.frontmatter().get(key).cloned(),
    }
}

fn sort_key(d: &DocMeta, key: &str) -> Option<Value> {
    field_value(d, key)
}

fn compare_values(a: &Option<Value>, b: &Option<Value>) -> std::cmp::Ordering {
    use std::cmp::Ordering::*;
    match (a, b) {
        (None, None) => Equal,
        (None, _) => Greater, // missing values last
        (_, None) => Less,
        (Some(x), Some(y)) => match (as_number(x.clone()), as_number(y.clone())) {
            (Some(p), Some(q)) => p.partial_cmp(&q).unwrap_or(Equal),
            _ => scalar(x)
                .unwrap_or_default()
                .cmp(&scalar(y).unwrap_or_default()),
        },
    }
}

fn as_number(v: Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        _ => None,
    }
}

pub(crate) fn scalar(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

fn row(d: &DocMeta) -> QueryRow {
    let extra = d
        .frontmatter()
        .iter()
        .filter(|(k, v)| !STANDARD_KEYS.contains(&k.as_str()) && scalar(v).is_some())
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    QueryRow {
        id: d.id.clone(),
        title: d.title.clone(),
        concept_type: d.concept_type.clone(),
        status: d.status.clone(),
        lang: d.lang.clone(),
        updated: d.updated.clone(),
        tags: d.tags().to_vec(),
        extra,
    }
}

impl QueryResult {
    /// The compact text form used by the MCP tool (as in the spike's `kb_query`).
    pub fn to_text(&self) -> String {
        let mut out = format!("total: {} documents\n", self.total);
        if let Some(s) = &self.sum {
            let units = if s.units.is_empty() {
                String::new()
            } else {
                format!(
                    " (units: {})",
                    s.units
                        .iter()
                        .map(|(k, n)| format!("{k}={n}"))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            };
            let _ = writeln!(
                out,
                "sum({}) = {} over {} docs{units}",
                s.field,
                fmt_num(s.value),
                s.count
            );
        }
        for (f, counts) in &self.facets {
            let _ = writeln!(
                out,
                "facet {f}: {}",
                counts
                    .iter()
                    .map(|(k, n)| format!("{k}={n}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
        if !self.docs.is_empty() {
            out.push_str("| id | title | type | status | lang | updated | tags | extra |\n");
            let o = |v: &Option<String>| v.clone().unwrap_or_default();
            for d in &self.docs {
                let extra = d
                    .extra
                    .iter()
                    .map(|(k, v)| {
                        format!(
                            "{k}={}",
                            scalar(v)
                                .unwrap_or_default()
                                .chars()
                                .take(40)
                                .collect::<String>()
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(" ");
                let _ = writeln!(
                    out,
                    "| {} | {} | {} | {} | {} | {} | {} | {} |",
                    d.id,
                    d.title,
                    o(&d.concept_type),
                    o(&d.status),
                    o(&d.lang),
                    o(&d.updated),
                    d.tags.join(", "),
                    extra
                );
            }
        }
        if self.more > 0 {
            let _ = writeln!(
                out,
                "... {} more (raise limit or narrow filters)",
                self.more
            );
        }
        out.trim_end().to_owned()
    }
}

fn fmt_num(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        v.to_string()
    }
}
