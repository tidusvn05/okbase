//! Evals for okfkit (`docs/PLAN.md` §12).
//!
//! The **lexical** eval replays tool calls that real agents made in the spikes (recorded in
//! `spikes/*/results/runs.jsonl.gz`) against the fixture bundles, through the MCP tool layer
//! ([`okfkit_mcp::KbServer::call`]), and checks that the output an agent would read holds the
//! answer. It needs no model, no network and no agent: it runs in seconds, in CI, and catches a
//! change to `grep`, `query`, `get`, `catalog` or the data module that would have hidden an
//! answer an agent found before.
//!
//! Each case records whether it passed when it was written (`baseline`); a run reports
//! **regressions** (baseline pass, now fail) and **improvements** separately. Cases are in
//! `fixtures/eval/*.json`, built by `fixtures/eval/mine.py`.

#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::sync::Arc;

use okfkit::{Bundle, Scope};
use okfkit_mcp::{KbServer, ServerOptions};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// Errors.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// Reading a suite file failed.
    #[error("{0}: {1}")]
    Io(PathBuf, std::io::Error),
    /// A suite file is not valid.
    #[error("{0}: {1}")]
    Parse(PathBuf, serde_json::Error),
    /// The fixture bundle could not be opened.
    #[error("bundle {0}: {1}")]
    Bundle(PathBuf, okfkit::Error),
}

/// A suite: cases over one fixture bundle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Suite {
    /// Suite name (`business`, `multilingual`).
    pub suite: String,
    /// Fixture bundle, relative to the fixtures directory.
    pub bundle: String,
    /// Where the cases come from.
    pub about: String,
    /// The cases.
    pub cases: Vec<Case>,
}

/// One question: the calls an agent made, and what their output must contain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Case {
    /// Unique id within the suite.
    pub id: String,
    /// Question category (`list`, `count`, `grep`…).
    pub cat: String,
    /// Question language.
    pub lang: String,
    /// The question (for reports).
    pub q: String,
    /// Tool calls, in order; the expectation is checked on all their outputs together.
    pub calls: Vec<Call>,
    /// What the outputs must contain.
    pub expect: Expect,
    /// Whether the case passed when it was recorded.
    pub baseline: Outcome,
}

/// A tool call (MCP name and arguments).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Call {
    /// Tool name as an agent sees it (`kb_grep`, `data_query`…).
    pub tool: String,
    /// Arguments.
    pub args: Map<String, Value>,
}

/// Expectations; every one given must hold.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Expect {
    /// Document ids that must appear in the outputs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ids: Vec<String>,
    /// Values that must appear: numbers equal to a number in the outputs; strings equal to a
    /// string or key (ignoring case), or contained in one when 8 characters or longer; objects
    /// and arrays element by element.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub values: Vec<Value>,
    /// A document that must be among the first `k` documents listed by the first call.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rank: Option<Rank>,
}

/// See [`Expect::rank`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rank {
    /// Document id.
    pub id: String,
    /// Cut-off.
    pub k: usize,
}

/// A case result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Outcome {
    /// The expectation held.
    Pass,
    /// It did not.
    Fail,
}

/// The result of one case.
#[derive(Debug, Clone, Serialize)]
pub struct CaseResult {
    /// Case id.
    pub id: String,
    /// Category.
    pub cat: String,
    /// Now.
    pub outcome: Outcome,
    /// When recorded.
    pub baseline: Outcome,
    /// What was missing (empty on pass).
    pub missing: Vec<String>,
}

/// The result of a suite.
#[derive(Debug, Clone, Serialize)]
pub struct SuiteReport {
    /// Suite name.
    pub suite: String,
    /// Cases.
    pub total: usize,
    /// Passing now.
    pub pass: usize,
    /// Passing when recorded.
    pub baseline_pass: usize,
    /// Pass → fail.
    pub regressions: Vec<String>,
    /// Fail → pass.
    pub improvements: Vec<String>,
    /// Per category: (category, pass, total).
    pub by_cat: Vec<(String, usize, usize)>,
    /// Every case.
    pub cases: Vec<CaseResult>,
}

impl Suite {
    /// Reads a suite file.
    pub fn load(path: &Path) -> Result<Suite, Error> {
        let text = std::fs::read_to_string(path).map_err(|e| Error::Io(path.into(), e))?;
        serde_json::from_str(&text).map_err(|e| Error::Parse(path.into(), e))
    }

