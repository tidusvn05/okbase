//! Thí nghiệm "ngưỡng BM25": khi nào nên tin BM25 để cộng điểm vào kết quả dense.
//! final(d) = cos(q,d) + beta * g(q,d), với g là cổng (gate) dựa trên tín hiệu BM25.
//! Chọn tham số bằng 2-fold cross-validation (chia theo tài liệu đích) để tránh overfit.

use std::collections::{HashMap, HashSet};

use anyhow::Result;
use lingua::{Language, LanguageDetectorBuilder};

use crate::{Analyzer, Bm25, Doc, Query};

/// Tín hiệu của một cặp (query, doc)
#[derive(Clone, Copy, Default)]
struct Sig {
    cos: f32,
    bm25: f64, // 0 nếu không nằm trong top-20 BM25
    bm25_rank: usize,
    wcov: f64, // độ phủ token query có trọng số IDF
    same_lang: bool,
}

struct QData {
    gold: usize,
    sigs: Vec<Sig>, // theo index doc
    bm25_ratio: f64, // top1 / top2 BM25
}

#[derive(Clone, Copy, Debug)]
enum Gate {
    None,
    Abs { t: f64 },
    Top1Conf { t: f64, r: f64 },
    Cov { c: f64 },
    CovSoft { c: f64 },
}

#[derive(Clone, Copy, Debug)]
struct Cfg {
    gate: Gate,
    beta: f32,
    same_lang: bool,
}

fn g(cfg: &Cfg, q: &QData, s: &Sig) -> f32 {
    if cfg.same_lang && !s.same_lang {
        return 0.0;
    }
    match cfg.gate {
        Gate::None => 0.0,
        Gate::Abs { t } => (s.bm25 >= t) as u8 as f32,
        Gate::Top1Conf { t, r } => (s.bm25_rank == 1 && s.bm25 >= t && q.bm25_ratio >= r) as u8 as f32,
        Gate::Cov { c } => (s.wcov >= c) as u8 as f32,
        Gate::CovSoft { c } => (((s.wcov - c) / (1.0 - c)).max(0.0)) as f32,
    }
}

fn rank_of_gold(cfg: &Cfg, q: &QData) -> usize {
    let score = |s: &Sig| s.cos + cfg.beta * g(cfg, q, s);
    let gs = score(&q.sigs[q.gold]);
    q.sigs.iter().enumerate().filter(|(i, s)| *i != q.gold && score(s) > gs).count()
}

fn eval(cfg: &Cfg, qs: &[&QData]) -> (f64, f64) {
    let (mut r1, mut mrr) = (0.0, 0.0);
    for q in qs {
        let r = rank_of_gold(cfg, q);
        if r == 0 {
            r1 += 1.0;
        }
        mrr += 1.0 / (r as f64 + 1.0);
    }
    (r1 / qs.len() as f64, mrr / qs.len() as f64)
}

fn quantiles(mut v: Vec<f64>, ps: &[f64]) -> Vec<f64> {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    ps.iter().map(|p| v[((v.len() - 1) as f64 * p) as usize]).collect()
}

