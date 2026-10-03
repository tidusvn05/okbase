//! `grep` v2 from the okf-scale spike: regex over whole files (frontmatter
//! included), case- and accent-insensitive, with path globs, context lines,
//! a files-only mode and a hint when nothing matches.

use std::collections::HashSet;
use std::fmt::Write as _;

use okbase_analyze::fold;
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::query::{Filter, filter_docs};
use crate::scope::PathGlob;
use crate::{Error, Scope};

/// Longest line shown, in characters.
const MAX_LINE_CHARS: usize = 220;

/// A grep request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GrepRequest {
    /// Regular expression, matched case- and accent-insensitively. An invalid
    /// regex is searched as a literal string.
    pub pattern: String,
    /// Path glob or prefix (`gateway/**`, `channels/`).
    pub path: Option<String>,
    /// Lines of context around each match (0–5).
    pub context: usize,
    /// List matching documents ranked by match count instead of lines.
    pub files_only: bool,
    /// Maximum output lines (1–200).
    pub limit: usize,
    /// Metadata filter (same fields as `query`).
    pub filter: Option<Filter>,
}

impl Default for GrepRequest {
    fn default() -> Self {
        GrepRequest {
            pattern: String::new(),
            path: None,
            context: 1,
            files_only: false,
            limit: 40,
            filter: None,
        }
    }
}

/// A line of a grep result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GrepLine {
    /// 1-based line number in the file (frontmatter included).
    pub line: usize,
    /// The line, cut at 220 characters.
    pub text: String,
    /// Whether this line matches (otherwise it is context).
    pub hit: bool,
}

/// Matches in one document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GrepDoc {
    /// Concept ID.
    pub id: String,
    /// Title.
    pub title: String,
    /// Number of matching lines.
    pub matches: usize,
    /// Shown lines (empty in files-only mode). `None` entries in the text form are `--` gaps.
    pub lines: Vec<GrepLine>,
}

/// The result of a grep.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GrepResult {
    /// Documents with matches, most matches first (only those shown).
    pub docs: Vec<GrepDoc>,
    /// Matching lines in all documents.
    pub total_lines: usize,
    /// Documents with at least one match.
    pub total_docs: usize,
    /// Whether output was cut at `limit`.
    pub truncated: bool,
    /// Whether the pattern was searched as a literal because it is not a valid regex.
    pub literal: bool,
    /// Advice when there is no match or the output was cut.
    pub hint: Option<String>,
}

pub(crate) fn grep(
    index: &okbase_index::Index,
    req: &GrepRequest,
    scope: &Scope,
) -> Result<GrepResult, Error> {
    let pattern = fold(req.pattern.trim());
    if pattern.is_empty() {
        return Err(Error::InvalidArgument("empty pattern".into()));
    }
    let (re, literal) = match Regex::new(&format!("(?im){pattern}")) {
        Ok(re) => (re, false),
        Err(_) => (
            Regex::new(&format!("(?im){}", regex::escape(&pattern))).expect("escaped literal"),
            true,
        ),
    };
    let path = req
        .path
        .as_deref()
        .filter(|p| !p.trim().is_empty())
        .map(PathGlob::new)
        .transpose()?;
    let context = req.context.min(5);
    let limit = req.limit.clamp(1, 200);

    let conn = index.connection();
    let tags = req.filter.as_ref().is_some_and(Filter::uses_tags);
    let mut visible = crate::docs::load(
        conn,
        scope,
        crate::docs::Load {
            reserved: false,
            tags,
            prefilter: req.filter.as_ref(),
        },
    )?;
    if let Some(f) = &req.filter {
        visible = filter_docs(visible, f)?;
    }
    let titles: std::collections::HashMap<String, String> = visible
        .into_iter()
        .filter(|d| path.as_ref().is_none_or(|g| g.matches(&d.id)))
        .map(|d| (d.id, d.title))
        .collect();
    let wanted: HashSet<&str> = titles.keys().map(String::as_str).collect();

    let mut texts = Vec::with_capacity(wanted.len());
    {
        let mut st = conn
            .prepare_cached("SELECT id, fm_raw || body FROM docs WHERE reserved = 0 ORDER BY id")?;
        let rows = st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        for row in rows {
            let (id, text) = row?;
            if wanted.contains(id.as_str()) {
                texts.push((id, text));
            }
        }
    }

    let found = search_parallel(&texts, &re);
    let total_lines: usize = found.iter().map(|(_, hits)| hits.len()).sum();
    let total_docs = found.len();
    let mut ranked: Vec<(usize, Vec<usize>)> = found;
    ranked.sort_by(|a, b| {
        b.1.len()
            .cmp(&a.1.len())
            .then(texts[a.0].0.cmp(&texts[b.0].0))
    });

    let mut docs = Vec::new();
    let mut shown = 0;
    let mut truncated = false;
    'outer: for (i, hits) in &ranked {
        if shown >= limit {
            truncated = true;
            break;
        }
        let (id, text) = &texts[*i];
        let mut doc = GrepDoc {
            id: id.clone(),
            title: titles[id].clone(),
            matches: hits.len(),
            lines: Vec::new(),
        };
        shown += 1;
        if !req.files_only {
            let lines: Vec<&str> = text.lines().collect();
            let mut last_end = 0;
            for &h in hits {
                let (a, b) = (
                    h.saturating_sub(context).max(last_end),
                    (h + context + 1).min(lines.len()),
                );
                for (n, l) in lines.iter().enumerate().take(b).skip(a) {
                    doc.lines.push(GrepLine {
                        line: n + 1,
                        text: l.chars().take(MAX_LINE_CHARS).collect(),
                        hit: n == h,
                    });
                    shown += 1;
                    if shown >= limit {
                        truncated = true;
                        docs.push(doc);
                        break 'outer;
                    }
                }
                last_end = b;
            }
        }
        docs.push(doc);
    }
    truncated |= docs.len() < total_docs;
    let hint = if total_docs == 0 {
        Some("no match (try synonyms with alternation a|b, terms in the documents' language, or a shorter pattern)".into())
    } else if truncated && req.files_only {
        Some("list truncated, raise `limit` or narrow with `path`".into())
    } else if truncated {
        Some("output truncated, narrow with `path` or use files_only".into())
    } else {
        None
    };
    Ok(GrepResult {
        docs,
        total_lines,
        total_docs,
        truncated,
        literal,
        hint,
    })
}

