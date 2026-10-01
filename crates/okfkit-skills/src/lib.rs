//! Agent Skills bundled with okfkit, and installers that register the okfkit MCP
//! server and the skills with agent CLIs (Claude Code, Codex).
//!
//! Skills are templates: `{{bundle}}` and `{{p}}` (tool prefix) are filled in,
//! and blocks between `<!-- if CAP -->` and `<!-- end -->` are kept only when the
//! capability `CAP` is present (`!CAP` for absent). Installing is a two-step
//! process: [`plan`] lists every change, [`apply`] performs it.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

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

/// The agent to install for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Agent {
    /// Claude Code.
    Claude,
    /// OpenAI Codex CLI.
    Codex,
}

/// Where to install.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// Into a project directory (`.mcp.json`, `.claude/skills/`, `AGENTS.md`).
    Project(PathBuf),
    /// For the user (`~/.claude/skills/`, `~/.codex/`).
    User,
}

/// Install settings.
#[derive(Debug, Clone)]
pub struct InstallOptions {
    /// The agent.
    pub agent: Agent,
    /// Project or user.
    pub target: Target,
    /// The user's home directory.
    pub home: PathBuf,
    /// Command that starts okfkit (usually the absolute path of the running binary).
    pub command: PathBuf,
    /// MCP server name in the agent's configuration.
    pub server_name: String,
    /// Skill rendering context (bundle path, prefix, capabilities).
    pub skill: SkillContext,
}

/// One change `apply` will make.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Write a file (created or replaced) with this full content.
    Write {
        /// The file.
        path: PathBuf,
        /// What it is for.
        why: String,
        /// Full new content.
        content: String,
    },
    /// Run a command.
    Run {
        /// Program and arguments.
        argv: Vec<String>,
        /// What it is for.
        why: String,
    },
}

impl std::fmt::Display for Action {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Action::Write { path, why, content } => {
                writeln!(f, "# {why}\n# write {}", path.display())?;
                f.write_str(content)?;
                if !content.ends_with('\n') {
                    writeln!(f)?;
                }
                Ok(())
            }
            Action::Run { argv, why } => writeln!(f, "# {why}\n$ {}", shell_join(argv)),
        }
    }
}

