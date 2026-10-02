//! Registering okfkit with agent CLIs, removing it again, and remembering what was installed.
//!
//! Every change is planned first ([`plan`], [`plan_uninstall`]) and applied by [`apply`], so
//! `--print` can show it. Installs are recorded in a small registry ([`registry_path`]) so that
//! `okfkit agent status` can check them and `okfkit agent uninstall --all` can undo them all.
//!
//! Several bundles can be installed side by side: each gets its own MCP server name, tool
//! prefix, skill names and AGENTS.md block. A name already serving another bundle is never
//! overwritten silently ([`Error::Conflict`]).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{
    AGENTS_BEGIN, AGENTS_END, Error, Skill, SkillContext, builtin_skills, project_skills,
    read_json, read_text, remove_block, strip_frontmatter, upsert_block_between,
};

/// The default MCP server name.
pub const DEFAULT_NAME: &str = "okfkit";

/// The agent to install for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Agent {
    /// Claude Code.
    Claude,
    /// OpenAI Codex CLI.
    Codex,
}

impl Agent {
    /// Display name.
    pub fn label(self) -> &'static str {
        match self {
            Agent::Claude => "Claude Code",
            Agent::Codex => "Codex",
        }
    }
}

/// Where to install.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "scope", content = "dir", rename_all = "lowercase")]
pub enum Target {
    /// Into a project directory (`.mcp.json`, `.claude/skills/`, `AGENTS.md`).
    Project(PathBuf),
    /// For the user (`~/.claude/skills/`, `~/.codex/`).
    User,
}

/// How the agent reaches okfkit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Server {
    /// The agent starts `okfkit mcp serve --stdio` for a local bundle.
    Stdio {
        /// Command that starts okfkit (usually the absolute path of the running binary).
        command: PathBuf,
    },
    /// A shared okfkit server (`okfkit mcp serve --http`).
    Http {
        /// The server's MCP endpoint (`https://kb.example.com/mcp`).
        url: String,
        /// Environment variable holding the bearer token, if the server needs one.
        token_env: Option<String>,
    },
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
    /// How the agent reaches okfkit.
    pub server: Server,
    /// MCP server name in the agent's configuration.
    pub server_name: String,
    /// Skill rendering context (bundle path or URL, prefix, capabilities).
    pub skill: SkillContext,
    /// Overwrite a server of the same name that serves something else.
    pub replace: bool,
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
    /// Delete a file or a directory.
    Remove {
        /// The file or directory.
        path: PathBuf,
        /// What it is for.
        why: String,
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
            Action::Remove { path, why } => writeln!(f, "# {why}\n# remove {}", path.display()),
            Action::Run { argv, why } => writeln!(f, "# {why}\n$ {}", shell_join(argv)),
        }
    }
}