    /// Runs every case on `<fixtures>/<bundle>`, with the index in memory (nothing is written).
    pub fn run(&self, fixtures: &Path) -> Result<SuiteReport, Error> {
        let dir = fixtures.join(&self.bundle);
        let bundle = Bundle::open_in_memory(&dir).map_err(|e| Error::Bundle(dir.clone(), e))?;
        let server = KbServer::new(
            bundle,
            Arc::new(|_: &okfkit_mcp::RequestInfo<'_>| Scope::all()),
            &ServerOptions::default(),
        );
        let cases: Vec<CaseResult> = self.cases.iter().map(|c| run_case(&server, c)).collect();
        let mut by_cat: Vec<(String, usize, usize)> = Vec::new();
        for r in &cases {
            match by_cat.iter_mut().find(|(c, _, _)| *c == r.cat) {
                Some(e) => {
                    e.1 += usize::from(r.outcome == Outcome::Pass);
                    e.2 += 1;
                }
                None => by_cat.push((r.cat.clone(), usize::from(r.outcome == Outcome::Pass), 1)),
            }
        }
        let changed = |from: Outcome, to: Outcome| {
            cases
                .iter()
                .filter(|r| r.baseline == from && r.outcome == to)
                .map(|r| r.id.clone())
                .collect()
        };
        Ok(SuiteReport {
            suite: self.suite.clone(),
            total: cases.len(),
            pass: cases.iter().filter(|r| r.outcome == Outcome::Pass).count(),
            baseline_pass: cases.iter().filter(|r| r.baseline == Outcome::Pass).count(),
            regressions: changed(Outcome::Pass, Outcome::Fail),
            improvements: changed(Outcome::Fail, Outcome::Pass),
            by_cat,
            cases,
        })
    }
}

fn run_case(server: &KbServer, case: &Case) -> CaseResult {
    let outputs: Vec<Value> = case
        .calls
        .iter()
        .map(|c| match server.call(&c.tool, c.args.clone()) {
            Ok((_, json)) => json,
            Err(e) => Value::String(format!("error: {e}")),
        })
        .collect();
    let missing = check(&case.expect, &outputs);
    CaseResult {
        id: case.id.clone(),
        cat: case.cat.clone(),
        outcome: if missing.is_empty() {
            Outcome::Pass
        } else {
            Outcome::Fail
        },
        baseline: case.baseline,
        missing,
    }
}

/// What `expect` misses in `outputs` (empty when it holds).
pub fn check(expect: &Expect, outputs: &[Value]) -> Vec<String> {
    let mut leaves = Leaves::default();
    for o in outputs {
        leaves.collect(o);
    }
    let mut missing = Vec::new();
    for id in &expect.ids {
        if !leaves.strings.iter().any(|s| s == id) {
            missing.push(format!("id {id}"));
        }
    }
    for v in &expect.values {
        if !leaves.has(v) {
            missing.push(format!("value {v}"));
        }
    }
    if let Some(rank) = &expect.rank {
        let mut ids = Vec::new();
        if let Some(first) = outputs.first() {
            ids_in_order(first, &mut ids);
        }
        let mut seen: Vec<&str> = Vec::new();
        for id in &ids {
            if !seen.contains(&id.as_str()) {
                seen.push(id);
            }
        }
        match seen.iter().position(|s| *s == rank.id) {
            Some(p) if p < rank.k => {}
            Some(p) => missing.push(format!("{} ranked {} (cut-off {})", rank.id, p + 1, rank.k)),
            None => missing.push(format!("{} not listed", rank.id)),
        }
    }
    missing
}

#[derive(Default)]
struct Leaves {
    strings: Vec<String>,
    numbers: Vec<f64>,
}

impl Leaves {
    fn collect(&mut self, v: &Value) {
        match v {
            Value::String(s) => {
                self.strings.push(s.clone());
                // Numbers inside table cells or text ("2,647", "45 days") count too.
                self.numbers.extend(numbers_in(s));
            }
            Value::Number(n) => self.numbers.extend(n.as_f64()),
            Value::Array(a) => a.iter().for_each(|x| self.collect(x)),
            Value::Object(o) => {
                for (k, x) in o {
                    self.strings.push(k.clone());
                    self.collect(x);
                }
            }
            Value::Bool(_) | Value::Null => {}
        }
    }

    fn has(&self, v: &Value) -> bool {
        match v {
            Value::Number(n) => {
                let n = n.as_f64().unwrap_or(f64::NAN);
                self.numbers
                    .iter()
                    .any(|x| (x - n).abs() <= 1e-6 * n.abs().max(1.0))
            }
            Value::String(s) => {
                let s = s.to_lowercase();
                self.strings.iter().any(|x| {
                    let x = x.to_lowercase();
                    x == s || (s.chars().count() >= 8 && x.contains(&s))
                })
            }
            Value::Array(a) => a.iter().all(|x| self.has(x)),
            Value::Object(o) => o.values().all(|x| self.has(x)),
            Value::Bool(_) | Value::Null => true,
        }
    }
}

/// Numbers written in text: digits with `,` or `.` thousands separators or a decimal point.
fn numbers_in(s: &str) -> Vec<f64> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for c in s.chars().chain(std::iter::once(' ')) {
        if c.is_ascii_digit() || (!cur.is_empty() && matches!(c, ',' | '.')) {
            cur.push(c);
        } else if !cur.is_empty() {
            let t = cur.trim_end_matches([',', '.']);
            // "2,647" / "2.647" (thousands) and "1.5" (decimal) both count.
            let plain: String = t.chars().filter(char::is_ascii_digit).collect();
            out.extend(plain.parse::<f64>());
            out.extend(t.parse::<f64>());
            cur.clear();
        }
    }
    out
}

/// Values of `id` keys in document order.
fn ids_in_order(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::Array(a) => a.iter().for_each(|x| ids_in_order(x, out)),
        Value::Object(o) => {
            if let Some(Value::String(id)) = o.get("id") {
                out.push(id.clone());
            }
            o.values().for_each(|x| ids_in_order(x, out));
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn values_match_numbers_strings_and_keys() {
        let out = [json!({"facets": {"department": {"HR": 3}}, "rows": [["2,647"]], "total": 9})];
        let e = Expect {
            values: vec![json!(9), json!("hr"), json!(2647), json!({"n": 3})],
            ..Default::default()
        };
        assert!(check(&e, &out).is_empty());
        let e = Expect {
            values: vec![json!("H")],
            ..Default::default()
        };
        assert_eq!(check(&e, &out).len(), 1);
    }

    #[test]
    fn rank_uses_the_first_call() {
        let out = [json!({"docs": [{"id": "a"}, {"id": "b"}, {"id": "a"}, {"id": "c"}]})];
        let rank = |id: &str, k| Expect {
            rank: Some(Rank { id: id.into(), k }),
            ..Default::default()
        };
        assert!(check(&rank("c", 3), &out).is_empty());
        assert_eq!(check(&rank("c", 2), &out), ["c ranked 3 (cut-off 2)"]);
        assert_eq!(check(&rank("z", 2), &out), ["z not listed"]);
    }
}
