//! Làm việc trực tiếp trên một bundle OKF (thư mục .md):
//! - `bundle-index <dir>`: chia chunk theo heading, embed (cache theo nội dung), lưu vào <dir>/.qobot/
//! - `bundle-retrieve <dir> <questions.json> <out.json> [k]`: top-k chunk cho từng câu hỏi (để dựng prompt)
//! - `bundle-mcp <dir>`: MCP server (stdio) với kb_search / kb_get / kb_grep / kb_list

use std::{
    collections::{HashMap, hash_map::DefaultHasher},
    hash::{Hash, Hasher},
    io::{BufRead, Read, Write},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokenizers::Tokenizer;

use crate::Spec;

const MIN_TOK: usize = 150;
const MAX_TOK: usize = 450; // model đang giới hạn 512 token (gồm tiền tố title/heading)

#[derive(Serialize, Deserialize, Clone)]
pub struct Chunk {
    pub doc: String,
    pub title: String,
    pub heading: String,
    pub text: String,
}

struct DocFile {
    id: String,
    title: String,
    body: String,
    raw: String,
}

fn walk(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(p) = stack.pop() {
        for e in std::fs::read_dir(&p)?.flatten() {
            let ep = e.path();
            let name = e.file_name().to_string_lossy().to_string();
            if ep.is_dir() {
                if !name.starts_with('.') {
                    stack.push(ep);
                }
            } else if name.ends_with(".md") && name != "index.md" && name != "log.md" {
                out.push(ep);
            }
        }
    }
    out.sort();
    Ok(out)
}

fn read_doc(root: &Path, p: &Path) -> Result<DocFile> {
    let raw = std::fs::read_to_string(p)?;
    let id = p.strip_prefix(root)?.with_extension("").to_string_lossy().to_string();
    let (fm, body) = match raw.strip_prefix("---\n").and_then(|r| r.find("\n---").map(|i| (&r[..i], &r[i + 4..]))) {
        Some((fm, body)) => (fm.to_string(), body.trim_start_matches(['-', '\n']).to_string()),
        None => (String::new(), raw.clone()),
    };
    let title = fm
        .lines()
        .find_map(|l| l.strip_prefix("title:"))
        .map(|t| t.trim().trim_matches('"').to_string())
        .unwrap_or_else(|| id.clone());
    Ok(DocFile { id, title, body, raw })
}

fn tok_len(tok: &Tokenizer, s: &str) -> usize {
    tok.encode(s, false).map(|e| e.len()).unwrap_or(s.len() / 4)
}

/// Chia theo H2/H3 (bỏ qua trong code fence); gộp phần < MIN_TOK; cắt phần > MAX_TOK theo đoạn văn/dòng.
fn chunk_doc(tok: &Tokenizer, d: &DocFile) -> Vec<Chunk> {
    let mut secs: Vec<(String, String)> = vec![(String::new(), String::new())];
    let mut fence = false;
    let (mut h2, mut h3) = (String::new(), String::new());
    for line in d.body.lines() {
        if line.trim_start().starts_with("```") {
            fence = !fence;
        }
        if !fence && (line.starts_with("## ") || line.starts_with("### ")) {
            if line.starts_with("## ") {
                h2 = line[3..].trim().to_string();
                h3.clear();
            } else {
                h3 = line[4..].trim().to_string();
            }
            let heading = if h3.is_empty() { h2.clone() } else { format!("{h2} > {h3}") };
            secs.push((heading, String::new()));
        }
        let last = secs.last_mut().unwrap();
        last.1.push_str(line);
        last.1.push('\n');
    }
    secs.retain(|s| !s.1.trim().is_empty());
    let mk = |heading: &str, text: &str| Chunk { doc: d.id.clone(), title: d.title.clone(), heading: heading.to_string(), text: text.trim().to_string() };
    let mut out: Vec<Chunk> = Vec::new();
    let mut buf: Option<(String, String, usize)> = None;
    for (heading, text) in secs {
        let t = tok_len(tok, &text);
        if t > MAX_TOK {
            if let Some((h, b, _)) = buf.take() {
                out.push(mk(&h, &b));
            }
            let mut cur = String::new();
            let mut cur_t = 0;
            for piece in text.split_inclusive('\n') {
                let pt = tok_len(tok, piece);
                if cur_t > 0 && cur_t + pt > MAX_TOK {
                    out.push(mk(&heading, &cur));
                    cur.clear();
                    cur_t = 0;
                }
                // dòng quá dài: cắt cứng theo ký tự
                if pt > MAX_TOK {
                    let chars: Vec<char> = piece.chars().collect();
                    for c in chars.chunks(MAX_TOK * 3) {
                        out.push(mk(&heading, &c.iter().collect::<String>()));
                    }
                    continue;
                }
                cur.push_str(piece);
                cur_t += pt;
            }
            if !cur.trim().is_empty() {
                out.push(mk(&heading, &cur));
            }
        } else if let Some((h, b, bt)) = buf.as_mut().filter(|(_, _, bt)| *bt < MIN_TOK && *bt + t <= MAX_TOK) {
            let _ = h;
            b.push_str(&text);
            *bt += t;
        } else {
            if let Some((h, b, _)) = buf.take() {
                out.push(mk(&h, &b));
            }
            buf = Some((heading, text, t));
        }
    }
    if let Some((h, b, _)) = buf.take() {
        out.push(mk(&h, &b));
    }
    out.retain(|c| !c.text.is_empty());
    out
}

fn embed_text(c: &Chunk) -> String {
    let h = if c.heading.is_empty() { c.title.clone() } else { format!("{} > {}", c.title, c.heading) };
    format!("title: {h} | text: {}", c.text)
}

fn tokenizer(root: &Path) -> Result<Tokenizer> {
    let snaps = root.join(".fastembed_cache/models--onnx-community--embeddinggemma-300m-ONNX/snapshots");
    let snap = std::fs::read_dir(&snaps)?.flatten().next().context("no gemma snapshot")?.path();
    Tokenizer::from_file(snap.join("tokenizer.json")).map_err(anyhow::Error::msg)
}

fn write_vecs(p: &Path, v: &[Vec<f32>]) -> Result<()> {
    let mut f = std::io::BufWriter::new(std::fs::File::create(p)?);
    for x in v {
        for y in x {
            f.write_all(&y.to_le_bytes())?;
        }
    }
    Ok(())
}

fn read_vecs(p: &Path, dim: usize) -> Result<Vec<Vec<f32>>> {
    let mut b = Vec::new();
    std::fs::File::open(p)?.read_to_end(&mut b)?;
    Ok(b.chunks(dim * 4).map(|c| c.chunks(4).map(|x| f32::from_le_bytes([x[0], x[1], x[2], x[3]])).collect()).collect())
}

pub fn index(root: &Path, dir: &Path, spec: &mut Spec) -> Result<()> {
    let tok = tokenizer(root)?;
    let files = walk(dir)?;
    let mut chunks = Vec::new();
    for f in &files {
        chunks.extend(chunk_doc(&tok, &read_doc(dir, f)?));
    }
    // cache embedding theo nội dung (dùng chung giữa các bundle) — mô phỏng index tăng dần
    let cache_dir = root.join(".embcache");
    std::fs::create_dir_all(&cache_dir)?;
    let cache_idx = cache_dir.join("index.json");
    let mut cache: HashMap<String, usize> = std::fs::read_to_string(&cache_idx).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
    let mut cache_vecs = if cache_dir.join("vecs.bin").exists() { read_vecs(&cache_dir.join("vecs.bin"), 768)? } else { vec![] };
    let key = |c: &Chunk| {
        let mut h = DefaultHasher::new();
        embed_text(c).hash(&mut h);
        format!("{:016x}", h.finish())
    };
    let mut todo: Vec<&Chunk> = chunks.iter().filter(|c| !cache.contains_key(&key(c))).collect();
    todo.sort_by_key(|c| embed_text(c).len()); // gom chunk cùng độ dài để giảm padding trong batch
    let t = std::time::Instant::now();
    eprintln!("{}: {} files, {} chunks, {} need embedding", dir.display(), files.len(), chunks.len(), todo.len());
    for (n, batch) in todo.chunks(64).enumerate() {
        let texts: Vec<String> = batch.iter().map(|c| embed_text(c)).collect();
        let vs = spec.emb.embed(&texts, 16)?;
        for (c, v) in batch.iter().zip(vs) {
            cache.insert(key(c), cache_vecs.len());
            cache_vecs.push(v);
        }
        if n % 10 == 9 {
            eprintln!("  embedded {}/{} ({:.1} chunk/s)", (n + 1) * 64, todo.len(), ((n + 1) * 64) as f64 / t.elapsed().as_secs_f64());
            write_vecs(&cache_dir.join("vecs.bin"), &cache_vecs)?;
            std::fs::write(&cache_idx, serde_json::to_string(&cache)?)?;
        }
    }
    write_vecs(&cache_dir.join("vecs.bin"), &cache_vecs)?;
    std::fs::write(&cache_idx, serde_json::to_string(&cache)?)?;
    let vecs: Vec<Vec<f32>> = chunks.iter().map(|c| cache_vecs[cache[&key(c)]].clone()).collect();
    let q = dir.join(".qobot");
    std::fs::create_dir_all(&q)?;
    std::fs::write(q.join("chunks.json"), serde_json::to_string(&chunks)?)?;
    write_vecs(&q.join("vecs.bin"), &vecs)?;
    let toks: usize = chunks.iter().map(|c| tok_len(&tok, &c.text)).sum();
    println!(
        "{}: files={} chunks={} tokens={} embedded_now={} in {:.0}s",
        dir.display(),
        files.len(),
        chunks.len(),
        toks,
        todo.len(),
        t.elapsed().as_secs_f64()
    );
    Ok(())
}

struct Index {
    chunks: Vec<Chunk>,
    vecs: Vec<Vec<f32>>,
}

fn load_index(dir: &Path) -> Result<Index> {
    let q = dir.join(".qobot");
    let chunks: Vec<Chunk> = serde_json::from_str(&std::fs::read_to_string(q.join("chunks.json")).context("chạy bundle-index trước")?)?;
    let vecs = read_vecs(&q.join("vecs.bin"), 768)?;
    Ok(Index { chunks, vecs })
}

/// Top chunk theo cosine, tối đa `per_doc` chunk mỗi tài liệu
fn search(idx: &Index, spec: &mut Spec, q: &str, k: usize, per_doc: usize) -> Result<Vec<(f32, usize)>> {
    let qv = spec.emb.embed(&[(spec.q)(q)], 1)?.remove(0);
    let mut sc: Vec<(f32, usize)> = idx.vecs.iter().enumerate().map(|(i, v)| (crate::cosine(&qv, v), i)).collect();
    sc.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
    let mut per: HashMap<&str, usize> = HashMap::new();
    Ok(sc.into_iter()
        .filter(|(_, i)| {
            let n = per.entry(idx.chunks[*i].doc.as_str()).or_default();
            *n += 1;
            *n <= per_doc
        })
        .take(k)
        .collect())
}

pub fn retrieve(dir: &Path, questions: &Path, out: &Path, k: usize, spec: &mut Spec) -> Result<()> {
    let idx = load_index(dir)?;
    let qs: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(questions)?)?;
    let mut res = Vec::new();
    let mut hit1 = 0;
    let mut hitk = 0;
    for q in &qs {
        let r = search(&idx, spec, q["q"].as_str().unwrap_or(""), k, 2)?;
        let gold = q["gold"].as_str().unwrap_or("");
        let docs: Vec<&str> = r.iter().map(|(_, i)| idx.chunks[*i].doc.as_str()).collect();
        if docs.first() == Some(&gold) {
            hit1 += 1;
        }
        if docs.contains(&gold) {
            hitk += 1;
        }
        res.push(json!({"qid": q["qid"], "chunks": r.iter().map(|(s, i)| {
            let c = &idx.chunks[*i];
            json!({"doc": c.doc, "title": c.title, "heading": c.heading, "text": c.text, "score": s})
        }).collect::<Vec<_>>()}));
    }
    std::fs::write(out, serde_json::to_string_pretty(&res)?)?;
    println!("{}: gold doc@1={}/{} gold doc in top-{k} chunks={}/{}", dir.display(), hit1, qs.len(), hitk, qs.len());
    Ok(())
}