impl Action {
    /// One line for reports.
    pub fn summary(&self) -> String {
        match self {
            Action::Write { path, why, .. } => format!("{why}: {}", path.display()),
            Action::Remove { path, why } => format!("{why}: removed {}", path.display()),
            Action::Run { why, .. } => format!("{why}: done"),
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

fn slug(name: &str) -> String {
    let s: String = name
        .strip_prefix("okfkit-")
        .unwrap_or(name)
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let s = s.trim_matches('-').to_owned();
    if s.is_empty() { "kb".into() } else { s }
}

/// The tool prefix for a server name: `kb` for the default server, else derived from the name
/// (`okfkit-docs` → `docs`), so two bundles never share tool names in one agent.
pub fn default_prefix(server_name: &str) -> String {
    if server_name == DEFAULT_NAME {
        "kb".into()
    } else {
        slug(server_name).replace('-', "_")
    }
}

/// Skill names for a server: `okfkit-answer` for the default server, `okfkit-answer-docs` for
/// `okfkit-docs` (project skills from `_meta/skills` get the same suffix).
pub fn skill_name(base: &str, server_name: &str) -> String {
    if server_name == DEFAULT_NAME {
        base.to_owned()
    } else {
        format!("{base}-{}", slug(server_name))
    }
}

/// AGENTS.md markers of a server's block.
pub fn block_markers(server_name: &str) -> (String, String) {
    if server_name == DEFAULT_NAME {
        (AGENTS_BEGIN.into(), AGENTS_END.into())
    } else {
        (
            format!(
                "<!-- okfkit:begin {server_name} (managed by `okfkit agent install`; edits here are replaced) -->"
            ),
            format!("<!-- okfkit:end {server_name} -->"),
        )
    }
}

fn renamed(mut s: Skill, server_name: &str) -> Skill {
    let new = skill_name(&s.name, server_name);
    if new != s.name {
        s.content =
            s.content
                .replacen(&format!("name: {}\n", s.name), &format!("name: {new}\n"), 1);
        s.name = new;
    }
    s
}

/// The skills an install writes. A shared (HTTP) server gets only the answering skill, without
/// the CLI fallback: the other skills edit local files.
pub fn skills_for(opts: &InstallOptions) -> Result<Vec<Skill>, Error> {
    let mut skills = builtin_skills(&opts.skill);
    if matches!(opts.server, Server::Http { .. }) {
        skills.retain(|s| s.name == "okfkit-answer");
    } else {
        skills.extend(project_skills(&opts.skill)?);
    }
    Ok(skills
        .into_iter()
        .map(|s| renamed(s, &opts.server_name))
        .collect())
}

/// What a configured server serves: its bundle path or URL (`None` if it is not okfkit's).
fn identity(entry: &Value) -> Option<String> {
    if let Some(url) = entry.get("url").and_then(Value::as_str) {
        return Some(url.to_owned());
    }
    let args = entry.get("args")?.as_array()?;
    let i = args.iter().position(|a| a == "--bundle")?;
    args.get(i + 1)?.as_str().map(str::to_owned)
}

fn wanted(opts: &InstallOptions) -> String {
    match &opts.server {
        Server::Stdio { .. } => opts.skill.bundle.display().to_string(),
        Server::Http { url, .. } => url.clone(),
    }
}

fn check_conflict(
    opts: &InstallOptions,
    existing: Option<&Value>,
    path: &Path,
) -> Result<(), Error> {
    let Some(entry) = existing else {
        return Ok(());
    };
    let want = wanted(opts);
    let have = identity(entry);
    if opts.replace || have.as_deref() == Some(want.as_str()) {
        return Ok(());
    }
    Err(Error::Conflict {
        name: opts.server_name.clone(),
        existing: have.unwrap_or_else(|| "a server that is not okfkit".into()),
        wanted: want,
        path: path.to_owned(),
    })
}

fn codex_entry_json(item: &toml_edit::Item) -> Value {
    let t = match item.as_table_like() {
        Some(t) => t,
        None => return Value::Null,
    };
    let mut v = json!({});
    if let Some(url) = t.get("url").and_then(|u| u.as_str()) {
        v["url"] = json!(url);
    }
    if let Some(args) = t.get("args").and_then(|a| a.as_array()) {
        v["args"] = json!(args.iter().filter_map(|a| a.as_str()).collect::<Vec<_>>());
    }
    v
}

fn codex_config(home: &Path) -> Result<(PathBuf, toml_edit::DocumentMut), Error> {
    let path = home.join(".codex/config.toml");
    let doc = read_text(&path)?
        .parse()
        .map_err(|e: toml_edit::TomlError| Error::Config {
            path: path.clone(),
            message: e.to_string(),
        })?;
    Ok((path, doc))
}

/// Claude Code keeps user-scope servers in `~/.claude.json` (`mcpServers`).
fn claude_user_entry(home: &Path, name: &str) -> Option<Value> {
    let v = read_json(&home.join(".claude.json")).ok()?;
    v.get("mcpServers")?.get(name).cloned()
}

fn agents_path(opts_target: &Target, home: &Path) -> PathBuf {
    match opts_target {
        Target::Project(dir) => dir.join("AGENTS.md"),
        Target::User => home.join(".codex/AGENTS.md"),
    }
}

fn claude_skills_dir(target: &Target, home: &Path) -> PathBuf {
    match target {
        Target::Project(dir) => dir.join(".claude/skills"),
        Target::User => home.join(".claude/skills"),
    }
}

/// Lists the changes needed to install okfkit for an agent. Reads existing files; writes nothing.
pub fn plan(opts: &InstallOptions) -> Result<Vec<Action>, Error> {
    let skills = skills_for(opts)?;
    let mut actions = Vec::new();
    match (opts.agent, &opts.target) {
        (Agent::Claude, Target::Project(dir)) => {
            let path = dir.join(".mcp.json");
            let mut config = read_json(&path)?;
            check_conflict(opts, config["mcpServers"].get(&opts.server_name), &path)?;
            config["mcpServers"][&opts.server_name] = match &opts.server {
                Server::Stdio { command } => json!({
                    "type": "stdio",
                    "command": command.display().to_string(),
                    "args": server_args(&opts.skill),
                }),
                Server::Http { url, token_env } => {
                    let mut e = json!({"type": "http", "url": url});
                    if let Some(var) = token_env {
                        // Claude Code expands ${VAR} in .mcp.json: the token never lands in the file.
                        e["headers"] = json!({"Authorization": format!("Bearer ${{{var}}}")});
                    }
                    e
                }
            };
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
        }
        (Agent::Claude, Target::User) => {
            check_conflict(
                opts,
                claude_user_entry(&opts.home, &opts.server_name).as_ref(),
                &opts.home.join(".claude.json"),
            )?;
            let mut argv: Vec<String> = vec![
                "claude".into(),
                "mcp".into(),
                "add".into(),
                "--scope".into(),
                "user".into(),
            ];
            match &opts.server {
                Server::Stdio { command } => {
                    argv.extend([opts.server_name.clone(), "--".into()]);
                    argv.push(command.display().to_string());
                    argv.extend(server_args(&opts.skill));
                }
                Server::Http { url, token_env } => {
                    argv.extend(["--transport".into(), "http".into()]);
                    if let Some(var) = token_env {
                        argv.extend([
                            "--header".into(),
                            format!("Authorization: Bearer ${{{var}}}"),
                        ]);
                    }
                    argv.extend([opts.server_name.clone(), url.clone()]);
                }
            }
            if claude_user_entry(&opts.home, &opts.server_name).is_some() {
                actions.push(Action::Run {
                    argv: vec![
                        "claude".into(),
                        "mcp".into(),
                        "remove".into(),
                        "--scope".into(),
                        "user".into(),
                        opts.server_name.clone(),
                    ],
                    why: format!("replace the existing `{}` MCP server", opts.server_name),
                });
            }
            actions.push(Action::Run {
                argv,
                why: format!(
                    "register the `{}` MCP server for your user",
                    opts.server_name
                ),
            });
        }
        (Agent::Codex, target) => {
            let (path, mut doc) = codex_config(&opts.home)?;
            let existing = doc
                .get("mcp_servers")
                .and_then(|s| s.get(&opts.server_name))
                .map(codex_entry_json);
            check_conflict(opts, existing.as_ref(), &path)?;
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
            match &opts.server {
                Server::Stdio { command } => {
                    server["command"] = toml_edit::value(command.display().to_string());
                    let mut arr = toml_edit::Array::new();
                    for a in server_args(&opts.skill) {
                        arr.push(a);
                    }
                    server["args"] = toml_edit::value(arr);
                }
                Server::Http { url, token_env } => {
                    server["url"] = toml_edit::value(url.as_str());
                    if let Some(var) = token_env {
                        server["bearer_token_env_var"] = toml_edit::value(var.as_str());
                    }
                }
            }
            servers.insert(&opts.server_name, toml_edit::Item::Table(server));
            actions.push(Action::Write {
                why: format!(
                    "register the `{}` MCP server for Codex (Codex reads MCP servers from your user configuration)",
                    opts.server_name
                ),
                content: doc.to_string(),
                path,
            });
            let agents = agents_path(target, &opts.home);
            let existing = read_text(&agents)?;
            // AGENTS.md is always in context: only the answering skill goes in full.
            let answer = skill_name("okfkit-answer", &opts.server_name);
            let mut body: String = skills
                .iter()
                .filter(|s| s.name == answer || !s.name.starts_with("okfkit-"))
                .map(|s| strip_frontmatter(&s.content))
                .collect::<Vec<_>>()
                .join("\n");
            if matches!(opts.server, Server::Stdio { .. }) {
                body.push_str(&format!(
                    "\nTo improve the bundle itself, start from `okfkit --bundle {} lint --level L2 --json` (fix descriptions, index.md, tags, status); \
                     to convert a plain markdown folder, use `okfkit adopt DIR -v`.\n",
                    opts.skill.bundle.display()
                ));
            }
            let (begin, end) = block_markers(&opts.server_name);
            actions.push(Action::Write {
                why: "tell Codex how to use the knowledge bundle".into(),
                content: upsert_block_between(&existing, &body, &begin, &end),
                path: agents,
            });
            return Ok(actions);
        }
    }
    let dir = claude_skills_dir(&opts.target, &opts.home);
    for s in &skills {
        actions.push(Action::Write {
            path: dir.join(&s.name).join("SKILL.md"),
            why: format!("install the `{}` skill", s.name),
            content: s.content.clone(),
        });
    }
    Ok(actions)
}

/// What to remove.
#[derive(Debug, Clone)]
pub struct UninstallOptions {
    /// The agent.
    pub agent: Agent,
    /// Project or user.
    pub target: Target,
    /// The user's home directory.
    pub home: PathBuf,
    /// MCP server name.
    pub server_name: String,
    /// Skills to remove (from the registry); `None`: the built-in skills for this name.
    pub skills: Option<Vec<String>>,
}

/// Lists the changes that remove an install. Only okfkit's entries, skills and blocks are
/// touched; files that end up empty and were okfkit's alone are deleted.
pub fn plan_uninstall(opts: &UninstallOptions) -> Result<Vec<Action>, Error> {
    let mut actions = Vec::new();
    let name = &opts.server_name;
    match (opts.agent, &opts.target) {
        (Agent::Claude, Target::Project(dir)) => {
            let path = dir.join(".mcp.json");
            let mut config = read_json(&path)?;
            let removed = config
                .get_mut("mcpServers")
                .and_then(Value::as_object_mut)
                .and_then(|m| m.remove(name))
                .is_some();
            if removed {
                let empty = config["mcpServers"]
                    .as_object()
                    .is_none_or(|m| m.is_empty())
                    && config.as_object().is_some_and(|o| o.len() == 1);
                actions.push(if empty {
                    Action::Remove {
                        path,
                        why: format!(
                            "unregister the `{name}` MCP server (the file held nothing else)"
                        ),
                    }
                } else {
                    Action::Write {
                        why: format!("unregister the `{name}` MCP server"),
                        content: format!(
                            "{}\n",
                            serde_json::to_string_pretty(&config).expect("serializable")
                        ),
                        path,
                    }
                });
            }
        }
        (Agent::Claude, Target::User) => {
            if claude_user_entry(&opts.home, name).is_some() {
                actions.push(Action::Run {
                    argv: vec![
                        "claude".into(),
                        "mcp".into(),
                        "remove".into(),
                        "--scope".into(),
                        "user".into(),
                        name.clone(),
                    ],
                    why: format!("unregister the `{name}` MCP server for your user"),
                });
            }
        }
        (Agent::Codex, target) => {
            let (path, mut doc) = codex_config(&opts.home)?;
            let removed = doc
                .get_mut("mcp_servers")
                .and_then(|s| s.as_table_like_mut())
                .and_then(|t| t.remove(name))
                .is_some();
            if removed {
                actions.push(Action::Write {
                    why: format!("unregister the `{name}` MCP server for Codex"),
                    content: doc.to_string(),
                    path,
                });
            }
            let agents = agents_path(target, &opts.home);
            let (begin, end) = block_markers(name);
            if let Some(rest) = remove_block(&read_text(&agents)?, &begin, &end) {
                actions.push(if rest.trim().is_empty() {
                    Action::Remove {
                        path: agents,
                        why: "remove the okfkit instructions (the file held nothing else)".into(),
                    }
                } else {
                    Action::Write {
                        why: "remove the okfkit instructions from AGENTS.md".into(),
                        content: rest,
                        path: agents,
                    }
                });
            }
            return Ok(actions);
        }
    }
    let names = opts.skills.clone().unwrap_or_else(|| {
        crate::BUILTIN
            .iter()
            .map(|(n, _)| skill_name(n, name))
            .collect()
    });
    let dir = claude_skills_dir(&opts.target, &opts.home);
    for s in names {
        let path = dir.join(&s);
        if path.join("SKILL.md").is_file() {
            actions.push(Action::Remove {
                path,
                why: format!("remove the `{s}` skill"),
            });
        }
    }
    Ok(actions)
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
            Action::Remove { path, .. } => {
                let r = if path.is_dir() {
                    std::fs::remove_dir_all(path)
                } else {
                    std::fs::remove_file(path)
                };
                match r {
                    Ok(()) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(source) => {
                        return Err(Error::Io {
                            path: path.clone(),
                            source,
                        });
                    }
                }
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

/// A recorded install.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Install {
    /// Agent.
    pub agent: Agent,
    /// Project or user.
    pub target: Target,
    /// MCP server name.
    pub name: String,
    /// What it serves.
    pub server: Server,
    /// Bundle path (local server) or URL.
    pub source: String,
    /// Skills written (Claude Code).
    pub skills: Vec<String>,
}

impl Install {
    /// The record of an install.
    pub fn of(opts: &InstallOptions) -> Result<Install, Error> {
        Ok(Install {
            agent: opts.agent,
            target: opts.target.clone(),
            name: opts.server_name.clone(),
            server: opts.server.clone(),
            source: wanted(opts),
            skills: if opts.agent == Agent::Claude {
                skills_for(opts)?.into_iter().map(|s| s.name).collect()
            } else {
                Vec::new()
            },
        })
    }

    fn same_slot(&self, other: &Install) -> bool {
        self.agent == other.agent && self.target == other.target && self.name == other.name
    }

    /// Problems with this install today (empty: healthy).
    pub fn check(&self, home: &Path) -> Vec<String> {
        let mut out = Vec::new();
        let entry = match (self.agent, &self.target) {
            (Agent::Claude, Target::Project(dir)) => read_json(&dir.join(".mcp.json"))
                .ok()
                .and_then(|v| v.get("mcpServers")?.get(&self.name).cloned()),
            (Agent::Claude, Target::User) => claude_user_entry(home, &self.name),
            (Agent::Codex, _) => codex_config(home).ok().and_then(|(_, d)| {
                d.get("mcp_servers")
                    .and_then(|s| s.get(&self.name))
                    .map(codex_entry_json)
            }),
        };
        match entry {
            None => out.push(format!(
                "the `{}` server is no longer in the {} configuration",
                self.name,
                self.agent.label()
            )),
            Some(e) if identity(&e).as_deref() != Some(self.source.as_str()) => out.push(format!(
                "the `{}` server now serves {} instead",
                self.name,
                identity(&e).unwrap_or_else(|| "something else".into())
            )),
            Some(_) => {}
        }
        if let Server::Stdio { command } = &self.server {
            if !Path::new(&self.source).is_dir() {
                out.push(format!("the bundle {} no longer exists", self.source));
            }
            if !command.is_file() {
                out.push(format!(
                    "the okfkit binary {} no longer exists",
                    command.display()
                ));
            }
        }
        if let Target::Project(dir) = &self.target
            && !dir.is_dir()
        {
            out.push(format!("the project {} no longer exists", dir.display()));
        }
        out
    }
}

/// `<config dir>/okfkit/installs.json` (`OKFKIT_CONFIG_DIR` overrides the directory).
pub fn registry_path() -> Option<PathBuf> {
    let env = |k: &str| {
        std::env::var_os(k)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    if let Some(d) = env("OKFKIT_CONFIG_DIR") {
        return Some(d.join("installs.json"));
    }
    let base = if cfg!(windows) {
        env("APPDATA")
    } else if cfg!(target_os = "macos") {
        env("HOME").map(|h| h.join("Library/Application Support"))
    } else {
        env("XDG_CONFIG_HOME").or_else(|| env("HOME").map(|h| h.join(".config")))
    }?;
    Some(base.join("okfkit").join("installs.json"))
}

/// Recorded installs (empty if none).
pub fn load_registry(path: &Path) -> Result<Vec<Install>, Error> {
    let text = read_text(path)?;
    if text.trim().is_empty() {
        return Ok(Vec::new());
    }
    serde_json::from_str(&text).map_err(|e| Error::Config {
        path: path.to_owned(),
        message: e.to_string(),
    })
}

fn save_registry(path: &Path, installs: &[Install]) -> Result<(), Error> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| Error::Io {
            path: parent.to_owned(),
            source,
        })?;
    }
    let text = serde_json::to_string_pretty(installs).expect("serializable") + "\n";
    std::fs::write(path, text).map_err(|source| Error::Io {
        path: path.to_owned(),
        source,
    })
}

/// Records an install (replacing the record for the same agent, target and name).
pub fn record_install(path: &Path, install: Install) -> Result<(), Error> {
    let mut all = load_registry(path)?;
    all.retain(|i| !i.same_slot(&install));
    all.push(install);
    save_registry(path, &all)
}

/// Forgets an install.
pub fn forget_install(path: &Path, agent: Agent, target: &Target, name: &str) -> Result<(), Error> {
    let mut all = load_registry(path)?;
    all.retain(|i| !(i.agent == agent && &i.target == target && i.name == name));
    save_registry(path, &all)
}
