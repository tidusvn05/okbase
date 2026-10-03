//! Semantic search and retrieval for okbase bundles (module `embed`).
//!
//! Chunk vectors live in the bundle index (`chunk_vecs`, created on first use and
//! deleted with their chunks). [`embed_sync`] embeds the chunks that have no
//! vector yet, through the shared [`VectorCache`]. [`VectorStore`] loads them
//! into memory for brute-force cosine search (spike S1: fast enough below ~100k
//! chunks). Every query takes a `Scope`; ranking is dense only (spike S2: no
//! BM25 fusion).

use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;

use okbase_embed::cache::{from_bytes, to_bytes};
use okbase_embed::{Embedder, VectorCache, dot, embed_documents_cached};
use okbase_index::Index;
use okbase_query::{Filter, Scope};
use rusqlite::params;
use serde::{Deserialize, Serialize};

/// Errors returned by `okbase-search`.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// The embedder failed.
    #[error(transparent)]
    Embed(#[from] okbase_embed::Error),
    /// Reading the index failed.
    #[error("index database: {0}")]
    Sqlite(#[from] rusqlite::Error),
    /// Applying the scope or filter failed.
    #[error(transparent)]
    Query(#[from] okbase_query::Error),
    /// The bundle has no vectors for this model yet.
    #[error("no embeddings for {0} yet; run `okbase embed index`")]
    NotIndexed(String),
}

const CREATE: &str = "CREATE TABLE IF NOT EXISTS chunk_vecs (
    chunk_id INTEGER PRIMARY KEY REFERENCES chunks(id) ON DELETE CASCADE,
    model    TEXT NOT NULL,
    vec      BLOB NOT NULL
);";

/// Progress of embedding a bundle.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct EmbedStatus {
    /// Model id.
    pub model: String,
    /// Chunks in the index.
    pub chunks: usize,
    /// Chunks with a vector for this model.
    pub embedded: usize,
}

impl EmbedStatus {
    /// Whether every chunk has a vector.
    pub fn complete(&self) -> bool {
        self.embedded == self.chunks
    }
}

/// How many chunks have vectors for `model`.
pub fn status(index: &Index, model: &str) -> Result<EmbedStatus, Error> {
    let conn = index.connection();
    conn.execute_batch(CREATE)?;
    let chunks: i64 = conn.query_row("SELECT COUNT(*) FROM chunks", [], |r| r.get(0))?;
    let embedded: i64 = conn.query_row(
        "SELECT COUNT(*) FROM chunk_vecs WHERE model = ?1",
        [model],
        |r| r.get(0),
    )?;
    Ok(EmbedStatus {
        model: model.to_owned(),
        chunks: chunks as usize,
        embedded: embedded as usize,
    })
}

/// The `(title, text)` a chunk is embedded as: `title > heading` and the chunk text (as in the spike).
pub fn chunk_input(title: &str, heading: &str, text: &str) -> (String, String) {
    let t = if heading.is_empty() {
        title.to_owned()
    } else {
        format!("{title} > {heading}")
    };
    (t, text.to_owned())
}

