//! Spike: so sánh BM25 (FTS5 + lindera) với 3 model embedding đa ngôn ngữ trên bộ dữ liệu vi/en/ja.
//! Usage: [DATA=v2] [THREADS=2] [RERANK=1] embed-bench <mode>

mod bundle;
mod e2e;
mod gate;

use std::{borrow::Cow, collections::HashMap, time::Instant};

use anyhow::{Context, Result, bail};
use candle_core::{DType, Device};
use fastembed::{
    EmbeddingModel, InitOptionsUserDefined, NomicV2MoeTextEmbedding, Pooling, Qwen3TextEmbedding, RerankInitOptions,
    RerankerModel, TextEmbedding, TextInitOptions, TextRerank, TokenizerFiles, UserDefinedEmbeddingModel,
};
use lindera::{dictionary::load_dictionary, mode::Mode, segmenter::Segmenter};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use unicode_normalization::UnicodeNormalization;

#[derive(Deserialize)]
struct Doc {
    id: String,
    lang: String,
    title: String,
    text: String,
}

#[derive(Deserialize)]
struct Query {
    gold: String,
    lang: String,
    q: String,
}

#[derive(Serialize, Default, Clone)]
struct Metrics {
    n: usize,
    r1: f64,
    r3: f64,
    mrr: f64,
}

#[derive(Serialize)]
struct Report {
    mode: String,
    load_ms: u128,
    index_docs_ms: u128,
    throughput_chunks_per_s: Option<f64>,
    query_p50_ms: f64,
    query_p95_ms: f64,
    peak_rss_mb: f64,
    rss_after_load_mb: Option<f64>,
    model_disk_mb: Option<f64>,
    results: HashMap<String, HashMap<String, Metrics>>, // ranker -> bucket -> metrics
    misses: HashMap<String, Vec<String>>,                // ranker -> "query => top1"
}

// ---------------- Lexical analyzer (NFKC + vi fold + lindera ja) ----------------

fn is_cjk(c: char) -> bool {
    matches!(c as u32,
        0x3040..=0x30FF | 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF | 0xFF66..=0xFF9F)
}

fn fold_latin(s: &str) -> String {
    s.nfd()
        .filter(|c| !('\u{0300}'..='\u{036f}').contains(c))
        .map(|c| match c {
            'đ' | 'Đ' => 'd',
            c => c,
        })
        .collect::<String>()
        .to_lowercase()
}

struct Analyzer {
    seg: Segmenter,
}

impl Analyzer {
    fn new() -> Result<Self> {
        let dict = load_dictionary("embedded://ipadic")?;
        Ok(Self { seg: Segmenter::new(Mode::Normal, dict, None) })
    }

    fn tokens(&self, text: &str) -> Result<Vec<String>> {
        let text: String = text.nfkc().collect();
        let mut out = Vec::new();
        let mut run = String::new();
        let mut run_cjk = false;
        let flush = |run: &mut String, cjk: bool, out: &mut Vec<String>| -> Result<()> {
            if run.is_empty() {
                return Ok(());
            }
            if cjk {
                for mut t in self.seg.segment(Cow::Owned(run.clone()))? {
                    let surface = t.surface.to_string();
                    let d = t.details();
                    let pos = d.first().copied().unwrap_or("");
                    if matches!(pos, "助詞" | "助動詞" | "記号") {
                        continue;
                    }
                    let base = d.get(6).filter(|b| **b != "*").map(|b| b.to_string()).unwrap_or(surface);
                    out.push(base);
                }
            } else {
                for w in run.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()) {
                    out.push(fold_latin(w));
                }
            }
            run.clear();
            Ok(())
        };
        for c in text.chars() {
            let cjk = is_cjk(c);
            if cjk != run_cjk && !run.is_empty() {
                flush(&mut run, run_cjk, &mut out)?;
            }
            run_cjk = cjk;
            run.push(c);
        }
        flush(&mut run, run_cjk, &mut out)?;
        Ok(out)
    }
}

