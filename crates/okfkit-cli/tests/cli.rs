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
    assert_eq!(out.status.code(), Some(4));
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
        std::fs::read_to_string(project.join(".codex/config.toml"))
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

#[test]
fn data_commands() {
    let st = tempfile::tempdir().unwrap();
    let t = json_of(okfkit("business", st.path()).args(["data", "tables"]));
    assert_eq!(t["tables"].as_array().unwrap().len(), 3);
    let q = json_of(okfkit("business", st.path()).args([
        "data",
        "sql",
        "SELECT COUNT(*) AS n FROM sales_2026",
    ]));
    assert_eq!(q["rows"][0][0], 506);
    let out = okfkit("business", st.path())
        .args(["data", "sql", "DELETE FROM sales_2026"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("hint: list tables"));
    let hidden = okfkit("business", st.path())
        .args([
            "--deny",
            "data/sales*",
            "data",
            "sql",
            "SELECT 1 FROM sales_2026",
        ])
        .output()
        .unwrap();
    assert!(!hidden.status.success());
}

#[test]
fn adopt_plan_out_and_write_guard() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("docs");
    std::fs::create_dir_all(src.join("guides")).unwrap();
    std::fs::write(
        src.join("guides/setup.md"),
        "# Setup\n\nInstall the tool and run the setup wizard once.\n",
    )
    .unwrap();
    let run = |args: &[&str]| {
        Command::cargo_bin("okfkit")
            .unwrap()
            .arg("adopt")
            .arg(&src)
            .args(args)
            .output()
            .unwrap()
    };

    let plan = run(&[]);
    assert!(plan.status.success());
    assert!(String::from_utf8_lossy(&plan.stdout).contains("level: below L0 -> L1"));
    assert!(!src.join("index.md").exists(), "plan must not write");

    let refused = run(&["--write"]);
    assert!(!refused.status.success());
    assert!(String::from_utf8_lossy(&refused.stderr).contains("not in a git repository"));

    let out = tmp.path().join("okf");
    assert!(run(&["--out", out.to_str().unwrap()]).status.success());
    assert!(
        std::fs::read_to_string(out.join("guides/setup.md"))
            .unwrap()
            .contains("type: Guide")
    );
    assert!(out.join("guides/index.md").is_file() && !src.join("guides/index.md").exists());

    assert!(run(&["--write", "--force"]).status.success());
    assert!(src.join("guides/index.md").is_file());
}

#[test]
fn vocab_report_and_suggest() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(
        tmp.path().join("a.md"),
        "---\ntype: T\ntags: [Refunds, shipping]\n---\nx\n",
    )
    .unwrap();
    std::fs::write(
        tmp.path().join("b.md"),
        "---\ntype: T\ntags: [refund]\n---\nx\n",
    )
    .unwrap();
    let run = |args: &[&str]| {
        Command::cargo_bin("okfkit")
            .unwrap()
            .arg("-b")
            .arg(tmp.path())
            .arg("vocab")
            .args(args)
            .output()
            .unwrap()
    };
    let report: Value = serde_json::from_slice(&run(&["--json"]).stdout).unwrap();
    assert_eq!(report["tags"][0]["tag"], "refund");
    assert_eq!(report["tags"][0]["variants"][0], "Refunds");
    assert!(run(&["--suggest", "--write"]).status.success());
    assert!(
        !run(&["--suggest", "--write"]).status.success(),
        "never overwrites"
    );
    let text = String::from_utf8(run(&[]).stdout).unwrap();
    assert!(text.starts_with("2 refund (also: Refunds)\n"), "{text}");
}

