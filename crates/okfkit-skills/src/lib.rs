//! Agent Skills bundled with okfkit, and installers that register the okfkit MCP
//! server and the skills with agent CLIs (Claude Code, Codex).
//!
//! Skills are templates: `{{bundle}}` and `{{p}}` (tool prefix) are filled in,
//! and blocks between `<!-- if CAP -->` and `<!-- end -->` are kept only when the
//! capability `CAP` is present (`!CAP` for absent). Installing is a two-step
//! process: [`plan`] lists every change, [`apply`] performs it.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

mod install;
pub use install::*;

/// Built-in skill templates: (name, SKILL.md template).
pub const BUILTIN: &[(&str, &str)] = &[
    (
        "okfkit-answer",
        include_str!("../skills/okfkit-answer/SKILL.md"),
    ),
    (
        "okfkit-curate",
        include_str!("../skills/okfkit-curate/SKILL.md"),
    ),
    (
        "okfkit-adopt",
        include_str!("../skills/okfkit-adopt/SKILL.md"),
    ),
    (
        "okfkit-tune",
        include_str!("../skills/okfkit-tune/SKILL.md"),
    ),
    (
        "okfkit-setup",
        include_str!("../skills/okfkit-setup/SKILL.md"),
    ),
    (
        "okfkit-author",
        include_str!("../skills/okfkit-author/SKILL.md"),
    ),
];

/// Where projects keep their own skills.
pub const PROJECT_SKILLS_DIR: &str = "_meta/skills";

/// Marker lines around the okfkit block in AGENTS.md.
pub const AGENTS_BEGIN: &str =
    "<!-- okfkit:begin (managed by `okfkit agent install`; edits here are replaced) -->";
/// End marker of the okfkit block.
pub const AGENTS_END: &str = "<!-- okfkit:end -->";

/// Errors returned by `okfkit-skills`.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// A file could not be read or written.
    #[error("{path}: {source}")]
    Io {
        /// The file.
        path: PathBuf,
        /// The underlying error.
        source: std::io::Error,
    },
    /// An existing configuration file cannot be parsed, so it is not touched.
    #[error("cannot update {path}: {message}")]
    Config {
        /// The file.
        path: PathBuf,
        /// What is wrong.
        message: String,
    },
    /// The server name is taken by another bundle or server.
    #[error(
        "the MCP server name `{name}` already serves {existing} in {path}; to add {wanted} as well, pick another name \
         (e.g. --name okfkit-docs); to switch it, pass --replace"
    )]
    Conflict {
        /// Server name.
        name: String,
        /// What it serves now.
        existing: String,
        /// What was asked for.
        wanted: String,
        /// The configuration file.
        path: PathBuf,
    },
    /// A command failed.
    #[error("`{command}` failed: {message}")]
    Command {
        /// The command line.
        command: String,
        /// Output or reason.
        message: String,
    },
}

/// Values used to render skills.
#[derive(Debug, Clone)]
pub struct SkillContext {
    /// Absolute path of the bundle.
    pub bundle: PathBuf,
    /// Tool name prefix (`kb`).
    pub prefix: String,
    /// Available capabilities (`read.grep`, `data.sql`, `embed.search`, …).
    pub capabilities: Vec<String>,
    /// Path filters the server applies (`--allow` / `--deny` globs), kept in its arguments.
    pub allow: Vec<String>,
    /// See `allow`.
    pub deny: Vec<String>,
}

/// A rendered skill.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skill {
    /// Skill name (directory name).
    pub name: String,
    /// Rendered SKILL.md.
    pub content: String,
}

/// Renders a skill template.
pub fn render(template: &str, cx: &SkillContext) -> String {
    let mut out = String::new();
    let mut keep: Vec<bool> = Vec::new();
    for line in template.split_inclusive('\n') {
        let t = line.trim();
        if let Some(cond) = t
            .strip_prefix("<!-- if ")
            .and_then(|r| r.strip_suffix("-->"))
        {
            let cond = cond.trim();
            let (neg, cap) = cond.strip_prefix('!').map_or((false, cond), |c| (true, c));
            keep.push(cx.capabilities.iter().any(|c| c == cap) != neg);
            continue;
        }
        if t == "<!-- end -->" {
            keep.pop();
            continue;
        }
        if keep.iter().all(|k| *k) {
            out.push_str(line);
        }
    }
    out.replace("{{bundle}}", &cx.bundle.display().to_string())
        .replace("{{p}}", &cx.prefix)
}

/// The built-in skills, rendered.
pub fn builtin_skills(cx: &SkillContext) -> Vec<Skill> {
    BUILTIN
        .iter()
        .map(|(name, t)| Skill {
            name: (*name).to_owned(),
            content: render(t, cx),
        })
        .collect()
}

