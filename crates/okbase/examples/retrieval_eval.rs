//! Retrieval evals of v0.3 (design §13), CPU only:
//!
//! - S1: 300 vi/en/ja questions over 100 documents (`fixtures/multilingual`), document-level
//!   R@1 / R@3 / MRR, also by same-language vs cross-language.
//! - S4: the okf-scale questions on a bundle (e.g. size L), gold document in the top-6 chunks
//!   (at most 2 chunks per document, as `kb_search`) and in first place.
//!
//! `cargo run --release -p okbase --features embed-local --example retrieval_eval -- s1 <bundle> <queries.json> <model> <state-dir>`
//! `... -- s4 <bundle> <questions.json> <model> <state-dir>`

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use okbase::{Bundle, OpenOptions, Scope, SearchRequest, StateDir};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [suite, bundle, questions, model, state] = &args[..] else {
        eprintln!("usage: retrieval_eval s1|s4 <bundle> <questions.json> <model> <state-dir>");
        std::process::exit(2);
    };
    let t0 = std::time::Instant::now();
    let embedder = Arc::new(okbase_embed::LocalEmbedder::load(model, None).expect("load model"));
    eprintln!("model {model} loaded in {:.1?}", t0.elapsed());
    let opts = OpenOptions::default()
        .state_dir(StateDir::Path(Path::new(state).to_owned()))
        .vector_cache(Path::new(state).join("vectors.sqlite"))
        .embedder(embedder);
    let b = Bundle::open(Path::new(bundle), opts).expect("open bundle");
    b.sync().expect("sync");
    let t1 = std::time::Instant::now();
    let st = b
        .embed_sync(&mut |d, t| eprint!("\rembedding {d}/{t}   "))
        .expect("embed");
    eprintln!("\n{} chunks embedded in {:.1?}", st.chunks, t1.elapsed());

    let qs: Vec<serde_json::Value> =
        serde_json::from_str(&std::fs::read_to_string(questions).expect("questions"))
            .expect("json");
    let scope = Scope::all();
    let mut lat = Vec::new();
    let report = match suite.as_str() {
        "s1" => {
            // (bucket) -> (n, hits@1, hits@3, reciprocal rank sum)
            let mut agg: BTreeMap<String, (usize, usize, usize, f64)> = BTreeMap::new();
            for q in &qs {
                let gold = format!("knowledge/{}", q["gold"].as_str().unwrap());
                let qlang = q["lang"].as_str().unwrap();
                let doc_lang = &q["gold"].as_str().unwrap()[..2];
                let t = std::time::Instant::now();
                let r = b
                    .search(
                        &SearchRequest {
                            query: q["q"].as_str().unwrap().into(),
                            limit: 10,
                            per_doc: 1,
                            filter: None,
                        },
                        &scope,
                    )
                    .unwrap();
                lat.push(t.elapsed().as_secs_f64() * 1000.0);
                let rank = r.hits.iter().position(|h| h.id == gold);
                for bucket in [
                    "all",
                    if qlang == doc_lang {
                        "same-lang"
                    } else {
                        "cross-lang"
                    },
                    qlang,
                ] {
                    let e = agg.entry(bucket.into()).or_default();
                    e.0 += 1;
                    e.1 += usize::from(rank == Some(0));
                    e.2 += usize::from(rank.is_some_and(|r| r < 3));
                    e.3 += rank.map_or(0.0, |r| 1.0 / (r as f64 + 1.0));
                }
            }
            agg.into_iter()
                .map(|(k, (n, h1, h3, rr))| {
                    (k, serde_json::json!({"n": n, "r@1": h1 as f64 / n as f64, "r@3": h3 as f64 / n as f64, "mrr": rr / n as f64}))
                })
                .collect::<serde_json::Map<_, _>>()
        }
        "s4" => {
            let (mut top6, mut first, mut n) = (0, 0, 0);
            let mut misses = Vec::new();
            for q in &qs {
                let gold = q["gold"].as_str().unwrap();
                let t = std::time::Instant::now();
                let r = b
                    .search(
                        &SearchRequest {
                            query: q["q"].as_str().unwrap().into(),
                            limit: 6,
                            per_doc: 2,
                            filter: None,
                        },
                        &scope,
                    )
                    .unwrap();
                lat.push(t.elapsed().as_secs_f64() * 1000.0);
                n += 1;
                if r.hits.iter().any(|h| h.id == gold) {
                    top6 += 1;
                } else {
                    misses.push(q["qid"].clone());
                }
                first += usize::from(r.hits.first().is_some_and(|h| h.id == gold));
            }
            let mut m = serde_json::Map::new();
            m.insert("all".into(), serde_json::json!({"n": n, "gold_in_top6": top6, "gold_first": first, "misses": misses}));
            m
        }
        other => panic!("unknown suite {other}"),
    };
    lat.sort_by(f64::total_cmp);
    let out = serde_json::json!({
        "suite": suite, "model": model, "chunks": st.chunks,
        "query_ms_p50": lat[lat.len() / 2], "query_ms_p90": lat[lat.len() * 9 / 10],
        "results": report,
    });
    println!("{}", serde_json::to_string_pretty(&out).unwrap());
}