#[test]
fn http_off_loopback_needs_a_token() {
    let st = tempfile::tempdir().unwrap();
    let out = okfkit("okf-official/ga4", st.path())
        .env_remove("OKFKIT_MCP_TOKEN")
        .args(["mcp", "serve", "--http", "0.0.0.0:0"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("without a token"));
}

#[test]
fn advise_recommends_by_bundle() {
    let st = tempfile::tempdir().unwrap();
    let tiers = |v: &Value| -> Vec<(String, String)> {
        v["steps"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| {
                (
                    s["tier"].as_str().unwrap().into(),
                    s["when"].as_str().unwrap().into(),
                )
            })
            .collect()
    };
    // Small trilingual bundle: full context first; curation because descriptions are missing.
    let ml = json_of(okfkit("multilingual", st.path()).args(["advise", "--for", "claude"]));
    assert_eq!(ml["profile"]["docs"], 100);
    assert!(ml["profile"]["langs"]["vi"].as_f64().unwrap() > 0.2, "{ml}");
    assert_eq!(tiers(&ml)[0], ("full".into(), "now".into()));
    let cmds = ml["steps"][0]["commands"].to_string();
    assert!(
        cmds.contains("agent install --claude") && cmds.contains(" -b "),
        "{cmds}"
    );
    assert!(
        ml["skipped"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s["tier"] == "embed")
    );
    // Tables are detected.
    let biz = json_of(okfkit("business", st.path()).args(["advise"]));
    assert!(
        tiers(&biz).contains(&("data".into(), "now".into())),
        "{biz}"
    );
    // Text output.
    let text = stdout(okfkit("multilingual", st.path()).args(["advise"]));
    assert!(
        text.starts_with("Bundle: 100 docs") && text.contains("Recommended path"),
        "{text}"
    );
}

#[test]
fn custom_models_add_list_remove() {
    let tmp = tempfile::tempdir().unwrap();
    let models = tmp.path().join("models");
    let src = tmp.path().join("acme");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::write(src.join("model.onnx"), b"not really onnx").unwrap();
    for f in [
        "tokenizer.json",
        "config.json",
        "special_tokens_map.json",
        "tokenizer_config.json",
    ] {
        std::fs::write(src.join(f), "{}").unwrap();
    }
    std::fs::write(
        src.join("okfkit-model.json"),
        json!({"format": 1, "name": "acme", "license": "MIT", "onnx": "model.onnx",
               "pooling": "mean", "prompting": "plain", "dim": 384, "max_length": 512})
        .to_string(),
    )
    .unwrap();
    let cmd = |args: &[&str]| {
        let mut c = Command::cargo_bin("okfkit").unwrap();
        c.env("OKFKIT_MODELS_DIR", &models).args(args);
        c
    };
    let out = cmd(&["embed", "models", "add"]).arg(&src).output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let list = json_of(&mut cmd(&["embed", "models"]));
    let row = list
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == "custom:acme")
        .expect("listed");
    assert_eq!(row["custom"], true);
    assert_eq!(row["accepted"], true);
    // Installing twice needs --replace.
    let out = cmd(&["embed", "models", "add"]).arg(&src).output().unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("--replace"));
    assert!(
        cmd(&["embed", "models", "remove", "custom:acme"])
            .output()
            .unwrap()
            .status
            .success()
    );
    let list = json_of(&mut cmd(&["embed", "models"]));
    assert!(!list.to_string().contains("custom:acme"));
}