struct Bm25 {
    conn: Connection,
    an: Analyzer,
}

impl Bm25 {
    fn build(docs: &[Doc]) -> Result<Self> {
        let an = Analyzer::new()?;
        let conn = Connection::open_in_memory()?;
        conn.execute_batch("CREATE VIRTUAL TABLE d USING fts5(id UNINDEXED, body, tokenize='unicode61 remove_diacritics 2');")?;
        for d in docs {
            let body = an.tokens(&format!("{} {}", d.title, d.text))?.join(" ");
            conn.execute("INSERT INTO d(id, body) VALUES (?1, ?2)", (&d.id, &body))?;
        }
        Ok(Self { conn, an })
    }

    fn search(&self, q: &str, k: usize) -> Result<Vec<String>> {
        Ok(self.search_scored(q, k)?.into_iter().map(|(id, _)| id).collect())
    }

    fn search_scored(&self, q: &str, k: usize) -> Result<Vec<(String, f64)>> {
        let mut toks = self.an.tokens(q)?;
        toks.sort();
        toks.dedup();
        if toks.is_empty() {
            return Ok(vec![]);
        }
        let expr = toks.iter().map(|t| format!("\"{}\"", t.replace('"', ""))).collect::<Vec<_>>().join(" OR ");
        let mut st = self.conn.prepare_cached("SELECT id, -bm25(d) FROM d WHERE d MATCH ?1 ORDER BY bm25(d) LIMIT ?2")?;
        let ids = st.query_map((expr, k as i64), |r| Ok((r.get::<_, String>(0)?, r.get::<_, f64>(1)?)))?.collect::<Result<Vec<_>, _>>()?;
        Ok(ids)
    }
}

// ---------------- Helpers ----------------

fn peak_rss_mb() -> f64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| s.lines().find(|l| l.starts_with("VmHWM:")).map(|l| l.to_string()))
        .and_then(|l| l.split_whitespace().nth(1).and_then(|v| v.parse::<f64>().ok()))
        .map(|kb| kb / 1024.0)
        .unwrap_or(0.0)
}

fn rss_now_mb() -> f64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| s.lines().find(|l| l.starts_with("VmRSS:")).map(|l| l.to_string()))
        .and_then(|l| l.split_whitespace().nth(1).and_then(|v| v.parse::<f64>().ok()))
        .map(|kb| kb / 1024.0)
        .unwrap_or(0.0)
}

fn dir_size_mb(path: &std::path::Path) -> f64 {
    fn walk(p: &std::path::Path) -> u64 {
        let Ok(md) = std::fs::symlink_metadata(p) else { return 0 };
        if md.is_dir() {
            std::fs::read_dir(p).map(|rd| rd.flatten().map(|e| walk(&e.path())).sum()).unwrap_or(0)
        } else if md.file_type().is_symlink() {
            0 // HF cache: snapshots/ symlink tới blobs/, chỉ đếm blobs
        } else {
            md.len()
        }
    }
    walk(path) as f64 / 1_048_576.0
}

fn pct(v: &mut [f64], p: f64) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[((v.len() as f64 - 1.0) * p).round() as usize]
}

fn cosine(a: &[f32], b: &[f32]) -> f32 {
    let (mut dot, mut na, mut nb) = (0f32, 0f32, 0f32);
    for (x, y) in a.iter().zip(b) {
        dot += x * y;
        na += x * x;
        nb += y * y;
    }
    dot / (na.sqrt() * nb.sqrt())
}

