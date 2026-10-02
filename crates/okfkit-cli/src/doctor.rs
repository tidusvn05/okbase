//! `okfkit doctor`: one pass/fail report of the whole setup, with the command that fixes each
//! problem, including a real MCP round trip (docs/PLAN-onboarding.md §3.4).

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

use serde::Serialize;
use serde_json::{Value, json};

/// Result of one check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    /// Fine.
    Ok,
    /// Works, but something is missing or could be better.
    Warn,
    /// Broken.
    Fail,
}

/// One check.
#[derive(Debug, Clone, Serialize)]
pub struct Check {
    /// Stable id.
    pub id: &'static str,
    /// Outcome.
    pub status: Status,
    /// What was found.
    pub message: String,
    /// The command (or action) that fixes it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix: Option<String>,
}

/// The report.
#[derive(Debug, Clone, Serialize)]
pub struct Report {
    /// Checks in order.
    pub checks: Vec<Check>,
    /// No check failed.
    pub ok: bool,
    /// Commands to run next (the fixes of failed checks, then warnings).
    pub next: Vec<String>,
}

impl Report {
    /// Builds the report from checks.
    pub fn new(checks: Vec<Check>) -> Report {
        let ok = checks.iter().all(|c| c.status != Status::Fail);
        let mut next: Vec<String> = Vec::new();
        for want in [Status::Fail, Status::Warn] {
            for c in checks.iter().filter(|c| c.status == want) {
                if let Some(f) = &c.fix
                    && !next.contains(f)
                {
                    next.push(f.clone());
                }
            }
        }
        Report { checks, ok, next }
    }

    /// Text form.
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        for c in &self.checks {
            let tag = match c.status {
                Status::Ok => "ok  ",
                Status::Warn => "warn",
                Status::Fail => "FAIL",
            };
            out.push_str(&format!("{tag} {:<10} {}\n", c.id, c.message));
            if let Some(f) = c.fix.as_ref().filter(|_| c.status != Status::Ok) {
                out.push_str(&format!("     fix: {f}\n"));
            }
        }
        out.push_str(if self.ok {
            "\nThe setup works.\n"
        } else {
            "\nSome checks failed; run the fixes above (ask the user where a fix needs consent).\n"
        });
        out
    }
}

/// A check.
pub fn check(
    id: &'static str,
    status: Status,
    message: impl Into<String>,
    fix: Option<&str>,
) -> Check {
    Check {
        id,
        status,
        message: message.into(),
        fix: fix.map(str::to_owned),
    }
}

/// Starts `okfkit mcp serve --stdio` for the bundle and runs initialize → tools/list →
/// `<prefix>_catalog`. Returns the number of tools, or why it failed.
pub fn mcp_round_trip(
    exe: &Path,
    bundle: &Path,
    state_dir: Option<&str>,
    prefix: &str,
) -> Result<usize, String> {
    let mut cmd = Command::new(exe);
    cmd.arg("--bundle").arg(bundle);
    if let Some(s) = state_dir {
        cmd.args(["--state-dir", s]);
    }
    let mut child = cmd
        .args(["mcp", "serve", "--stdio", "--prefix", prefix])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("cannot start the MCP server: {e}"))?;
    let mut stdin = child.stdin.take().ok_or("no stdin")?;
    let stdout = child.stdout.take().ok_or("no stdout")?;
    let (tx, rx) = std::sync::mpsc::channel::<Value>();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if let Ok(v) = serde_json::from_str::<Value>(&line)
                && tx.send(v).is_err()
            {
                break;
            }
        }
    });
    let mut send = |v: Value| -> Result<(), String> {
        writeln!(stdin, "{v}").map_err(|e| format!("MCP server closed its input: {e}"))
    };
    let answer = |id: i64| -> Result<Value, String> {
        loop {
            let v = rx
                .recv_timeout(Duration::from_secs(60))
                .map_err(|_| "the MCP server did not answer within 60 s".to_owned())?;
            if v["id"] == id {
                if let Some(e) = v.get("error") {
                    return Err(format!("MCP error: {e}"));
                }
                return Ok(v["result"].clone());
            }
        }
    };
    let result = (|| {
        send(
            json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
            "protocolVersion": "2025-06-18", "capabilities": {},
            "clientInfo": {"name": "okfkit-doctor", "version": env!("CARGO_PKG_VERSION")}}}),
        )?;
        answer(1)?;
        send(json!({"jsonrpc": "2.0", "method": "notifications/initialized"}))?;
        send(json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}))?;
        let tools = answer(2)?["tools"].as_array().map_or(0, Vec::len);
        send(json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call",
            "params": {"name": format!("{prefix}_catalog"), "arguments": {}}}))?;
        let r = answer(3)?;
        if r["isError"] == true {
            return Err(format!("{prefix}_catalog failed: {}", r["content"]));
        }
        Ok(tools)
    })();
    let _ = child.kill();
    let _ = child.wait();
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_orders_fixes() {
        let r = Report::new(vec![
            check("a", Status::Warn, "meh", Some("okfkit embed index")),
            check("b", Status::Fail, "broken", Some("okfkit agent install")),
            check("c", Status::Ok, "fine", None),
        ]);
        assert!(!r.ok);
        assert_eq!(r.next, ["okfkit agent install", "okfkit embed index"]);
        assert!(r.to_text().contains("FAIL b"));
    }
}