#[test]
fn tune_question_workflow() {
    let tmp = tempfile::tempdir().unwrap();
    let kb = tmp.path().join("kb");
    std::fs::create_dir_all(&kb).unwrap();
    std::fs::write(
        kb.join("leave.md"),
        "---\ntitle: Annual leave\ndescription: Paid leave rules.\n---\n\nEmployees at the Tokyo and Osaka offices receive ten days of paid annual leave once they have worked for six months, rising by one or two days each year up to a maximum of twenty days. Unused days carry over for one year and then expire.\n",
    )
    .unwrap();
    let state = tmp.path().join("state");
    let cmd = |args: &[&str]| {
        let mut c = Command::cargo_bin("okfkit").unwrap();
        c.arg("-b")
            .arg(&kb)
            .arg("--state-dir")
            .arg(&state)
            .args(args);
        c
    };
    let plan = json_of(&mut cmd(&["embed", "tune", "init", "--langs", "en,vi,ja"]));
    assert_eq!(plan["standard"], "okfkit-questions/v1");
    assert_eq!(plan["batches"], 1);
    let batch = json_of(&mut cmd(&["embed", "tune", "next"]));
    assert_eq!(batch["passages"][0]["key"], "leave#0");
    assert_eq!(batch["passages"][0]["questions"], 4);
    // Rejected: exit code 1 and the reasons.
    let out = cmd(&["embed", "tune", "submit", "1", "-"])
        .write_stdin(
            r#"{"passage": "leave#0", "kind": "cross", "lang": "en", "q": "how many days"}"#,
        )
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(4));
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        text.contains("rejected") && text.contains("cross questions use another listed language"),
        "{text}"
    );
    let good = r#"{"passage": "leave#0", "kind": "natural", "lang": "en", "q": "How much vacation do new hires in Japan get?"}
{"passage": "leave#0", "kind": "keyword", "lang": "en", "q": "annual leave carry over"}
{"passage": "leave#0", "kind": "cross", "lang": "vi", "q": "Ngày phép chưa dùng có được chuyển sang năm sau không?"}
{"passage": "leave#0", "kind": "vague", "lang": "ja", "q": "休みが余ったらどうなるの"}"#;
    let sub = json_of(cmd(&["embed", "tune", "submit", "1", "-"]).write_stdin(good));
    assert_eq!(sub["accepted"], true);
    // Too small to train (standard: 300 pairs), but the files are written.
    let out = cmd(&["embed", "tune", "check"]).output().unwrap();
    assert_eq!(out.status.code(), Some(4));
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        text.contains("only 4 training pairs") && text.contains("train.jsonl"),
        "{text}"
    );
    let guide = stdout(&mut cmd(&["embed", "tune", "guide"]));
    assert!(guide.contains("okfkit embed tune init"));
    // The bundle itself is untouched.
    let names: Vec<_> = std::fs::read_dir(&kb)
        .unwrap()
        .flatten()
        .map(|e| e.file_name())
        .collect();
    assert_eq!(names, ["leave.md"]);
}

