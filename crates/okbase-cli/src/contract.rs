//! The machine contract for agents (docs/plans/onboarding.md §3.2): stable error codes,
//! exit codes, and errors that carry the question an agent must ask the user.

use std::path::{Path, PathBuf};

use serde::Serialize;

/// Success.
pub const EXIT_OK: u8 = 0;
/// An error.
pub const EXIT_ERROR: u8 = 1;
/// The user must agree first (a missing `--yes`, `--accept-license`, `--write`, `--replace`, `--force`, `--send-documents`), or must run a command themselves (`sandbox_blocked`).
pub const EXIT_CONSENT: u8 = 3;
/// The command worked and found problems (lint errors, a rejected batch, a failed gate).
pub const EXIT_FINDINGS: u8 = 4;

/// How to get the okbase-full build (the release installer; from source: `cargo install --locked
/// --path crates/okbase-cli --features full` in a clone).
pub const INSTALL_FULL: &str = "curl -fsSL https://raw.githubusercontent.com/tidusvn05/okbase/main/install.sh | sh -s -- --full";
/// [`INSTALL_FULL`] as a hint.
pub const INSTALL_FULL_HINT: &str = "install the okbase-full build: curl -fsSL \
     https://raw.githubusercontent.com/tidusvn05/okbase/main/install.sh | sh -s -- --full \
     (from source: cargo install --locked --path crates/okbase-cli --features full)";

/// A step that needs the user's agreement. Agents must ask `question` and, only on yes,
/// run the command again with `flag`.
#[derive(Debug, Clone, Serialize)]
pub struct Consent {
    /// `consent_required` or `license_required`.
    pub code: &'static str,
    /// What is blocked.
    pub message: String,
    /// What to ask the user, verbatim.
    pub question: String,
    /// The flag that records the agreement.
    pub flag: &'static str,
    /// Commands for each answer.
    pub next: Vec<String>,
}

impl std::fmt::Display for Consent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} (ask the user: {} If yes, add {})",
            self.message, self.question, self.flag
        )
    }
}

impl std::error::Error for Consent {}

/// An error as reported to agents.
#[derive(Debug, Clone, Serialize)]
pub struct ErrorReport {
    /// Stable code (see [`CODES`]).
    pub code: &'static str,
    /// Message.
    pub message: String,
    /// What to do about it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
    /// For consent errors: what to ask the user.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub question: Option<String>,
    /// For consent errors: the flag that records the agreement.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flag: Option<&'static str>,
    /// Commands to run next.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub next: Vec<String>,
    /// The process exit code.
    #[serde(skip)]
    pub exit: u8,
}

/// Every error code, with its exit code and meaning (documented in `okbase help --agent`).
pub const CODES: &[(&str, u8, &str)] = &[
    (
        "consent_required",
        EXIT_CONSENT,
        "the user must agree first; ask `question`, then add `flag`",
    ),
    (
        "license_required",
        EXIT_CONSENT,
        "a model license must be accepted by the user (--accept-license)",
    ),
    (
        "name_conflict",
        EXIT_CONSENT,
        "the MCP server name serves another bundle; use another --name, or --replace with the user's consent",
    ),
    (
        "sandbox_blocked",
        EXIT_CONSENT,
        "the agent's sandbox protects its own configuration (Codex: .codex/); the user must run `next` in their own terminal",
    ),
    (
        "gate_failed",
        EXIT_FINDINGS,
        "the tuned model did not beat the base model; keep the base model",
    ),
    (
        "bundle_not_found",
        EXIT_ERROR,
        "no bundle directory; pass -b DIR",
    ),
    ("not_found", EXIT_ERROR, "an id or file does not exist"),
    ("invalid_argument", EXIT_ERROR, "a value is not accepted"),
    (
        "not_built",
        EXIT_ERROR,
        "the feature needs the okbase-full build",
    ),
    (
        "embeddings_off",
        EXIT_ERROR,
        "semantic search is not enabled for this bundle",
    ),
    ("no_vectors", EXIT_ERROR, "run okbase embed index first"),
    ("sql_error", EXIT_ERROR, "the SQL query is invalid"),
    (
        "no_datasets",
        EXIT_ERROR,
        "the bundle has no CSV/TSV/XLSX files",
    ),
    (
        "index_error",
        EXIT_ERROR,
        "the index is unreadable; okbase index --rebuild",
    ),
    (
        "tune_error",
        EXIT_ERROR,
        "a fine-tuning step cannot run yet; see `next`",
    ),
    (
        "io_error",
        EXIT_ERROR,
        "a file could not be read or written",
    ),
    ("error", EXIT_ERROR, "anything else"),
];

fn report(code: &'static str, message: String, hint: Option<&str>) -> ErrorReport {
    let exit = CODES
        .iter()
        .find(|(c, ..)| *c == code)
        .map_or(EXIT_ERROR, |c| c.1);
    ErrorReport {
        code,
        message,
        hint: hint.map(str::to_owned),
        question: None,
        flag: None,
        next: Vec::new(),
        exit,
    }
}

