//! End-to-end tests of the `okfkit` binary.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command as StdCommand, Stdio};
use std::sync::Arc;

use assert_cmd::Command;
use serde_json::{Value, json};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(name)
}

/// `okfkit -b <fixture> --state-dir <tmp> ...`: the index never goes into the fixture.
fn okfkit(bundle: &str, state: &Path) -> Command {
    let mut c = Command::cargo_bin("okfkit").unwrap();
    c.arg("-b")
        .arg(fixture(bundle))
        .arg("--state-dir")
        .arg(state);
    c
}

fn stdout(c: &mut Command) -> String {
    let out = c.output().unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap()
}

fn json_of(c: &mut Command) -> Value {
    serde_json::from_str(&stdout(c.arg("--json"))).unwrap()
}

#[test]
fn read_commands_text_snapshots() {
    let st = tempfile::tempdir().unwrap();
    let acme = |args: &[&str]| stdout(okfkit("okf-official/acme_retail", st.path()).args(args));
    insta::assert_snapshot!("cli_grep", acme(&["grep", "refund", "-C", "0"]));
    insta::assert_snapshot!("cli_list", acme(&["list", "metrics"]));
    insta::assert_snapshot!(
        "cli_get_section",
        acme(&["get", "metrics/revenue", "--section", "reporting cuts"])
    );
    insta::assert_snapshot!("cli_links", acme(&["links", "metrics/revenue"]));
    let status = acme(&["status"]);
    assert!(
        status.contains("documents: 9")
            && status.contains("level:     L1")
            && status.contains("mode:      Full"),
        "{status}"
    );
}

#[test]
fn json_equals_mcp_structured_content() {
    let st = tempfile::tempdir().unwrap();
    let cli = json_of(okfkit("business", st.path()).args([
        "query",
        "--type",
        "Contract",
        "--field",
        "region=VN",
        "--sum",
        "contract_value",
        "--count-only",
    ]));
    assert_eq!(cli["total"], 17);

    let bundle = okfkit::Bundle::open_in_memory(&fixture("business")).unwrap();
    bundle.sync().unwrap();
    let server =
        okfkit_mcp::KbServer::new(bundle, Arc::new(okfkit::Scope::all()), &Default::default());
    let args = json!({"type": ["Contract"], "fields": {"region": ["VN"]}, "sum_field": "contract_value", "count_only": true});
    let (_, mcp) = server
        .call("kb_query", args.as_object().unwrap().clone())
        .unwrap();
    assert_eq!(cli, mcp);

    let cli = json_of(okfkit("business", st.path()).args([
        "grep",
        "bảo hành",
        "--files-only",
        "--under",
        "policies",
    ]));
    let args = json!({"pattern": "bảo hành", "files_only": true, "filter": {"path": "policies"}});
    let (_, mcp) = server
        .call("kb_grep", args.as_object().unwrap().clone())
        .unwrap();
    assert_eq!(cli, mcp);
}

#[test]
fn scope_flags_hide_documents() {
    let st = tempfile::tempdir().unwrap();
    let out = okfkit("business", st.path())
        .args([
            "--deny",
            "contracts/**",
            "get",
            "contracts/acme-corp/supply-202504",
        ])
        .output()
        .unwrap();
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("not found") && err.contains("hint:"), "{err}");
    let q = json_of(okfkit("business", st.path()).args([
        "--deny",
        "contracts/**",
        "query",
        "--type",
        "Contract",
        "--count-only",
    ]));
    assert_eq!(q["total"], 0);
}