#[test]
fn agent_scenarios_many_bundles_status_uninstall_clean() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    let project = tmp.path().join("project");
    let (a, b) = (tmp.path().join("policies"), tmp.path().join("docs"));
    for d in [&home, &project, &a, &b] {
        std::fs::create_dir_all(d).unwrap();
    }
    std::fs::create_dir_all(home.join(".codex")).unwrap();
    for (d, t) in [(&a, "Refund policy"), (&b, "API guide")] {
        std::fs::write(
            d.join("x.md"),
            format!("---\ntitle: {t}\ndescription: About {t}.\n---\n\nText.\n"),
        )
        .unwrap();
    }
    let cmd = |bundle: &Path, args: &[&str]| {
        let mut c = Command::cargo_bin("okfkit").unwrap();
        c.env("HOME", &home)
            .env("XDG_CONFIG_HOME", home.join(".config"))
            .env("XDG_CACHE_HOME", home.join(".cache"))
            .current_dir(&project)
            .arg("-b")
            .arg(bundle)
            .args(args);
        c
    };
    // One bundle for both agents in this project.
    let out = stdout(&mut cmd(
        &a,
        &[
            "--deny",
            "drafts/**",
            "agent",
            "install",
            "--claude",
            "--codex",
        ],
    ));
    assert!(
        out.contains("Claude Code:") && out.contains("Codex:"),
        "{out}"
    );
    // The path filter is part of the server's arguments, so the agent sees the same scope.
    let mcp = std::fs::read_to_string(project.join(".mcp.json")).unwrap();
    assert!(
        mcp.contains("\"--deny\"") && mcp.contains("drafts/**"),
        "{mcp}"
    );
    // AGENTS.md written for Codex is not a document of the bundle.
    let st = json_of(&mut cmd(&project, &["status"]));
    assert!(st.to_string().contains("\"docs\""), "{st}");
    // A second bundle with the default name is refused (nothing is overwritten)…
    let out = cmd(&b, &["agent", "install", "--claude"]).output().unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("--name"));
    // …and installs next to the first under its own name and tools.
    let out = stdout(&mut cmd(
        &b,
        &[
            "agent",
            "install",
            "--claude",
            "--codex",
            "--name",
            "okfkit-docs",
        ],
    ));
    assert!(out.contains("tools docs_*"), "{out}");
    let status = json_of(&mut cmd(&a, &["agent", "status"]));
    assert_eq!(status.as_array().unwrap().len(), 4, "{status}");
    assert!(
        status
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["problems"].as_array().unwrap().is_empty()),
        "{status}"
    );
    // Remove the second bundle only.
    stdout(&mut cmd(
        &b,
        &["agent", "uninstall", "--name", "okfkit-docs"],
    ));
    assert!(!project.join(".claude/skills/okfkit-answer-docs").exists());
    assert!(
        project
            .join(".claude/skills/okfkit-answer/SKILL.md")
            .is_file()
    );
    let agents = std::fs::read_to_string(project.join("AGENTS.md")).unwrap();
    assert!(
        !agents.contains("okfkit-docs") && agents.contains("okfkit:begin"),
        "{agents}"
    );
    // The bundle moves: status says so.
    std::fs::rename(&a, tmp.path().join("moved")).unwrap();
    let text = stdout(&mut cmd(&b, &["agent", "status"]));
    assert!(
        text.contains("FAIL") && text.contains("no longer exists"),
        "{text}"
    );
    // Stop using okfkit: every install goes, other config stays.
    stdout(&mut cmd(&b, &["agent", "uninstall", "--all"]));
    assert!(!project.join(".mcp.json").exists() && !project.join("AGENTS.md").exists());
    assert!(!project.join(".claude/skills/okfkit-answer").exists());
    assert!(stdout(&mut cmd(&b, &["agent", "status"])).contains("no installs recorded"));
    // Clean: shows first, deletes with --yes, never touches documents.
    stdout(&mut cmd(&b, &["status"]));
    let dry = stdout(&mut cmd(&b, &["clean", "--index"]));
    assert!(
        dry.contains("would delete") && b.join(".okfkit").is_dir(),
        "{dry}"
    );
    stdout(&mut cmd(&b, &["clean", "--index", "--yes"]));
    assert!(!b.join(".okfkit").exists() && b.join("x.md").is_file());
}