/// Classifies an error.
pub fn classify(e: &anyhow::Error) -> ErrorReport {
    if let Some(c) = e.downcast_ref::<Consent>() {
        return ErrorReport {
            code: c.code,
            message: c.message.clone(),
            hint: Some(format!(
                "ask the user; only on yes run again with {}",
                c.flag
            )),
            question: Some(c.question.clone()),
            flag: Some(c.flag),
            next: c.next.clone(),
            exit: EXIT_CONSENT,
        };
    }
    if let Some(okbase_skills::Error::Protected { path }) = e.downcast_ref::<okbase_skills::Error>()
    {
        let home = std::env::var_os("HOME").map_or_else(|| PathBuf::from("."), PathBuf::from);
        let user = path.starts_with(okbase_skills::codex_home(&home));
        let cmd = if user {
            "okbase agent install --codex --user"
        } else {
            "okbase agent install --codex"
        };
        return ErrorReport {
            code: "sandbox_blocked",
            message: format!(
                "cannot write {}: Codex's sandbox keeps .codex/ read-only, so okbase cannot connect Codex from inside Codex",
                path.display()
            ),
            hint: Some(format!(
                "nothing was written; ask the user to run `{cmd}` in their own terminal{}, then restart Codex. Until then, use the okbase CLI (okbase help --agent)",
                if user {
                    String::new()
                } else {
                    format!(
                        " in {}",
                        path.parent()
                            .and_then(Path::parent)
                            .unwrap_or(path)
                            .display()
                    )
                }
            )),
            question: Some(format!(
                "Codex's sandbox does not let me register okbase for Codex. Please run `{cmd}` in your own terminal, then restart Codex."
            )),
            flag: None,
            next: vec![cmd.into()],
            exit: EXIT_CONSENT,
        };
    }
    if let Some(okbase_skills::Error::Conflict {
        name,
        existing,
        wanted,
        ..
    }) = e.downcast_ref::<okbase_skills::Error>()
    {
        return ErrorReport {
            question: Some(format!(
                "The okbase server `{name}` already serves {existing}. Switch it to {wanted} (that project or user loses access to {existing}), or add {wanted} under a new name?"
            )),
            flag: Some("--replace"),
            next: vec![
                format!("okbase -b {wanted} agent install --name okbase-<short name>"),
                format!("okbase -b {wanted} agent install --replace"),
            ],
            ..report(
                "name_conflict",
                format!("{e:#}"),
                Some("prefer a new --name; --replace only with the user's consent"),
            )
        };
    }
    let msg = format!("{e:#}");
    let (code, hint): (&'static str, Option<&str>) = if msg.contains("bundle not found") {
        (
            "bundle_not_found",
            Some(
                "pass --bundle <DIR> (or set OKBASE_BUNDLE), or run okbase inside the bundle directory",
            ),
        )
    } else if msg.contains("accept it first") || msg.contains("--accept-license") {
        (
            "license_required",
            Some("ask the user to accept the license; MIT alternative: --model bge-m3-int8"),
        )
    } else if msg.contains("not found:")
        || msg.contains("unknown model")
        || msg.contains("no run `")
    {
        (
            "not_found",
            Some(
                "find ids with `okbase list`, `okbase catalog` or `okbase grep PATTERN --files-only`",
            ),
        )
    } else if msg.contains("invalid argument") {
        (
            "invalid_argument",
            Some("see `okbase help <command>` for the accepted values"),
        )
    } else if msg.contains("SQL error") || msg.contains("query interrupted") {
        (
            "sql_error",
            Some(
                "list tables and columns with `okbase data tables`; only one SELECT (or WITH ... SELECT) is allowed",
            ),
        )
    } else if msg.contains("no datasets") {
        (
            "no_datasets",
            Some("put CSV, TSV or XLSX files in the bundle (for example under data/)"),
        )
    } else if msg.contains("not available in this build") || msg.contains("not in this build") {
        ("not_built", Some(INSTALL_FULL_HINT))
    } else if msg.contains("embeddings are off") {
        (
            "embeddings_off",
            Some(
                "enable them with `okbase embed enable`; the lexical tools (grep, query, get) work without it",
            ),
        )
    } else if msg.contains("no embeddings for") {
        ("no_vectors", Some("run `okbase embed index` first"))
    } else if msg.contains("gate") && msg.contains("failed") {
        (
            "gate_failed",
            Some("keep the base model; --force only with the user's explicit consent"),
        )
    } else if msg.contains("index database") {
        (
            "index_error",
            Some("the index may be corrupt; rebuild it with `okbase index --rebuild`"),
        )
    } else if msg.starts_with("fine-tuning:") || msg.contains("embed tune") {
        ("tune_error", None)
    } else if e.downcast_ref::<std::io::Error>().is_some() {
        ("io_error", None)
    } else {
        ("error", None)
    };
    report(code, msg, hint)
}

