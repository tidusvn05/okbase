//! `graph`: the link graph of the visible documents (nodes and edges).

use std::collections::{BTreeMap, HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::query::Filter;
use crate::{Error, Scope};

/// Largest graph returned when no limit is given.
pub const DEFAULT_GRAPH_NODES: usize = 2000;

/// What to include in the graph.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GraphRequest {
    /// Keep only documents matching this filter (edges need both ends kept).
    pub filter: Option<Filter>,
    /// Maximum nodes, keeping the most linked ones [default: 2000].
    pub limit: Option<usize>,
}

/// A document in the graph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GraphNode {
    /// Document id.
    pub id: String,
    /// Title.
    pub title: String,
    /// `type` field.
    #[serde(rename = "type")]
    pub concept_type: Option<String>,
    /// `status` field.
    pub status: Option<String>,
    /// Tags as written.
    pub tags: Vec<String>,
    /// Edges leaving this node in the returned graph.
    pub out_degree: usize,
    /// Edges arriving at this node in the returned graph.
    pub in_degree: usize,
    /// Links from this document to documents or wikilinks that do not exist.
    pub broken: usize,
}

/// Links from one document to another, merged.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GraphEdge {
    /// Source document.
    pub src: String,
    /// Target document.
    pub target: String,
    /// Number of links from `src` to `target`.
    pub count: usize,
}

/// The link graph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GraphResult {
    /// Documents, sorted by id.
    pub nodes: Vec<GraphNode>,
    /// Edges between returned documents, sorted by source then target.
    pub edges: Vec<GraphEdge>,
    /// Documents left out by the limit.
    pub truncated: usize,
}

impl GraphResult {
    /// Compact text form: one line per node with its outgoing edges.
    pub fn to_text(&self) -> String {
        let mut out: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for e in &self.edges {
            out.entry(&e.src).or_default().push(&e.target);
        }
        let mut s = format!(
            "{} documents, {} edges{}\n",
            self.nodes.len(),
            self.edges.len(),
            if self.truncated > 0 {
                format!(" ({} more documents left out)", self.truncated)
            } else {
                String::new()
            }
        );
        for n in &self.nodes {
            s.push_str(&n.id);
            if let Some(targets) = out.get(n.id.as_str()) {
                s.push_str(" -> ");
                s.push_str(&targets.join(", "));
            }
            if n.broken > 0 {
                s.push_str(&format!(" ({} broken)", n.broken));
            }
            s.push('\n');
        }
        s
    }
}

pub(crate) fn graph(
    index: &okbase_index::Index,
    req: &GraphRequest,
    scope: &Scope,
) -> Result<GraphResult, Error> {
    let conn = index.connection();
    let mut docs = crate::docs::load(
        conn,
        scope,
        crate::docs::Load {
            reserved: false,
            tags: true,
            prefilter: req.filter.as_ref(),
        },
    )?;
    if let Some(f) = &req.filter {
        docs = crate::query::filter_docs(docs, f)?;
    }
    let kept: HashSet<&str> = docs.iter().map(|d| d.id.as_str()).collect();
    let all: HashSet<String> = {
        let mut st = conn.prepare_cached("SELECT id FROM docs")?;
        st.query_map([], |r| r.get(0))?.collect::<Result<_, _>>()?
    };

    let mut edges: BTreeMap<(String, String), usize> = BTreeMap::new();
    let mut broken: HashMap<String, usize> = HashMap::new();
    let mut st = conn.prepare_cached("SELECT src, target, kind FROM links")?;
    let rows = st.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, Option<String>>(1)?,
            r.get::<_, String>(2)?,
        ))
    })?;
    for row in rows {
        let (src, target, kind) = row?;
        if !kept.contains(src.as_str()) {
            continue;
        }
        match target {
            // Unresolved wikilink; a markdown link without a target is an external URL.
            None if kind == "wiki" => *broken.entry(src).or_default() += 1,
            None => {}
            Some(t) if t == src => {}
            Some(t) if kept.contains(t.as_str()) => *edges.entry((src, t)).or_default() += 1,
            // A missing file inside the scope is broken; hidden or filtered targets are not revealed.
            Some(t) if !all.contains(&t) && scope.permits_path(&t) => {
                *broken.entry(src).or_default() += 1;
            }
            Some(_) => {}
        }
    }

    let limit = req.limit.unwrap_or(DEFAULT_GRAPH_NODES);
    let mut truncated = 0;
    if docs.len() > limit {
        let mut degree: HashMap<&str, usize> = HashMap::new();
        for (s, t) in edges.keys() {
            *degree.entry(s).or_default() += 1;
            *degree.entry(t).or_default() += 1;
        }
        let mut order: Vec<(usize, &str)> = docs
            .iter()
            .map(|d| {
                (
                    degree.get(d.id.as_str()).copied().unwrap_or(0),
                    d.id.as_str(),
                )
            })
            .collect();
        order.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(b.1)));
        let keep: HashSet<String> = order[..limit]
            .iter()
            .map(|(_, id)| (*id).to_owned())
            .collect();
        truncated = docs.len() - limit;
        docs.retain(|d| keep.contains(&d.id));
        edges.retain(|(s, t), _| keep.contains(s) && keep.contains(t));
    }

    let mut out_degree: HashMap<&str, usize> = HashMap::new();
    let mut in_degree: HashMap<&str, usize> = HashMap::new();
    for (s, t) in edges.keys() {
        *out_degree.entry(s).or_default() += 1;
        *in_degree.entry(t).or_default() += 1;
    }
    let nodes = docs
        .iter()
        .map(|d| GraphNode {
            id: d.id.clone(),
            title: d.title.clone(),
            concept_type: d.concept_type.clone(),
            status: d.status.clone(),
            tags: d.tags().to_vec(),
            out_degree: out_degree.get(d.id.as_str()).copied().unwrap_or(0),
            in_degree: in_degree.get(d.id.as_str()).copied().unwrap_or(0),
            broken: broken.get(&d.id).copied().unwrap_or(0),
        })
        .collect();
    let edges = edges
        .into_iter()
        .map(|((src, target), count)| GraphEdge { src, target, count })
        .collect();
    Ok(GraphResult {
        nodes,
        edges,
        truncated,
    })
}
