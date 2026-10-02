//! tools/list snapshot and a JSON-RPC session over an in-memory pipe.

use std::path::Path;
use std::sync::Arc;

use okfkit::{Bundle, Scope};
use okfkit_mcp::{KbServer, ServerOptions};
use rmcp::ServiceExt;
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

fn bundle(name: &str) -> Bundle {
    let b = Bundle::open_in_memory(
        &Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures")
            .join(name),
    )
    .unwrap();
    b.sync().unwrap();
    b
}

#[test]
fn tools_list_snapshot() {
    let server = KbServer::new(
        bundle("business"),
        Arc::new(Scope::all()),
        &ServerOptions::default(),
    );
    insta::assert_json_snapshot!("tools_list_business", server.list());
    let english = KbServer::new(
        bundle("openclaw-s"),
        Arc::new(Scope::all()),
        &ServerOptions::default(),
    );
    let grep = english
        .list()
        .into_iter()
        .find(|t| t.name == "kb_grep")
        .unwrap();
    // Monolingual English bundle: the spike's wording.
    assert!(
        grep.description
            .unwrap()
            .contains("Documents are in English: translate key terms first.")
    );
    let custom = KbServer::new(
        bundle("business"),
        Arc::new(Scope::all()),
        &ServerOptions {
            prefix: "docs".into(),
            disable: vec!["links".into()],
        },
    );
    let names: Vec<_> = custom
        .list()
        .into_iter()
        .map(|t| t.name.to_string())
        .collect();
    assert_eq!(
        names,
        [
            "docs_catalog",
            "docs_list",
            "docs_grep",
            "docs_get",
            "docs_query",
            "data_tables",
            "data_query"
        ]
    );
}

async fn rpc(
    lines: &mut tokio::io::Lines<BufReader<tokio::io::ReadHalf<tokio::io::DuplexStream>>>,
) -> Value {
    loop {
        let line = lines.next_line().await.unwrap().expect("server closed");
        let v: Value = serde_json::from_str(&line).unwrap();
        if v.get("id").is_some() {
            return v;
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn json_rpc_session() {
    let scope = Scope::all().deny("contracts/minh-phat-jsc/**").unwrap();
    let server = KbServer::new(
        bundle("business"),
        Arc::new(scope),
        &ServerOptions::default(),
    );
    let (server_io, client_io) = tokio::io::duplex(1 << 20);
    tokio::spawn(async move {
        let running = server.serve(server_io).await.expect("serve");
        let _ = running.waiting().await;
    });
    let (read, mut write) = tokio::io::split(client_io);
    let mut lines = BufReader::new(read).lines();
    let mut send = async |v: Value| {
        write.write_all(format!("{v}\n").as_bytes()).await.unwrap();
    };

    send(json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
        "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "test", "version": "0"}}}))
    .await;
    let init = rpc(&mut lines).await;
    assert_eq!(init["result"]["serverInfo"]["name"], "okfkit", "{init}");
    send(json!({"jsonrpc": "2.0", "method": "notifications/initialized"})).await;

    send(json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"})).await;
    let list = rpc(&mut lines).await;
    let names: Vec<&str> = list["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        [
            "kb_catalog",
            "kb_list",
            "kb_grep",
            "kb_get",
            "kb_query",
            "data_tables",
            "data_query",
            "kb_links"
        ]
    );

    // Spike-style call: `region` as a top-level filter.
    send(json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "kb_query", "arguments": {
        "type": ["Contract"], "region": ["VN"], "sum_field": "contract_value", "count_only": true}}}))
    .await;
    let q = rpc(&mut lines).await;
    let text = q["result"]["content"][0]["text"].as_str().unwrap();
    assert!(text.starts_with("total: "), "{text}");
    assert!(
        q["result"]["structuredContent"]["sum"]["count"]
            .as_u64()
            .unwrap()
            > 0
    );
    // Scope hides Minh Phat contracts, so the total is below the 17 VN contracts.
    assert!(q["result"]["structuredContent"]["total"].as_u64().unwrap() < 17);

    send(json!({"jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": {"name": "kb_get", "arguments": {
        "id": "contracts/minh-phat-jsc/maintenance-202401"}}}))
    .await;
    let hidden = rpc(&mut lines).await;
    assert_eq!(hidden["result"]["isError"], true, "{hidden}");
    assert!(
        hidden["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("not found")
    );

    send(json!({"jsonrpc": "2.0", "id": 6, "method": "tools/call", "params": {"name": "data_query", "arguments": {
        "sql": "SELECT region, SUM(units) AS units FROM sales_2026 GROUP BY region ORDER BY region"}}}))
    .await;
    let d = rpc(&mut lines).await;
    assert!(
        d["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .starts_with("| region | units |\n| JP |"),
        "{d}"
    );
    send(json!({"jsonrpc": "2.0", "id": 7, "method": "tools/call", "params": {"name": "data_query", "arguments": {"sql": "DROP TABLE sales_2026"}}}))
    .await;
    assert_eq!(rpc(&mut lines).await["result"]["isError"], true);

    send(json!({"jsonrpc": "2.0", "id": 5, "method": "tools/call", "params": {"name": "kb_grep", "arguments": {
        "pattern": "bảo hành|warranty", "files_only": true, "path": "policies/**"}}}))
    .await;
    let g = rpc(&mut lines).await;
    assert!(
        g["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("matches"),
        "{g}"
    );
}

#[test]
fn edits_during_a_session_are_seen() {
    let tmp = tempfile::tempdir().unwrap();
    let kb = tmp.path().join("kb");
    std::fs::create_dir_all(&kb).unwrap();
    std::fs::write(
        kb.join("a.md"),
        "---\ntitle: A\ndescription: First.\n---\n\nApples.\n",
    )
    .unwrap();
    let b = Bundle::open(
        &kb,
        okfkit::OpenOptions::default().state_dir(okfkit::StateDir::Path(tmp.path().join("state"))),
    )
    .unwrap();
    b.sync().unwrap();
    let server = KbServer::new(b, Arc::new(Scope::all()), &ServerOptions::default());
    let grep = |p: &str| {
        let args = json!({"pattern": p, "files_only": true});
        server
            .call("kb_grep", args.as_object().unwrap().clone())
            .unwrap()
            .1
    };
    assert_eq!(grep("zebra")["total_docs"], 0);
    // An agent (or an editor) changes the bundle mid-session.
    std::fs::write(
        kb.join("b.md"),
        "---\ntitle: B\ndescription: Second.\n---\n\nZebras.\n",
    )
    .unwrap();
    std::thread::sleep(okfkit::REFRESH_INTERVAL + std::time::Duration::from_millis(100));
    assert_eq!(grep("zebra")["total_docs"], 1);
}
