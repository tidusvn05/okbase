//! `get`: read a document, a section of it, or a line range, within a token budget.

use okfkit_analyze::fold;
use serde::{Deserialize, Serialize};

use crate::{Error, Scope};

/// A get request.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct GetRequest {
    /// Concept ID (`gateway/configuration`); a leading `/` or trailing `.md` is accepted.
    pub id: String,
    /// Heading text (case- and accent-insensitive substring): return only that section.
    pub section: Option<String>,
    /// Line range `start-end` (1-based, inclusive, counted in the body).
    pub lines: Option<String>,
    /// Maximum tokens returned (estimated); default 4000.
    pub max_tokens: Option<usize>,
}

/// Default token budget for `get`.
pub const DEFAULT_GET_TOKENS: usize = 4000;

/// The result of a get.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GetResult {
    /// Concept ID.
    pub id: String,
    /// Bundle-relative path.
    pub path: String,
    /// Title.
    pub title: String,
    /// Parsed frontmatter.
    pub frontmatter: serde_json::Value,
    /// The returned text (body, section or lines).
    pub content: String,
    /// The matched section heading, when `section` was given.
    pub section: Option<String>,
    /// Whether `content` was cut to the token budget.
    pub truncated: bool,
    /// H1/H2 headings of the document, when the content was cut or the section was not found.
    pub headings: Vec<String>,
    /// Estimated tokens of the whole document.
    pub tokens: usize,
}

pub(crate) fn get(
    index: &okfkit_index::Index,
    req: &GetRequest,
    scope: &Scope,
) -> Result<GetResult, Error> {
    let id = normalize_id(&req.id);
    let conn = index.connection();
    let row = conn
        .query_row(
            "SELECT path, title, frontmatter, body, tokens FROM docs WHERE id = ?1",
            [&id],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, i64>(4)?,
                ))
            },
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Error::NotFound(id.clone()),
            e => e.into(),
        })?;
    let (path, title, fm, body, tokens) = row;
    let frontmatter: serde_json::Value = serde_json::from_str(&fm).unwrap_or_default();
    let empty = serde_json::Map::new();
    if !scope.permits(&id, frontmatter.as_object().unwrap_or(&empty)) {
        return Err(Error::NotFound(id));
    }
    let h2: Vec<String> = headings(&body)
        .into_iter()
        .filter(|(l, _)| *l <= 2)
        .map(|(_, t)| t)
        .collect();
    let mut section = None;
    let mut content = body.clone();
    if let Some(sec) = req.section.as_deref().filter(|s| !s.trim().is_empty()) {
        match find_section(&body, sec) {
            Some((heading, text)) => {
                section = Some(heading);
                content = text;
            }
            None => {
                return Ok(GetResult {
                    id,
                    path,
                    title,
                    frontmatter,
                    content: String::new(),
                    section: None,
                    truncated: false,
                    headings: h2,
                    tokens: usize::try_from(tokens).unwrap_or(0),
                });
            }
        }
    } else if let Some(range) = req.lines.as_deref() {
        let (a, b) = parse_range(range)?;
        content = body
            .lines()
            .skip(a - 1)
            .take(b + 1 - a)
            .collect::<Vec<_>>()
            .join("\n");
    }
    let budget = req.max_tokens.unwrap_or(DEFAULT_GET_TOKENS).max(100);
    let (content, truncated) = cut_to_tokens(&content, budget);
    Ok(GetResult {
        id,
        path,
        title,
        frontmatter,
        content,
        section,
        truncated,
        headings: if truncated { h2 } else { Vec::new() },
        tokens: usize::try_from(tokens).unwrap_or(0),
    })
}

pub(crate) fn normalize_id(id: &str) -> String {
    let id = id.trim().trim_start_matches("./").trim_start_matches('/');
    id.strip_suffix(".md").unwrap_or(id).to_owned()
}

fn parse_range(s: &str) -> Result<(usize, usize), Error> {
    let bad = || Error::InvalidArgument(format!("lines must look like `10-40`, got {s:?}"));
    let (a, b) = s.split_once('-').unwrap_or((s, s));
    let a: usize = a.trim().parse().map_err(|_| bad())?;
    let b: usize = if b.trim().is_empty() {
        usize::MAX / 2
    } else {
        b.trim().parse().map_err(|_| bad())?
    };
    if a == 0 || b < a {
        Err(bad())
    } else {
        Ok((a, b))
    }
}