fn shell_join(argv: &[String]) -> String {
    argv.iter()
        .map(|a| {
            if a.chars()
                .all(|c| c.is_ascii_alphanumeric() || "-_./:=@".contains(c))
            {
                a.clone()
            } else {
                format!("'{}'", a.replace('\'', "'\\''"))
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Arguments that start the okfkit MCP server for the bundle.
pub fn server_args(cx: &SkillContext) -> Vec<String> {
    vec![
        "--bundle".into(),
        cx.bundle.display().to_string(),
        "mcp".into(),
        "serve".into(),
        "--stdio".into(),
    ]
}

/// Lists the changes needed to install okfkit for an agent. Reads existing files; writes nothing.
pub fn plan(opts: &InstallOptions) -> Result<Vec<Action>, Error> {
    let mut skills = builtin_skills(&opts.skill);
    skills.extend(project_skills(&opts.skill)?);
    let command = opts.command.display().to_string();
    let args = server_args(&opts.skill);
    let mut actions = Vec::new();
    match (opts.agent, &opts.target) {
        (Agent::Claude, Target::Project(dir)) => {
            let path = dir.join(".mcp.json");
            let mut config = read_json(&path)?;
            config["mcpServers"][&opts.server_name] =
                json!({"type": "stdio", "command": command, "args": args});
            actions.push(Action::Write {
                why: format!(
                    "register the `{}` MCP server for this project",
                    opts.server_name
                ),
                content: format!(
                    "{}\n",
                    serde_json::to_string_pretty(&config).expect("serializable")
                ),
                path,
            });
            for s in &skills {
                actions.push(skill_write(dir.join(".claude/skills"), s));
            }
        }
        (Agent::Claude, Target::User) => {
            let mut argv = vec![
                "claude".into(),
                "mcp".into(),
                "add".into(),
                "--scope".into(),
                "user".into(),
                opts.server_name.clone(),
                "--".into(),
                command,
            ];
            argv.extend(args);
            actions.push(Action::Run {
                argv,
                why: format!(
                    "register the `{}` MCP server for your user",
                    opts.server_name
                ),
            });
            for s in &skills {
                actions.push(skill_write(opts.home.join(".claude/skills"), s));
            }
        }
        (Agent::Codex, target) => {
            let path = opts.home.join(".codex/config.toml");
            let text = read_text(&path)?;
            let mut doc: toml_edit::DocumentMut =
                text.parse()
                    .map_err(|e: toml_edit::TomlError| Error::Config {
                        path: path.clone(),
                        message: e.to_string(),
                    })?;
            let servers = doc.entry("mcp_servers").or_insert_with(|| {
                let mut t = toml_edit::Table::new();
                t.set_implicit(true);
                toml_edit::Item::Table(t)
            });
            let Some(servers) = servers.as_table_mut() else {
                return Err(Error::Config {
                    path,
                    message: "`mcp_servers` is not a table".into(),
                });
            };
            let mut server = toml_edit::Table::new();
            server["command"] = toml_edit::value(command);
            let mut arr = toml_edit::Array::new();
            for a in &args {
                arr.push(a.as_str());
            }
            server["args"] = toml_edit::value(arr);
            servers.insert(&opts.server_name, toml_edit::Item::Table(server));
            actions.push(Action::Write {
                why: format!("register the `{}` MCP server for Codex", opts.server_name),
                content: doc.to_string(),
                path,
            });
            let agents = match target {
                Target::Project(dir) => dir.join("AGENTS.md"),
                Target::User => opts.home.join(".codex/AGENTS.md"),
            };
            let existing = read_text(&agents)?;
            // AGENTS.md is always in context: only the answering skill goes in full.
            let mut body: String = skills
                .iter()
                .filter(|s| s.name == "okfkit-answer" || !s.name.starts_with("okfkit-"))
                .map(|s| strip_frontmatter(&s.content))
                .collect::<Vec<_>>()
                .join("\n");
            body.push_str(
                "\nTo improve the bundle itself, start from `okfkit lint --level L2 --json` (fix descriptions, index.md, tags, status); \
                 to convert a plain markdown folder, use `okfkit adopt DIR -v`.\n",
            );
            actions.push(Action::Write {
                why: "tell Codex how to use the knowledge bundle".into(),
                content: upsert_block(&existing, &body),
                path: agents,
            });
        }
    }
    Ok(actions)
}

fn skill_write(dir: PathBuf, s: &Skill) -> Action {
    Action::Write {
        path: dir.join(&s.name).join("SKILL.md"),
        why: format!("install the `{}` skill", s.name),
        content: s.content.clone(),
    }
}

fn read_text(path: &Path) -> Result<String, Error> {
    match std::fs::read_to_string(path) {
        Ok(t) => Ok(t),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(source) => Err(Error::Io {
            path: path.to_owned(),
            source,
        }),
    }
}

fn read_json(path: &Path) -> Result<Value, Error> {
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

fn strip_frontmatter(s: &str) -> String {
    s.strip_prefix("---\n")
        .and_then(|r| r.split_once("\n---\n"))
        .map_or(s, |(_, body)| body)
        .trim_start_matches('\n')
        .to_owned()
}

/// Inserts or replaces the okfkit block in an AGENTS.md text, leaving the rest unchanged.
pub fn upsert_block(existing: &str, body: &str) -> String {
    let block = format!("{AGENTS_BEGIN}\n{}\n{AGENTS_END}\n", body.trim_end());
    if let (Some(a), Some(b)) = (existing.find(AGENTS_BEGIN), existing.find(AGENTS_END))
        && a < b
    {
        let end = b + AGENTS_END.len();
        let end = if existing[end..].starts_with('\n') {
            end + 1
        } else {
            end
        };
        return format!("{}{block}{}", &existing[..a], &existing[end..]);
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

/// Performs the planned changes.
pub fn apply(actions: &[Action]) -> Result<(), Error> {
    for a in actions {
        match a {
            Action::Write { path, content, .. } => {
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent).map_err(|source| Error::Io {
                        path: parent.to_owned(),
                        source,
                    })?;
                }
                std::fs::write(path, content).map_err(|source| Error::Io {
                    path: path.clone(),
                    source,
                })?;
            }
            Action::Run { argv, .. } => {
                let out = std::process::Command::new(&argv[0])
                    .args(&argv[1..])
                    .output()
                    .map_err(|e| Error::Command {
                        command: shell_join(argv),
                        message: e.to_string(),
                    })?;
                if !out.status.success() {
                    let msg = String::from_utf8_lossy(&out.stderr).trim().to_owned();
                    return Err(Error::Command {
                        command: shell_join(argv),
                        message: msg,
                    });
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cx(caps: &[&str]) -> SkillContext {
        SkillContext {
            bundle: "/kb".into(),
            prefix: "kb".into(),
            capabilities: caps.iter().map(|c| (*c).to_owned()).collect(),
        }
    }

    #[test]
    fn conditional_blocks() {
        let t = "a\n<!-- if data.sql -->\nsql {{p}}\n<!-- end -->\n<!-- if !data.sql -->\nno sql\n<!-- end -->\nz {{bundle}}\n";
        assert_eq!(render(t, &cx(&[])), "a\nno sql\nz /kb\n");
        assert_eq!(render(t, &cx(&["data.sql"])), "a\nsql kb\nz /kb\n");
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