/// Embeds every chunk without a vector for the embedder's model. Vectors of other
/// models are replaced. `progress(done, total)` reports new embeddings (cache hits are free).
pub fn embed_sync(
    index: &Index,
    embedder: &dyn Embedder,
    cache: &mut VectorCache,
    progress: &mut dyn FnMut(usize, usize),
) -> Result<EmbedStatus, Error> {
    let conn = index.connection();
    conn.execute_batch(CREATE)?;
    let model = embedder.model_id().to_owned();
    conn.execute("DELETE FROM chunk_vecs WHERE model != ?1", [&model])?;
    let todo: Vec<(i64, (String, String))> = {
        let mut st = conn.prepare(
            "SELECT c.id, d.title, c.heading, c.text FROM chunks c JOIN docs d ON d.id = c.doc_id \
             LEFT JOIN chunk_vecs v ON v.chunk_id = c.id WHERE v.chunk_id IS NULL ORDER BY c.id",
        )?;
        st.query_map([], |r| {
            Ok((
                r.get(0)?,
                chunk_input(
                    &r.get::<_, String>(1)?,
                    &r.get::<_, String>(2)?,
                    &r.get::<_, String>(3)?,
                ),
            ))
        })?
        .collect::<Result<_, _>>()?
    };
    // Embed in slices so progress is saved even if a long run is interrupted.
    let mut done = 0;
    for slice in todo.chunks(256) {
        let inputs: Vec<(String, String)> = slice.iter().map(|(_, i)| i.clone()).collect();
        let vecs = embed_documents_cached(embedder, cache, &inputs, 16, &mut |_, _| {})?;
        let mut st = conn.prepare_cached(
            "INSERT OR REPLACE INTO chunk_vecs (chunk_id, model, vec) VALUES (?1, ?2, ?3)",
        )?;
        for ((id, _), v) in slice.iter().zip(vecs) {
            st.execute(params![id, model, to_bytes(&v)])?;
        }
        done += slice.len();
        progress(done, todo.len());
    }
    status(index, &model)
}

/// Chunk vectors of one model, in memory.
#[derive(Debug, Clone)]
pub struct VectorStore {
    model: String,
    dim: usize,
    data: Vec<f32>,
    chunks: Vec<i64>,
    docs: Vec<String>,
}

impl VectorStore {
    /// Loads the vectors of `model` from the index.
    pub fn load(index: &Index, model: &str) -> Result<Self, Error> {
        let conn = index.connection();
        conn.execute_batch(CREATE)?;
        let mut st = conn.prepare("SELECT v.chunk_id, c.doc_id, v.vec FROM chunk_vecs v JOIN chunks c ON c.id = v.chunk_id WHERE v.model = ?1 ORDER BY v.chunk_id")?;
        let mut store = VectorStore {
            model: model.to_owned(),
            dim: 0,
            data: Vec::new(),
            chunks: Vec::new(),
            docs: Vec::new(),
        };
        let rows = st.query_map([model], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Vec<u8>>(2)?,
            ))
        })?;
        for row in rows {
            let (id, doc, bytes) = row?;
            let v = from_bytes(&bytes);
            if store.dim == 0 {
                store.dim = v.len();
            }
            if v.len() != store.dim {
                continue;
            }
            store.data.extend(v);
            store.chunks.push(id);
            store.docs.push(doc);
        }
        if store.chunks.is_empty() {
            return Err(Error::NotIndexed(model.to_owned()));
        }
        Ok(store)
    }

    /// Number of vectors.
    pub fn len(&self) -> usize {
        self.chunks.len()
    }

    /// Whether the store is empty.
    pub fn is_empty(&self) -> bool {
        self.chunks.is_empty()
    }

    /// The model id.
    pub fn model(&self) -> &str {
        &self.model
    }

    /// `(score, chunk index)` best first, at most `per_doc` chunks per document,
    /// only documents in `allowed`.
    fn rank(
        &self,
        q: &[f32],
        k: usize,
        per_doc: usize,
        allowed: &HashSet<String>,
    ) -> Vec<(f32, usize)> {
        let mut scored: Vec<(f32, usize)> = (0..self.chunks.len())
            .filter(|&i| allowed.contains(&self.docs[i]))
            .map(|i| (dot(q, &self.data[i * self.dim..(i + 1) * self.dim]), i))
            .collect();
        scored.sort_by(|a, b| b.0.total_cmp(&a.0));
        let mut per: HashMap<&str, usize> = HashMap::new();
        scored
            .into_iter()
            .filter(|(_, i)| {
                let n = per.entry(self.docs[*i].as_str()).or_default();
                *n += 1;
                *n <= per_doc.max(1)
            })
            .take(k)
            .collect()
    }
}

/// A semantic search request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SearchRequest {
    /// The question, in any language.
    pub query: String,
    /// Maximum hits (default 8, as in the spike's `kb_search`).
    pub limit: usize,
    /// Maximum chunks per document (default 2).
    pub per_doc: usize,
    /// Metadata filter (same fields as `query`).
    pub filter: Option<Filter>,
}

