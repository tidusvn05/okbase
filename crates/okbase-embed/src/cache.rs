//! A vector cache keyed by model and text hash (`~/.cache/okbase/emb`), so
//! re-indexing, renaming files or sharing chunks between bundles never embeds
//! the same text twice.

use std::path::Path;

use rusqlite::{Connection, params};

use crate::{Embedder, Error};

/// Vectors by `(model, blake3(text))`.
#[derive(Debug)]
pub struct VectorCache {
    conn: Connection,
}

fn key(model: &str, text: &str) -> Vec<u8> {
    let mut h = blake3::Hasher::new();
    h.update(model.as_bytes());
    h.update(&[0]);
    h.update(text.as_bytes());
    h.finalize().as_bytes().to_vec()
}

/// f32 little-endian bytes.
pub fn to_bytes(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_le_bytes()).collect()
}

/// Inverse of [`to_bytes`].
pub fn from_bytes(b: &[u8]) -> Vec<f32> {
    b.as_chunks::<4>()
        .0
        .iter()
        .map(|c| f32::from_le_bytes(*c))
        .collect()
}

impl VectorCache {
    /// Opens (or creates) the cache database.
    pub fn open(path: &Path) -> Result<Self, Error> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)
                .map_err(|e| Error::Model(format!("{}: {e}", dir.display())))?;
        }
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.busy_timeout(std::time::Duration::from_secs(30))?;
        conn.execute_batch("CREATE TABLE IF NOT EXISTS vecs (key BLOB PRIMARY KEY, vec BLOB NOT NULL) WITHOUT ROWID;")?;
        Ok(VectorCache { conn })
    }

    /// An in-memory cache (tests).
    pub fn in_memory() -> Result<Self, Error> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch(
            "CREATE TABLE vecs (key BLOB PRIMARY KEY, vec BLOB NOT NULL) WITHOUT ROWID;",
        )?;
        Ok(VectorCache { conn })
    }

    /// The cached vector of `text` for `model`.
    pub fn get(&self, model: &str, text: &str) -> Result<Option<Vec<f32>>, Error> {
        let mut st = self
            .conn
            .prepare_cached("SELECT vec FROM vecs WHERE key = ?1")?;
        let mut rows = st.query([key(model, text)])?;
        Ok(rows
            .next()?
            .map(|r| from_bytes(&r.get::<_, Vec<u8>>(0).unwrap_or_default())))
    }

    /// Stores vectors.
    pub fn put_many(&mut self, model: &str, items: &[(String, Vec<f32>)]) -> Result<(), Error> {
        let tx = self.conn.transaction()?;
        {
            let mut st =
                tx.prepare_cached("INSERT OR REPLACE INTO vecs (key, vec) VALUES (?1, ?2)")?;
            for (text, v) in items {
                st.execute(params![key(model, text), to_bytes(v)])?;
            }
        }
        tx.commit()?;
        Ok(())
    }
}

/// Embeds documents through the cache: cached vectors are reused, the rest are
/// embedded in batches of similar length (less padding) and stored.
/// `progress(done, total)` is called after each batch.
pub fn embed_documents_cached(
    embedder: &dyn Embedder,
    cache: &mut VectorCache,
    docs: &[(String, String)],
    batch: usize,
    progress: &mut dyn FnMut(usize, usize),
) -> Result<Vec<Vec<f32>>, Error> {
    let model = embedder.model_id().to_owned();
    let cache_text = |d: &(String, String)| format!("{}\u{0}{}", d.0, d.1);
    let mut out: Vec<Option<Vec<f32>>> = Vec::with_capacity(docs.len());
    for d in docs {
        out.push(cache.get(&model, &cache_text(d))?);
    }
    let mut todo: Vec<usize> = (0..docs.len()).filter(|&i| out[i].is_none()).collect();
    todo.sort_by_key(|&i| docs[i].0.len() + docs[i].1.len());
    let total = todo.len();
    let mut done = 0;
    for chunk in todo.chunks(batch.max(1)) {
        let inputs: Vec<(String, String)> = chunk.iter().map(|&i| docs[i].clone()).collect();
        let vecs = embedder.embed_documents(&inputs)?;
        let items: Vec<(String, Vec<f32>)> = chunk
            .iter()
            .zip(&vecs)
            .map(|(&i, v)| (cache_text(&docs[i]), v.clone()))
            .collect();
        cache.put_many(&model, &items)?;
        for (&i, v) in chunk.iter().zip(vecs) {
            out[i] = Some(v);
        }
        done += chunk.len();
        progress(done, total);
    }
    Ok(out.into_iter().map(|v| v.unwrap_or_default()).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// A fake embedder: one dimension per character class, counting calls.
    struct Fake(AtomicUsize);
    impl Embedder for Fake {
        fn model_id(&self) -> &str {
            "fake"
        }
        fn embed_queries(&self, q: &[String]) -> Result<Vec<Vec<f32>>, Error> {
            Ok(q.iter()
                .map(|t| crate::normalize(vec![t.len() as f32, 1.0]))
                .collect())
        }
        fn embed_documents(&self, d: &[(String, String)]) -> Result<Vec<Vec<f32>>, Error> {
            self.0.fetch_add(d.len(), Ordering::SeqCst);
            Ok(d.iter()
                .map(|(a, b)| crate::normalize(vec![a.len() as f32, b.len() as f32]))
                .collect())
        }
    }

    #[test]
    fn cache_avoids_re_embedding() {
        let fake = Fake(AtomicUsize::new(0));
        let mut cache = VectorCache::in_memory().unwrap();
        let docs: Vec<(String, String)> =
            (0..10).map(|i| (format!("t{i}"), "x".repeat(i))).collect();
        let mut calls = Vec::new();
        let a = embed_documents_cached(&fake, &mut cache, &docs, 4, &mut |d, t| calls.push((d, t)))
            .unwrap();
        assert_eq!(fake.0.load(Ordering::SeqCst), 10);
        assert_eq!(calls, [(4, 10), (8, 10), (10, 10)]);
        let b = embed_documents_cached(&fake, &mut cache, &docs, 4, &mut |_, _| {}).unwrap();
        assert_eq!(
            fake.0.load(Ordering::SeqCst),
            10,
            "second pass uses the cache"
        );
        assert_eq!(a, b);
        assert_eq!(from_bytes(&to_bytes(&a[3])), a[3]);
    }
}