#[test]
fn machine_contract_errors_consent_and_no_prompts() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    let run = |args: &[&str]| {
        let mut c = Command::cargo_bin("okfkit").unwrap();
        c.env("HOME", &home)
            .env("XDG_CONFIG_HOME", home.join(".config"))
            .env("XDG_CACHE_HOME", home.join(".cache"))
            .env("OKFKIT_MODELS_DIR", home.join("models"))
            .args(args)
            // Agents cannot answer prompts: every command must finish with stdin closed.
            .write_stdin("")
            .timeout(std::time::Duration::from_secs(60));
        c.output().unwrap()
    };
    // Errors are JSON on stdout with a stable code.
    let out = run(&["-b", "/definitely/missing", "status", "--json"]);
    assert_eq!(out.status.code(), Some(1));
    let e: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(e["error"]["code"], "bundle_not_found");
    assert!(e["error"]["hint"].as_str().unwrap().contains("--bundle"));
    // A license is the user's decision: exit 3 with the question to ask.
    let kb = tmp.path().join("kb");
    std::fs::create_dir_all(&kb).unwrap();
    let kb_s = kb.to_str().unwrap();
    let out = run(&[
        "-b",
        kb_s,
        "--state-dir",
        tmp.path().join("st").to_str().unwrap(),
        "embed",
        "enable",
        "--json",
    ]);
    assert_eq!(out.status.code(), Some(3));
    let e: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(e["error"]["code"], "license_required");
    assert_eq!(e["error"]["flag"], "--accept-license");
    assert!(
        e["error"]["question"]
            .as_str()
            .unwrap()
            .contains("Gemma Terms of Use")
    );
    assert!(
        !kb.join("okfkit.toml").exists(),
        "nothing written without consent"
    );
    // Text mode tells people the same.
    let out = run(&[
        "-b",
        kb_s,
        "--state-dir",
        tmp.path().join("st").to_str().unwrap(),
        "embed",
        "enable",
    ]);
    assert!(String::from_utf8_lossy(&out.stderr).contains("ask the user:"));
    // Write commands report changes and next steps.
    let out = run(&[
        "-b",
        kb_s,
        "embed",
        "enable",
        "--model",
        "bge-m3-int8",
        "--json",
    ]);
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["next"][0], "okfkit embed index");
    // A sweep of commands with stdin closed: none waits for input.
    let st = tmp.path().join("st2");
    let st = st.to_str().unwrap();
    for args in [
        vec!["status"],
        vec!["advise"],
        vec!["lint"],
        vec!["catalog"],
        vec!["agent", "status"],
        vec!["agent", "uninstall"],
        vec!["clean"],
        vec!["embed", "models"],
        vec!["embed", "tune", "guide"],
        vec!["embed", "tune", "status"],
        vec!["embed", "tune", "activate"],
        vec!["embed", "tune", "rollback"],
    ] {
        let mut full = vec!["-b", kb_s, "--state-dir", st];
        full.extend(args.iter().copied());
        let out = run(&full);
        assert!(out.status.code().is_some(), "{args:?} did not finish");
    }
}

#[test]
fn onboard_plan_converges() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    let project = tmp.path().join("project");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::create_dir_all(&project).unwrap();
    let run = |args: &[&str]| {
        let mut c = Command::cargo_bin("okfkit").unwrap();
        c.env("HOME", &home)
            .env("XDG_CONFIG_HOME", home.join(".config"))
            .env("XDG_CACHE_HOME", home.join(".cache"))
            .env("PATH", "/usr/bin:/bin")
            .current_dir(&project)
            .arg("-b")
            .arg(fixture("okf-official/acme_retail"))
            .arg("--state-dir")
            .arg(tmp.path().join("st"))
            .args(args);
        c
    };
    let ids = |v: &Value| -> Vec<String> {
        v["steps"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["id"].as_str().unwrap().to_owned())
            .collect()
    };
    // No agent on this machine: the plan says so.
    let p = json_of(&mut run(&["onboard"]));
    assert_eq!(p["steps"][0]["id"], "agents");
    assert_eq!(p["steps"][0]["kind"], "tell");
    assert!(
        p["rules"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r.as_str().unwrap().contains("--accept-license"))
    );
    // Claude Code appears: connect it (a run step: project config needs no consent).
    std::fs::create_dir_all(home.join(".claude")).unwrap();
    let p = json_of(&mut run(&["onboard"]));
    assert_eq!(p["steps"][0]["kind"], "run");
    let cmd0 = p["steps"][0]["commands"][0].as_str().unwrap();
    assert!(
        cmd0.starts_with("okfkit -b ") && cmd0.ends_with(" agent install"),
        "{cmd0}"
    );
    // Do it; the step is done next time.
    stdout(&mut run(&["agent", "install"]));
    let p = json_of(&mut run(&["onboard"]));
    assert!(!ids(&p).contains(&"agents".to_owned()), "{p}");
    assert!(
        p["done"].to_string().contains("connected: Claude Code"),
        "{p}"
    );
    // Removing is a plan too, with consent.
    let r = json_of(&mut run(&["onboard", "--goal", "remove"]));
    assert_eq!(ids(&r), ["remove-agents", "remove-data", "remove-binary"]);
    assert_eq!(r["steps"][0]["kind"], "ask");
    // The agent contract.
    let g = json_of(&mut run(&["help", "--agent"]));
    assert!(
        g["guide"]
            .as_str()
            .unwrap()
            .starts_with("# okfkit for agents")
    );
    assert!(
        g["error_codes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["code"] == "license_required" && c["exit"] == 3)
    );
    assert!(g["commands"].to_string().contains("okfkit embed tune init"));
    assert!(stdout(&mut run(&["--help"])).contains("Agents: start with `okfkit onboard`"));
}

