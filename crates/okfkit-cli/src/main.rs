//! The `okfkit` command-line interface.

mod cli;
mod contract;
mod doctor;
mod onboard;

use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

use anyhow::{Context as _, Result, bail};
use clap::Parser as _;
use okfkit::{Bundle, OpenOptions, Scope, StateDir};
use serde::Serialize;

use cli::{
    AgentCmd, AudienceArg, Cli, Command, DataCmd, DictCmd, EmbedCmd, FilterArgs, GoalArg,
    LintFormat, McpCmd, ModelsCmd, TrainBackend, TuneCmd,
};

fn main() -> ExitCode {
    let cli = Cli::parse();
    let json = cli.json;
    match run(cli) {
        Ok(code) => code,
        Err(e) => {
            let r = contract::classify(&e);
            contract::print_error(&r, json);
            ExitCode::from(r.exit)
        }
    }
}

fn run(cli: Cli) -> Result<ExitCode> {
    let json = cli.json;
    let state_arg = cli.state_dir.clone();
    let filters = (cli.allow.clone(), cli.deny.clone());
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
        Command::Doctor => {
            use doctor::{Status, check};
            let home = std::env::var_os("HOME")
                .or_else(|| std::env::var_os("USERPROFILE"))
                .map(PathBuf::from)
                .unwrap_or_default();
            let feats = okfkit::build_features();
            let mut checks = vec![check(
                "okfkit",
                Status::Ok,
                format!(
                    "{} ({})",
                    env!("CARGO_PKG_VERSION"),
                    if feats.embed_local {
                        "okfkit-full"
                    } else {
                        "default build"
                    }
                ),
                None,
            )];
            if !bundle_dir.is_dir() {
                checks.push(check(
                    "bundle",
                    Status::Fail,
                    format!("{} is not a folder", bundle_dir.display()),
                    Some("okfkit -b <folder> doctor"),
                ));
                let r = doctor::Report::new(checks);
                emit(json, &r, || r.to_text())?;
                return Ok(ExitCode::from(contract::EXIT_FINDINGS));
            }
            let bundle_abs = abs(&bundle_dir);
            let b = Bundle::open(
                &bundle_dir,
                OpenOptions::default().state_dir(state_dir.clone()),
            )?;
            match b.sync() {
                Ok(st) if st.skipped.is_empty() => {
                    checks.push(check("index", Status::Ok, "up to date", None));
                }
                Ok(st) => checks.push(check(
                    "index",
                    Status::Warn,
                    format!(
                        "{} files could not be read (first: {}: {})",
                        st.skipped.len(),
                        st.skipped[0].0,
                        st.skipped[0].1
                    ),
                    Some("okfkit lint --level L1"),
                )),
                Err(e) => checks.push(check(
                    "index",
                    Status::Fail,
                    format!("{e}"),
                    Some("okfkit index --rebuild"),
                )),
            }
            let scope = scope()?;
            let advice = b.advise(&okfkit::AdviseOptions::default(), &scope)?;
            let p = &advice.profile;
            checks.push(match p.level {
                None => check(
                    "level",
                    Status::Warn,
                    "plain markdown, not OKF yet",
                    Some("okfkit adopt . --out <new folder> (ask the user)"),
                ),
                Some(l) if l < okfkit::Level::L2 => check(
                    "level",
                    Status::Warn,
                    if p.missing_descriptions > 0 {
                        format!(
                            "level {l}; {} documents without a description",
                            p.missing_descriptions
                        )
                    } else {
                        format!("level {l} (L2 helps agents pick documents)")
                    },
                    Some("okfkit lint --level L2"),
                ),
                Some(l) => check("level", Status::Ok, format!("level {l}"), None),
            });
            // Agents connected to this bundle.
            let mut prefix = "kb".to_owned();
            let installs: Vec<okfkit_skills::Install> = okfkit_skills::registry_path()
                .map(|r| okfkit_skills::load_registry(&r).unwrap_or_default())
                .unwrap_or_default()
                .into_iter()
                .filter(|i| {
                    // Installs for this bundle, and any install of this project (it may still
                    // point at a bundle that moved).
                    i.source == bundle_abs.display().to_string()
                        || matches!(&i.target, okfkit_skills::Target::Project(d) if Some(d) == std::env::current_dir().ok().map(|c| abs(&c)).as_ref())
                })
                .collect();
            for i in &installs {
                let problems = i.check(&home);
                if problems.is_empty() && i.source != bundle_abs.display().to_string() {
                    // Another healthy bundle of this project (several bundles side by side).
                    continue;
                }
                let place = match &i.target {
                    okfkit_skills::Target::User => "user".to_owned(),
                    okfkit_skills::Target::Project(d) => d.display().to_string(),
                };
                if problems.is_empty() {
                    checks.push(check(
                        "agents",
                        Status::Ok,
                        format!("{} ({place}) as `{}`", i.agent.label(), i.name),
                        None,
                    ));
                } else {
                    checks.push(check(
                        "agents",
                        Status::Fail,
                        format!("{} ({place}): {}", i.agent.label(), problems.join("; ")),
                        Some("okfkit agent install --replace (ask the user)"),
                    ));
                }
                if let okfkit_skills::Server::Stdio { .. } = i.server
                    && i.name != okfkit_skills::DEFAULT_NAME
                {
                    prefix = okfkit_skills::default_prefix(&i.name);
                }
            }
            if !checks.iter().any(|c| c.id == "agents") {
                checks.push(check(
                    "agents",
                    Status::Warn,
                    "no agent is connected to this bundle",
                    Some("okfkit agent install"),
                ));
            }
            // Embeddings.
            match (b.embedding_model(), p.embed.as_ref()) {
                (None, _) => {
                    checks.push(check("embed", Status::Ok, "off (lexical tools only)", None))
                }
                (Some(m), _) if !(feats.embed_local || feats.embed_api) => checks.push(check(
                    "embed",
                    Status::Fail,
                    format!("okfkit.toml asks for {m}, but this build has no embed module"),
                    Some("cargo install okfkit-cli --features full"),
                )),
                (Some(m), st) => {
                    let base = m.split('@').next().unwrap_or(&m).to_owned();
                    let license = okfkit::find_model(&base).or_else(|| {
                        okfkit::find_custom(&base)
                            .ok()
                            .and_then(|c| c.manifest.base.as_deref().and_then(okfkit::find_model))
                    });
                    if let Some(info) = license.filter(|i| !okfkit::license_accepted(i)) {
                        checks.push(check(
                            "embed",
                            Status::Fail,
                            format!("{base} needs the {} accepted", info.license),
                            Some("okfkit embed enable --accept-license (ask the user)"),
                        ));
                    } else if let Some(st) = st.filter(|s| s.chunks > 0 && !s.complete()) {
                        checks.push(check(
                            "embed",
                            Status::Warn,
                            format!("{base}: {}/{} chunks embedded", st.embedded, st.chunks),
                            Some("okfkit embed index"),
                        ));
                    } else {
                        checks.push(check(
                            "embed",
                            Status::Ok,
                            format!("{base}, every chunk embedded"),
                            None,
                        ));
                    }
                }
            }
            // Japanese dictionary.
            if p.langs
                .get("ja")
                .is_some_and(|s| *s >= okfkit::advise::LANG_MIN_SHARE)
            {
                let dict = okfkit::analyze::dict::embedded() || okfkit::analyze::dict::installed();
                checks.push(if dict {
                    check("dict", Status::Ok, "Japanese dictionary ready", None)
                } else if okfkit::analyze::dict::downloads_allowed() {
                    check(
                        "dict",
                        Status::Warn,
                        "Japanese dictionary not installed yet (downloaded on first use)",
                        Some("okfkit dict install"),
                    )
                } else {
                    check(
                        "dict",
                        Status::Fail,
                        "Japanese dictionary missing and downloads are off (OKFKIT_OFFLINE)",
                        Some("okfkit dict install"),
                    )
                });
            }
            // A real MCP round trip.
            let exe = std::env::current_exe().context("locating the okfkit binary")?;
            checks.push(
                match doctor::mcp_round_trip(&exe, &bundle_abs, state_arg.as_deref(), &prefix) {
                    Ok(n) => check(
                        "mcp",
                        Status::Ok,
                        format!("server starts; {n} tools; {prefix}_catalog answers"),
                        None,
                    ),
                    Err(e) => check(
                        "mcp",
                        Status::Fail,
                        e,
                        Some("okfkit mcp serve --stdio (run it to see the error)"),
                    ),
                },
            );
            let r = doctor::Report::new(checks);
            emit(json, &r, || r.to_text())?;
            if !r.ok {
                return Ok(ExitCode::from(contract::EXIT_FINDINGS));
            }
        }
        Command::Help { command, agent } => {
            let mut root = <Cli as clap::CommandFactory>::command();
            if agent {
                let mut cmds = Vec::new();
                fn walk(c: &clap::Command, prefix: &str, out: &mut Vec<(String, String)>) {
                    for sub in c.get_subcommands() {
                        let name = format!("{prefix}{}", sub.get_name());
                        if sub.has_subcommands() && sub.get_name() != "help" {
                            walk(sub, &format!("{name} "), out);
                        } else if sub.get_name() != "help" {
                            out.push((
                                name,
                                sub.get_about().map(|a| a.to_string()).unwrap_or_default(),
                            ));
                        }
                    }
                }
                walk(&root, "", &mut cmds);
                let text = contract::agent_guide(&cmds);
                let v = serde_json::json!({
                    "guide": text,
                    "exit_codes": {"0": "success", "1": "error", "2": "usage", "3": "consent required", "4": "findings"},
                    "error_codes": contract::CODES.iter().map(|(c, e, w)| serde_json::json!({"code": c, "exit": e, "meaning": w})).collect::<Vec<_>>(),
                    "consent": contract::CONSENT.iter().map(|(a, f, t)| serde_json::json!({"action": a, "flag": f, "tell": t})).collect::<Vec<_>>(),
                    "commands": cmds.iter().map(|(n, a)| serde_json::json!({"command": format!("okfkit {n}"), "about": a})).collect::<Vec<_>>(),
                    "next": ["okfkit onboard"],
                });
                emit(json, &v, || text.clone())?;
            } else {
                root.build();
                let mut cmd = &mut root;
                for name in &command {
                    cmd = cmd.find_subcommand_mut(name).with_context(|| {
                        format!("invalid argument: unknown command `{}`", command.join(" "))
                    })?;
                }
                cmd.print_long_help()?;
            }
        }
        Command::Onboard {
            user_langs,
            audience,
            private,
            goal,
        } => {
            let home = std::env::var_os("HOME")
                .or_else(|| std::env::var_os("USERPROFILE"))
                .map(PathBuf::from)
                .unwrap_or_default();
            let goal = match goal {
                GoalArg::Answer => onboard::Goal::Answer,
                GoalArg::Curate => onboard::Goal::Curate,
                GoalArg::Remove => onboard::Goal::Remove,
            };
            let feats = okfkit::build_features();
            let mut st = onboard::State {
                version: env!("CARGO_PKG_VERSION").into(),
                full_build: feats.embed_local,
                bundle_exists: bundle_dir.is_dir(),
                bundle: abs(&bundle_dir).display().to_string(),
                agents_found: detect_agents(&home)
                    .into_iter()
                    .map(|a| a.label())
                    .collect(),
                gemma_accepted: okfkit::find_model("embeddinggemma-300m-q4")
                    .is_some_and(okfkit::license_accepted),
                ..Default::default()
            };
            if st.bundle_exists && goal != onboard::Goal::Remove {
                let b = open()?;
                let options = okfkit::AdviseOptions {
                    user_langs,
                    audience: audience.map(audience_of),
                    private,
                };
                st.advice = Some(b.advise(&options, &scope()?)?);
                st.tune_started = b.tune_runs().is_ok_and(|r| !r.is_empty());
                if let Some(reg) = okfkit_skills::registry_path() {
                    for i in okfkit_skills::load_registry(&reg).unwrap_or_default() {
                        if i.source != st.bundle {
                            continue;
                        }
                        let problems = i.check(&home);
                        if problems.is_empty() {
                            if !st.installed_for.contains(&i.agent.label()) {
                                st.installed_for.push(i.agent.label());
                            }
                        } else {
                            st.install_problems.extend(problems);
                        }
                    }
                }
            }
            let plan = onboard::plan(&st, goal);
            emit(json, &plan, || plan.to_text())?;
        }
        Command::Advise {
            user_langs,
            audience,
            private,
        } => {
            let b = open()?;
            let scope = scope()?;
            let options = okfkit::AdviseOptions {
                user_langs,
                audience: audience.map(audience_of),
                private,
            };
            let mut advice = b.advise(&options, &scope)?;
            if let Some(dir) = &cli.bundle {
                // Commands must work from where the user is.
                let flag = format!("okfkit -b {} ", shell_quote(&dir.display().to_string()));
                for step in &mut advice.steps {
                    for c in &mut step.commands {
                        if let Some(rest) = c.strip_prefix("okfkit ") {
                            *c = format!("{flag}{rest}");
                        } else if let Some(i) = c.find(" okfkit ") {
                            c.replace_range(i + 1..i + 8, &flag);
                        }
                    }
                }
            }
            emit(json, &advice, || advice.to_text())?;
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
                ExitCode::from(contract::EXIT_FINDINGS)
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
            only,
        } => {
            let dir = dir.unwrap_or_else(|| bundle_dir.clone());
            if !dir.is_dir() {
                bail!("bundle not found: {} is not a directory", dir.display());
            }
            let mut opts = okfkit_adopt::AdoptOptions::new(&today());
            opts.level = level.into();
            opts.only = only;
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
                done(
                    json,
                    serde_json::json!({"wrote": path}),
                    format!("wrote {}", path.display()),
                    &["okfkit lint --level L2"],
                )?;
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
        Command::Embed { command } => match command {
            EmbedCmd::Models { command } => match command.unwrap_or(ModelsCmd::List) {
                ModelsCmd::List => {
                    #[derive(Serialize)]
                    struct Row {
                        id: String,
                        name: String,
                        license: String,
                        license_url: String,
                        accepted: bool,
                        size_mb: usize,
                        s1_r_at_1: Option<f32>,
                        custom: bool,
                    }
                    let mut rows: Vec<Row> = okfkit::embedding_models()
                        .iter()
                        .map(|m| Row {
                            id: m.id.into(),
                            name: m.name.into(),
                            license: m.license.into(),
                            license_url: m.license_url.into(),
                            accepted: okfkit::license_accepted(m),
                            size_mb: m.size_mb,
                            s1_r_at_1: Some(m.s1_r_at_1),
                            custom: false,
                        })
                        .collect();
                    for c in okfkit::custom_models()? {
                        let m = &c.manifest;
                        let base = m.base.as_deref().and_then(okfkit::find_model);
                        let size: u64 = std::iter::once(&m.onnx)
                            .chain(&m.external_data)
                            .filter_map(|f| std::fs::metadata(c.dir.join(f)).ok())
                            .map(|md| md.len())
                            .sum();
                        rows.push(Row {
                            id: format!("custom:{}", m.name),
                            name: if m.description.is_empty() {
                                m.name.clone()
                            } else {
                                m.description.clone()
                            },
                            license: m.license.clone(),
                            license_url: m.license_url.clone(),
                            accepted: base.is_none_or(okfkit::license_accepted),
                            size_mb: (size / 1_000_000) as usize,
                            s1_r_at_1: None,
                            custom: true,
                        });
                    }
                    emit(json, &rows, || {
                        rows.iter()
                            .map(|r| {
                                format!(
                                    "{:<24} {:<26} ~{} MB  {}  {}{}\n",
                                    r.id,
                                    r.name,
                                    r.size_mb,
                                    r.s1_r_at_1.map_or("custom".to_owned(), |q| format!(
                                        "R@1 {q:.3} (spike S1)"
                                    )),
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
                ModelsCmd::Add { dir, name, replace } => {
                    let c = okfkit::install_custom(&dir, name.as_deref(), replace)?;
                    let id = format!("custom:{}", c.manifest.name);
                    done(
                        json,
                        serde_json::json!({"model": id, "dir": c.dir}),
                        format!("installed {id} in {}", c.dir.display()),
                        &[&format!("okfkit embed enable --model {id}")],
                    )?;
                }
                ModelsCmd::Remove { name } => {
                    okfkit::remove_custom(&name)?;
                    done(
                        json,
                        serde_json::json!({"removed": name}),
                        format!("removed {name}"),
                        &["okfkit embed models"],
                    )?;
                }
                ModelsCmd::Pull { id, accept_license } => {
                    let info = okfkit::find_model(&id).with_context(|| {
                        format!("invalid argument: unknown model {id}; see `okfkit embed models`")
                    })?;
                    ensure_license(info, accept_license)?;
                    okfkit::load_local_model(&id)?;
                    done(
                        json,
                        serde_json::json!({"model": id, "downloaded": true}),
                        format!("{id} is downloaded"),
                        &[&format!("okfkit embed enable --model {id}")],
                    )?;
                }
            },
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
                        let info = if model.starts_with("custom:") {
                            okfkit::find_custom(&model)?
                                .manifest
                                .base
                                .as_deref()
                                .and_then(okfkit::find_model)
                        } else {
                            Some(okfkit::find_model(&model).with_context(|| format!("invalid argument: unknown model {model}; see `okfkit embed models`"))?)
                        };
                        if let Some(info) = info {
                            ensure_license(info, accept_license)?;
                        }
                        okfkit::config::EmbedConfig::Local { model }
                    }
                };
                let path = okfkit::config::write_embed(&bundle_dir, &cfg)?;
                done(
                    json,
                    serde_json::json!({"config": path, "embed": cfg}),
                    format!("embeddings enabled in {}", path.display()),
                    &["okfkit embed index"],
                )?;
            }
            EmbedCmd::Disable => {
                let path =
                    okfkit::config::write_embed(&bundle_dir, &okfkit::config::EmbedConfig::Off)?;
                done(
                    json,
                    serde_json::json!({"config": path, "embed": okfkit::config::EmbedConfig::Off}),
                    format!(
                        "embeddings disabled in {} (vectors stay cached)",
                        path.display()
                    ),
                    &[],
                )?;
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
            EmbedCmd::Eval {
                models,
                questions,
                limit,
            } => {
                let b = open()?;
                let scope = scope()?;
                let models = if models.is_empty() {
                    vec![b.embedding_model().map(|m| m.split('@').next().unwrap_or(&m).to_owned()).context(
                        "no model to measure: pass --model, or enable embeddings (okfkit embed enable)",
                    )?]
                } else {
                    models
                };
                let (path, source) = match questions {
                    Some(p) => (p.clone(), p.display().to_string()),
                    None => {
                        let human = bundle_dir.join(okfkit::tune::HUMAN_EVAL_PATH);
                        if human.is_file() {
                            (human, okfkit::tune::HUMAN_EVAL_PATH.to_owned())
                        } else {
                            let run = b.tune_run(None).context(
                                "no questions: pass --questions, add _meta/eval/questions.jsonl, or finish a tune run",
                            )?;
                            let p = run.heldout_path();
                            if !p.is_file() {
                                bail!(
                                    "run {} has no held-out set yet; run `okfkit embed tune check`",
                                    run.plan().id
                                );
                            }
                            (p, format!("held-out questions of {}", run.plan().id))
                        }
                    }
                };
                let mut qs = okfkit::tune::load_questions(&path)?;
                if let Some(n) = limit {
                    qs.truncate(n);
                }
                let report = okfkit::tune::eval_models(
                    &okfkit::tune::EvalRequest {
                        root: &bundle_dir,
                        state_dir: state_dir.clone(),
                        vector_cache: None,
                        models: &models,
                        questions: &qs,
                        source: &source,
                        scope: &scope,
                    },
                    &mut |m, d, n| eprint!("\r{m}: embedding {d}/{n} chunks   "),
                )?;
                eprintln!();
                emit(json, &report, || report.to_text())?;
            }
            EmbedCmd::Tune { command } => {
                if let Some(code) = tune_command(command, json, &state_dir, &open, &scope)? {
                    return Ok(code);
                }
            }
        },
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
                installed: okfkit::analyze::dict::embedded() || okfkit::analyze::dict::installed(),
                dir: okfkit::analyze::dict::dictionary_dir(),
                downloads_allowed: okfkit::analyze::dict::downloads_allowed(),
                source: okfkit::analyze::dict::IPADIC_URL,
            };
            emit(json, &st, || {
                format!(
                    "Japanese dictionary (mecab-ipadic): {}
location: {}
{}",
                    if okfkit::analyze::dict::embedded() {
                        "embedded in this build"
                    } else if st.installed {
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
        Command::Agent { command } => {
            let home = std::env::var_os("HOME")
                .or_else(|| std::env::var_os("USERPROFILE"))
                .map(PathBuf::from)
                .context("cannot find the home directory (set HOME)")?;
            let registry = okfkit_skills::registry_path();
            match command {
                AgentCmd::Install {
                    claude,
                    codex,
                    user,
                    project,
                    print,
                    name,
                    prefix,
                    url,
                    token_env,
                    replace,
                } => {
                    let agents = chosen_agents(claude, codex, &home)?;
                    let target = if user {
                        okfkit_skills::Target::User
                    } else {
                        okfkit_skills::Target::Project(abs(
                            &project.unwrap_or_else(|| PathBuf::from("."))
                        ))
                    };
                    let prefix = prefix.unwrap_or_else(|| okfkit_skills::default_prefix(&name));
                    let (server, skill) = match url {
                        Some(url) => (
                            okfkit_skills::Server::Http {
                                url: url.clone(),
                                token_env,
                            },
                            okfkit_skills::SkillContext {
                                bundle: PathBuf::from(&url),
                                prefix,
                                capabilities: vec!["remote".into()],
                                allow: vec![],
                                deny: vec![],
                            },
                        ),
                        None => {
                            if !bundle_dir.is_dir() {
                                bail!(
                                    "bundle not found: {} is not a directory",
                                    bundle_dir.display()
                                );
                            }
                            let caps = Bundle::open_in_memory(&bundle_dir)?
                                .capabilities()
                                .iter()
                                .map(str::to_owned)
                                .collect();
                            (
                                okfkit_skills::Server::Stdio {
                                    command: std::env::current_exe()
                                        .context("locating the okfkit binary")?,
                                },
                                okfkit_skills::SkillContext {
                                    bundle: abs(&bundle_dir),
                                    prefix,
                                    capabilities: caps,
                                    allow: filters.0.clone(),
                                    deny: filters.1.clone(),
                                },
                            )
                        }
                    };
                    // Plan every agent first, so a conflict leaves nothing half-installed.
                    let mut plans = Vec::new();
                    for agent in agents {
                        let opts = okfkit_skills::InstallOptions {
                            agent,
                            target: target.clone(),
                            home: home.clone(),
                            server: server.clone(),
                            server_name: name.clone(),
                            skill: skill.clone(),
                            replace,
                        };
                        let actions = okfkit_skills::plan(&opts)?;
                        plans.push((opts, actions));
                    }
                    let (mut changes, mut notes) = (Vec::new(), Vec::new());
                    for (opts, actions) in &plans {
                        if print && !json {
                            for a in actions {
                                println!("{a}");
                            }
                            continue;
                        }
                        if !print {
                            okfkit_skills::apply(actions)?;
                            if let Some(reg) = &registry {
                                okfkit_skills::record_install(
                                    reg,
                                    okfkit_skills::Install::of(opts)?,
                                )?;
                            }
                        }
                        changes.extend(actions.iter().map(|a| change_json(opts.agent, a)));
                        if let (okfkit_skills::Agent::Codex, okfkit_skills::Target::Project(dir)) =
                            (opts.agent, &opts.target)
                            && !okfkit_skills::codex_trusts(dir, &home)
                        {
                            notes.push(format!(
                                "Codex reads {} only for trusted projects; trust this project when Codex asks (or install with --user)",
                                dir.join(".codex/config.toml").display()
                            ));
                        }
                    }
                    if print && !json {
                        return Ok(ExitCode::SUCCESS);
                    }
                    let undo = format!(
                        "okfkit agent uninstall{}",
                        if name == okfkit_skills::DEFAULT_NAME {
                            String::new()
                        } else {
                            format!(" --name {name}")
                        }
                    );
                    notes.push(format!(
                        "Restart the agent session to load the MCP server and skills (tools {}_*).",
                        skill.prefix
                    ));
                    let out = serde_json::json!({
                        "applied": !print,
                        "changes": changes,
                        "notes": notes,
                        "tools_prefix": skill.prefix,
                        "next": ["okfkit doctor", undo],
                    });
                    emit(json, &out, || {
                        let mut t = String::new();
                        let mut last = "";
                        for c in &changes {
                            let agent = c["agent"].as_str().unwrap_or("");
                            if agent != last {
                                t.push_str(&format!("{agent}:\n"));
                                last = agent;
                            }
                            t.push_str(&format!("  {}\n", c["summary"].as_str().unwrap_or("")));
                        }
                        for n in &notes {
                            t.push_str(&format!("note: {n}\n"));
                        }
                        t.push_str(&format!("undo: {undo}\n"));
                        t
                    })?;
                }
                AgentCmd::Uninstall {
                    claude,
                    codex,
                    user,
                    project,
                    name,
                    all,
                    print,
                } => {
                    let recorded = match &registry {
                        Some(r) => okfkit_skills::load_registry(r)?,
                        None => Vec::new(),
                    };
                    let todo: Vec<(
                        okfkit_skills::Agent,
                        okfkit_skills::Target,
                        String,
                        Option<Vec<String>>,
                    )> = if all {
                        recorded
                            .iter()
                            .map(|i| {
                                (
                                    i.agent,
                                    i.target.clone(),
                                    i.name.clone(),
                                    Some(i.skills.clone()),
                                )
                            })
                            .collect()
                    } else {
                        let target = if user {
                            okfkit_skills::Target::User
                        } else {
                            okfkit_skills::Target::Project(abs(
                                &project.unwrap_or_else(|| PathBuf::from("."))
                            ))
                        };
                        let agents = if claude || codex {
                            [
                                (claude, okfkit_skills::Agent::Claude),
                                (codex, okfkit_skills::Agent::Codex),
                            ]
                            .into_iter()
                            .filter_map(|(on, a)| on.then_some(a))
                            .collect()
                        } else {
                            vec![okfkit_skills::Agent::Claude, okfkit_skills::Agent::Codex]
                        };
                        agents
                            .into_iter()
                            .map(|a| {
                                let skills = recorded
                                    .iter()
                                    .find(|i| i.agent == a && i.target == target && i.name == name)
                                    .map(|i| i.skills.clone());
                                (a, target.clone(), name.clone(), skills)
                            })
                            .collect()
                    };
                    let mut changes = Vec::new();
                    for (agent, target, name, skills) in todo {
                        let actions =
                            okfkit_skills::plan_uninstall(&okfkit_skills::UninstallOptions {
                                agent,
                                target: target.clone(),
                                home: home.clone(),
                                server_name: name.clone(),
                                skills,
                            })?;
                        if print && !json {
                            for a in &actions {
                                println!("{a}");
                            }
                            continue;
                        }
                        if !print {
                            okfkit_skills::apply(&actions)?;
                            if let Some(r) = &registry {
                                okfkit_skills::forget_install(r, agent, &target, &name)?;
                            }
                        }
                        changes.extend(actions.iter().map(|a| change_json(agent, a)));
                    }
                    if print && !json {
                        return Ok(ExitCode::SUCCESS);
                    }
                    let out = serde_json::json!({
                        "applied": !print,
                        "changes": changes,
                        "notes": if changes.is_empty() { vec![] } else { vec!["Restart the agent session. okfkit's index and cache stay; remove them with `okfkit clean`."] },
                        "next": if changes.is_empty() { vec!["okfkit agent status"] } else { vec!["okfkit clean"] },
                    });
                    emit(json, &out, || {
                        if changes.is_empty() {
                            return "nothing to remove (see `okfkit agent status` for recorded installs)\n".into();
                        }
                        let mut t = String::new();
                        for c in &changes {
                            t.push_str(&format!(
                                "{}: {}\n",
                                c["agent"].as_str().unwrap_or(""),
                                c["summary"].as_str().unwrap_or("")
                            ));
                        }
                        t.push_str("Restart the agent session. okfkit's index and cache stay; remove them with `okfkit clean`.\n");
                        t
                    })?;
                }
                AgentCmd::Status => {
                    let recorded = match &registry {
                        Some(r) => okfkit_skills::load_registry(r)?,
                        None => Vec::new(),
                    };
                    #[derive(Serialize)]
                    struct Row<'a> {
                        #[serde(flatten)]
                        install: &'a okfkit_skills::Install,
                        problems: Vec<String>,
                    }
                    let rows: Vec<Row<'_>> = recorded
                        .iter()
                        .map(|i| Row {
                            install: i,
                            problems: i.check(&home),
                        })
                        .collect();
                    emit(json, &rows, || {
                        if rows.is_empty() {
                            return "no installs recorded (okfkit agent install records them)\n"
                                .into();
                        }
                        let mut out = String::new();
                        for r in &rows {
                            let i = r.install;
                            let place = match &i.target {
                                okfkit_skills::Target::User => "user".to_owned(),
                                okfkit_skills::Target::Project(d) => d.display().to_string(),
                            };
                            out.push_str(&format!(
                                "{} {:<12} {} → {}  [{}]\n",
                                if r.problems.is_empty() {
                                    "ok  "
                                } else {
                                    "FAIL"
                                },
                                i.agent.label(),
                                place,
                                i.source,
                                i.name
                            ));
                            for p in &r.problems {
                                out.push_str(&format!("       - {p}\n"));
                            }
                        }
                        if rows.iter().any(|r| !r.problems.is_empty()) {
                            out.push_str("fix: run `okfkit agent install` again from the bundle (add --replace if asked), or `okfkit agent uninstall` with the same --name/--user/--project\n");
                        }
                        out
                    })?;
                }
            }
        }
        Command::Clean {
            index,
            cache,
            all,
            yes,
        } => {
            let (index, cache) = (index || all, cache || all);
            let mut targets: Vec<(String, PathBuf)> = Vec::new();
            // Where this bundle's index may be, without creating anything.
            if bundle_dir.is_dir() {
                let mut dirs = Vec::new();
                match &state_dir {
                    StateDir::Path(p) => {
                        if p.join("index.sqlite").is_file() {
                            dirs.push(p.clone());
                        }
                    }
                    StateDir::Cache => dirs.extend(okfkit::bundle_cache_dir(&bundle_dir).ok()),
                    _ => {
                        dirs.push(bundle_dir.join(okfkit::BUNDLE_STATE_DIR));
                        dirs.extend(okfkit::bundle_cache_dir(&bundle_dir).ok());
                    }
                }
                for d in dirs.into_iter().filter(|d| d.is_dir()) {
                    targets.push((format!("index of {}", abs(&bundle_dir).display()), d));
                }
            }
            let cache_root = okfkit::analyze::dict::user_cache_dir().map(|c| c.join("okfkit"));
            if let Some(c) = cache_root.as_ref().filter(|c| c.is_dir()) {
                targets.push((
                    "user cache (models, vectors, dictionary, training environment, indexes of bundles kept there)".into(),
                    c.clone(),
                ));
            }
            #[derive(Serialize)]
            struct Item {
                what: String,
                path: PathBuf,
                bytes: u64,
                selected: bool,
                deleted: bool,
            }
            let mut items: Vec<Item> = targets
                .into_iter()
                .map(|(what, path)| {
                    let selected = if what.starts_with("index") {
                        index
                    } else {
                        cache
                    };
                    Item {
                        bytes: dir_size(&path),
                        what,
                        path,
                        selected,
                        deleted: false,
                    }
                })
                .collect();
            if yes {
                for it in items.iter_mut().filter(|i| i.selected) {
                    std::fs::remove_dir_all(&it.path)
                        .with_context(|| format!("deleting {}", it.path.display()))?;
                    it.deleted = true;
                }
            }
            emit(json, &items, || {
                let mut out = String::new();
                if items.is_empty() {
                    out.push_str("nothing to clean\n");
                }
                for it in &items {
                    out.push_str(&format!(
                        "{} {:>9}  {}\n           {}\n",
                        if it.deleted {
                            "deleted"
                        } else if it.selected {
                            "would delete"
                        } else {
                            "kept   "
                        },
                        human_size(it.bytes),
                        it.what,
                        it.path.display()
                    ));
                }
                let overrides: Vec<&str> =
                    ["OKFKIT_MODELS_DIR", "OKFKIT_EMB_CACHE", "OKFKIT_DICT_DIR"]
                        .into_iter()
                        .filter(|v| std::env::var_os(v).is_some_and(|x| !x.is_empty()))
                        .collect();
                if !overrides.is_empty() {
                    out.push_str(&format!(
                        "not covered (set elsewhere by {}): delete those directories yourself\n",
                        overrides.join(", ")
                    ));
                }
                if !yes && items.iter().any(|i| i.selected) {
                    out.push_str("run again with --yes to delete\n");
                } else if !yes {
                    out.push_str("choose --index, --cache or --all, then add --yes to delete\n");
                }
                out
            })?;
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
            let built = okfkit::build_features();
            let embed_status = |on: bool, enabled: bool| match (on, enabled) {
                (false, _) => "not in this build (use okfkit-full)",
                (true, true) => "on",
                (true, false) => "available; off for this bundle (okfkit embed enable)",
            };
            let bundle_embed = okfkit::config::load(&bundle_dir)
                .map(|c| c.embed)
                .unwrap_or_default();
            let local_on = matches!(bundle_embed, okfkit::config::EmbedConfig::Local { .. });
            let api_on = matches!(bundle_embed, okfkit::config::EmbedConfig::Api { .. });
            let modules = vec![
                Module {
                    name: "core",
                    status: "on",
                    capabilities: caps
                        .into_iter()
                        .filter(|c| !c.starts_with("embed."))
                        .collect(),
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
                    status: embed_status(built.embed_local, local_on),
                    capabilities: if built.embed_local && local_on {
                        vec![okfkit::capability::EMBED_SEARCH]
                    } else {
                        vec![]
                    },
                },
                Module {
                    name: "embed-api",
                    status: embed_status(built.embed_api, api_on),
                    capabilities: if built.embed_api && api_on {
                        vec![okfkit::capability::EMBED_SEARCH]
                    } else {
                        vec![]
                    },
                },
                Module {
                    name: "mcp-http",
                    status: "on (okfkit mcp serve --http)",
                    capabilities: vec![],
                },
                Module {
                    name: "ja-dictionary",
                    status: if built.ja_embedded {
                        "embedded"
                    } else {
                        "downloaded on first use (okfkit dict status)"
                    },
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

/// Quotes `s` for a POSIX shell when it contains anything but safe characters.
fn shell_quote(s: &str) -> String {
    if !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_./:@%+=".contains(c))
    {
        s.to_owned()
    } else {
        format!("'{}'", s.replace('\'', "'\\''"))
    }
}

/// Fails with instructions unless the model's license is accepted (or `accept` records it now).
fn ensure_license(info: &okfkit::ModelInfo, accept: bool) -> Result<()> {
    if !info.requires_acceptance || okfkit::license_accepted(info) {
        return Ok(());
    }
    if !accept {
        return Err(contract::Consent {
            code: "license_required",
            message: format!("{} needs its license accepted", info.id),
            question: format!(
                "{} is released under the {} ({}); about {} MB is downloaded once. Do you accept the license? (MIT alternative: bge-m3-int8, ~570 MB, weaker on cross-language questions)",
                info.name, info.license, info.license_url, info.size_mb
            ),
            flag: "--accept-license",
            next: vec![
                format!("okfkit embed enable --model {} --accept-license", info.id),
                "okfkit embed enable --model bge-m3-int8".into(),
            ],
        }
        .into());
    }
    okfkit::accept_license(info)?;
    eprintln!(
        "recorded acceptance of the {} for {}",
        info.license, info.id
    );
    Ok(())
}

/// `okfkit embed tune …`; returns an exit code when the command failed softly (rejected batch).
fn tune_command(
    command: TuneCmd,
    json: bool,
    state_dir: &StateDir,
    open: &dyn Fn() -> Result<Bundle>,
    scope: &dyn Fn() -> Result<Scope>,
) -> Result<Option<ExitCode>> {
    match command {
        TuneCmd::Guide => print!("{}", okfkit::tune::GUIDE),
        TuneCmd::Init {
            langs,
            max_passages,
        } => {
            let b = open()?;
            let scope = scope()?;
            let langs = if langs.is_empty() {
                b.default_tune_langs(&scope)?
            } else {
                langs.into_iter().map(|l| l.trim().to_lowercase()).collect()
            };
            let run = b.tune_init(
                okfkit::tune::Settings {
                    langs,
                    max_passages,
                    ..Default::default()
                },
                &scope,
            )?;
            let p = run.plan();
            emit(json, p, || {
                format!(
                    "run {} ({}): {} passages from {} documents ({} held out for evaluation), {} batches, languages {}\n\
                     next: okfkit embed tune next   (or read `okfkit embed tune guide` first)\n",
                    p.id,
                    p.standard,
                    p.passages,
                    p.docs,
                    p.heldout_docs.len(),
                    p.batches,
                    p.settings.langs.join(", ")
                )
            })?;
        }
        TuneCmd::Next {
            batch,
            no_claim,
            run,
        } => {
            let b = open()?;
            let run = b.tune_run(run.as_deref())?;
            let view = match batch {
                Some(n) => Some(run.view(n)?),
                None => run.next(!no_claim)?,
            };
            match view {
                Some(v) => emit(json, &v, || v.to_text())?,
                None => {
                    let st = run.status()?;
                    let msg = if st.answered == st.batches {
                        "every batch is answered; next: okfkit embed tune check".to_owned()
                    } else {
                        format!(
                            "no free batch: {} claimed by other agents (claims expire after 30 minutes)",
                            st.claimed
                        )
                    };
                    emit(
                        json,
                        &serde_json::json!({"batch": null, "next": msg}),
                        || format!("{msg}\n"),
                    )?;
                }
            }
        }
        TuneCmd::Submit { batch, file, run } => {
            let b = open()?;
            let run = b.tune_run(run.as_deref())?;
            let text = if file.as_os_str() == "-" {
                let mut s = String::new();
                std::io::Read::read_to_string(&mut std::io::stdin(), &mut s)?;
                s
            } else {
                std::fs::read_to_string(&file)
                    .with_context(|| format!("reading {}", file.display()))?
            };
            let sub = run.submit(batch, &text, &b.human_eval_questions()?)?;
            emit(json, &sub, || {
                if sub.accepted {
                    format!(
                        "batch {} accepted: {} questions; {} batches left\nnext: {}\n",
                        sub.batch, sub.questions, sub.remaining, sub.next
                    )
                } else {
                    format!(
                        "batch {} rejected ({} problems):\n{}\nnext: {}\n",
                        sub.batch,
                        sub.errors.len(),
                        sub.errors
                            .iter()
                            .map(|e| format!("  - {e}"))
                            .collect::<Vec<_>>()
                            .join("\n"),
                        sub.next
                    )
                }
            })?;
            if !sub.accepted {
                return Ok(Some(ExitCode::from(contract::EXIT_FINDINGS)));
            }
        }
        TuneCmd::Status { run } => {
            let b = open()?;
            let st = b.tune_run(run.as_deref())?.status()?;
            emit(json, &st, || tune_status_text(&st))?;
        }
        TuneCmd::Check { run } => {
            let b = open()?;
            let run = b.tune_run(run.as_deref())?;
            let st = run.finalize()?;
            emit(json, &st, || {
                let mut t = tune_status_text(&st);
                if st.answered == st.batches {
                    t.push_str(&format!(
                        "wrote {} and {}\n",
                        run.train_path().display(),
                        run.heldout_path().display()
                    ));
                }
                t
            })?;
            if !st.ready {
                return Ok(Some(ExitCode::from(contract::EXIT_FINDINGS)));
            }
        }
        TuneCmd::Setup { yes } => {
            let env = tune_env(yes)?;
            emit(
                json,
                &serde_json::json!({"ready": true, "gpu": env.gpu}),
                || {
                    format!(
                        "training environment ready ({})\n",
                        if env.gpu { "CUDA GPU" } else { "CPU" }
                    )
                },
            )?;
        }
        TuneCmd::Train {
            backend,
            yes,
            epochs,
            allow_small,
            run,
        } => {
            let b = open()?;
            let run = b.tune_run(run.as_deref())?;
            let st = okfkit::tune::check_trainable(&run, allow_small)?;
            match backend {
                TrainBackend::Colab => {
                    let nb = colab(&run)?;
                    let msg = format!(
                        "wrote {}\nopen it in Colab (File → Upload notebook), pick a T4 GPU and Run all; \
                         then put the two downloaded files in a folder and run\n  okfkit embed tune import <folder>\n  okfkit embed tune export\n",
                        nb.display()
                    );
                    emit(
                        json,
                        &serde_json::json!({"notebook": nb, "next": "okfkit embed tune import <folder>"}),
                        || msg,
                    )?;
                }
                TrainBackend::Local => {
                    let env = tune_env(yes)?;
                    eprintln!(
                        "training on {} pairs ({}); this takes a while on a CPU",
                        st.train_pairs,
                        if env.gpu { "CUDA GPU" } else { "CPU" }
                    );
                    let adapter = train_local(&run, &env, epochs)?;
                    emit(
                        json,
                        &serde_json::json!({"adapter": adapter, "next": "okfkit embed tune export"}),
                        || {
                            format!(
                                "adapter: {}\nnext: okfkit embed tune export\n",
                                adapter.display()
                            )
                        },
                    )?;
                }
            }
        }
        TuneCmd::Import { dir, run } => {
            let b = open()?;
            let run = b.tune_run(run.as_deref())?;
            let to = okfkit::tune::import_adapter(&run, &dir)?;
            emit(
                json,
                &serde_json::json!({"adapter": to, "next": "okfkit embed tune export"}),
                || {
                    format!(
                        "adapter imported into {}\nnext: okfkit embed tune export\n",
                        to.display()
                    )
                },
            )?;
        }
        TuneCmd::Export { name, yes, run } => {
            let b = open()?;
            let run = b.tune_run(run.as_deref())?;
            let env = tune_env(yes)?;
            let model = export_model(&run, &env, b.root(), name.as_deref())?;
            let id = format!("custom:{}", model.manifest.name);
            emit(
                json,
                &serde_json::json!({"model": id, "dir": model.dir, "next": "okfkit embed tune eval"}),
                || {
                    format!(
                        "installed {id}\nnext: okfkit embed tune eval   (compares it with the base model on held-out questions)\n"
                    )
                },
            )?;
        }
        TuneCmd::Eval { run } => {
            let b = open()?;
            let run = b.tune_run(run.as_deref())?;
            let gate = okfkit::tune::evaluate_run(
                b.root(),
                state_dir.clone(),
                &run,
                &scope()?,
                &mut |m, d, n| eprint!("\r{m}: embedding {d}/{n} chunks   "),
            )?;
            eprintln!();
            emit(json, &gate, || {
                format!(
                    "{}next: {}\n",
                    gate.to_text(),
                    if gate.passed {
                        "okfkit embed tune activate --write"
                    } else {
                        "keep the base model (or write more/better questions in a new run)"
                    }
                )
            })?;
            if !gate.passed {
                return Ok(Some(ExitCode::from(contract::EXIT_FINDINGS)));
            }
        }
        TuneCmd::Activate {
            write,
            force,
            no_index,
            run,
        } => {
            let b = open()?;
            let run = b.tune_run(run.as_deref())?;
            if !write {
                let gate = okfkit::tune::recorded_gate(&run);
                let model =
                    okfkit::tune::exported_model(&run).unwrap_or_else(|| "(not exported)".into());
                let gate = gate.map_or("not evaluated".to_owned(), |g| {
                    format!(
                        "{}: R@1 {:.3} -> {:.3}",
                        if g.passed { "passed" } else { "failed" },
                        g.r_at_1.0,
                        g.r_at_1.1
                    )
                });
                return Err(contract::Consent {
                    code: "consent_required",
                    message: format!("activating {model} changes okfkit.toml and re-embeds the bundle"),
                    question: format!(
                        "Switch this bundle's search to the tuned model {model} (gate {gate})? It can be undone with rollback."
                    ),
                    flag: "--write",
                    next: vec!["okfkit embed tune activate --write".into()],
                }
                .into());
            }
            let model = okfkit::tune::activate(&b, &run, force)?;
            eprintln!(
                "okfkit.toml: [embed] model = \"{model}\" (undo: okfkit embed tune rollback --write)"
            );
            if !no_index {
                let b = open()?;
                let st = b.embed_sync(&mut |d, n| eprint!("\rembedding {d}/{n} chunks   "))?;
                eprintln!();
                emit(json, &st, || {
                    format!(
                        "model: {}\nembedded: {}/{} chunks\n",
                        st.model, st.embedded, st.chunks
                    )
                })?;
            }
        }
        TuneCmd::Rollback { write } => {
            let b = open()?;
            if !write {
                return Err(contract::Consent {
                    code: "consent_required",
                    message: "rollback changes okfkit.toml".into(),
                    question: "Restore the embedding setting from before the last activate?".into(),
                    flag: "--write",
                    next: vec!["okfkit embed tune rollback --write".into()],
                }
                .into());
            }
            let prev = okfkit::tune::rollback(&b)?;
            let what = match &prev {
                okfkit::config::EmbedConfig::Off => "embeddings off".to_owned(),
                okfkit::config::EmbedConfig::Local { model } => format!("model {model}"),
                okfkit::config::EmbedConfig::Api { model, .. } => format!("API model {model}"),
            };
            eprintln!(
                "okfkit.toml restored: {what}; run `okfkit embed index` if vectors are missing"
            );
        }
        TuneCmd::Runs => {
            let b = open()?;
            let runs = b.tune_runs()?;
            emit(json, &runs, || {
                runs.iter().map(|r| format!("{r}\n")).collect()
            })?;
        }
    }
    Ok(None)
}

fn tune_status_text(st: &okfkit::tune::TuneStatus) -> String {
    let map = |m: &BTreeMap<String, usize>| {
        m.iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    format!(
        "run:       {} ({})\nlanguages: {}\nbatches:   {}/{} answered ({} claimed)\nquestions: {} for training, {} held out\nkinds:     {}\nwritten:   {}\nready:     {}\nnext:      {}\n",
        st.run,
        st.standard,
        st.langs.join(", "),
        st.answered,
        st.batches,
        st.claimed,
        st.train_pairs,
        st.heldout_questions,
        map(&st.kinds),
        map(&st.langs_written),
        st.ready,
        st.next
    )
}

#[cfg(feature = "embed-tune")]
type TuneEnv = okfkit::tune::PyEnv;
#[cfg(not(feature = "embed-tune"))]
struct TuneEnv {
    gpu: bool,
}

#[cfg(not(feature = "embed-tune"))]
const NO_TUNE: &str =
    "training is not in this build: use okfkit-full (cargo install okfkit-cli --features full)";

/// The training environment; creates it only with `--yes` (it downloads packages).
#[cfg(feature = "embed-tune")]
fn tune_env(yes: bool) -> Result<TuneEnv> {
    let mut log = |l: &str| eprintln!("  {l}");
    if let Some(env) = okfkit::tune::python_env(false, &mut log)? {
        return Ok(env);
    }
    if !yes {
        return Err(contract::Consent {
            code: "consent_required",
            message: "training needs a private Python environment in the user cache".into(),
            question: format!(
                "Fine-tuning needs a private Python environment ({}) in your user cache. Download and create it?",
                okfkit::tune::PYTHON_DOWNLOAD_HINT
            ),
            flag: "--yes",
            next: vec!["okfkit embed tune setup --yes".into()],
        }
        .into());
    }
    okfkit::tune::python_env(true, &mut log)?.context("creating the Python environment")
}

#[cfg(not(feature = "embed-tune"))]
fn tune_env(_yes: bool) -> Result<TuneEnv> {
    bail!(NO_TUNE)
}

#[cfg(feature = "embed-tune")]
fn train_local(run: &okfkit::tune::Run, env: &TuneEnv, epochs: usize) -> Result<PathBuf> {
    Ok(okfkit::tune::train(run, env, epochs, &mut |l| {
        eprintln!("  {l}")
    })?)
}

#[cfg(not(feature = "embed-tune"))]
fn train_local(_: &okfkit::tune::Run, env: &TuneEnv, _: usize) -> Result<PathBuf> {
    let _ = env.gpu;
    bail!(NO_TUNE)
}

#[cfg(feature = "embed-tune")]
fn colab(run: &okfkit::tune::Run) -> Result<PathBuf> {
    Ok(okfkit::tune::colab_notebook(run)?)
}

#[cfg(not(feature = "embed-tune"))]
fn colab(_: &okfkit::tune::Run) -> Result<PathBuf> {
    bail!(NO_TUNE)
}

#[cfg(feature = "embed-tune")]
fn export_model(
    run: &okfkit::tune::Run,
    env: &TuneEnv,
    root: &Path,
    name: Option<&str>,
) -> Result<okfkit::CustomModel> {
    Ok(okfkit::tune::export(run, env, root, name, &mut |l| {
        eprintln!("  {l}")
    })?)
}

#[cfg(not(feature = "embed-tune"))]
fn export_model(
    _: &okfkit::tune::Run,
    _: &TuneEnv,
    _: &Path,
    _: Option<&str>,
) -> Result<okfkit::CustomModel> {
    bail!(NO_TUNE)
}

fn audience_of(a: AudienceArg) -> okfkit::Audience {
    match a {
        AudienceArg::Claude => okfkit::Audience::Claude,
        AudienceArg::Codex => okfkit::Audience::Codex,
        AudienceArg::Team => okfkit::Audience::Team,
        AudienceArg::Host => okfkit::Audience::Host,
    }
}

/// Agent CLIs on this machine (on PATH, or their configuration directory exists).
fn detect_agents(home: &Path) -> Vec<okfkit_skills::Agent> {
    use okfkit_skills::Agent;
    let found = |program: &str, dir: &str| on_path(program) || home.join(dir).is_dir();
    [
        (found("claude", ".claude"), Agent::Claude),
        (found("codex", ".codex"), Agent::Codex),
    ]
    .into_iter()
    .filter_map(|(on, a)| on.then_some(a))
    .collect()
}

/// The agents to install for: the flags, else every agent found on this machine.
fn chosen_agents(claude: bool, codex: bool, home: &Path) -> Result<Vec<okfkit_skills::Agent>> {
    use okfkit_skills::Agent;
    if claude || codex {
        return Ok([(claude, Agent::Claude), (codex, Agent::Codex)]
            .into_iter()
            .filter_map(|(on, a)| on.then_some(a))
            .collect());
    }
    let agents = detect_agents(home);
    if agents.is_empty() {
        bail!(
            "no agent found (claude or codex on PATH, ~/.claude or ~/.codex); pass --claude or --codex"
        );
    }
    Ok(agents)
}

fn on_path(program: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|p| {
        std::env::split_paths(&p).any(|d| {
            let f = d.join(program);
            f.is_file() || f.with_extension("exe").is_file()
        })
    })
}

fn dir_size(path: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(path) else {
        return 0;
    };
    entries
        .flatten()
        .map(|e| match e.file_type() {
            Ok(t) if t.is_dir() => dir_size(&e.path()),
            Ok(t) if t.is_file() => e.metadata().map_or(0, |m| m.len()),
            _ => 0,
        })
        .sum()
}

fn human_size(bytes: u64) -> String {
    let b = bytes as f64;
    if b >= 1e9 {
        format!("{:.1} GB", b / 1e9)
    } else if b >= 1e6 {
        format!("{:.0} MB", b / 1e6)
    } else {
        format!("{:.0} KB", b / 1e3)
    }
}

/// One planned or applied change, for `--json`.
fn change_json(agent: okfkit_skills::Agent, a: &okfkit_skills::Action) -> serde_json::Value {
    let (action, target, why) = match a {
        okfkit_skills::Action::Write { path, why, .. } => {
            ("write", path.display().to_string(), why)
        }
        okfkit_skills::Action::Remove { path, why } => ("remove", path.display().to_string(), why),
        okfkit_skills::Action::Run { argv, why } => ("run", argv.join(" "), why),
    };
    serde_json::json!({
        "agent": agent.label(),
        "action": action,
        "target": target,
        "why": why,
        "summary": a.summary(),
    })
}

/// Reports a finished write command: `{…, "next": […]}` with --json, else a line and the next steps.
fn done(json: bool, mut value: serde_json::Value, text: String, next: &[&str]) -> Result<()> {
    value["next"] = serde_json::json!(next);
    emit(json, &value, || {
        let mut t = format!("{text}\n");
        for n in next {
            t.push_str(&format!("next: {n}\n"));
        }
        t
    })
}
