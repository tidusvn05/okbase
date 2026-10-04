//! Read-only web viewer for okbase bundles (`okbase view`).
//!
//! [`router`] serves a JSON API over the bundle's read functions (the same schemas
//! as the CLI's `--json`) and the embedded single-page viewer. Every request reads
//! with the scope the host's [`ScopeProvider`] returns for it; nothing writes to
//! the bundle. [`serve`] runs it on a loopback address.

#![forbid(unsafe_code)]

mod api;
pub mod render;

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::extract::Request;
use axum::http::{HeaderValue, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use okbase::Bundle;
pub use okbase_mcp::{RequestInfo, ScopeProvider};

mod assets {
    include!(concat!(env!("OUT_DIR"), "/assets.rs"));
}

/// Default address of `okbase view`.
pub const DEFAULT_ADDR: &str = "127.0.0.1:7332";

/// Options of the viewer.
#[derive(Debug, Clone, Default)]
pub struct ViewOptions {
    /// Extra `Host` values to accept (DNS-rebinding protection allows loopback names only by default).
    pub allowed_hosts: Vec<String>,
}

/// Errors from running the viewer.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// The address is not a loopback address.
    #[error(
        "refusing to serve the viewer on {0}: it has no authentication, so bind to 127.0.0.1 (or mount okbase_web::router behind your own)"
    )]
    NotLoopback(SocketAddr),
    /// Binding or serving failed.
    #[error("web viewer: {0}")]
    Serve(String),
}

pub(crate) struct AppState {
    pub bundle: Bundle,
    pub scopes: Arc<dyn ScopeProvider>,
    /// Last lint report of the whole bundle and when it was made.
    pub lint: Mutex<Option<(std::time::Instant, okbase::LintReport)>>,
}

/// An axum router with the viewer at `/` and its API at `/api/*`.
/// The host decides each caller's scope through `scopes` (it sees the request headers).
pub fn router(bundle: Bundle, scopes: Arc<dyn ScopeProvider>, options: &ViewOptions) -> Router {
    let state = Arc::new(AppState {
        bundle,
        scopes,
        lint: Mutex::new(None),
    });
    let hosts = Arc::new(options.allowed_hosts.clone());
    Router::new()
        .route("/api/overview", get(api::overview))
        .route("/api/query", post(api::query))
        .route("/api/graph", post(api::graph))
        .route("/api/grep", post(api::grep))
        .route("/api/catalog", get(api::catalog))
        .route("/api/lint", get(api::lint))
        .route("/api/doc/{*id}", get(api::doc))
        .route("/files/{*path}", get(api::file))
        .route("/api/{*rest}", get(api::not_found).post(api::not_found))
        .fallback(static_file)
        .with_state(state)
        .layer(middleware::from_fn(security_headers))
        .layer(middleware::from_fn(move |req: Request, next: Next| {
            let hosts = hosts.clone();
            async move { check_host(&hosts, req, next).await }
        }))
}

/// Serves the viewer at `addr` until Ctrl-C. Only loopback addresses are accepted.
pub async fn serve(
    bundle: Bundle,
    scopes: Arc<dyn ScopeProvider>,
    options: &ViewOptions,
    addr: SocketAddr,
    on_ready: impl FnOnce(SocketAddr),
) -> Result<(), Error> {
    if !addr.ip().is_loopback() {
        return Err(Error::NotLoopback(addr));
    }
    let app = router(bundle, scopes, options);
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|e| Error::Serve(format!("bind {addr}: {e}")))?;
    let local = listener
        .local_addr()
        .map_err(|e| Error::Serve(e.to_string()))?;
    on_ready(local);
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
        .map_err(|e| Error::Serve(e.to_string()))
}

/// Rejects requests whose `Host` is not a loopback name or an allowed host (DNS rebinding).
async fn check_host(allowed: &[String], req: Request, next: Next) -> Result<Response, StatusCode> {
    let host = req
        .headers()
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let name = if let Some(rest) = host.strip_prefix('[') {
        rest.split(']').next().unwrap_or("")
    } else {
        host.rsplit_once(':').map_or(host, |(h, _)| h)
    };
    let ok = matches!(name, "localhost" | "127.0.0.1" | "::1")
        || allowed
            .iter()
            .any(|a| a.eq_ignore_ascii_case(name) || a.eq_ignore_ascii_case(host));
    if ok {
        Ok(next.run(req).await)
    } else {
        Err(StatusCode::FORBIDDEN)
    }
}

async fn security_headers(req: Request, next: Next) -> Response {
    let mut resp = next.run(req).await;
    let h = resp.headers_mut();
    // Handlers may set a stricter policy (bundle files are sandboxed).
    h.entry(header::CONTENT_SECURITY_POLICY)
        .or_insert(HeaderValue::from_static(
            "default-src 'self'; img-src 'self' data:; style-src 'self' 'unsafe-inline'; \
             script-src 'self'; worker-src 'self' blob:; object-src 'none'; base-uri 'none'; \
             frame-ancestors 'none'",
        ));
    h.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    h.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    resp
}

async fn static_file(req: Request) -> Response {
    let path = req.uri().path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };
    match assets::ASSETS.iter().find(|(p, _)| *p == path) {
        Some((p, bytes)) => {
            let cache = if p.starts_with("assets/") {
                "public, max-age=31536000, immutable"
            } else {
                "no-cache"
            };
            (
                [
                    (header::CONTENT_TYPE, content_type(p)),
                    (header::CACHE_CONTROL, cache),
                ],
                *bytes,
            )
                .into_response()
        }
        None if path == "index.html" => (
            StatusCode::SERVICE_UNAVAILABLE,
            "the viewer was built without its web assets (crates/okbase-web/web/dist)",
        )
            .into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

pub(crate) fn content_type(path: &str) -> &'static str {
    let ext = path.rsplit_once('.').map_or("", |(_, e)| e);
    match ext.to_ascii_lowercase().as_str() {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "avif" => "image/avif",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        "txt" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}