#[test]
fn doctor_checks_the_setup_end_to_end() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    let project = tmp.path().join("project");
    let kb = tmp.path().join("kb");
    for d in [&home, &project, &kb] {
        std::fs::create_dir_all(d).unwrap();
    }
    std::fs::write(
        kb.join("a.md"),
        "---\ntitle: A\ndescription: About a.\n---\n\nText.\n",
    )
    .unwrap();
    let run = |bundle: &Path, args: &[&str]| {
        let mut c = Command::cargo_bin("okfkit").unwrap();
        c.env("HOME", &home)
            .env("XDG_CONFIG_HOME", home.join(".config"))
            .env("XDG_CACHE_HOME", home.join(".cache"))
            .current_dir(&project)
            .arg("-b")
            .arg(bundle)
            .args(args);
        c
    };
    stdout(&mut run(&kb, &["agent", "install", "--claude"]));
    let r = json_of(&mut run(&kb, &["doctor"]));
    assert_eq!(r["ok"], true, "{r}");
    let mcp = r["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "mcp")
        .unwrap();
    assert_eq!(mcp["status"], "ok", "{mcp}");
    assert!(r["checks"].to_string().contains("Claude Code"));
    // The bundle moves: doctor fails (exit 4) and says how to fix it.
    let moved = tmp.path().join("moved");
    std::fs::rename(&kb, &moved).unwrap();
    let out = run(&moved, &["doctor", "--json"]).output().unwrap();
    assert_eq!(out.status.code(), Some(4));
    let r: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(r["ok"], false);
    // This project's install still points at the old path.
    assert!(r["checks"].to_string().contains("no longer exists"), "{r}");
    assert!(
        r["next"]
            .to_string()
            .contains("okfkit agent install --replace"),
        "{r}"
    );
    let out = run(&kb, &["doctor", "--json"]).output().unwrap();
    let r: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(r["checks"][1]["id"], "bundle");
    assert_eq!(r["checks"][1]["status"], "fail");
}

#[test]
fn agent_instructions_agree_everywhere() {
    // The same rules reach agents through README, llms.txt, help --agent, onboard and the skill.
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let readme = std::fs::read_to_string(root.join("README.md")).unwrap();
    let llms = std::fs::read_to_string(root.join("llms.txt")).unwrap();
    let skill =
        std::fs::read_to_string(root.join("crates/okfkit-skills/skills/okfkit-setup/SKILL.md"))
            .unwrap();
    let guide = stdout(
        Command::cargo_bin("okfkit")
            .unwrap()
            .args(["help", "--agent"]),
    );
    for text in [&readme, &llms, &skill, &guide] {
        for must in [
            "onboard",
            "--accept-license",
            "--yes",
            "--write",
            "--force",
            "--replace",
            "help --agent",
        ] {
            if std::ptr::eq(text, &guide) && must == "help --agent" {
                continue;
            }
            assert!(
                text.contains(must),
                "`{must}` missing from one of the agent instructions"
            );
        }
    }
    assert!(
        llms.starts_with("# okfkit\n\n> "),
        "llms.txt format: title, then a summary quote"
    );
}

