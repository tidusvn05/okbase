//! `list`: a directory's `index.md`, or a generated listing when there is none
//! (or when the scope hides documents, so hidden titles never leak).

use std::collections::BTreeMap;
use std::fmt::Write as _;

use serde::Serialize;

use crate::docs::DocMeta;
use crate::{Error, Scope};

/// The result of a list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ListResult {
    /// The directory, `.` for the root.
    pub dir: String,
    /// `index.md` when the directory's index file is returned as written, else `generated`.
    pub source: &'static str,
    /// Markdown listing in OKF `index.md` form.
    pub content: String,
}

pub(crate) fn list(
    index: &okbase_index::Index,
    dir: &str,
    scope: &Scope,
) -> Result<ListResult, Error> {
    let dir = dir.trim().trim_start_matches("./").trim_matches('/');
    let dir = if dir == "." { "" } else { dir };
    let shown = if dir.is_empty() { "." } else { dir };
    let index_id = if dir.is_empty() {
        "index".to_owned()
    } else {
        format!("{dir}/index")
    };
    let docs = crate::docs::load(
        index.connection(),
        scope,
        crate::docs::Load {
            reserved: true,
            ..Default::default()
        },
    )?;
    if scope.is_unrestricted()
        && let Some(body) = index
            .connection()
            .query_row("SELECT body FROM docs WHERE id = ?1", [&index_id], |r| {
                r.get::<_, String>(0)
            })
            .ok()
    {
        return Ok(ListResult {
            dir: shown.into(),
            source: "index.md",
            content: body,
        });
    }
    let content = generate(&docs, dir);
    if content.is_empty() {
        return Err(Error::NotFound(format!("{shown}/")));
    }
    Ok(ListResult {
        dir: shown.into(),
        source: "generated",
        content,
    })
}

/// An `index.md`-style listing of a directory's visible documents and subdirectories.
pub(crate) fn generate(docs: &[DocMeta], dir: &str) -> String {
    let prefix = if dir.is_empty() {
        String::new()
    } else {
        format!("{dir}/")
    };
    let mut subdirs: BTreeMap<&str, usize> = BTreeMap::new();
    let mut here = Vec::new();
    for d in docs.iter().filter(|d| !d.reserved) {
        let Some(rest) = d.id.strip_prefix(&prefix) else {
            continue;
        };
        match rest.split_once('/') {
            Some((sub, _)) => *subdirs.entry(sub).or_default() += 1,
            None => here.push(d),
        }
    }
    let mut out = String::new();
    if !subdirs.is_empty() {
        out.push_str("# Subdirectories\n\n");
        for (sub, n) in &subdirs {
            let _ = writeln!(
                out,
                "* [{sub}]({sub}/index.md) - {n} document{}",
                if *n == 1 { "" } else { "s" }
            );
        }
    }
    if !here.is_empty() {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str("# Documents\n\n");
        for d in here {
            let name = &d.id[prefix.len()..];
            match &d.description {
                Some(desc) => {
                    let _ = writeln!(out, "* [{}]({name}.md) - {desc}", d.title);
                }
                None => {
                    let _ = writeln!(out, "* [{}]({name}.md)", d.title);
                }
            }
        }
    }
    out
}
