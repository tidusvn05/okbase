//! Splits a markdown body into sections by H2/H3 headings (ignoring fenced code),
//! merges small sections and splits large ones. Ported from the embed-bench spike.

use crate::tokens::estimate_tokens;

/// Sections below this many (estimated) tokens are merged with the next one.
pub const MIN_TOKENS: usize = 150;
/// Sections above this many (estimated) tokens are split by lines.
pub const MAX_TOKENS: usize = 450;

/// A piece of a document body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chunk {
    /// Heading path, such as `Setup > Linux`; empty before the first H2.
    pub heading: String,
    /// The chunk text, trimmed.
    pub text: String,
    /// First line in the body (1-based).
    pub start_line: usize,
    /// Last line in the body (1-based, inclusive).
    pub end_line: usize,
    /// Estimated tokens of `text`.
    pub tokens: usize,
}

struct Section {
    heading: String,
    text: String,
    start: usize,
    end: usize,
}

/// Chunks a markdown body.
pub fn chunk_body(body: &str) -> Vec<Chunk> {
    let mut secs = vec![Section {
        heading: String::new(),
        text: String::new(),
        start: 1,
        end: 0,
    }];
    let mut fence: Option<&str> = None;
    let (mut h2, mut h3) = (String::new(), String::new());
    for (i, line) in body.lines().enumerate() {
        let n = i + 1;
        let trimmed = line.trim_start();
        match fence {
            Some(f) if trimmed.starts_with(f) => fence = None,
            None if trimmed.starts_with("```") => fence = Some("```"),
            None if trimmed.starts_with("~~~") => fence = Some("~~~"),
            _ => {}
        }
        let heading = if fence.is_none() {
            heading_level(line)
        } else {
            None
        };
        if let Some((level, text)) = heading {
            if level == 2 {
                h2 = text;
                h3.clear();
            } else {
                h3 = text;
            }
            let heading = if h3.is_empty() {
                h2.clone()
            } else {
                format!("{h2} > {h3}")
            };
            secs.push(Section {
                heading,
                text: String::new(),
                start: n,
                end: n,
            });
        }
        let last = secs.last_mut().expect("at least one section");
        last.text.push_str(line);
        last.text.push('\n');
        last.end = n;
    }
    secs.retain(|s| !s.text.trim().is_empty());

    let mut out = Vec::new();
    let mut buf: Option<(Section, usize)> = None;
    for sec in secs {
        let t = estimate_tokens(&sec.text);
        if t > MAX_TOKENS {
            if let Some((b, _)) = buf.take() {
                push(&mut out, b);
            }
            split_large(sec, &mut out);
        } else if let Some((b, bt)) = buf
            .as_mut()
            .filter(|(_, bt)| *bt < MIN_TOKENS && *bt + t <= MAX_TOKENS)
        {
            b.text.push_str(&sec.text);
            b.end = sec.end;
            *bt += t;
        } else {
            if let Some((b, _)) = buf.take() {
                push(&mut out, b);
            }
            buf = Some((sec, t));
        }
    }
    if let Some((b, _)) = buf {
        push(&mut out, b);
    }
    out
}

/// `## Heading` → `(2, "Heading")` for H2 and H3 only.
fn heading_level(line: &str) -> Option<(u8, String)> {
    let level = if line.starts_with("### ") {
        3
    } else if line.starts_with("## ") {
        2
    } else {
        return None;
    };
    let text = line[level as usize + 1..]
        .trim()
        .trim_end_matches('#')
        .trim();
    Some((level, text.to_owned()))
}

fn split_large(sec: Section, out: &mut Vec<Chunk>) {
    let mut cur = Section {
        heading: sec.heading.clone(),
        text: String::new(),
        start: sec.start,
        end: sec.start,
    };
    let mut cur_t = 0;
    for (i, piece) in sec.text.split_inclusive('\n').enumerate() {
        let n = sec.start + i;
        let pt = estimate_tokens(piece);
        if cur_t > 0 && cur_t + pt > MAX_TOKENS {
            let next = Section {
                heading: sec.heading.clone(),
                text: String::new(),
                start: n,
                end: n,
            };
            push(out, std::mem::replace(&mut cur, next));
            cur_t = 0;
        }
        if pt > MAX_TOKENS {
            // A single huge line: cut by characters.
            let chars: Vec<char> = piece.chars().collect();
            for part in chars.chunks(MAX_TOKENS * 3) {
                let text: String = part.iter().collect();
                push(
                    out,
                    Section {
                        heading: sec.heading.clone(),
                        text,
                        start: n,
                        end: n,
                    },
                );
            }
            cur.start = n + 1;
            continue;
        }
        cur.text.push_str(piece);
        cur.end = n;
        cur_t += pt;
    }
    push(out, cur);
}

fn push(out: &mut Vec<Chunk>, s: Section) {
    let text = s.text.trim();
    if !text.is_empty() {
        out.push(Chunk {
            heading: s.heading,
            tokens: estimate_tokens(text),
            text: text.to_owned(),
            start_line: s.start,
            end_line: s.end.max(s.start),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(n: usize) -> String {
        vec!["word"; n].join(" ")
    }

    #[test]
    fn sections_merge_and_fences() {
        let body = format!(
            "Intro.\n\n## A\n\n{}\n\n```\n## not a heading\n```\n\n### A1\n\ntiny\n\n## B\n\n{}\n",
            words(200),
            words(200)
        );
        let c = chunk_body(&body);
        let heads: Vec<_> = c.iter().map(|c| c.heading.as_str()).collect();
        // As in the spike, a small section absorbs the next one and keeps its own heading:
        // the intro absorbs A, and the tiny A1 absorbs B.
        assert_eq!(heads, ["", "A > A1"]);
        assert!(c[0].text.contains("## not a heading"));
        assert!(c[1].text.contains("## B"));
        assert_eq!((c[0].start_line, c[1].end_line), (1, body.lines().count()));
    }

    #[test]
    fn splits_large_sections() {
        let body = format!(
            "## Big\n\n{}\n",
            (0..60).map(|_| words(20)).collect::<Vec<_>>().join("\n")
        );
        let c = chunk_body(&body);
        assert!(c.len() >= 3, "{}", c.len());
        assert!(
            c.iter()
                .all(|c| c.tokens <= MAX_TOKENS && c.heading == "Big")
        );
        let long = format!("## L\n{}\n", words(2000));
        assert!(chunk_body(&long).len() > 1);
    }
}