fn evaluate(
    name: &str,
    rankings: &[Vec<String>],
    queries: &[Query],
    doc_lang: &HashMap<String, String>,
    report: &mut Report,
) {
    let mut buckets: HashMap<String, Metrics> = HashMap::new();
    let mut misses = Vec::new();
    for (q, rank) in queries.iter().zip(rankings) {
        let pos = rank.iter().take(10).position(|id| *id == q.gold);
        let same = doc_lang[&q.gold] == q.lang;
        let keys = [
            "all".to_string(),
            if same { "same-lang".into() } else { "cross-lang".into() },
            format!("q:{}", q.lang),
            format!("doc:{}", doc_lang[&q.gold]),
        ];
        for k in keys {
            let m = buckets.entry(k).or_default();
            m.n += 1;
            if pos == Some(0) {
                m.r1 += 1.0;
            }
            if pos.is_some_and(|p| p < 3) {
                m.r3 += 1.0;
            }
            if let Some(p) = pos {
                m.mrr += 1.0 / (p as f64 + 1.0);
            }
        }
        if pos != Some(0) {
            misses.push(format!("[{}→{}] {} => {}", q.lang, q.gold, q.q, rank.first().map(String::as_str).unwrap_or("-")));
        }
    }
    for m in buckets.values_mut() {
        let n = m.n as f64;
        m.r1 /= n;
        m.r3 /= n;
        m.mrr /= n;
    }
    let a = &buckets["all"];
    println!(
        "{name:<18} R@1={:.3} R@3={:.3} MRR={:.3} | same R@1={:.3} cross R@1={:.3}",
        a.r1, a.r3, a.mrr, buckets["same-lang"].r1, buckets["cross-lang"].r1
    );
    report.results.insert(name.into(), buckets);
    report.misses.insert(name.into(), misses);
}

// ---------------- Embedders ----------------

enum Embedder {
    Fast(TextEmbedding),
    /// Model lượng tử hoá động: fastembed không cho chia batch
    FastDynamic(TextEmbedding),
    Qwen(Qwen3TextEmbedding),
    Nomic(NomicV2MoeTextEmbedding),
}

impl Embedder {
    fn embed(&mut self, texts: &[String], bs: usize) -> Result<Vec<Vec<f32>>> {
        Ok(match self {
            Embedder::Fast(e) => e.embed(texts, Some(bs))?,
            Embedder::FastDynamic(e) => e.embed(texts, None)?,
            Embedder::Qwen(e) => {
                let mut out = Vec::new();
                for c in texts.chunks(bs) {
                    out.extend(e.embed(c)?);
                }
                out
            }
            Embedder::Nomic(e) => {
                let mut out = Vec::new();
                for c in texts.chunks(bs) {
                    out.extend(e.embed(c)?);
                }
                out
            }
        })
    }
}

struct Spec {
    emb: Embedder,
    q: Box<dyn Fn(&str) -> String>,
    d: Box<dyn Fn(&str, &str) -> String>,
    disk: std::path::PathBuf,
}

fn prefixed(qp: &'static str, dp: &'static str) -> (Box<dyn Fn(&str) -> String>, Box<dyn Fn(&str, &str) -> String>) {
    (Box::new(move |q| format!("{qp}{q}")), Box::new(move |t, x| format!("{dp}{t}\n{x}")))
}

fn user_defined(dir: &std::path::Path, threads: Option<usize>) -> Result<TextEmbedding> {
    let rd = |f: &str| std::fs::read(dir.join(f)).with_context(|| format!("{}/{f}", dir.display()));
    let tok = TokenizerFiles {
        tokenizer_file: rd("tokenizer.json")?,
        config_file: rd("config.json")?,
        special_tokens_map_file: rd("special_tokens_map.json")?,
        tokenizer_config_file: rd("tokenizer_config.json")?,
    };
    let model = UserDefinedEmbeddingModel::new(rd("model.onnx")?, tok).with_pooling(Pooling::Cls);
    let mut opts = InitOptionsUserDefined::new().with_max_length(512);
    if let Some(t) = threads {
        opts = opts.with_intra_threads(t);
    }
    Ok(TextEmbedding::try_new_from_user_defined(model, opts)?)
}

