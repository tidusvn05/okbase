//! Tool definitions. Descriptions are prompts for agents: they come from the
//! spikes (okf-scale G2, biz-meta) and are snapshot-tested. Change them only with eval data.

use serde_json::{Value, json};

use okfkit::{Bundle, Scope, capability};

/// A tool the server can expose.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolDef {
    /// Name without prefix (`grep` → `kb_grep`).
    pub name: &'static str,
    /// The capability it needs.
    pub capability: &'static str,
    /// Description shown to the agent.
    pub description: String,
    /// JSON Schema of the arguments.
    pub input_schema: Value,
}

/// Facts about the bundle that make descriptions concrete.
#[derive(Debug, Clone, Default)]
pub struct BundleFacts {
    /// Languages of the documents (`lang` values), most common first.
    pub langs: Vec<String>,
    /// Document types, most common first.
    pub types: Vec<String>,
    /// Frontmatter fields worth filtering or faceting on.
    pub fields: Vec<String>,
    /// Dataset table names (module `data`).
    pub tables: Vec<String>,
}

impl BundleFacts {
    /// Reads the facts from the visible part of a bundle.
    pub fn from_bundle(bundle: &Bundle, scope: &Scope) -> Self {
        let Ok(stats) = bundle.stats(scope) else {
            return BundleFacts::default();
        };
        let ranked = |m: &std::collections::BTreeMap<String, usize>| {
            let mut v: Vec<(&String, &usize)> =
                m.iter().filter(|(k, _)| k.as_str() != "(none)").collect();
            v.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
            v.into_iter().map(|(k, _)| k.clone()).collect::<Vec<_>>()
        };
        let mut langs = ranked(&stats.langs);
        if langs.is_empty() {
            langs = detected_langs(bundle, scope);
        }
        BundleFacts {
            langs,
            types: ranked(&stats.types),
            fields: frequent_fields(bundle, scope),
            tables: bundle
                .data_tables(scope)
                .map(|t| t.tables.into_iter().map(|t| t.name).collect())
                .unwrap_or_default(),
        }
    }
}

/// Without `lang` fields: the languages detected in titles and descriptions (a hint).
fn detected_langs(bundle: &Bundle, scope: &Scope) -> Vec<String> {
    let Ok(cat) = bundle.catalog(
        &okfkit::CatalogOptions {
            max_tokens: usize::MAX,
        },
        scope,
    ) else {
        return Vec::new();
    };
    let mut counts: std::collections::BTreeMap<&'static str, usize> =
        std::collections::BTreeMap::new();
    for line in cat
        .content
        .lines()
        .filter(|l| l.starts_with("- ["))
        .take(200)
    {
        let text = line.split_once("] ").map_or(line, |(_, t)| t);
        if let Some(l) = okfkit::analyze::detect_lang(text) {
            *counts.entry(l.code()).or_default() += 1;
        }
    }
    let total: usize = counts.values().sum();
    let mut v: Vec<(&str, usize)> = counts
        .into_iter()
        .filter(|(_, n)| *n * 10 >= total)
        .collect();
    v.sort_by_key(|a| std::cmp::Reverse(a.1));
    v.into_iter().map(|(l, _)| l.to_owned()).collect()
}

/// Custom scalar fields present in at least 10% of documents (standard OKF fields excluded).
fn frequent_fields(bundle: &Bundle, scope: &Scope) -> Vec<String> {
    let Ok(r) = bundle.query(
        &okfkit::QueryRequest {
            limit: Some(usize::MAX),
            ..Default::default()
        },
        scope,
    ) else {
        return Vec::new();
    };
    let mut counts: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
    for d in &r.docs {
        for k in d.extra.keys() {
            *counts.entry(k.as_str()).or_default() += 1;
        }
    }
    let min = (r.total / 10).max(1);
    let mut v: Vec<(&str, usize)> = counts.into_iter().filter(|(_, n)| *n >= min).collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    v.into_iter().take(12).map(|(k, _)| k.to_owned()).collect()
}

