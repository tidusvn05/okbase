//! Thí nghiệm end-to-end với agent CLI thật.
//! - `e2e-prep`: dựng bundle OKF + index.md từ data/v2, chọn 30 câu khó, tính sẵn top-5 và vector tài liệu.
//! - `mcp`: MCP server (stdio, JSON-RPC) với các tool kb_search / kb_grep / kb_get / kb_list.

use std::{
    collections::HashMap,
    io::{BufRead, Write},
    path::Path,
};

use anyhow::{Context, Result};
use serde_json::{Value, json};

use crate::{Bm25, Doc, Query, Spec};

fn description(text: &str) -> String {
    let end = text.find(['。', '.']).map(|i| i + text[i..].chars().next().unwrap().len_utf8()).unwrap_or(text.len());
    let s: String = text[..end].chars().take(140).collect();
    s.trim().to_string()
}

fn catalog(docs: &[Doc]) -> String {
    let mut s = String::from("# Knowledge catalog (index.md)\n\n");
    for d in docs {
        s.push_str(&format!("- [{}] ({}) {} — {}\n", d.id, d.lang, d.title, description(&d.text)));
    }
    s
}

fn doc_md(d: &Doc) -> String {
    format!(
        "---\ntype: Support Article\ntitle: {}\ndescription: {}\nlang: {}\n---\n# {}\n\n{}\n",
        d.title,
        description(&d.text),
        d.lang,
        d.title,
        d.text
    )
}

pub fn prep(root: &Path, docs: &[Doc], queries: &[Query], spec: &mut Spec) -> Result<()> {
    let out = root.join("e2e");
    let kdir = out.join("bundle/knowledge");
    std::fs::create_dir_all(&kdir)?;
    for d in docs {
        std::fs::write(kdir.join(format!("{}.md", d.id)), doc_md(d))?;
    }
    std::fs::write(out.join("bundle/index.md"), catalog(docs))?;

    let dtexts: Vec<String> = docs.iter().map(|d| (spec.d)(&d.title, &d.text)).collect();
    let dvec = spec.emb.embed(&dtexts, 16)?;
    let qtexts: Vec<String> = queries.iter().map(|q| (spec.q)(&q.q)).collect();
    let qvec = spec.emb.embed(&qtexts, 16)?;
    std::fs::write(out.join("doc_vecs.json"), serde_json::to_string(&dvec)?)?;

    // top-5 cho từng query
    let mut rows = Vec::new();
    for (i, q) in queries.iter().enumerate() {
        let mut sc: Vec<(f32, &str)> = dvec.iter().zip(docs).map(|(v, d)| (crate::cosine(&qvec[i], v), d.id.as_str())).collect();
        sc.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
        let top5: Vec<&str> = sc.iter().take(5).map(|x| x.1).collect();
        rows.push((i, top5[0] == q.gold, top5.contains(&q.gold.as_str()), top5));
    }
    // Chọn 30 câu: 20 câu top-1 sai + 10 câu top-1 đúng, xoay vòng theo ngôn ngữ để cân bằng
    let pick = |want_ok: bool, n: usize| -> Vec<usize> {
        let mut by_lang: HashMap<&str, Vec<usize>> = HashMap::new();
        for r in rows.iter().filter(|r| r.1 == want_ok) {
            by_lang.entry(queries[r.0].lang.as_str()).or_default().push(r.0);
        }
        let mut out = Vec::new();
        let langs = ["vi", "en", "ja"];
        let mut k = 0;
        while out.len() < n && langs.iter().any(|l| by_lang.get(l).is_some_and(|v| k < v.len())) {
            for l in langs {
                if let Some(v) = by_lang.get(l) {
                    // lấy rải đều trong danh sách thay vì lấy đầu danh sách
                    let step = (v.len() / (n / 3 + 1)).max(1);
                    if k * step < v.len() && out.len() < n {
                        out.push(v[k * step]);
                    }
                }
            }
            k += 1;
        }
        out
    };
    let mut sel = pick(false, 20);
    sel.extend(pick(true, 10));
    let items: Vec<Value> = sel
        .iter()
        .map(|&i| {
            let r = &rows[i];
            json!({"qid": i, "q": queries[i].q, "lang": queries[i].lang, "gold": queries[i].gold,
                   "dense_top1_ok": r.1, "gold_in_top5": r.2, "top5": r.3})
        })
        .collect();
    std::fs::write(out.join("queries.json"), serde_json::to_string_pretty(&items)?)?;
    println!(
        "e2e/: bundle {} docs, {} queries ({} top-1 sai, gold trong top-5: {}/{})",
        docs.len(),
        items.len(),
        items.iter().filter(|x| !x["dense_top1_ok"].as_bool().unwrap()).count(),
        items.iter().filter(|x| x["gold_in_top5"].as_bool().unwrap()).count(),
        items.len()
    );
    Ok(())
}