fn tools_list(with_search: bool) -> Value {
    let mut tools = vec![
        json!({"name": "kb_get", "description": "Read a document by id (path without .md, e.g. 'gateway/configuration'). Optional `section`: a heading text to return only that section. Long documents are truncated — use `section` then.",
               "inputSchema": {"type": "object", "properties": {"id": {"type": "string"}, "section": {"type": "string"}}, "required": ["id"]}}),
        json!({"name": "kb_grep", "description": "Lexical search over all documents (including frontmatter title/description). `pattern` is a regex, case-insensitive and accent-insensitive (e.g. 'dmScope|dm_scope', 'preload.*false', 'heartbeat'); use alternation to try synonyms. Documents are in English: translate key terms first. Options: `path` (glob or prefix, e.g. 'gateway/**', 'channels/'), `context` (lines around each match, 0-5), `files_only` (list matching documents ranked by match count — good first step), `limit` (max output lines).",
               "inputSchema": {"type": "object", "properties": {"pattern": {"type": "string"}, "path": {"type": "string"}, "context": {"type": "integer", "default": 1}, "files_only": {"type": "boolean", "default": false}, "limit": {"type": "integer", "default": 40}}, "required": ["pattern"]}}),
        json!({"name": "kb_list", "description": "Show the index.md of a directory in the knowledge bundle (subdirectories and documents with one-line descriptions). Use dir='.' for the root.",
               "inputSchema": {"type": "object", "properties": {"dir": {"type": "string", "default": "."}}}}),
    ];
    if with_search {
        tools.insert(0, json!({"name": "kb_search", "description": "Semantic search over all document sections (multilingual: the query may be in any language; documents are English). Returns `doc_id # heading | score | snippet`.",
               "inputSchema": {"type": "object", "properties": {"query": {"type": "string"}, "limit": {"type": "integer", "default": 8}}, "required": ["query"]}}));
    }
    json!({ "tools": tools })
}