/// ATX headings outside fenced code: (level, text).
pub(crate) fn headings(body: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut fenced = false;
    for line in body.lines() {
        let t = line.trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            fenced = !fenced;
        }
        if !fenced && let Some(h) = heading(line) {
            out.push(h);
        }
    }
    out
}

fn heading(line: &str) -> Option<(usize, String)> {
    let level = line.chars().take_while(|c| *c == '#').count();
    (1..=6).contains(&level).then_some(())?;
    let rest = line[level..].strip_prefix(' ')?;
    Some((level, rest.trim().trim_end_matches('#').trim().to_owned()))
}

/// The first section whose heading contains `wanted`, up to the next heading of the same or higher level.
fn find_section(body: &str, wanted: &str) -> Option<(String, String)> {
    let wanted = fold(wanted.trim().trim_start_matches('#').trim());
    let mut out = String::new();
    let mut on: Option<(usize, String)> = None;
    let mut fenced = false;
    for line in body.lines() {
        let t = line.trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            fenced = !fenced;
        }
        if !fenced && let Some((level, text)) = heading(line) {
            match &on {
                Some((l, _)) if level <= *l => break,
                None if fold(&text).contains(&wanted) => on = Some((level, text)),
                _ => {}
            }
        }
        if on.is_some() {
            out.push_str(line);
            out.push('\n');
        }
    }
    on.map(|(_, h)| (h, out))
}

/// Cuts `text` at a line boundary so its estimate fits `budget`; a single long line is cut by characters.
fn cut_to_tokens(text: &str, budget: usize) -> (String, bool) {
    if okfkit_index::estimate_tokens(text) <= budget {
        return (text.to_owned(), false);
    }
    let mut out = String::new();
    let mut used = 0;
    for line in text.split_inclusive('\n') {
        let t = okfkit_index::estimate_tokens(line);
        if used + t > budget {
            if out.is_empty() {
                out = line.chars().take(budget * 3).collect();
            }
            break;
        }
        out.push_str(line);
        used += t;
    }
    (out, true)
}

impl GetResult {
    /// The text form used by the MCP tool (as in the spike: `# title` then the content).
    pub fn to_text(&self) -> String {
        if self.content.is_empty() && !self.headings.is_empty() {
            return format!(
                "section not found in {}; headings: {}",
                self.id,
                self.headings.join(" | ")
            );
        }
        let mut s = format!("# {}\n{}", self.title, self.content);
        if self.truncated {
            s.push_str(&format!(
                "\n\n[truncated at ~{} tokens of {}; request a `section`. Headings: {}]",
                okfkit_index::estimate_tokens(&self.content),
                self.tokens,
                self.headings.join(" | ")
            ));
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BODY: &str = "# T\n\nintro\n\n## Setup\n\ntext\n\n### Linux\n\nlinux\n\n```\n## not\n```\n\n## Đổi trả\n\nrefunds\n";

    #[test]
    fn sections() {
        let (h, t) = find_section(BODY, "setup").unwrap();
        assert_eq!(h, "Setup");
        assert!(t.contains("linux") && t.contains("## not") && !t.contains("refunds"));
        assert_eq!(
            find_section(BODY, "doi tra").unwrap().1,
            "## Đổi trả\n\nrefunds\n"
        );
        assert!(find_section(BODY, "missing").is_none());
        let h2: Vec<_> = headings(BODY)
            .into_iter()
            .filter(|h| h.0 == 2)
            .map(|h| h.1)
            .collect();
        assert_eq!(h2, ["Setup", "Đổi trả"]);
    }

    #[test]
    fn ranges_and_cuts() {
        assert_eq!(parse_range("3-5").unwrap(), (3, 5));
        assert_eq!(parse_range("7").unwrap(), (7, 7));
        assert!(parse_range("5-3").is_err() && parse_range("x").is_err());
        let text = "word ".repeat(1000);
        let (cut, t) = cut_to_tokens(&text, 100);
        assert!(t && cut.len() < text.len());
        assert_eq!(normalize_id("/a/b.md"), "a/b");
    }
}