#[test]
fn lint_exit_codes_and_formats() {
    let st = tempfile::tempdir().unwrap();
    let out = okfkit("lint", st.path())
        .args(["lint", "--level", "L3"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let sarif: Value = serde_json::from_slice(
        &okfkit("lint", st.path())
            .args(["lint", "--format", "sarif"])
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap();
    assert_eq!(sarif["version"], "2.1.0");
    let ok = okfkit("okf-official/acme_retail", st.path())
        .args(["lint", "--level", "L1"])
        .output()
        .unwrap();
    assert_eq!(
        ok.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&ok.stdout)
    );
}

#[test]
fn errors_have_hints() {
    let out = Command::cargo_bin("okfkit")
        .unwrap()
        .args(["-b", "/definitely/missing", "status"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("bundle not found") && err.contains("hint: pass --bundle"),
        "{err}"
    );
    let out = Command::cargo_bin("okfkit")
        .unwrap()
        .arg("frobnicate")
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&out.stderr).contains("no `okfkit-frobnicate` plugin"));
}

#[test]
fn modules_and_index() {
    let st = tempfile::tempdir().unwrap();
    let m = json_of(Command::cargo_bin("okfkit").unwrap().arg("modules"));
    assert_eq!(m[0]["name"], "core");
    let first = json_of(okfkit("okf-official/ga4", st.path()).arg("index"));
    assert_eq!(first["added"], 14);
    let again = json_of(okfkit("okf-official/ga4", st.path()).arg("index"));
    assert_eq!(
        (again["added"].as_u64(), again["unchanged"].as_u64()),
        (Some(0), Some(14))
    );
    let rebuilt = json_of(okfkit("okf-official/ga4", st.path()).args(["index", "--rebuild"]));
    assert_eq!(rebuilt["added"], 14);
}

#[test]
fn agent_install_print_and_apply() {
    let tmp = tempfile::tempdir().unwrap();
    let (home, project) = (tmp.path().join("home"), tmp.path().join("proj"));
    std::fs::create_dir_all(&home).unwrap();
    std::fs::create_dir_all(&project).unwrap();
    let run = |extra: &[&str]| {
        let mut c = Command::cargo_bin("okfkit").unwrap();
        c.env("HOME", &home)
            .arg("-b")
            .arg(fixture("business"))
            .args(["agent", "install"])
            .args(extra)
            .arg("--project")
            .arg(&project);
        stdout(&mut c)
    };
    let printed = run(&["--claude", "--print"]);
    assert!(printed.contains(".mcp.json") && printed.contains("name: okfkit-answer"));
    assert!(!project.join(".mcp.json").exists());
    run(&["--claude"]);
    let mcp: Value =
        serde_json::from_str(&std::fs::read_to_string(project.join(".mcp.json")).unwrap()).unwrap();
    assert_eq!(mcp["mcpServers"]["okfkit"]["args"][2], "mcp");
    assert!(
        project
            .join(".claude/skills/okfkit-answer/SKILL.md")
            .is_file()
    );
    run(&["--codex"]);
    assert!(
        std::fs::read_to_string(home.join(".codex/config.toml"))
            .unwrap()
            .contains("[mcp_servers.okfkit]")
    );
    assert!(
        std::fs::read_to_string(project.join("AGENTS.md"))
            .unwrap()
            .contains("okfkit:begin")
    );
}

#[test]
fn mcp_serve_stdio() {
    let st = tempfile::tempdir().unwrap();
    let bin = assert_cmd::cargo::cargo_bin("okfkit");
    let mut child = StdCommand::new(bin)
        .arg("-b")
        .arg(fixture("okf-official/acme_retail"))
        .arg("--state-dir")
        .arg(st.path())
        .args(["mcp", "serve", "--stdio"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
    let mut send = |v: Value| writeln!(stdin, "{v}").unwrap();
    let mut recv = || loop {
        let v: Value = serde_json::from_str(&lines.next().unwrap().unwrap()).unwrap();
        if v.get("id").is_some() {
            break v;
        }
    };
    send(
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "t", "version": "0"}}}),
    );
    assert_eq!(recv()["result"]["serverInfo"]["name"], "okfkit");
    send(json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
    send(
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "kb_get", "arguments": {"id": "metrics/revenue", "section": "definition"}}}),
    );
    let r = recv();
    assert!(
        r["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .starts_with("# Revenue\n# Definition"),
        "{r}"
    );
    child.kill().ok();
    child.wait().ok();
}