/// Skills the bundle ships in `_meta/skills/<name>/SKILL.md`, rendered with the same context.
pub fn project_skills(cx: &SkillContext) -> Result<Vec<Skill>, Error> {
    let dir = cx.bundle.join(PROJECT_SKILLS_DIR);
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Ok(out);
    };
    for e in entries.flatten() {
        let file = e.path().join("SKILL.md");
        if file.is_file() {
            let text = std::fs::read_to_string(&file).map_err(|source| Error::Io {
                path: file.clone(),
                source,
            })?;
            out.push(Skill {
                name: e.file_name().to_string_lossy().into_owned(),
                content: render(&text, cx),
            });
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

pub(crate) fn read_text(path: &Path) -> Result<String, Error> {
    match std::fs::read_to_string(path) {
        Ok(t) => Ok(t),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(source) => Err(Error::Io {
            path: path.to_owned(),
            source,
        }),
    }
}

pub(crate) fn read_json(path: &Path) -> Result<Value, Error> {
    let text = read_text(path)?;
    if text.trim().is_empty() {
        return Ok(json!({}));
    }
    match serde_json::from_str::<Value>(&text) {
        Ok(v @ Value::Object(_)) => Ok(v),
        Ok(_) => Err(Error::Config {
            path: path.to_owned(),
            message: "not a JSON object".into(),
        }),
        Err(e) => Err(Error::Config {
            path: path.to_owned(),
            message: e.to_string(),
        }),
    }
}

pub(crate) fn strip_frontmatter(s: &str) -> String {
    s.strip_prefix("---\n")
        .and_then(|r| r.split_once("\n---\n"))
        .map_or(s, |(_, body)| body)
        .trim_start_matches('\n')
        .to_owned()
}

/// Inserts or replaces the okfkit block in an AGENTS.md text, leaving the rest unchanged.
pub fn upsert_block(existing: &str, body: &str) -> String {
    upsert_block_between(existing, body, AGENTS_BEGIN, AGENTS_END)
}

fn find_block(existing: &str, begin: &str, end: &str) -> Option<(usize, usize)> {
    let a = existing.find(begin)?;
    let b = a + existing[a..].find(end)?;
    let stop = b + end.len();
    Some((
        a,
        if existing[stop..].starts_with('\n') {
            stop + 1
        } else {
            stop
        },
    ))
}

/// Inserts or replaces the block between `begin` and `end`, leaving the rest unchanged.
pub fn upsert_block_between(existing: &str, body: &str, begin: &str, end: &str) -> String {
    let block = format!("{begin}\n{}\n{end}\n", body.trim_end());
    if let Some((a, z)) = find_block(existing, begin, end) {
        return format!("{}{block}{}", &existing[..a], &existing[z..]);
    }
    if existing.is_empty() {
        block
    } else if existing.ends_with("\n\n") {
        format!("{existing}{block}")
    } else if existing.ends_with('\n') {
        format!("{existing}\n{block}")
    } else {
        format!("{existing}\n\n{block}")
    }
}

/// Removes the block between `begin` and `end` (and the blank line before it, if the block
/// was appended). `None` when there is no block.
pub fn remove_block(existing: &str, begin: &str, end: &str) -> Option<String> {
    let (a, z) = find_block(existing, begin, end)?;
    let head = &existing[..a];
    let head = if z == existing.len() && head.ends_with("\n\n") {
        &head[..head.len() - 1]
    } else {
        head
    };
    Some(format!("{head}{}", &existing[z..]))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cx(caps: &[&str]) -> SkillContext {
        SkillContext {
            bundle: "/kb".into(),
            prefix: "kb".into(),
            capabilities: caps.iter().map(|c| (*c).to_owned()).collect(),
            allow: vec![],
            deny: vec![],
        }
    }

    #[test]
    fn conditional_blocks() {
        let t = "a\n<!-- if data.sql -->\nsql {{p}}\n<!-- end -->\n<!-- if !data.sql -->\nno sql\n<!-- end -->\nz {{bundle}}\n";
        assert_eq!(render(t, &cx(&[])), "a\nno sql\nz /kb\n");
        assert_eq!(render(t, &cx(&["data.sql"])), "a\nsql kb\nz /kb\n");
    }

    #[test]
    fn blocks_are_removed_cleanly() {
        let base = "# Project\n\nRules.\n";
        let with = upsert_block(base, "hi");
        assert_eq!(remove_block(&with, AGENTS_BEGIN, AGENTS_END).unwrap(), base);
        assert_eq!(
            remove_block(&upsert_block("", "hi"), AGENTS_BEGIN, AGENTS_END).unwrap(),
            ""
        );
        assert!(remove_block(base, AGENTS_BEGIN, AGENTS_END).is_none());
        // Named blocks are independent.
        let two = upsert_block_between(
            &with,
            "docs",
            "<!-- okfkit:begin docs -->",
            "<!-- okfkit:end docs -->",
        );
        let one = remove_block(&two, AGENTS_BEGIN, AGENTS_END).unwrap();
        assert!(one.contains("docs") && !one.contains("\nhi\n"), "{one}");
    }

    #[test]
    fn agents_block_is_idempotent() {
        let once = upsert_block("# Project\n\nRules.\n", "okfkit says hi");
        assert_eq!(
            once,
            format!("# Project\n\nRules.\n\n{AGENTS_BEGIN}\nokfkit says hi\n{AGENTS_END}\n")
        );
        let twice = upsert_block(&(once.clone() + "More.\n"), "updated");
        assert_eq!(
            twice,
            format!("# Project\n\nRules.\n\n{AGENTS_BEGIN}\nupdated\n{AGENTS_END}\nMore.\n")
        );
    }
}