pub fn serve(dir: &Path, mut spec: Option<Spec>) -> Result<()> {
    let idx = if spec.is_some() { Some(load_index(dir)?) } else { None };
    let log_path = std::env::var("KB_LOG").ok();
    let files = walk(dir)?;
    let docs: Vec<DocFile> = files.iter().map(|f| read_doc(dir, f)).collect::<Result<_>>()?;
    let by_id: HashMap<&str, &DocFile> = docs.iter().map(|d| (d.id.as_str(), d)).collect();
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    for line in stdin.lock().lines() {
        let line = line?;
        let Ok(msg) = serde_json::from_str::<Value>(&line) else { continue };
        let Some(id) = msg.get("id").cloned() else { continue };
        let method = msg["method"].as_str().unwrap_or("");
        let result: Result<Value> = (|| {
            Ok(match method {
                "initialize" => json!({"protocolVersion": msg["params"]["protocolVersion"].as_str().unwrap_or("2025-06-18"),
                                        "capabilities": {"tools": {}}, "serverInfo": {"name": "kb", "version": "0.2.0"}}),
                "ping" => json!({}),
                "tools/list" => tools_list(spec.is_some()),
                "tools/call" => {
                    let name = msg["params"]["name"].as_str().unwrap_or("");
                    let args = &msg["params"]["arguments"];
                    if let Some(p) = &log_path {
                        let mut f = std::fs::OpenOptions::new().create(true).append(true).open(p)?;
                        writeln!(f, "{}", json!({"tool": name, "args": args}))?;
                    }
                    let text = match name {
                        "kb_search" => {
                            let (Some(sp), Some(ix)) = (spec.as_mut(), idx.as_ref()) else { anyhow::bail!("kb_search disabled") };
                            let k = args["limit"].as_u64().unwrap_or(8).clamp(1, 20) as usize;
                            search(ix, sp, args["query"].as_str().unwrap_or(""), k, 2)?
                                .iter()
                                .map(|(s, i)| {
                                    let c = &ix.chunks[*i];
                                    let snip: String = c.text.chars().take(220).collect::<String>().replace('\n', " ");
                                    format!("{} # {} | {:.3} | {}", c.doc, c.heading, s, snip)
                                })
                                .collect::<Vec<_>>()
                                .join("\n")
                        }
                        "kb_get" => {
                            let id = args["id"].as_str().unwrap_or("").trim_start_matches('/').trim_end_matches(".md");
                            match by_id.get(id) {
                                None => format!("not found: {id}"),
                                Some(d) => {
                                    let body = match args["section"].as_str().filter(|s| !s.is_empty()) {
                                        Some(sec) => {
                                            let sec_l = sec.to_lowercase();
                                            let mut out = String::new();
                                            let mut on = false;
                                            let mut level = 0;
                                            for l in d.body.lines() {
                                                let hl = l.chars().take_while(|c| *c == '#').count();
                                                if hl > 0 && l.chars().nth(hl) == Some(' ') {
                                                    if on && hl <= level {
                                                        break;
                                                    }
                                                    if !on && l.to_lowercase().contains(&sec_l) {
                                                        on = true;
                                                        level = hl;
                                                    }
                                                }
                                                if on {
                                                    out.push_str(l);
                                                    out.push('\n');
                                                }
                                            }
                                            if out.is_empty() { format!("section not found; headings: {}", d.body.lines().filter(|l| l.starts_with("## ")).collect::<Vec<_>>().join(" | ")) } else { out }
                                        }
                                        None => d.body.clone(),
                                    };
                                    let max = 16000;
                                    if body.len() > max {
                                        let cut: String = body.chars().take(max).collect();
                                        format!("# {}\n{}\n\n[truncated at {max} chars; request a `section`. Headings: {}]", d.title, cut,
                                                d.body.lines().filter(|l| l.starts_with("## ")).collect::<Vec<_>>().join(" | "))
                                    } else {
                                        format!("# {}\n{}", d.title, body)
                                    }
                                }
                            }
                        }
                        "kb_grep" => grep(&docs, args)?,
                        "kb_list" => {
                            let d = args["dir"].as_str().unwrap_or(".").trim_matches('/');
                            let d = if d.is_empty() { "." } else { d };
                            std::fs::read_to_string(dir.join(d).join("index.md")).unwrap_or_else(|_| format!("no index for {d}"))
                        }
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

/// Chuẩn hoá để so khớp: NFKC → bỏ dấu Latin (tiếng Việt) → lowercase
fn fold(s: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    s.nfkc()
        .collect::<String>()
        .nfd()
        .filter(|c| !('\u{0300}'..='\u{036f}').contains(c))
        .map(|c| if c == 'đ' || c == 'Đ' { 'd' } else { c })
        .collect::<String>()
        .to_lowercase()
}

fn glob_re(g: &str) -> Result<regex::Regex> {
    let g = g.trim_start_matches('/');
    if !g.contains('*') {
        return Ok(regex::Regex::new(&format!("^{}", regex::escape(g)))?);
    }
    let mut re = String::from("^");
    let mut it = g.chars().peekable();
    while let Some(c) = it.next() {
        if c == '*' {
            if it.peek() == Some(&'*') {
                it.next();
                re.push_str(".*");
            } else {
                re.push_str("[^/]*");
            }
        } else {
            re.push_str(&regex::escape(&c.to_string()));
        }
    }
    Ok(regex::Regex::new(&re)?)
}

fn grep(docs: &[DocFile], args: &Value) -> Result<String> {
    let pat = fold(args["pattern"].as_str().unwrap_or(""));
    if pat.trim().is_empty() {
        return Ok("empty pattern".into());
    }
    let re = regex::Regex::new(&format!("(?i){pat}")).or_else(|_| regex::Regex::new(&format!("(?i){}", regex::escape(&pat))))?;
    let path_re = args["path"].as_str().filter(|p| !p.is_empty() && *p != ".").map(glob_re).transpose()?;
    let ctx = args["context"].as_u64().unwrap_or(1).min(5) as usize;
    let files_only = args["files_only"].as_bool().unwrap_or(false);
    let limit = args["limit"].as_u64().unwrap_or(40).clamp(1, 200) as usize;
    let mut per_doc: Vec<(&DocFile, Vec<usize>, Vec<&str>)> = Vec::new();
    let mut total = 0;
    for d in docs {
        if path_re.as_ref().is_some_and(|r| !r.is_match(&d.id)) {
            continue;
        }
        let lines: Vec<&str> = d.raw.lines().collect();
        let hits: Vec<usize> = lines.iter().enumerate().filter(|(_, l)| re.is_match(&fold(l))).map(|(i, _)| i).collect();
        if !hits.is_empty() {
            total += hits.len();
            per_doc.push((d, hits, lines));
        }
    }
    if per_doc.is_empty() {
        return Ok("no match (try synonyms with alternation a|b, English terms, or a shorter pattern)".into());
    }
    per_doc.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then(a.0.id.cmp(&b.0.id)));
    let mut out = Vec::new();
    if files_only {
        for (d, hits, _) in per_doc.iter().take(limit) {
            out.push(format!("{} ({} matches) — {}", d.id, hits.len(), d.title));
        }
    } else {
        'outer: for (d, hits, lines) in &per_doc {
            out.push(format!("## {} — {} ({} matches)", d.id, d.title, hits.len()));
            let mut last_end = 0usize;
            for &h in hits {
                let (a, b) = (h.saturating_sub(ctx).max(last_end), (h + ctx + 1).min(lines.len()));
                if a > last_end && last_end > 0 {
                    out.push("  --".into());
                }
                for (i, l) in lines.iter().enumerate().take(b).skip(a) {
                    out.push(format!("  {}{}: {}", i + 1, if i == h { ">" } else { " " }, l.chars().take(220).collect::<String>()));
                    if out.len() >= limit {
                        break 'outer;
                    }
                }
                last_end = b;
            }
        }
    }
    Ok(format!("{}\n({} matching lines in {} documents{})", out.join("\n"), total, per_doc.len(), if out.len() >= limit { "; output truncated, narrow with `path` or use files_only" } else { "" }))
}