fn load(mode: &str, root: &std::path::Path, threads: Option<usize>) -> Result<Spec> {
    let cache = root.join(".fastembed_cache");
    let builtin = |m: EmbeddingModel| -> Result<Embedder> {
        let mut o = TextInitOptions::new(m).with_cache_dir(cache.clone()).with_max_length(512);
        if let Some(t) = threads {
            o = o.with_intra_threads(t);
        }
        Ok(Embedder::Fast(TextEmbedding::try_new(o)?))
    };
    let hf = |repo: &str| cache.join(format!("models--{}", repo.replace('/', "--")));
    let e5 = || prefixed("query: ", "passage: ");
    let gemma = || -> (Box<dyn Fn(&str) -> String>, Box<dyn Fn(&str, &str) -> String>) {
        (Box::new(|q| format!("task: search result | query: {q}")), Box::new(|t, x| format!("title: {t} | text: {x}")))
    };
    let (emb, (q, d), disk) = match mode {
        "e5-small" => (builtin(EmbeddingModel::MultilingualE5Small)?, e5(), hf("intfloat/multilingual-e5-small")),
        "e5-base" => (builtin(EmbeddingModel::MultilingualE5Base)?, e5(), hf("intfloat/multilingual-e5-base")),
        "e5-large" => (builtin(EmbeddingModel::MultilingualE5Large)?, e5(), hf("Qdrant/multilingual-e5-large-onnx")),
        "bge-m3" => (builtin(EmbeddingModel::BGEM3)?, prefixed("", ""), hf("BAAI/bge-m3")),
        "gemma" => (builtin(EmbeddingModel::EmbeddingGemma300M)?, gemma(), hf("onnx-community/embeddinggemma-300m-ONNX")),
        "gemma-q" => {
            let Embedder::Fast(e) = builtin(EmbeddingModel::EmbeddingGemma300MQ)? else { unreachable!() };
            (Embedder::FastDynamic(e), gemma(), hf("onnx-community/embeddinggemma-300m-ONNX"))
        }
        "gemma-q4" => (builtin(EmbeddingModel::EmbeddingGemma300MQ4)?, gemma(), hf("onnx-community/embeddinggemma-300m-ONNX")),
        "bge-m3-int8" | "gte-mb" | "gte-mb-int8" => {
            let dir = root.join(".models").join(mode);
            (Embedder::Fast(user_defined(&dir, threads)?), prefixed("", ""), dir)
        }
        "qwen3-0.6b" => {
            let e = Qwen3TextEmbedding::from_hf("Qwen/Qwen3-Embedding-0.6B", &Device::Cpu, DType::F32, 512)?;
            let q: Box<dyn Fn(&str) -> String> = Box::new(|q| {
                format!("Instruct: Given a question, retrieve the document that answers it\nQuery:{q}")
            });
            let d: Box<dyn Fn(&str, &str) -> String> = Box::new(|t, x| format!("{t}\n{x}"));
            (Embedder::Qwen(e), (q, d), hf("Qwen/Qwen3-Embedding-0.6B"))
        }
        "nomic-v2" => {
            let e = NomicV2MoeTextEmbedding::from_hf("nomic-ai/nomic-embed-text-v2-moe", &Device::Cpu, DType::F32, 512)?;
            (Embedder::Nomic(e), prefixed("search_query: ", "search_document: "), hf("nomic-ai/nomic-embed-text-v2-moe"))
        }
        m => bail!("unknown mode {m}"),
    };
    Ok(Spec { emb, q, d, disk })
}

// ---------------- Main ----------------

