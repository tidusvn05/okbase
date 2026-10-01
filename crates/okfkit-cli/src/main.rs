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

use cli::{AgentCmd, Cli, Command, DataCmd, DictCmd, EmbedCmd, FilterArgs, LintFormat, McpCmd};

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
    } else if msg.contains("SQL error") || msg.contains("query interrupted") {
        Some(
            "list tables and columns with `okfkit data tables`; only one SELECT (or WITH ... SELECT) is allowed",
        )
    } else if msg.contains("no datasets") {
        Some("put CSV, TSV or XLSX files in the bundle (for example under data/)")
    } else if msg.contains("embeddings are off") || msg.contains("not available in this build") {
        Some(
            "semantic search needs the okfkit-full build and `okfkit embed enable`; the lexical tools (grep, query, get) work without it",
        )
    } else if msg.contains("no embeddings for") {
        Some("run `okfkit embed index` first")
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
        Command::Adopt {
            dir,
            out,
            write,
            force,
            level,
            verbose,
        } => {
            let dir = dir.unwrap_or_else(|| bundle_dir.clone());
            if !dir.is_dir() {
                bail!("bundle not found: {} is not a directory", dir.display());
            }
            let mut opts = okfkit_adopt::AdoptOptions::new(&today());
            opts.level = level.into();
            let plan = okfkit_adopt::plan(&dir, &opts)?;
            emit(json, &plan, || plan.to_text(verbose))?;
            if let Some(out) = out {
                okfkit_adopt::apply_to(&dir, &plan, &out)?;
                eprintln!("wrote the adopted bundle to {}", out.display());
            } else if write {
                if !force {
                    let status = std::process::Command::new("git")
                        .args(["status", "--porcelain", "--", "."])
                        .current_dir(&dir)
                        .output();
                    match status {
                        Ok(o) if o.status.success() && o.stdout.is_empty() => {}
                        Ok(o) if o.status.success() => bail!(
                            "refusing to edit in place: {} has uncommitted changes (commit them, or pass --force)",
                            dir.display()
                        ),
                        _ => bail!(
                            "refusing to edit in place: {} is not in a git repository (use --out, or pass --force)",
                            dir.display()
                        ),
                    }
                }
                okfkit_adopt::apply_in_place(&dir, &plan)?;
                eprintln!("updated {} files in place", plan.changes.len());
            } else if !json {
                eprintln!("plan only; nothing was written. Use --out DIR or --write to apply it.");
            }
        }
        Command::Vocab { suggest, write } => {
            if !bundle_dir.is_dir() {
                bail!(
                    "bundle not found: {} is not a directory",
                    bundle_dir.display()
                );
            }
            let report = okfkit_adopt::vocab::report(&bundle_dir)?;
            if !suggest {
                emit(json, &report, || report.to_text())?;
                return Ok(ExitCode::SUCCESS);
            }
            let actor = format!("okfkit-vocab/{}", env!("CARGO_PKG_VERSION"));
            let text = okfkit_adopt::vocab::suggest(&report, &actor, &today())?;
            if write {
                let path = bundle_dir.join(okfkit_adopt::vocab::VOCABULARY_PATH);
                if path.exists() {
                    bail!(
                        "{} already exists; edit it instead (or delete it first)",
                        path.display()
                    );
                }
                std::fs::create_dir_all(path.parent().expect("has parent"))?;
                std::fs::write(&path, &text)?;
                eprintln!("wrote {}", path.display());
            } else {
                print!("{text}");
            }
        }
        Command::Search {
            query,
            limit,
            filter,
        } => {
            let req = okfkit::SearchRequest {
                query,
                limit,
                per_doc: 2,
                filter: filter.to_filter()?,
            };
            let r = open()?.search(&req, &scope()?)?;
            emit(json, &r, || r.to_text() + "\n")?;
        }
        Command::Retrieve { query, budget } => {
            let r = open()?.retrieve(&query, budget, &scope()?)?;
            emit(json, &r, || r.to_text())?;
        }
        Command::Embed { command } => {
            match command {
                EmbedCmd::Models => {
                    #[derive(Serialize)]
                    struct Row {
                        id: &'static str,
                        name: &'static str,
                        license: &'static str,
                        license_url: &'static str,
                        accepted: bool,
                        size_mb: usize,
                        s1_r_at_1: f32,
                    }
                    let rows: Vec<Row> = okfkit::embedding_models()
                        .iter()
                        .map(|m| Row {
                            id: m.id,
                            name: m.name,
                            license: m.license,
                            license_url: m.license_url,
                            accepted: okfkit::license_accepted(m),
                            size_mb: m.size_mb,
                            s1_r_at_1: m.s1_r_at_1,
                        })
                        .collect();
                    emit(json, &rows, || {
                        rows.iter()
                            .map(|r| {
                                format!(
                                    "{:<24} {:<26} ~{} MB  R@1 {:.3} (spike S1)  {}{}\n",
                                    r.id,
                                    r.name,
                                    r.size_mb,
                                    r.s1_r_at_1,
                                    r.license,
                                    if r.accepted {
                                        ""
                                    } else {
                                        " — needs --accept-license"
                                    }
                                )
                            })
                            .collect()
                    })?;
                }
                EmbedCmd::Enable {
                    model,
                    accept_license,
                    api_url,
                    api_model,
                    api_key_env,
                } => {
                    if !bundle_dir.is_dir() {
                        bail!(
                            "bundle not found: {} is not a directory",
                            bundle_dir.display()
                        );
                    }
                    let cfg = match (api_url, api_model) {
                        (Some(base_url), Some(model)) => okfkit::config::EmbedConfig::Api {
                            base_url,
                            model,
                            key_env: api_key_env,
                        },
                        _ => {
                            let info = okfkit::find_model(&model).with_context(|| format!("invalid argument: unknown model {model}; see `okfkit embed models`"))?;
                            if info.requires_acceptance && !okfkit::license_accepted(info) {
                                if !accept_license {
                                    bail!(
                                        "{} is released under the {} ({}). Read them, then run again with --accept-license \
                                     (or choose --model bge-m3-int8, MIT)",
                                        info.name,
                                        info.license,
                                        info.license_url
                                    );
                                }
                                okfkit::accept_license(info)?;
                                eprintln!(
                                    "recorded acceptance of the {} for {}",
                                    info.license, info.id
                                );
                            }
                            okfkit::config::EmbedConfig::Local { model }
                        }
                    };
                    let path = okfkit::config::write_embed(&bundle_dir, &cfg)?;
                    eprintln!(
                        "embeddings enabled in {}; run `okfkit embed index` to embed the bundle",
                        path.display()
                    );
                }
                EmbedCmd::Disable => {
                    let path = okfkit::config::write_embed(
                        &bundle_dir,
                        &okfkit::config::EmbedConfig::Off,
                    )?;
                    eprintln!("embeddings disabled in {}", path.display());
                }
                EmbedCmd::Status => {
                    let b = open()?;
                    let st = b.embed_status()?;
                    emit(json, &st, || {
                        format!(
                            "model: {}\nembedded: {}/{} chunks\n",
                            st.model, st.embedded, st.chunks
                        )
                    })?;
                }
                EmbedCmd::Index => {
                    let b = open()?;
                    let started = std::time::Instant::now();
                    let st = b.embed_sync(&mut |done, total| {
                        eprint!(
                            "\rembedding {done}/{total} chunks ({:.1}/s)   ",
                            done as f64 / started.elapsed().as_secs_f64().max(0.001)
                        );
                    })?;
                    eprintln!();
                    emit(json, &st, || {
                        format!(
                            "model: {}\nembedded: {}/{} chunks in {:.0?}\n",
                            st.model,
                            st.embedded,
                            st.chunks,
                            started.elapsed()
                        )
                    })?;
                }
            }
        }
        Command::Dict { command } => {
            #[derive(Serialize)]
            struct DictStatus {
                installed: bool,
                dir: Option<PathBuf>,
                downloads_allowed: bool,
                source: &'static str,
            }
            if matches!(command, DictCmd::Install) {
                let dir = okfkit::analyze::dict::install()?;
                eprintln!("Japanese dictionary ready in {}", dir.display());
            }
            let st = DictStatus {
                installed: okfkit::analyze::dict::installed(),
                dir: okfkit::analyze::dict::dictionary_dir(),
                downloads_allowed: okfkit::analyze::dict::downloads_allowed(),
                source: okfkit::analyze::dict::IPADIC_URL,
            };
            emit(json, &st, || {
                format!(
                    "Japanese dictionary (mecab-ipadic): {}
location: {}
{}",
                    if st.installed {
                        "installed"
                    } else {
                        "not installed"
                    },
                    st.dir
                        .as_ref()
                        .map_or("(no cache directory)".into(), |d| d.display().to_string()),
                    if st.installed {
                        String::new()
                    } else if st.downloads_allowed {
                        "it is downloaded the first time Japanese text is indexed (`okfkit dict install` does it now)\n".into()
                    } else {
                        "downloads are disabled (OKFKIT_OFFLINE); Japanese text is indexed as character bigrams\n".into()
                    }
                )
            })?;
        }
        Command::Data { command } => {
            let b = open()?;
            let scope = scope()?;
            match command {
                DataCmd::Tables => {
                    let r = b.data_tables(&scope)?;
                    emit(json, &r, || r.to_text())?;
                }
                DataCmd::Sql {
                    query,
                    max_rows,
                    timeout,
                } => {
                    let limits = okfkit::DataLimits {
                        max_rows,
                        timeout: std::time::Duration::from_secs(timeout),
                    };
                    let r = b.data_query(&query, &limits, &scope)?;
                    emit(json, &r, || r.to_text())?;
                }
            }
        }
        Command::Mcp {
            command:
                McpCmd::Serve {
                    stdio: _,
                    http,
                    allow_host,
                    token_env,
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
            match http {
                None => rt.block_on(okfkit_mcp::serve_stdio(b, scopes, &options))?,
                Some(addr) => {
                    let token = std::env::var(&token_env).ok().filter(|t| !t.is_empty());
                    let http = okfkit_mcp::HttpOptions {
                        token,
                        allowed_hosts: allow_host,
                    };
                    rt.block_on(okfkit_mcp::serve_http(b, scopes, &options, &http, addr))?;
                }
            }
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
            let all: Vec<&'static str> = Bundle::open_in_memory(&bundle_dir)
                .map(|b| b.capabilities().iter().collect())
                .unwrap_or_default();
            let data_on = all.contains(&okfkit::capability::DATA_SQL);
            let caps: Vec<&'static str> = all
                .into_iter()
                .filter(|c| !c.starts_with("data."))
                .collect();
            let modules = vec![
                Module {
                    name: "core",
                    status: "on",
                    capabilities: caps,
                },
                Module {
                    name: "data",
                    status: if data_on {
                        "on"
                    } else {
                        "available; no CSV/TSV/XLSX files in this bundle"
                    },
                    capabilities: if data_on {
                        vec![okfkit::capability::DATA_SQL]
                    } else {
                        vec![]
                    },
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

/// Today's date (UTC) as `YYYY-MM-DD`, without a date-time dependency.
fn today() -> String {
    let days = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() / 86_400) as i64;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    format!("{:04}-{m:02}-{d:02}", yoe + era * 400 + i64::from(m <= 2))
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