fn tools_list() -> Value {
    json!({"tools": [
        {"name": "kb_search", "description": "Semantic search over the knowledge base (multilingual vi/en/ja; query may be in any language). Returns top documents: id | lang | title | score | snippet.",
         "inputSchema": {"type": "object", "properties": {"query": {"type": "string"}, "limit": {"type": "integer", "default": 5}}, "required": ["query"]}},
        {"name": "kb_grep", "description": "Exact / lexical search: finds documents containing the given string (case-insensitive substring, e.g. an error code 'E2', '429', a product name). Falls back to keyword match.",
         "inputSchema": {"type": "object", "properties": {"pattern": {"type": "string"}, "limit": {"type": "integer", "default": 10}}, "required": ["pattern"]}},
        {"name": "kb_get", "description": "Read the full content of a document by id.",
         "inputSchema": {"type": "object", "properties": {"id": {"type": "string"}}, "required": ["id"]}},
        {"name": "kb_list", "description": "List all documents (catalog: id, lang, title, one-line description).",
         "inputSchema": {"type": "object", "properties": {}}}
    ]})
}

pub fn serve(root: &Path, docs: &[Doc], bm25: &Bm25, mut spec: Spec) -> Result<()> {
    let dvec: Vec<Vec<f32>> = serde_json::from_str(&std::fs::read_to_string(root.join("e2e/doc_vecs.json")).context("chạy e2e-prep trước")?)?;
    let by_id: HashMap<&str, &Doc> = docs.iter().map(|d| (d.id.as_str(), d)).collect();
    let log_path = std::env::var("KB_LOG").ok();
    let snippet = |t: &str| t.chars().take(160).collect::<String>();

    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    for line in stdin.lock().lines() {
        let line = line?;
        let Ok(msg) = serde_json::from_str::<Value>(&line) else { continue };
        let Some(id) = msg.get("id").cloned() else { continue }; // notification
        let method = msg["method"].as_str().unwrap_or("");
        let result: Result<Value> = (|| {
            Ok(match method {
                "initialize" => json!({
                    "protocolVersion": msg["params"]["protocolVersion"].as_str().unwrap_or("2025-06-18"),
                    "capabilities": {"tools": {}},
                    "serverInfo": {"name": "kb", "version": "0.1.0"}
                }),
                "ping" => json!({}),
                "tools/list" => tools_list(),
                "tools/call" => {
                    let name = msg["params"]["name"].as_str().unwrap_or("");
                    let args = &msg["params"]["arguments"];
                    if let Some(p) = &log_path {
                        let mut f = std::fs::OpenOptions::new().create(true).append(true).open(p)?;
                        writeln!(f, "{}", json!({"tool": name, "args": args}))?;
                    }
                    let text = match name {
                        "kb_search" => {
                            let q = args["query"].as_str().unwrap_or("");
                            let limit = args["limit"].as_u64().unwrap_or(5).clamp(1, 10) as usize;
                            let qv = spec.emb.embed(&[(spec.q)(q)], 1)?.remove(0);
                            let mut sc: Vec<(f32, &Doc)> = dvec.iter().zip(docs).map(|(v, d)| (crate::cosine(&qv, v), d)).collect();
                            sc.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
                            sc.iter()
                                .take(limit)
                                .map(|(s, d)| format!("{} | {} | {} | {:.3} | {}", d.id, d.lang, d.title, s, snippet(&d.text)))
                                .collect::<Vec<_>>()
                                .join("\n")
                        }
                        "kb_grep" => {
                            let p = args["pattern"].as_str().unwrap_or("").to_lowercase();
                            let limit = args["limit"].as_u64().unwrap_or(10).clamp(1, 20) as usize;
                            let mut hits: Vec<String> = docs
                                .iter()
                                .filter(|d| format!("{} {}", d.title, d.text).to_lowercase().contains(&p))
                                .take(limit)
                                .map(|d| format!("{} | {} | {} | {}", d.id, d.lang, d.title, snippet(&d.text)))
                                .collect();
                            if hits.is_empty() {
                                hits = bm25
                                    .search(&p, limit)?
                                    .iter()
                                    .map(|id| {
                                        let d = by_id[id.as_str()];
                                        format!("{} | {} | {} | (keyword) {}", d.id, d.lang, d.title, snippet(&d.text))
                                    })
                                    .collect();
                            }
                            if hits.is_empty() { "no match".into() } else { hits.join("\n") }
                        }
                        "kb_get" => {
                            let id = args["id"].as_str().unwrap_or("").trim_start_matches("knowledge/").trim_end_matches(".md");
                            by_id.get(id).map(|d| doc_md(d)).unwrap_or_else(|| format!("not found: {id}"))
                        }
                        "kb_list" => catalog(docs),
                        other => anyhow::bail!("unknown tool {other}"),
                    };
                    json!({"content": [{"type": "text", "text": text}]})
                }
                other => anyhow::bail!("method not found: {other}"),
            })
        })();
        let resp = match result {
            Ok(r) => json!({"jsonrpc": "2.0", "id": id, "result": r}),
            Err(e) => json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32601, "message": e.to_string()}}),
        };
        writeln!(stdout, "{resp}")?;
        stdout.flush()?;
    }
    Ok(())
}