impl Default for SearchRequest {
    fn default() -> Self {
        SearchRequest {
            query: String::new(),
            limit: 8,
            per_doc: 2,
            filter: None,
        }
    }
}

/// A search hit.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Hit {
    /// Document id.
    pub id: String,
    /// Section heading (`A > B`), empty before the first H2.
    pub heading: String,
    /// Cosine similarity.
    pub score: f32,
    /// The chunk text.
    pub text: String,
    /// Estimated tokens of `text`.
    pub tokens: usize,
}

/// Search results.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SearchResult {
    /// Model used.
    pub model: String,
    /// Hits, best first.
    pub hits: Vec<Hit>,
}

fn hits(index: &Index, store: &VectorStore, ranked: Vec<(f32, usize)>) -> Result<Vec<Hit>, Error> {
    let conn = index.connection();
    let mut st = conn.prepare_cached("SELECT heading, text, tokens FROM chunks WHERE id = ?1")?;
    let mut out = Vec::with_capacity(ranked.len());
    for (score, i) in ranked {
        let (heading, text, tokens) = st.query_row([store.chunks[i]], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
            ))
        })?;
        out.push(Hit {
            id: store.docs[i].clone(),
            heading,
            score,
            text,
            tokens: tokens as usize,
        });
    }
    Ok(out)
}

/// Semantic search over the chunks visible in `scope`.
pub fn search(
    index: &Index,
    store: &VectorStore,
    embedder: &dyn Embedder,
    req: &SearchRequest,
    scope: &Scope,
) -> Result<SearchResult, Error> {
    let allowed = okbase_query::visible_ids(index, scope, req.filter.as_ref())?;
    let q = embedder
        .embed_queries(std::slice::from_ref(&req.query))?
        .remove(0);
    let ranked = store.rank(&q, req.limit.clamp(1, 50), req.per_doc, &allowed);
    Ok(SearchResult {
        model: store.model.clone(),
        hits: hits(index, store, ranked)?,
    })
}

/// Default token budget of `retrieve`.
pub const DEFAULT_BUDGET: usize = 3000;

/// Sections for a prompt, within a token budget (pre-retrieval for hosts).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RetrieveResult {
    /// Model used.
    pub model: String,
    /// Chosen chunks, best first.
    pub hits: Vec<Hit>,
    /// Estimated tokens of the chosen chunks.
    pub tokens: usize,
}

/// The best chunks visible in `scope` until `budget` (estimated) tokens are used;
/// at most two chunks per document, at most 12 chunks.
pub fn retrieve(
    index: &Index,
    store: &VectorStore,
    embedder: &dyn Embedder,
    query: &str,
    budget: usize,
    scope: &Scope,
) -> Result<RetrieveResult, Error> {
    let allowed = okbase_query::visible_ids(index, scope, None)?;
    let q = embedder.embed_queries(&[query.to_owned()])?.remove(0);
    let ranked = store.rank(&q, 12, 2, &allowed);
    let mut chosen = Vec::new();
    let mut used = 0;
    for h in hits(index, store, ranked)? {
        if used + h.tokens > budget && !chosen.is_empty() {
            continue;
        }
        used += h.tokens;
        chosen.push(h);
    }
    Ok(RetrieveResult {
        model: store.model.clone(),
        hits: chosen,
        tokens: used,
    })
}

