//! The `okfkit` command-line interface.

mod cli;

use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

use anyhow::{Context as _, Result, bail};
use clap::Parser as _;
use okfkit::{Bundle, OpenOptions, Scope, StateDir};
use serde::Serialize;

use cli::{AgentCmd, Cli, Command, FilterArgs, LintFormat, McpCmd};

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("error: {e:#}");
            if let Some(hint) = hint(&e) {
                eprintln!("hint: {hint}");
            }
            ExitCode::from(1)
        }
    }
}

/// A suggestion for common failures.
fn hint(e: &anyhow::Error) -> Option<&'static str> {
    let msg = format!("{e:#}");
    if msg.contains("bundle not found") {
        Some(
            "pass --bundle <DIR> (or set OKFKIT_BUNDLE), or run okfkit inside the bundle directory",
        )
    } else if msg.contains("not found:") {
        Some("find ids with `okfkit list`, `okfkit catalog` or `okfkit grep PATTERN --files-only`")
    } else if msg.contains("invalid argument") {
        Some("see `okfkit help <command>` for the accepted values")
    } else if msg.contains("index database") {
        Some("the index may be corrupt; rebuild it with `okfkit index --rebuild`")
    } else {
        None
    }
}

fn run(cli: Cli) -> Result<ExitCode> {
    let json = cli.json;
    let bundle_dir = cli.bundle.clone().unwrap_or_else(|| PathBuf::from("."));
    let state_dir = match cli.state_dir.as_deref() {
        None | Some("auto") => StateDir::Auto,
        Some("cache") => StateDir::Cache,
        Some(p) => StateDir::Path(PathBuf::from(p)),
    };
    let scope = || -> Result<Scope> {
        let mut s = Scope::all();
        for a in &cli.allow {
            s = s.allow(a)?;
        }
        for d in &cli.deny {
            s = s.deny(d)?;
        }
        Ok(s)
    };
    let open = || -> Result<Bundle> {
        let b = Bundle::open(
            &bundle_dir,
            OpenOptions::default().state_dir(state_dir.clone()),
        )?;
        b.sync().context("updating the index")?;
        Ok(b)
    };

    match cli.command {
        Command::Status => {
            let b = open()?;
            let scope = scope()?;
            let stats = b.stats(&scope)?;
            let lint = b.lint(&okfkit::LintConfig::level(okfkit::Level::L2))?;
            #[derive(Serialize)]
            struct Status {
                bundle: PathBuf,
                index: Option<PathBuf>,
                level: Option<okfkit::Level>,
                #[serde(flatten)]
                stats: okfkit::Stats,
            }
            let s = Status {
                bundle: abs(&bundle_dir),
                index: b.index_path(),
                level: lint.level,
                stats,
            };
            emit(json, &s, || {
                let top = |m: &BTreeMap<String, usize>| {
                    let mut v: Vec<_> = m.iter().collect();
                    v.sort_by(|a, b| b.1.cmp(a.1));
                    v.iter()
                        .take(8)
                        .map(|(k, n)| format!("{k}={n}"))
                        .collect::<Vec<_>>()
                        .join(", ")
                };
                let level = s.level.map_or("below L0".to_owned(), |l| l.to_string());
                format!(
                    "bundle:    {}\nindex:     {}\ndocuments: {} ({} index/log files)\ntokens:    ~{} (estimate)\nchunks:    {}\ntypes:     {}\nlangs:     {}\nlevel:     {level} (run `okfkit lint` for details)\nmode:      {:?} (Full = fits in context, Lexical = catalog + tools)\n",
                    s.bundle.display(),
                    s.index
                        .as_ref()
                        .map_or("(memory)".into(), |p| p.display().to_string()),
                    s.stats.docs,
                    s.stats.reserved,
                    s.stats.tokens,
                    s.stats.chunks,
                    top(&s.stats.types),
                    top(&s.stats.langs),
                    s.stats.mode
                )
            })?;
        }
        Command::Index { rebuild } => {
            if rebuild {
                let b = Bundle::open(
                    &bundle_dir,
                    OpenOptions::default().state_dir(state_dir.clone()),
                )?;
                if let Some(p) = b.index_path() {
                    drop(b);
                    for suffix in ["", "-wal", "-shm"] {
                        let f = PathBuf::from(format!("{}{suffix}", p.display()));
                        if f.exists() {
                            std::fs::remove_file(&f)
                                .with_context(|| format!("removing {}", f.display()))?;
                        }
                    }
                }
            }
            let b = Bundle::open(&bundle_dir, OpenOptions::default().state_dir(state_dir))?;
            let stats = b.sync()?;
            emit(json, &stats, || {
                let mut s = format!(
                    "indexed: {} added, {} updated, {} removed, {} unchanged\nindex: {}\n",
                    stats.added,
                    stats.updated,
                    stats.removed,
                    stats.unchanged,
                    b.index_path()
                        .map_or("(memory)".into(), |p| p.display().to_string())
                );
                for (p, why) in &stats.skipped {
                    s.push_str(&format!("skipped {p}: {why}\n"));
                }
                s
            })?;
        }
        Command::Grep {
            pattern,
            path,
            context,
            files_only,
            limit,
            filter,
        } => {
            let req = okfkit::GrepRequest {
                pattern,
                path,
                context,
                files_only,
                limit,
                filter: filter.to_filter()?,
            };
            let r = open()?.grep(&req, &scope()?)?;
            emit(json, &r, || r.to_text(files_only) + "\n")?;
        }
        Command::Get {
            id,
            section,
            lines,
            max_tokens,
        } => {
            let r = open()?.get(
                &okfkit::GetRequest {
                    id,
                    section,
                    lines,
                    max_tokens,
                },
                &scope()?,
            )?;
            emit(json, &r, || r.to_text() + "\n")?;
        }
        Command::List { dir } => {
            let r = open()?.list(dir.as_deref().unwrap_or("."), &scope()?)?;
            emit(json, &r, || r.content.clone())?;
        }
        Command::Query {
            filter,
            sort,
            limit,
            count_only,
            facet,
            sum,
        } => {
            let req = okfkit::QueryRequest {
                filter: filter.to_filter()?.unwrap_or_default(),
                sort,
                limit,
                count_only,
                facets: facet,
                sum_field: sum,
            };
            let r = open()?.query(&req, &scope()?)?;
            emit(json, &r, || r.to_text() + "\n")?;
        }
        Command::Catalog { max_tokens } => {
            let mut opts = okfkit::CatalogOptions::default();
            if let Some(m) = max_tokens {
                opts.max_tokens = m;
            }
            let r = open()?.catalog(&opts, &scope()?)?;
            emit(json, &r, || r.content.clone())?;
        }
        Command::Links { id } => {
            let r = open()?.links(&id, &scope()?)?;
            emit(json, &r, || {
                let mut s = format!("outgoing ({}):\n", r.outgoing.len());
                for l in &r.outgoing {
                    s.push_str(&format!(
                        "  {}{}\n",
                        l.id.as_deref().unwrap_or(&l.raw),
                        if l.exists { "" } else { " (missing)" }
                    ));
                }
                s.push_str(&format!("backlinks ({}):\n", r.backlinks.len()));
                for l in &r.backlinks {
                    s.push_str(&format!("  {}\n", l.id.as_deref().unwrap_or("")));
                }
                s
            })?;
        }
        Command::Lint {
            level,
            format,
            fix_safe,
            disable,
        } => {
            if !bundle_dir.is_dir() {
                bail!(
                    "bundle not found: {} is not a directory",
                    bundle_dir.display()
                );
            }
            let config = okfkit_lint::LintConfig {
                disabled: disable,
                ..okfkit_lint::LintConfig::level(level.into())
            };
            if fix_safe {
                let fixed = okfkit_lint::fix_safe(&bundle_dir, &config)?;
                for p in &fixed.created {
                    eprintln!("fixed: created {p}");
                }
                for p in &fixed.updated {
                    eprintln!("fixed: updated {p}");
                }
            }
            let report = okfkit_lint::lint(&bundle_dir, &config)?;
            let format = if json { LintFormat::Json } else { format };
            let mut out = std::io::stdout().lock();
            match format {
                LintFormat::Text => write!(out, "{}", report.to_text())?,
                LintFormat::Json => writeln!(out, "{}", serde_json::to_string_pretty(&report)?)?,
                LintFormat::Sarif => {
                    writeln!(out, "{}", serde_json::to_string_pretty(&report.to_sarif())?)?
                }
            }
            return Ok(if report.errors > 0 {
                ExitCode::from(1)
            } else {
                ExitCode::SUCCESS
            });
        }
        Command::Mcp {
            command:
                McpCmd::Serve {
                    stdio: _,
                    prefix,
                    disable,
                },
        } => {
            let b = open()?;
            let options = okfkit_mcp::ServerOptions { prefix, disable };
            let scopes: Arc<dyn okfkit_mcp::ScopeProvider> = Arc::new(scope()?);
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()?;
            rt.block_on(okfkit_mcp::serve_stdio(b, scopes, &options))?;
        }
        Command::Agent {
            command:
                AgentCmd::Install {
                    claude,
                    codex,
                    user,
                    project,
                    print,
                    name,
                    prefix,
                },
        } => {
            if claude == codex {
                bail!("invalid argument: choose exactly one of --claude or --codex");
            }
            if !bundle_dir.is_dir() {
                bail!(
                    "bundle not found: {} is not a directory",
                    bundle_dir.display()
                );
            }
            let home = std::env::var_os("HOME")
                .or_else(|| std::env::var_os("USERPROFILE"))
                .map(PathBuf::from)
                .context("cannot find the home directory (set HOME)")?;
            let project = abs(&project.unwrap_or_else(|| PathBuf::from(".")));
            let caps = Bundle::open_in_memory(&bundle_dir)?
                .capabilities()
                .iter()
                .map(str::to_owned)
                .collect();
            let opts = okfkit_skills::InstallOptions {
                agent: if claude {
                    okfkit_skills::Agent::Claude
                } else {
                    okfkit_skills::Agent::Codex
                },
                target: if user {
                    okfkit_skills::Target::User
                } else {
                    okfkit_skills::Target::Project(project)
                },
                home,
                command: std::env::current_exe().context("locating the okfkit binary")?,
                server_name: name,
                skill: okfkit_skills::SkillContext {
                    bundle: abs(&bundle_dir),
                    prefix,
                    capabilities: caps,
                },
            };
            let actions = okfkit_skills::plan(&opts)?;
            if print {
                for a in &actions {
                    println!("{a}");
                }
            } else {
                okfkit_skills::apply(&actions)?;
                for a in &actions {
                    match a {
                        okfkit_skills::Action::Write { path, why, .. } => {
                            println!("{why}: wrote {}", path.display())
                        }
                        okfkit_skills::Action::Run { why, .. } => println!("{why}: done"),
                    }
                }
                println!("Restart the agent to pick up the MCP server and skills.");
            }
        }
        Command::Modules => {
            #[derive(Serialize)]
            struct Module {
                name: &'static str,
                status: &'static str,
                capabilities: Vec<&'static str>,
            }
            let caps: Vec<&'static str> = Bundle::open_in_memory(Path::new("."))
                .map(|b| b.capabilities().iter().collect())
                .unwrap_or_default();
            let modules = vec![
                Module {
                    name: "core",
                    status: "on",
                    capabilities: caps,
                },
                Module {
                    name: "data",
                    status: "not in this build (planned for v0.2)",
                    capabilities: vec![],
                },
                Module {
                    name: "embed-local",
                    status: "not in this build (planned for v0.3)",
                    capabilities: vec![],
                },
                Module {
                    name: "embed-api",
                    status: "not in this build (planned for v0.3)",
                    capabilities: vec![],
                },
                Module {
                    name: "import",
                    status: "not in this build (planned for v0.4)",
                    capabilities: vec![],
                },
                Module {
                    name: "write",
                    status: "not in this build",
                    capabilities: vec![],
                },
            ];
            emit(json, &modules, || {
                modules
                    .iter()
                    .map(|m| {
                        format!(
                            "{:<12} {:<40} {}\n",
                            m.name,
                            m.status,
                            m.capabilities.join(" ")
                        )
                    })
                    .collect()
            })?;
        }
        Command::External(args) => return run_plugin(&args, cli.bundle.as_deref()),
    }
    Ok(ExitCode::SUCCESS)
}

