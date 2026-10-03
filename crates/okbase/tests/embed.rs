//! Embeddings through the facade with a host-provided embedder (no model download).

use std::path::Path;
use std::sync::Arc;

use okbase::{
    Bundle, EmbedError, Embedder, OpenOptions, Scope, SearchRequest, StateDir, capability,
};

/// Bag-of-letters embedder over accent-folded text: deterministic and model-free.
struct Letters;
fn vec_of(t: &str) -> Vec<f32> {
    let mut v = vec![0.0f32; 26];
    for c in okbase::analyze::fold(t)
        .chars()
        .filter(char::is_ascii_lowercase)
    {
        v[(c as u8 - b'a') as usize] += 1.0;
    }
    let n = v.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-9);
    v.into_iter().map(|x| x / n).collect()
}
impl Embedder for Letters {
    fn model_id(&self) -> &str {
        "letters"
    }
    fn embed_queries(&self, q: &[String]) -> Result<Vec<Vec<f32>>, EmbedError> {
        Ok(q.iter().map(|t| vec_of(t)).collect())
    }
    fn embed_documents(&self, d: &[(String, String)]) -> Result<Vec<Vec<f32>>, EmbedError> {
        Ok(d.iter().map(|(a, b)| vec_of(&format!("{a} {b}"))).collect())
    }
}

#[test]
fn host_embedder_search_retrieve_and_mode() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/business");
    let tmp = tempfile::tempdir().unwrap();
    let opts = OpenOptions::default()
        .state_dir(StateDir::Path(tmp.path().join("state")))
        .vector_cache(tmp.path().join("vectors.sqlite"))
        .embedder(Arc::new(Letters));
    let b = Bundle::open(&fixture, opts).unwrap();
    b.sync().unwrap();
    assert!(b.capabilities().has(capability::EMBED_SEARCH));
    assert_eq!(b.embedding_model().as_deref(), Some("letters"));
    assert_eq!(
        b.recommend_mode(&Scope::all()).unwrap(),
        okbase::Mode::Lexical,
        "not embedded yet"
    );
    assert!(
        b.search(
            &SearchRequest {
                query: "x".into(),
                ..Default::default()
            },
            &Scope::all()
        )
        .is_err()
    );

    let st = b.embed_sync(&mut |_, _| {}).unwrap();
    assert!(st.complete() && st.chunks > 100);
    assert_eq!(
        b.recommend_mode(&Scope::all()).unwrap(),
        okbase::Mode::Retrieval
    );

    let scope = Scope::all().deny("contracts/**").unwrap();
    let r = b
        .search(
            &SearchRequest {
                query: "warranty policy".into(),
                limit: 5,
                ..Default::default()
            },
            &scope,
        )
        .unwrap();
    assert_eq!(r.hits.len(), 5);
    assert!(r.hits.iter().all(|h| !h.id.starts_with("contracts/")));
    let rr = b.retrieve("warranty", 400, &scope).unwrap();
    assert!(!rr.hits.is_empty() && rr.tokens <= 400.max(rr.hits[0].tokens));

    // Without an embedder the capability is absent and search explains how to enable it.
    let plain = Bundle::open_in_memory(&fixture).unwrap();
    assert!(!plain.capabilities().has(capability::EMBED_SEARCH));
    let err = plain
        .search(
            &SearchRequest {
                query: "x".into(),
                ..Default::default()
            },
            &Scope::all(),
        )
        .unwrap_err();
    assert!(err.to_string().contains("okbase embed enable"), "{err}");
}
