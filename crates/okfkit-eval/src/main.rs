//! `okfkit-eval lexical [--json] [--fixtures DIR] [SUITE.json…]`: replays the recorded agent
//! tool calls in `fixtures/eval/*.json` and reports pass rates. Exits 1 on a regression (a case
//! that passed when it was recorded and fails now).

#![forbid(unsafe_code)]

use std::path::PathBuf;
use std::process::ExitCode;

use okfkit_eval::{Outcome, Suite};

const USAGE: &str = "usage: okfkit-eval lexical [--json] [--fixtures DIR] [SUITE.json...]";

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    if args.next().as_deref() != Some("lexical") {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    let (mut json, mut fixtures, mut files) = (false, root, Vec::new());
    while let Some(a) = args.next() {
        match a.as_str() {
            "--json" => json = true,
            "--fixtures" => match args.next() {
                Some(d) => fixtures = PathBuf::from(d),
                None => {
                    eprintln!("{USAGE}");
                    return ExitCode::from(2);
                }
            },
            _ => files.push(PathBuf::from(a)),
        }
    }
    if files.is_empty() {
        let dir = fixtures.join("eval");
        match std::fs::read_dir(&dir) {
            Ok(entries) => {
                files = entries
                    .flatten()
                    .map(|e| e.path())
                    .filter(|p| p.extension().is_some_and(|e| e == "json"))
                    .collect();
                files.sort();
            }
            Err(e) => {
                eprintln!("{}: {e}", dir.display());
                return ExitCode::from(2);
            }
        }
    }
    let mut reports = Vec::new();
    for f in &files {
        match Suite::load(f).and_then(|s| s.run(&fixtures)) {
            Ok(r) => reports.push(r),
            Err(e) => {
                eprintln!("{e}");
                return ExitCode::from(2);
            }
        }
    }
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&reports).unwrap_or_default()
        );
    } else {
        for r in &reports {
            let cats: Vec<String> = r
                .by_cat
                .iter()
                .map(|(c, p, t)| format!("{c} {p}/{t}"))
                .collect();
            println!(
                "{:14} {}/{} (recorded {}/{})  {}",
                r.suite,
                r.pass,
                r.total,
                r.baseline_pass,
                r.total,
                cats.join(", ")
            );
            for c in r.cases.iter().filter(|c| c.outcome != c.baseline) {
                let what = if c.outcome == Outcome::Fail {
                    "REGRESSION"
                } else {
                    "improved"
                };
                println!("  {what} {}: {}", c.id, c.missing.join("; "));
            }
        }
    }
    if reports.iter().any(|r| !r.regressions.is_empty()) {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}