fn abs(p: &Path) -> PathBuf {
    p.canonicalize().unwrap_or_else(|_| p.to_owned())
}

/// Prints JSON (pretty) or the text form.
fn emit<T: Serialize>(json: bool, value: &T, text: impl FnOnce() -> String) -> Result<()> {
    let mut out = std::io::stdout().lock();
    if json {
        writeln!(out, "{}", serde_json::to_string_pretty(value)?)?;
    } else {
        let t = text();
        write!(out, "{t}")?;
        if !t.ends_with('\n') {
            writeln!(out)?;
        }
    }
    Ok(())
}

/// Runs `okfkit-<name>` from PATH (git-style plugins), passing the bundle in `OKFKIT_BUNDLE`.
fn run_plugin(args: &[String], bundle: Option<&Path>) -> Result<ExitCode> {
    let (name, rest) = args.split_first().context("missing command")?;
    let program = format!("okfkit-{name}");
    let mut cmd = std::process::Command::new(&program);
    cmd.args(rest);
    if let Some(b) = bundle {
        cmd.env("OKFKIT_BUNDLE", abs(b));
    }
    match cmd.status() {
        Ok(status) => Ok(ExitCode::from(
            status.code().unwrap_or(1).clamp(0, 255) as u8
        )),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            bail!("unknown command `{name}` (no `{program}` plugin in PATH); see `okfkit --help`")
        }
        Err(e) => Err(e).with_context(|| format!("running {program}")),
    }
}

impl FilterArgs {
    /// The metadata filter, or `None` if no filter flag was given.
    fn to_filter(&self) -> Result<Option<okfkit::Filter>> {
        let mut fields: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for kv in &self.field {
            let (k, v) = kv.split_once('=').with_context(|| {
                format!("invalid argument: --field expects KEY=VALUE, got {kv:?}")
            })?;
            fields
                .entry(k.trim().to_owned())
                .or_default()
                .extend(v.split(',').map(|x| x.trim().to_owned()));
        }
        let f = okfkit::Filter {
            types: self.r#type.clone(),
            tags_all: self.tag.clone(),
            tags_any: self.any_tag.clone(),
            status: self.status.clone(),
            status_not: self.status_not.clone(),
            lang: self.lang.clone(),
            fields,
            path: self.under.clone(),
            text: self.text.clone(),
            updated_from: self.updated_from.clone(),
            updated_to: self.updated_to.clone(),
            ranges: BTreeMap::new(),
            active_on: self.active_on.clone(),
        };
        Ok((f != okfkit::Filter::default()).then_some(f))
    }
}