#[test]
fn onboard_understands_real_folders() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    let write = |rel: &str, text: &str| {
        let p = tmp.path().join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    };
    let okf = "---\ntype: Guide\ntitle: T\ndescription: About t.\n---\n# T\n";
    // A software repository with MkDocs docs and installed dependencies.
    write("repo/Cargo.toml", "[package]\nname = \"x\"\n");
    write("repo/README.md", "# X\n");
    write("repo/mkdocs.yml", "site_name: X\n");
    write("repo/node_modules/dep/README.md", "# dep\n");
    write(
        "repo/docs/index.md",
        "# X docs\n\nWelcome to the X documentation, start here.\n",
    );
    write(
        "repo/docs/guide/install.md",
        "# Install\n\nRun the installer and restart your shell.\n",
    );
    // An empty folder; a mostly-OKF bundle with one broken file; a bot repository.
    std::fs::create_dir_all(tmp.path().join("empty")).unwrap();
    for i in 0..5 {
        write(&format!("partial/p{i}.md"), okf);
    }
    write("partial/loose.md", "# Loose\n\nNo frontmatter.\n");
    write("bot/.git/HEAD", "ref\n");
    write("bot/app/main.py", "print(1)\n");
    write("bot/README.md", "# Bot\n");
    for i in 0..5 {
        write(&format!("bot/knowledge/k{i}.md"), okf);
    }
    let plan = |dir: &str, extra: &[&str]| -> Value {
        let mut c = Command::cargo_bin("okfkit").unwrap();
        c.env("HOME", &home)
            .env("XDG_CONFIG_HOME", home.join(".config"))
            .env("XDG_CACHE_HOME", home.join(".cache"))
            .env("PATH", "/usr/bin:/bin")
            .current_dir(tmp.path().join(dir))
            .args(extra)
            .args([
                "--state-dir",
                tmp.path()
                    .join(format!("st-{}", dir.replace('/', "-")))
                    .to_str()
                    .unwrap(),
            ])
            .arg("onboard");
        json_of(&mut c)
    };
    let ids = |v: &Value| -> Vec<String> {
        v["steps"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["id"].as_str().unwrap().to_owned())
            .collect()
    };
    // Repository root: go to docs/, nothing else.
    let p = plan("repo", &[]);
    assert_eq!(ids(&p), ["bundle"], "{p}");
    assert_eq!(p["steps"][0]["commands"][0], "okfkit -b docs onboard");
    // From the root with -b docs: every command keeps -b docs; the site is used as is or annotated in place.
    let p = plan("repo", &["-b", "docs"]);
    assert!(ids(&p).contains(&"metadata".to_owned()), "{p}");
    let meta = p["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["id"] == "metadata")
        .unwrap();
    assert_eq!(
        meta["options"][0]["commands"][0],
        "okfkit -b docs adopt --write"
    );
    // Empty folder: start a knowledge base.
    assert_eq!(ids(&plan("empty", &[])), ["init"]);
    // Mostly OKF: fix the one file in place.
    let p = plan("partial", &[]);
    let fix = p["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["id"] == "fix")
        .expect("fix step");
    assert_eq!(
        fix["options"][0]["commands"][0],
        "okfkit adopt --only loose.md --write"
    );
    // Bot repository: the bundle is knowledge/.
    let p = plan("bot", &[]);
    assert_eq!(p["steps"][0]["commands"][0], "okfkit -b knowledge onboard");
    // adopt on the docs site keeps index.md and touches no dependency.
    let mut c = Command::cargo_bin("okfkit").unwrap();
    c.current_dir(tmp.path().join("repo")).args(["adopt", "."]);
    let a = json_of(&mut c);
    let paths = a["changes"].to_string();
    assert!(
        !paths.contains("node_modules") && !paths.contains("overview.md"),
        "{paths}"
    );
}
