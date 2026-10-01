//! MCP over streamable HTTP through `router()`: handshake, per-request scope, bearer token.
#![cfg(feature = "http")]

use std::path::Path;
use std::sync::Arc;

use okfkit::{Bundle, Scope};
use okfkit_mcp::{HttpOptions, RequestInfo, ServerOptions, router};
use serde_json::{Value, json};

async fn start(token: Option<&str>) -> String {
    let b = Bundle::open_in_memory(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/business"),
    )
    .unwrap();
    b.sync().unwrap();
    // The host decides the scope per request: guests do not see contracts.
    let scopes = Arc::new(|req: &RequestInfo<'_>| match req.header("x-user") {
        Some("guest") => Scope::all().deny("contracts/**").unwrap(),
        _ => Scope::all(),
    });
    let app = router(
        b,
        scopes,
        &ServerOptions::default(),
        &HttpOptions {
            token: token.map(str::to_owned),
            allowed_hosts: vec![],
        },
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    format!("http://{addr}/mcp")
}

/// POSTs one JSON-RPC message; returns (status, session id, JSON body or SSE `data:` payload).
fn post(
    url: &str,
    session: Option<&str>,
    headers: &[(&str, &str)],
    body: Value,
) -> (u16, Option<String>, Option<Value>) {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .build()
        .into();
    let mut req = agent
        .post(url)
        .header("Accept", "application/json, text/event-stream")
        .header("Content-Type", "application/json")
        .header("MCP-Protocol-Version", "2025-06-18");
    if let Some(s) = session {
        req = req.header("Mcp-Session-Id", s);
    }
    for (k, v) in headers {
        req = req.header(*k, *v);
    }
    let resp = req.send_json(body).unwrap();
    let status = resp.status().as_u16();
    let sid = resp
        .headers()
        .get("mcp-session-id")
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let text = resp.into_body().read_to_string().unwrap_or_default();
    let json = serde_json::from_str::<Value>(&text).ok().or_else(|| {
        text.lines()
            .filter_map(|l| l.strip_prefix("data:"))
            .filter_map(|d| serde_json::from_str::<Value>(d.trim()).ok())
            .find(|v| v.get("id").is_some())
    });
    (status, sid, json)
}

#[tokio::test(flavor = "multi_thread")]
async fn http_handshake_scope_and_token() {
    let url = start(Some("s3cret")).await;
    let auth = [("Authorization", "Bearer s3cret")];
    let init = json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
        "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "t", "version": "0"}}});

    let u = url.clone();
    let (status, _, _) = tokio::task::spawn_blocking(move || post(&u, None, &[], init))
        .await
        .unwrap();
    assert_eq!(status, 401, "no token");

    let u = url.clone();
    let init = json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
        "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "t", "version": "0"}}});
    let (status, sid, body) = tokio::task::spawn_blocking(move || post(&u, None, &auth, init))
        .await
        .unwrap();
    assert_eq!(status, 200);
    assert_eq!(body.unwrap()["result"]["serverInfo"]["name"], "okfkit");
    let sid = sid.expect("session id");

    let (u, s) = (url.clone(), sid.clone());
    tokio::task::spawn_blocking(move || {
        post(
            &u,
            Some(&s),
            &auth,
            json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
        )
    })
    .await
    .unwrap();

    let call = |user: &'static str| {
        let (u, s) = (url.clone(), sid.clone());
        async move {
            tokio::task::spawn_blocking(move || {
                let headers = [("Authorization", "Bearer s3cret"), ("x-user", user)];
                post(
                    &u,
                    Some(&s),
                    &headers,
                    json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {
                    "name": "kb_get", "arguments": {"id": "contracts/acme-corp/supply-202504"}}}),
                )
            })
            .await
            .unwrap()
            .2
            .unwrap()
        }
    };
    let admin = call("admin").await;
    assert_ne!(admin["result"]["isError"], true, "{admin}");
    let guest = call("guest").await;
    assert_eq!(guest["result"]["isError"], true, "{guest}");
}