fn language_hint(langs: &[String]) -> String {
    let name = |c: &str| match c {
        "en" => "English".to_owned(),
        "vi" => "Vietnamese".to_owned(),
        "ja" => "Japanese".to_owned(),
        other => other.to_owned(),
    };
    match langs {
        [] => "Use the documents' own terms: translate key terms first.".into(),
        [one] => format!("Documents are in {}: translate key terms first.", name(one)),
        many => format!(
            "Documents are in {}: try key terms in each language (alternation).",
            many.iter().map(|l| name(l)).collect::<Vec<_>>().join(", ")
        ),
    }
}

/// All tools, in the order they are listed.
pub fn all_tools(facts: &BundleFacts) -> Vec<ToolDef> {
    let fields_hint = if facts.fields.is_empty() {
        String::new()
    } else {
        format!(" Other fields in this bundle: {}.", facts.fields.join(", "))
    };
    let types_hint = if facts.types.is_empty() {
        String::new()
    } else {
        format!(" Types: {}.", facts.types.join(", "))
    };
    let str_list = json!({"type": "array", "items": {"type": "string"}});
    let filter_props = json!({
        "type": str_list, "tags_all": str_list, "tags_any": str_list,
        "status": str_list, "status_not": str_list, "lang": str_list,
        "fields": {"type": "object", "additionalProperties": str_list,
                   "description": "other frontmatter field -> allowed values, e.g. {\"region\": [\"VN\"]}"},
        "path": {"type": "string", "description": "id prefix or glob, e.g. 'policies/jp/'"},
        "text": {"type": "string", "description": "case- and accent-insensitive substring of id/title/description"},
        "updated_from": {"type": "string"}, "updated_to": {"type": "string"},
        "ranges": {"type": "object", "additionalProperties": {"type": "object", "properties": {"from": {"type": "string"}, "to": {"type": "string"}}},
                   "description": "field -> {from, to}, inclusive; numbers compare numerically, dates as text, e.g. {\"effective_to\": {\"from\": \"2026-01-01\"}}"},
        "active_on": {"type": "string", "description": "YYYY-MM-DD"},
    });
    // grep and search take the same filter as query; repeating its schema made kb_grep's
    // definition twice as long (spike S15: same accuracy without it).
    let filter_ref =
        json!({"type": "object", "description": "metadata filter: the same fields as kb_query"});
    let mut query_props = filter_props;
    let q = query_props.as_object_mut().expect("object");
    q.insert("sort".into(), json!({"type": "string", "description": "field name, prefix '-' for descending, e.g. '-updated', 'effective_to'"}));
    q.insert("limit".into(), json!({"type": "integer", "default": 50}));
    q.insert(
        "count_only".into(),
        json!({"type": "boolean", "default": false}),
    );
    q.insert("facets".into(), json!({"type": "array", "items": {"type": "string"}, "description": "fields to count over the filtered set, e.g. type, tags, status, lang"}));
    q.insert("sum_field".into(), json!({"type": "string", "description": "numeric field to sum over the filtered set, e.g. contract_value"}));

    vec![
        ToolDef {
            name: "search",
            capability: capability::EMBED_SEARCH,
            description: "Semantic search over all document sections (multilingual: the query may be in any language). \
                          Returns `doc_id # heading | score | snippet`. Use kb_grep for exact strings (config keys, codes, error text)."
                .into(),
            input_schema: json!({"type": "object", "properties": {
                "query": {"type": "string"}, "limit": {"type": "integer", "default": 8},
                "filter": filter_ref.clone(),
            }, "required": ["query"]}),
        },
        ToolDef {
            name: "catalog",
            capability: capability::CATALOG,
            description: "Catalog of the knowledge bundle: every document (id, title, one-line description) for small bundles, \
                          or the root index for large ones, plus the tag vocabulary and facet counts. \
                          Call it once at the start unless the catalog is already in your context."
                .into(),
            input_schema: json!({"type": "object", "properties": {}}),
        },
        ToolDef {
            name: "list",
            capability: capability::LIST,
            description: "Show the index.md of a directory in the knowledge bundle (subdirectories and documents with one-line descriptions). Use dir='.' for the root.".into(),
            input_schema: json!({"type": "object", "properties": {"dir": {"type": "string", "default": "."}}}),
        },
        ToolDef {
            name: "grep",
            capability: capability::GREP,
            description: format!(
                "Lexical search over all documents (including frontmatter title/description). `pattern` is a regex, case-insensitive and \
                 accent-insensitive (e.g. 'dmScope|dm_scope', 'preload.*false', 'heartbeat'); use alternation to try synonyms. {} \
                 Options: `path` (glob or prefix, e.g. 'gateway/**', 'channels/'), `context` (lines around each match, 0-5), \
                 `files_only` (list matching documents ranked by match count — good first step), `limit` (max output lines), \
                 `filter` (metadata filter, same fields as kb_query).",
                language_hint(&facts.langs)
            ),
            input_schema: json!({"type": "object", "properties": {
                "pattern": {"type": "string"}, "path": {"type": "string"},
                "context": {"type": "integer", "default": 1}, "files_only": {"type": "boolean", "default": false},
                "limit": {"type": "integer", "default": 40},
                "filter": filter_ref,
            }, "required": ["pattern"]}),
        },
        ToolDef {
            name: "get",
            capability: capability::GET,
            description: "Read a document by id (path without .md, e.g. 'gateway/configuration'). Optional `section`: a heading text to return \
                          only that section. Long documents are truncated — use `section` then. `lines` ('10-40') returns a line range."
                .into(),
            input_schema: json!({"type": "object", "properties": {
                "id": {"type": "string"}, "section": {"type": "string"}, "lines": {"type": "string"},
                "max_tokens": {"type": "integer", "default": 4000},
            }, "required": ["id"]}),
        },
        ToolDef {
            name: "query",
            capability: capability::QUERY,
            description: format!(
                "Filter documents by frontmatter metadata and get counts/facets. All filters are optional and combined with AND. \
                 status: stable=current/in force, deprecated=old/superseded, draft=not yet approved. \
                 `active_on` (YYYY-MM-DD) keeps documents in force on that date: stable (or no status) and effective_from <= date <= effective_to where those fields exist. \
                 Use it to list, count or filter documents, and `sum_field` to add up numbers instead of computing them yourself. \
                 Returns: total count, a table of matching documents (id, title, type, status, lang, updated, tags, other fields), optional facet counts and sum.{types_hint}{fields_hint}"
            ),
            input_schema: json!({"type": "object", "properties": query_props}),
        },
        ToolDef {
            name: "data_tables",
            capability: capability::DATA_SQL,
            description: "List SQL tables (imported spreadsheets: CSV, TSV, XLSX) with columns and row counts.".into(),
            input_schema: json!({"type": "object", "properties": {}}),
        },
        ToolDef {
            name: "data_query",
            capability: capability::DATA_SQL,
            description: format!(
                "Run a read-only SQLite SELECT over the spreadsheet tables{}. Use aggregates (SUM, COUNT, GROUP BY) instead of reading rows. Max 100 rows returned.",
                if facts.tables.is_empty() { String::new() } else { format!(" ({})", facts.tables.join(", ")) }
            ),
            input_schema: json!({"type": "object", "properties": {"sql": {"type": "string"}}, "required": ["sql"]}),
        },
        ToolDef {
            name: "links",
            capability: capability::LINKS,
            description: "Links from a document to other documents, and backlinks to it. Use it to follow related documents.".into(),
            input_schema: json!({"type": "object", "properties": {"id": {"type": "string"}}, "required": ["id"]}),
        },
    ]
}