fn main() -> Result<()> {
    let mode = std::env::args().nth(1).context("usage: embed-bench <bm25|e5-small|...> (env DATA=v2 THREADS=2 RERANK=1)")?;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let data = std::env::var("DATA").unwrap_or_default();
    let threads: Option<usize> = std::env::var("THREADS").ok().and_then(|v| v.parse().ok());
    let rerank = std::env::var("RERANK").is_ok_and(|v| v == "1");
    // SAFETY: đặt trước khi tạo thread nào
    unsafe {
        std::env::set_var("FASTEMBED_CACHE_DIR", root.join(".fastembed_cache"));
        if let Some(t) = threads {
            std::env::set_var("RAYON_NUM_THREADS", t.to_string());
        }
    }
    if let Some(sub) = mode.strip_prefix("bundle-") {
        let args: Vec<String> = std::env::args().skip(2).collect();
        let dir = std::path::PathBuf::from(args.first().context("bundle dir")?);
        let model = std::env::var("MODEL").unwrap_or_else(|_| "gemma-q4".into());
        return match sub {
            "index" => bundle::index(root, &dir, &mut load(&model, root, threads)?),
            "retrieve" => {
                let k = args.get(3).and_then(|v| v.parse().ok()).unwrap_or(6);
                bundle::retrieve(&dir, args[1].as_ref(), args[2].as_ref(), k, &mut load(&model, root, threads)?)
            }
            "mcp" => {
                let spec = if std::env::var("NOSEARCH").is_ok() { None } else { Some(load(&model, root, threads)?) };
                bundle::serve(&dir, spec)
            }
            other => bail!("unknown bundle mode {other}"),
        };
    }
    let ddir = root.join("data").join(&data);
    let docs: Vec<Doc> = serde_json::from_str(&std::fs::read_to_string(ddir.join("docs.json"))?)?;
    let queries: Vec<Query> = serde_json::from_str(&std::fs::read_to_string(ddir.join("queries.json"))?)?;
    let doc_lang: HashMap<_, _> = docs.iter().map(|d| (d.id.clone(), d.lang.clone())).collect();
    for q in &queries {
        if !doc_lang.contains_key(&q.gold) {
            bail!("unknown gold {}", q.gold);
        }
    }
    let tag = format!(
        "{mode}{}{}",
        threads.map(|t| format!("-t{t}")).unwrap_or_default(),
        if rerank { "-rr" } else { "" }
    );

    let mut report = Report {
        mode: tag.clone(),
        load_ms: 0,
        index_docs_ms: 0,
        throughput_chunks_per_s: None,
        query_p50_ms: 0.0,
        query_p95_ms: 0.0,
        peak_rss_mb: 0.0,
        rss_after_load_mb: None,
        model_disk_mb: None,
        results: HashMap::new(),
        misses: HashMap::new(),
    };

    let bm25 = Bm25::build(&docs)?;
    let bm25_rank: Vec<Vec<String>> = queries.iter().map(|q| bm25.search(&q.q, 10)).collect::<Result<_>>()?;

    if mode == "e2e-prep" || mode == "mcp" {
        let model = std::env::var("MODEL").unwrap_or_else(|_| "gemma-q4".into());
        let mut spec = load(&model, root, threads)?;
        return if mode == "mcp" { e2e::serve(root, &docs, &bm25, spec) } else { e2e::prep(root, &docs, &queries, &mut spec) };
    }

    if mode == "gate" {
        // Thí nghiệm ngưỡng BM25 trên model dense chỉ định (MODEL, mặc định gemma-q4)
        let model = std::env::var("MODEL").unwrap_or_else(|_| "gemma-q4".into());
        println!("# Gate experiment — model={model}, data={}", if data.is_empty() { "v1" } else { &data });
        let mut spec = load(&model, root, threads)?;
        let mut texts: Vec<String> = docs.iter().map(|d| (spec.d)(&d.title, &d.text)).collect();
        texts.extend(queries.iter().map(|q| (spec.q)(&q.q)));
        let vecs = spec.emb.embed(&texts, 16)?;
        return gate::run(&docs, &queries, &vecs, &bm25, &bm25.an);
    }

    if mode == "bm25" {
        let mut lat: Vec<f64> = queries
            .iter()
            .map(|q| {
                let t = Instant::now();
                let _ = bm25.search(&q.q, 10);
                t.elapsed().as_secs_f64() * 1000.0
            })
            .collect();
        report.query_p50_ms = pct(&mut lat.clone(), 0.5);
        report.query_p95_ms = pct(&mut lat, 0.95);
        evaluate("bm25", &bm25_rank, &queries, &doc_lang, &mut report);
    } else {
        let t = Instant::now();
        let mut spec = load(&mode, root, threads)?;
        report.load_ms = t.elapsed().as_millis();
        report.model_disk_mb = Some(dir_size_mb(&spec.disk));
        report.rss_after_load_mb = Some(rss_now_mb());

        let passages: Vec<String> = docs.iter().map(|d| (spec.d)(&d.title, &d.text)).collect();
        let t = Instant::now();
        let dvec = spec.emb.embed(&passages, 16)?;
        report.index_docs_ms = t.elapsed().as_millis();
        report.throughput_chunks_per_s = Some(passages.len() as f64 / t.elapsed().as_secs_f64());

        let mut lat = Vec::new();
        let mut dense_rank = Vec::new();
        for q in &queries {
            let t = Instant::now();
            let qv = spec.emb.embed(&[(spec.q)(&q.q)], 1)?.remove(0);
            let mut scored: Vec<(f32, &str)> = dvec.iter().zip(&docs).map(|(v, d)| (cosine(&qv, v), d.id.as_str())).collect();
            scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
            lat.push(t.elapsed().as_secs_f64() * 1000.0);
            dense_rank.push(scored.iter().take(20).map(|(_, id)| id.to_string()).collect::<Vec<_>>());
        }
        report.query_p50_ms = pct(&mut lat.clone(), 0.5);
        report.query_p95_ms = pct(&mut lat, 0.95);
        evaluate("bm25", &bm25_rank, &queries, &doc_lang, &mut report);
        evaluate(&mode, &dense_rank, &queries, &doc_lang, &mut report);

        if rerank {
            let cache = root.join(".fastembed_cache");
            let mut o = RerankInitOptions::new(RerankerModel::BGERerankerV2M3).with_cache_dir(cache);
            if let Some(t) = threads {
                o = o.with_intra_threads(t);
            }
            let mut rr = TextRerank::try_new(o)?;
            let by_id: HashMap<&str, String> = docs.iter().map(|d| (d.id.as_str(), format!("{}\n{}", d.title, d.text))).collect();
            let mut rr_rank = Vec::new();
            let mut rr_lat = Vec::new();
            for (q, cand) in queries.iter().zip(&dense_rank) {
                let texts: Vec<&str> = cand.iter().map(|id| by_id[id.as_str()].as_str()).collect();
                let t = Instant::now();
                let res = rr.rerank(q.q.as_str(), &texts, false, Some(20))?;
                rr_lat.push(t.elapsed().as_secs_f64() * 1000.0);
                rr_rank.push(res.iter().map(|r| cand[r.index].clone()).collect::<Vec<_>>());
            }
            println!("rerank top-20 latency p50={:.0}ms p95={:.0}ms", pct(&mut rr_lat.clone(), 0.5), pct(&mut rr_lat, 0.95));
            evaluate(&format!("{mode}+rerank"), &rr_rank, &queries, &doc_lang, &mut report);
        }
    }

    report.peak_rss_mb = peak_rss_mb();
    println!(
        "load={}ms thr={:?}/s q_p50={:.1}ms q_p95={:.1}ms rss_load={:?}MB rss_peak={:.0}MB disk={:?}MB",
        report.load_ms,
        report.throughput_chunks_per_s.map(|v| (v * 10.0).round() / 10.0),
        report.query_p50_ms,
        report.query_p95_ms,
        report.rss_after_load_mb.map(|v| v.round()),
        report.peak_rss_mb,
        report.model_disk_mb.map(|v| v.round())
    );
    let out = root.join("results").join(if data.is_empty() { "v1" } else { &data });
    std::fs::create_dir_all(&out)?;
    std::fs::write(out.join(format!("{tag}.json")), serde_json::to_string_pretty(&report)?)?;
    Ok(())
}
