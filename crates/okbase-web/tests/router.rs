//! The viewer's router: API schemas, scope enforcement, Host checks and static files.

use std::path::Path;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt;
use okbase::{Bundle, Scope};
use okbase_web::{RequestInfo, ViewOptions, router};
use serde_json::{Value, json};
use tower::ServiceExt;

fn app(fixture: &str) -> axum::Router {
    let b = Bundle::open_in_memory(
        &Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures")
            .join(fixture),
    )
    .unwrap();
    b.sync().unwrap();
    // Guests do not see computations.
    let scopes = Arc::new(|req: &RequestInfo<'_>| match req.header("x-user") {
        Some("guest") => Scope::all().deny("computations/**").unwrap(),
        _ => Scope::all(),
    });
    router(b, scopes, &ViewOptions::default())
}

async fn send(app: &axum::Router, req: Request<Body>) -> (StatusCode, Vec<u8>, http::HeaderMap) {
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let headers = resp.headers().clone();
    let body = resp
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes()
        .to_vec();
    (status, body, headers)
}

fn get(path: &str) -> Request<Body> {
    Request::get(path)
        .header(header::HOST, "127.0.0.1:7332")
        .body(Body::empty())
        .unwrap()
}

fn post(path: &str, body: Value) -> Request<Body> {
    Request::post(path)
        .header(header::HOST, "localhost:7332")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

async fn json_of(app: &axum::Router, req: Request<Body>) -> (StatusCode, Value) {
    let (status, body, _) = send(app, req).await;
    (status, serde_json::from_slice(&body).unwrap_or(Value::Null))
}

#[tokio::test]
async fn api_matches_the_library() {
    let app = app("okf-official/acme_retail");
    let (status, o) = json_of(&app, get("/api/overview")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(o["name"], "acme_retail");
    assert_eq!(o["stats"]["docs"], 9);
    assert!(
        o["capabilities"]
            .as_array()
            .unwrap()
            .contains(&json!("read.links"))
    );

    let (_, g) = json_of(&app, post("/api/graph", json!({}))).await;
    assert_eq!(g["nodes"].as_array().unwrap().len(), 9);
    assert_eq!(g["edges"].as_array().unwrap().len(), 14);

    let (_, q) = json_of(
        &app,
        post(
            "/api/query",
            json!({ "type": ["Metric"], "facets": ["tags"] }),
        ),
    )
    .await;
    assert_eq!(q["total"], 3);
    assert!(q["facets"]["tags"].is_array());

    let (_, r) = json_of(
        &app,
        post("/api/grep", json!({ "pattern": "gross margin" })),
    )
    .await;
    assert!(r["total_docs"].as_u64().unwrap() > 0);

    let (status, d) = json_of(&app, get("/api/doc/metrics/gross-margin")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(d["id"], "metrics/gross-margin");
    let html = d["html"].as_str().unwrap();
    assert!(html.contains(r##"href="#/doc/metrics/revenue""##), "{html}");
    assert!(d["toc"].as_array().unwrap().len() >= 2);
    assert!(!d["links"]["backlinks"].as_array().unwrap().is_empty());

    let (status, l) = json_of(&app, get("/api/lint")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(l["diagnostics"].is_array());

    let (status, e) = json_of(&app, get("/api/doc/nope")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(e["error"].as_str().unwrap().contains("nope"));
    let (status, _) = json_of(&app, get("/api/unknown")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn scope_hides_documents_everywhere() {
    let app = app("okf-official/acme_retail");
    let guest = |mut req: Request<Body>| {
        req.headers_mut().insert("x-user", "guest".parse().unwrap());
        req
    };
    let (status, _) = json_of(&app, guest(get("/api/doc/computations/revenue-ytd"))).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = json_of(&app, get("/api/doc/computations/revenue-ytd")).await;
    assert_eq!(status, StatusCode::OK);

    let (_, g) = json_of(&app, guest(post("/api/graph", json!({})))).await;
    let text = g.to_string();
    assert!(!text.contains("computations/"), "{text}");

    let (_, d) = json_of(&app, guest(get("/api/doc/metrics/revenue"))).await;
    // The body may mention the path as written, but the link resolves to nothing for guests.
    let out = &d["links"]["outgoing"][0];
    assert_eq!(
        (out["id"].clone(), out["exists"].clone()),
        (Value::Null, json!(false))
    );
    let html = d["html"].as_str().unwrap();
    assert!(!html.contains("#/doc/computations/") && html.contains("#/missing"));

    let (_, l) = json_of(&app, guest(get("/api/lint"))).await;
    assert!(!l["diagnostics"].to_string().contains("computations/"));
    let (_, r) = json_of(
        &app,
        guest(post("/api/grep", json!({ "pattern": "revenue" }))),
    )
    .await;
    let ids: Vec<&str> = r["docs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["id"].as_str().unwrap())
        .collect();
    assert!(!ids.is_empty() && ids.iter().all(|id| !id.starts_with("computations/")));
}

#[tokio::test]
async fn rejects_foreign_hosts() {
    let app = app("okf-official/acme_retail");
    let req = Request::get("/api/overview")
        .header(header::HOST, "evil.example:7332")
        .body(Body::empty())
        .unwrap();
    assert_eq!(send(&app, req).await.0, StatusCode::FORBIDDEN);
    let req = Request::get("/api/overview")
        .header(header::HOST, "[::1]:7332")
        .body(Body::empty())
        .unwrap();
    assert_eq!(send(&app, req).await.0, StatusCode::OK);
}

#[tokio::test]
async fn serves_the_viewer_with_security_headers() {
    let app = app("okf-official/acme_retail");
    let (status, body, headers) = send(&app, get("/")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(String::from_utf8_lossy(&body).contains("<div id=\"app\">"));
    assert_eq!(headers[header::CONTENT_TYPE], "text/html; charset=utf-8");
    assert!(
        headers[header::CONTENT_SECURITY_POLICY]
            .to_str()
            .unwrap()
            .contains("default-src 'self'")
    );
    assert_eq!(headers[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
    assert_eq!(send(&app, get("/nope.js")).await.0, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn files_stay_inside_the_bundle() {
    let app = app("okf-official/acme_retail");
    for path in [
        "/files/../Cargo.toml",
        "/files/%2e%2e/%2e%2e/Cargo.toml",
        "/files/metrics/revenue.md",
        "/files/.okbase/index.png",
        "/files/missing.png",
    ] {
        assert_eq!(
            send(&app, get(path)).await.0,
            StatusCode::NOT_FOUND,
            "{path}"
        );
    }
}