/// Prints an error for people (stderr) or agents (`{"error": …}` on stdout).
pub fn print_error(r: &ErrorReport, json: bool) {
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({ "error": r })).unwrap_or_default()
        );
        return;
    }
    eprintln!("error: {}", r.message);
    if let Some(q) = &r.question {
        eprintln!("ask the user: {q}");
    }
    if let Some(h) = &r.hint {
        eprintln!("hint: {h}");
    }
    for n in &r.next {
        eprintln!("next: {n}");
    }
}

/// What an agent must never decide alone: (action, flag or step, what to tell the user).
pub const CONSENT: &[(&str, &str, &str)] = &[
    (
        "accept a model license (EmbeddingGemma: Gemma Terms of Use)",
        "--accept-license",
        "the license name and URL, the download size, the MIT alternative (bge-m3)",
    ),
    (
        "download large files (models, Python training environment)",
        "--yes / embed index",
        "the size and where it is stored",
    ),
    (
        "send document text to the agent's model provider (writing tune questions)",
        "embed tune next",
        "that passages leave the machine",
    ),
    (
        "send document text to an embeddings API (embed enable --api-url)",
        "--send-documents",
        "the service URL, and that every chunk and query is sent there",
    ),
    (
        "change documents (adopt --write, lint --fix-safe, vocab --write, curating)",
        "--write / editing files",
        "which files change; suggest reviewing with git diff",
    ),
    (
        "register okbase for every project (agent install --user)",
        "--user",
        "that it applies to all projects",
    ),
    (
        "point an existing server name at another bundle",
        "--replace",
        "what it serves now and what it will serve",
    ),
    (
        "delete okbase data or installs (clean, agent uninstall --all)",
        "--yes / --all",
        "what is deleted and its size",
    ),
    (
        "activate a model the quality gate rejected",
        "--force",
        "the gate numbers",
    ),
    (
        "change okbase.toml to switch models (tune activate/rollback)",
        "--write",
        "the model before and after",
    ),
];

/// `okbase help --agent`: everything an agent needs to drive okbase, in one page.
pub fn agent_guide(commands: &[(String, String)]) -> String {
    let mut out = String::from(
        "# okbase for agents\n\n\
         Start: `okbase onboard` (add --json). It prints the setup plan from the real state of this machine\n\
         and bundle. Do the first step, run it again, repeat until nothing is left. At every ASK step, ask\n\
         the user and wait. Finish with `okbase doctor`.\n\n\
         ## Contract\n\
         - Every command accepts --json. Read commands print the same JSON as the MCP tools; setup commands\n\
         \x20 print {\"changes\" or result fields, \"next\": [commands to run next]}.\n\
         - With --json, errors are printed to stdout as {\"error\": {code, message, hint, question, flag, next}}.\n\
         \x20 Exception: invalid command lines exit 2 with plain-text usage errors on stderr and empty stdout, even with --json.\n\
         - No command waits for input. Commands that write, delete or download need an explicit flag.\n\n\
         ## Exit codes\n",
    );
    for (code, what) in [
        (EXIT_OK, "success"),
        (EXIT_ERROR, "error (see error.code)"),
        (2, "invalid command line (usage)"),
        (
            EXIT_CONSENT,
            "the user must agree first: ask error.question; on yes add error.flag",
        ),
        (
            EXIT_FINDINGS,
            "done, with findings: lint errors, a rejected question batch, a failed quality gate",
        ),
    ] {
        out.push_str(&format!("- {code}: {what}\n"));
    }
    out.push_str("\n## Error codes\n");
    for (code, exit, what) in CODES {
        out.push_str(&format!("- `{code}` (exit {exit}): {what}\n"));
    }
    out.push_str("\n## Ask the user before you\n");
    for (action, flag, tell) in CONSENT {
        out.push_str(&format!("- {action} [{flag}]: tell them {tell}\n"));
    }
    out.push_str(
        "\nNever add --accept-license, --yes, --write, --force, --replace or --send-documents on your own.\n\n## Commands\n",
    );
    for (name, about) in commands {
        out.push_str(&format!("- `okbase {name}`: {about}\n"));
    }
    out.push_str(
        "\nDetails: `okbase help <command>`. Fine-tuning: `okbase embed tune guide`. Usage by project size and\n\
         every scenario (several bundles, shared servers, removing okbase): docs/usage.md.\n",
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_and_exit_codes() {
        let consent = anyhow::Error::new(Consent {
            code: "consent_required",
            message: "needs a download".into(),
            question: "Download 1 GB?".into(),
            flag: "--yes",
            next: vec!["okbase embed tune setup --yes".into()],
        });
        let r = classify(&consent);
        assert_eq!(
            (r.code, r.exit, r.flag),
            ("consent_required", 3, Some("--yes"))
        );
        let r = classify(&anyhow::anyhow!("bundle not found: x is not a directory"));
        assert_eq!((r.code, r.exit), ("bundle_not_found", 1));
        let r = classify(&anyhow::anyhow!("something odd"));
        assert_eq!((r.code, r.exit), ("error", 1));
        let codes: std::collections::HashSet<_> = CODES.iter().map(|c| c.0).collect();
        assert_eq!(codes.len(), CODES.len(), "codes are unique");
    }
}
