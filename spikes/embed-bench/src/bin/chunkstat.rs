//! Đếm số chunk theo quy tắc thiết kế (§5.5): tách theo H2/H3, gộp phần < 200 token, cắt phần > 600 token theo đoạn.
//! Usage: chunkstat <tokenizer.json> <dir|docs.json>...

use anyhow::Result;
use tokenizers::Tokenizer;

const MIN: usize = 200;
const MAX: usize = 600;

fn strip_frontmatter(s: &str) -> &str {
    if let Some(rest) = s.strip_prefix("---\n") {
        if let Some(i) = rest.find("\n---") {
            return rest[i + 4..].trim_start_matches(['-', '\n']);
        }
    }
    s
}

fn sections(body: &str) -> Vec<String> {
    let mut out = vec![String::new()];
    let mut fence = false;
    for line in body.lines() {
        if line.trim_start().starts_with("```") {
            fence = !fence;
        }
        if !fence && (line.starts_with("## ") || line.starts_with("### ")) && !out.last().unwrap().trim().is_empty() {
            out.push(String::new());
        }
        let last = out.last_mut().unwrap();
        last.push_str(line);
        last.push('\n');
    }
    out.retain(|s| !s.trim().is_empty());
    out
}

fn chunk_count(tok: &Tokenizer, body: &str) -> Result<(usize, usize)> {
    let n = |s: &str| -> Result<usize> { Ok(tok.encode(s, false).map_err(anyhow::Error::msg)?.len()) };
    let total = n(body)?;
    let mut chunks = 0;
    let mut buf = 0usize;
    for sec in sections(body) {
        let t = n(&sec)?;
        if t > MAX {
            if buf > 0 {
                chunks += 1;
                buf = 0;
            }
            // cắt theo đoạn văn
            let mut cur = 0usize;
            for para in sec.split("\n\n") {
                let p = n(para)?;
                if cur > 0 && cur + p > MAX {
                    chunks += 1;
                    cur = 0;
                }
                cur += p;
                while cur > MAX {
                    chunks += 1;
                    cur -= MAX;
                }
            }
            if cur > 0 {
                chunks += 1;
            }
        } else if buf > 0 && buf < MIN && buf + t <= MAX {
            buf += t;
        } else {
            if buf > 0 {
                chunks += 1;
            }
            buf = t;
        }
    }
    if buf > 0 {
        chunks += 1;
    }
    Ok((total, chunks.max(1)))
}

fn pct(v: &mut [usize], p: f64) -> usize {
    v.sort();
    v[((v.len() as f64 - 1.0) * p).round() as usize]
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let tok = Tokenizer::from_file(&args[0]).map_err(anyhow::Error::msg)?;
    println!("{:<34} {:>6} {:>9} {:>8} {:>8} {:>7} {:>9} {:>9} {:>12}", "corpus", "files", "tok/file", "p50", "p90", "chunks", "ch/file", "ch p90", "files/1k ch");
    for path in &args[1..] {
        let mut bodies: Vec<String> = Vec::new();
        if path.ends_with(".json") {
            let v: Vec<serde_json::Value> = serde_json::from_str(&std::fs::read_to_string(path)?)?;
            let mut per: std::collections::BTreeMap<String, (usize, usize, usize)> = Default::default();
            for d in &v {
                let text = d["text"].as_str().unwrap_or("");
                let e = per.entry(d["lang"].as_str().unwrap_or("?").to_string()).or_default();
                e.0 += tok.encode(text, false).map_err(anyhow::Error::msg)?.len();
                e.1 += text.split_whitespace().count();
                e.2 += text.chars().filter(|c| !c.is_whitespace()).count();
            }
            for (l, (t, w, c)) in per {
                eprintln!("  [{l}] token/word={:.2} token/char={:.2}", t as f64 / w as f64, t as f64 / c as f64);
            }
            for d in v {
                bodies.push(format!("# {}\n\n{}", d["title"].as_str().unwrap_or(""), d["text"].as_str().unwrap_or("")));
            }
        } else {
            let mut stack = vec![std::path::PathBuf::from(path)];
            while let Some(p) = stack.pop() {
                for e in std::fs::read_dir(&p)?.flatten() {
                    let ep = e.path();
                    if ep.is_dir() {
                        stack.push(ep);
                    } else if ep.extension().is_some_and(|x| x == "md") {
                        let name = ep.file_name().unwrap().to_string_lossy();
                        if name == "index.md" || name == "log.md" {
                            continue; // file dành riêng của OKF
                        }
                        bodies.push(strip_frontmatter(&std::fs::read_to_string(&ep)?).to_string());
                    }
                }
            }
        }
        let mut toks = Vec::new();
        let mut chs = Vec::new();
        for b in &bodies {
            let (t, c) = chunk_count(&tok, b)?;
            toks.push(t);
            chs.push(c);
        }
        let files = bodies.len();
        let total_ch: usize = chs.iter().sum();
        println!(
            "{:<34} {:>6} {:>9} {:>8} {:>8} {:>7} {:>9.1} {:>9} {:>12.0}",
            path.trim_start_matches("./").chars().rev().take(34).collect::<String>().chars().rev().collect::<String>(),
            files,
            toks.iter().sum::<usize>() / files,
            pct(&mut toks.clone(), 0.5),
            pct(&mut toks, 0.9),
            total_ch,
            total_ch as f64 / files as f64,
            pct(&mut chs, 0.9),
            1000.0 * files as f64 / total_ch as f64
        );
    }
    Ok(())
}