impl SearchResult {
    /// The spike's `kb_search` text: `doc_id # heading | score | snippet`.
    pub fn to_text(&self) -> String {
        if self.hits.is_empty() {
            return "no results (try other words, or kb_grep for exact terms)".into();
        }
        self.hits
            .iter()
            .map(|h| {
                let snippet: String = h
                    .text
                    .chars()
                    .take(220)
                    .collect::<String>()
                    .replace('\n', " ");
                format!("{} # {} | {:.3} | {snippet}", h.id, h.heading, h.score)
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

impl RetrieveResult {
    /// The spike's prompt format: `<knowledge source="…" section="…">…</knowledge>` blocks.
    pub fn to_text(&self) -> String {
        let mut out =
            String::from("Retrieved sections (semantic search; may be partially relevant):\n");
        for h in &self.hits {
            let _ = write!(
                out,
                "\n<knowledge source=\"{}\" section=\"{}\">\n{}\n</knowledge>\n",
                h.id,
                h.heading.replace('"', "'"),
                h.text
            );
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use okbase_embed::normalize;

    /// Bag-of-letters embedder: deterministic and good enough to rank toy documents.
    struct Letters;
    impl Embedder for Letters {
        fn model_id(&self) -> &str {
            "letters"
        }
        fn embed_queries(&self, q: &[String]) -> Result<Vec<Vec<f32>>, okbase_embed::Error> {
            Ok(q.iter().map(|t| vec_of(t)).collect())
        }
        fn embed_documents(
            &self,
            d: &[(String, String)],
        ) -> Result<Vec<Vec<f32>>, okbase_embed::Error> {
            Ok(d.iter().map(|(a, b)| vec_of(&format!("{a} {b}"))).collect())
        }
    }
    fn vec_of(t: &str) -> Vec<f32> {
        let mut v = vec![0.0; 26];
        for c in okbase_analyze::fold(t)
            .chars()
            .filter(char::is_ascii_lowercase)
        {
            v[(c as u8 - b'a') as usize] += 1.0;
        }
        normalize(v)
    }

    fn bundle() -> (tempfile::TempDir, Index) {
        let tmp = tempfile::tempdir().unwrap();
        let w = |p: &str, t: &str| {
            let f = tmp.path().join(p);
            std::fs::create_dir_all(f.parent().unwrap()).unwrap();
            std::fs::write(f, t).unwrap();
        };
        w(
            "zebra.md",
            "---\ntype: Note\ntitle: Zebra\n---\nzzzz zebra zz\n",
        );
        w(
            "apple.md",
            "---\ntype: Note\ntitle: Apple\n---\napple aaaa pple\n",
        );
        w(
            "private/apples.md",
            "---\ntype: Note\ntitle: Apples\n---\napple apple aaaa\n",
        );
        let mut idx = Index::open_in_memory(tmp.path()).unwrap();
        idx.sync().unwrap();
        (tmp, idx)
    }

    #[test]
    fn embed_search_retrieve_with_scope() {
        let (_tmp, idx) = bundle();
        let mut cache = VectorCache::in_memory().unwrap();
        let mut calls = 0;
        let st = embed_sync(&idx, &Letters, &mut cache, &mut |_, _| calls += 1).unwrap();
        assert!(st.complete() && st.chunks == 3 && calls == 1);
        assert!(
            embed_sync(&idx, &Letters, &mut cache, &mut |_, _| panic!(
                "nothing to do"
            ))
            .unwrap()
            .complete()
        );

        let store = VectorStore::load(&idx, "letters").unwrap();
        let req = SearchRequest {
            query: "apple".into(),
            ..Default::default()
        };
        let all = search(&idx, &store, &Letters, &req, &Scope::all()).unwrap();
        assert!(all.hits[0].id.starts_with("private/") || all.hits[0].id == "apple");
        assert_eq!(all.hits.last().unwrap().id, "zebra");
        let scoped = search(
            &idx,
            &store,
            &Letters,
            &req,
            &Scope::all().deny("private/**").unwrap(),
        )
        .unwrap();
        assert!(scoped.hits.iter().all(|h| !h.id.starts_with("private/")));
        assert!(scoped.to_text().starts_with("apple # "));

        let r = retrieve(&idx, &store, &Letters, "zebra", 1, &Scope::all()).unwrap();
        assert_eq!(r.hits.len(), 1, "budget allows one chunk");
        assert_eq!(r.hits[0].id, "zebra");
        assert!(r.to_text().contains("<knowledge source=\"zebra\""));
    }

    #[test]
    fn missing_vectors_are_reported() {
        let (_tmp, idx) = bundle();
        assert!(matches!(
            VectorStore::load(&idx, "letters"),
            Err(Error::NotIndexed(_))
        ));
        assert_eq!(status(&idx, "letters").unwrap().embedded, 0);
    }
}