pub fn run(docs: &[Doc], queries: &[Query], dense: &[Vec<f32>], bm25: &Bm25, an: &Analyzer) -> Result<()> {
    // ---- Nhận diện ngôn ngữ (lingua, chỉ vi/en/ja)
    let det = LanguageDetectorBuilder::from_languages(&[Language::English, Language::Japanese, Language::Vietnamese]).build();
    let code = |l: Language| match l {
        Language::English => "en",
        Language::Japanese => "ja",
        _ => "vi",
    };
    let mut det_ok = HashMap::<String, (usize, usize)>::new();
    let qlang: Vec<&str> = queries
        .iter()
        .map(|q| {
            let l = det.detect_language_of(&q.q).map(code).unwrap_or("en");
            let e = det_ok.entry(q.lang.clone()).or_default();
            e.1 += 1;
            if l == q.lang {
                e.0 += 1;
            }
            l
        })
        .collect();
    println!("\n## Nhận diện ngôn ngữ query (lingua)");
    for (l, (ok, n)) in &det_ok {
        println!("  {l}: {ok}/{n} đúng ({:.1}%)", 100.0 * *ok as f64 / *n as f64);
    }

    // ---- IDF và tập token của từng doc
    let doc_toks: Vec<HashSet<String>> =
        docs.iter().map(|d| an.tokens(&format!("{} {}", d.title, d.text)).map(|v| v.into_iter().collect())).collect::<Result<_>>()?;
    let n = docs.len() as f64;
    let mut df: HashMap<&str, usize> = HashMap::new();
    for t in &doc_toks {
        for w in t {
            *df.entry(w.as_str()).or_default() += 1;
        }
    }
    let idf = |w: &str| (n / (*df.get(w).unwrap_or(&0) as f64 + 0.5)).ln().max(0.0);

    // ---- Tín hiệu cho mọi cặp (query, doc)
    let id_idx: HashMap<&str, usize> = docs.iter().enumerate().map(|(i, d)| (d.id.as_str(), i)).collect();
    let mut data = Vec::new();
    for (qi, q) in queries.iter().enumerate() {
        let qv = &dense[docs.len() + qi];
        let qt: HashSet<String> = an.tokens(&q.q)?.into_iter().collect();
        let denom: f64 = qt.iter().map(|w| idf(w)).sum::<f64>().max(1e-9);
        let hits = bm25.search_scored(&q.q, 20)?;
        let bm: HashMap<usize, (f64, usize)> = hits.iter().enumerate().map(|(r, (id, s))| (id_idx[id.as_str()], (*s, r + 1))).collect();
        let ratio = match (hits.first(), hits.get(1)) {
            (Some(a), Some(b)) => a.1 / b.1.max(1e-9),
            (Some(_), None) => 10.0,
            _ => 0.0,
        };
        let sigs = docs
            .iter()
            .enumerate()
            .map(|(di, d)| {
                let (b, r) = bm.get(&di).copied().unwrap_or((0.0, 0));
                let matched: f64 = qt.iter().filter(|w| doc_toks[di].contains(*w)).map(|w| idf(w)).sum();
                Sig { cos: crate::cosine(qv, &dense[di]), bm25: b, bm25_rank: r, wcov: matched / denom, same_lang: d.lang == qlang[qi] }
            })
            .collect();
        data.push(QData { gold: id_idx[q.gold.as_str()], sigs, bm25_ratio: ratio });
    }

    // ---- 1) Độ tin cậy của BM25 top-1 theo ngưỡng điểm (đây là "ngưỡng" đo trực tiếp)
    let top1: Vec<(f64, bool, bool)> = data
        .iter()
        .filter_map(|q| {
            q.sigs.iter().enumerate().find(|(_, s)| s.bm25_rank == 1).map(|(i, s)| (s.bm25, i == q.gold, s.same_lang))
        })
        .collect();
    let ts = quantiles(top1.iter().map(|x| x.0).collect(), &[0.0, 0.2, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 0.95]);
    println!("\n## Độ chính xác BM25 top-1 theo ngưỡng điểm (−bm25)");
    println!("  {:>7} | {:>9} {:>7} | {:>13} {:>7}", "T", "precision", "phủ", "same-lang prec", "phủ");
    for t in &ts {
        let sel: Vec<_> = top1.iter().filter(|x| x.0 >= *t).collect();
        let sl: Vec<_> = sel.iter().filter(|x| x.2).collect();
        println!(
            "  {:>7.2} | {:>8.1}% {:>6.1}% | {:>12.1}% {:>6.1}%",
            t,
            100.0 * sel.iter().filter(|x| x.1).count() as f64 / sel.len().max(1) as f64,
            100.0 * sel.len() as f64 / queries.len() as f64,
            100.0 * sl.iter().filter(|x| x.1).count() as f64 / sl.len().max(1) as f64,
            100.0 * sl.len() as f64 / queries.len() as f64,
        );
    }

    // ---- 2) Grid search + 2-fold CV
    let betas = [0.02f32, 0.04, 0.06, 0.08, 0.1, 0.15, 0.2, 0.3];
    let mut gates = vec![Gate::None];
    for t in &ts[1..] {
        gates.push(Gate::Abs { t: *t });
        for r in [1.0, 1.2, 1.5, 2.0] {
            gates.push(Gate::Top1Conf { t: *t, r });
        }
    }
    for c in [0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9] {
        gates.push(Gate::Cov { c });
        gates.push(Gate::CovSoft { c });
    }
    let mut cfgs = vec![Cfg { gate: Gate::None, beta: 0.0, same_lang: false }];
    for gt in &gates[1..] {
        for b in betas {
            for sl in [false, true] {
                cfgs.push(Cfg { gate: *gt, beta: b, same_lang: sl });
            }
        }
    }
    let family = |c: &Cfg| -> &'static str {
        match (c.gate, c.same_lang) {
            (Gate::None, _) => "dense (không BM25)",
            (Gate::Abs { .. }, false) => "abs: bm25≥T",
            (Gate::Abs { .. }, true) => "abs + cùng ngôn ngữ",
            (Gate::Top1Conf { .. }, false) => "top1 tự tin: bm25≥T & top1/top2≥R",
            (Gate::Top1Conf { .. }, true) => "top1 tự tin + cùng ngôn ngữ",
            (Gate::Cov { .. }, false) => "độ phủ IDF ≥ C",
            (Gate::Cov { .. }, true) => "độ phủ IDF ≥ C + cùng ngôn ngữ",
            (Gate::CovSoft { .. }, false) => "độ phủ IDF mềm",
            (Gate::CovSoft { .. }, true) => "độ phủ IDF mềm + cùng ngôn ngữ",
        }
    };
    let all: Vec<&QData> = data.iter().collect();
    let fold = |k: usize| -> Vec<&QData> { data.iter().filter(|q| q.gold % 2 == k).collect() };
    let (f0, f1) = (fold(0), fold(1));
    let best_in = |pool: &[&Cfg], qs: &[&QData]| -> Cfg {
        *pool
            .iter()
            .max_by(|a, b| {
                let (ea, eb) = (eval(a, qs), eval(b, qs));
                (ea.0, ea.1).partial_cmp(&(eb.0, eb.1)).unwrap()
            })
            .copied()
            .unwrap()
    };
    println!("\n## Kết quả theo họ cổng (R@1 held-out = trung bình 2-fold; full = tối ưu trên toàn bộ, lạc quan)");
    println!("  {:<38} {:>10} {:>9} {:>8}  tham số tốt nhất (full)", "cổng", "held-out", "full R@1", "MRR");
    let mut fams: Vec<&str> = cfgs.iter().map(family).collect();
    fams.dedup();
    let mut seen = HashSet::new();
    for fam in fams {
        if !seen.insert(fam) {
            continue;
        }
        let pool: Vec<&Cfg> = cfgs.iter().filter(|c| family(c) == fam).collect();
        let a = best_in(&pool, &f0);
        let b = best_in(&pool, &f1);
        let held = (eval(&a, &f1).0 * f1.len() as f64 + eval(&b, &f0).0 * f0.len() as f64) / all.len() as f64;
        let full = best_in(&pool, &all);
        let (r1, mrr) = eval(&full, &all);
        println!("  {:<38} {:>9.1}% {:>8.1}% {:>8.3}  {:?} beta={}", fam, 100.0 * held, 100.0 * r1, mrr, full.gate, full.beta);
    }

    // ---- 3) Trần lý thuyết
    let oracle = data
        .iter()
        .filter(|q| {
            let dense_top = q.sigs.iter().enumerate().max_by(|a, b| a.1.cos.partial_cmp(&b.1.cos).unwrap()).unwrap().0;
            dense_top == q.gold || q.sigs[q.gold].bm25_rank == 1
        })
        .count();
    // ---- 4) Nhóm query có định danh (mã lỗi, số hiệu: chữ+số, hoặc số ≥ 3 chữ số)
    let is_ident = |w: &str| {
        let has_d = w.chars().any(|c| c.is_ascii_digit());
        let has_a = w.chars().any(|c| c.is_ascii_alphabetic());
        (has_d && has_a && w.len() <= 8) || (has_d && w.chars().filter(|c| c.is_ascii_digit()).count() >= 3)
    };
    let ident_q: Vec<usize> = queries.iter().enumerate().filter(|(_, q)| q.q.split(|c: char| !c.is_ascii_alphanumeric()).any(is_ident)).map(|(i, _)| i).collect();
    if !ident_q.is_empty() {
        let sub: Vec<&QData> = ident_q.iter().map(|i| &data[*i]).collect();
        let dense_r1 = eval(&cfgs[0], &sub).0;
        let bm_r1 = sub.iter().filter(|q| q.sigs[q.gold].bm25_rank == 1).count() as f64 / sub.len() as f64;
        let best = best_in(&cfgs.iter().collect::<Vec<_>>(), &all);
        println!(
            "\n## Query có định danh ({} câu, vd: {}): dense R@1={:.1}%  BM25 R@1={:.1}%  cổng tốt nhất (full) trên nhóm này={:.1}%",
            sub.len(),
            ident_q.iter().take(3).map(|i| queries[*i].q.as_str()).collect::<Vec<_>>().join(" | "),
            100.0 * dense_r1,
            100.0 * bm_r1,
            100.0 * eval(&best, &sub).0
        );
    }
    println!("\n  Trần (dense top-1 đúng HOẶC BM25 top-1 đúng): {:.1}%", 100.0 * oracle as f64 / all.len() as f64);
    Ok(())
}