/// Returns `(index into texts, matching 0-based line numbers)` for texts with matches.
fn search_parallel(texts: &[(String, String)], re: &Regex) -> Vec<(usize, Vec<usize>)> {
    let threads = std::thread::available_parallelism()
        .map_or(1, |n| n.get())
        .min(texts.len().max(1));
    let per = texts.len().div_ceil(threads).max(1);
    std::thread::scope(|s| {
        let handles: Vec<_> = texts
            .chunks(per)
            .enumerate()
            .map(|(c, part)| {
                s.spawn(move || {
                    part.iter()
                        .enumerate()
                        .filter_map(|(j, (_, text))| {
                            let hits = matching_lines(&fold(text), re);
                            (!hits.is_empty()).then_some((c * per + j, hits))
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().expect("grep worker panicked"))
            .collect()
    })
}

/// 0-based numbers of lines containing a match. `fold` keeps line breaks, so
/// line numbers in the folded text equal those in the original.
fn matching_lines(folded: &str, re: &Regex) -> Vec<usize> {
    let mut hits = Vec::new();
    let mut line = 0;
    let mut pos = 0;
    for m in re.find_iter(folded) {
        if m.start() == m.end() && m.start() >= folded.len() {
            break;
        }
        line += folded[pos..m.start()].matches('\n').count();
        pos = m.start();
        if hits.last() != Some(&line) {
            hits.push(line);
        }
    }
    hits
}

impl GrepResult {
    /// The compact text form used by the MCP tool (same as the spike's `kb_grep`).
    pub fn to_text(&self, files_only: bool) -> String {
        let mut out = String::new();
        if self.total_docs == 0 {
            return self.hint.clone().unwrap_or_default();
        }
        for d in &self.docs {
            if files_only {
                let _ = writeln!(
                    out,
                    "{} ({} {}) — {}",
                    d.id,
                    d.matches,
                    plural(d.matches, "match", "matches"),
                    d.title
                );
                continue;
            }
            let _ = writeln!(
                out,
                "## {} — {} ({} {})",
                d.id,
                d.title,
                d.matches,
                plural(d.matches, "match", "matches")
            );
            let mut prev = 0;
            for l in &d.lines {
                if prev > 0 && l.line > prev + 1 {
                    out.push_str("  --\n");
                }
                let _ = writeln!(
                    out,
                    "  {}{}: {}",
                    l.line,
                    if l.hit { ">" } else { " " },
                    l.text
                );
                prev = l.line;
            }
        }
        let _ = write!(
            out,
            "({} matching lines in {} documents{}{})",
            self.total_lines,
            self.total_docs,
            match (self.truncated, files_only) {
                (false, _) => "",
                (true, true) => "; list truncated, raise `limit` or narrow with `path`",
                (true, false) => "; output truncated, narrow with `path` or use files_only",
            },
            if self.literal {
                "; pattern searched as a literal string (invalid regex)"
            } else {
                ""
            }
        );
        out
    }
}

fn plural(n: usize, one: &'static str, many: &'static str) -> &'static str {
    if n == 1 { one } else { many }
}

#[cfg(test)]
mod tests {
    use super::matching_lines;
    use regex::Regex;

    #[test]
    fn lines_of_matches() {
        let re = Regex::new("(?im)doi tra|^b").unwrap();
        assert_eq!(
            matching_lines("a\nb doi tra\nc\ndoi tra doi tra\n", &re),
            [1, 3]
        );
    }
}
