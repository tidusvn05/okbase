//! MCP over streamable HTTP (design §4.6, §10): `router()` for hosts that run their
//! own axum server, and `serve_http()` for `okbase mcp serve --http`.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::Router;
use axum::extract::Request;
use axum::http::StatusCode;
use axum::middleware::{self, Next};
use axum::response::Response;
use okbase::Bundle;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};

use crate::{Error, KbServer, ScopeProvider, ServerOptions};

/// Path the MCP endpoint is mounted at.
pub const MCP_PATH: &str = "/mcp";

/// Options of the HTTP transport.
#[derive(Debug, Clone, Default)]
pub struct HttpOptions {
    /// Required `Authorization: Bearer` token. `serve_http` refuses non-loopback
    /// addresses without one.
    pub token: Option<String>,
    /// Extra `Host` values to accept (DNS-rebinding protection allows loopback only by default).
    pub allowed_hosts: Vec<String>,
}

/// An axum router serving the bundle's MCP tools at `/mcp`, for hosts such as qobot.
/// The host decides each caller's scope through `scopes` (it sees the request headers).
pub fn router(
    bundle: Bundle,
    scopes: Arc<dyn ScopeProvider>,
    options: &ServerOptions,
    http: &HttpOptions,
) -> Router {
    let server = KbServer::new(bundle, scopes, options);
    let mut config = StreamableHttpServerConfig::default();
    config
        .allowed_hosts
        .extend(http.allowed_hosts.iter().cloned());
    let service = StreamableHttpService::new(
        move || Ok(server.clone()),
        Arc::new(LocalSessionManager::default()),
        config,
    );
    let router = Router::new().nest_service(MCP_PATH, service);
    match &http.token {
        Some(token) => {
            let token = Arc::new(token.clone());
            router.layer(middleware::from_fn(move |req: Request, next: Next| {
                let token = token.clone();
                async move { check_bearer(&token, req, next).await }
            }))
        }
        None => router,
    }
}

async fn check_bearer(token: &str, req: Request, next: Next) -> Result<Response, StatusCode> {
    let got = req
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));
    match got {
        Some(t) if constant_time_eq(t.trim().as_bytes(), token.as_bytes()) => {
            Ok(next.run(req).await)
        }
        _ => Err(StatusCode::UNAUTHORIZED),
    }
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Serves the bundle over HTTP at `addr` until Ctrl-C. Non-loopback addresses need a token.
pub async fn serve_http(
    bundle: Bundle,
    scopes: Arc<dyn ScopeProvider>,
    options: &ServerOptions,
    http: &HttpOptions,
    addr: SocketAddr,
) -> Result<(), Error> {
    if !addr.ip().is_loopback() && http.token.is_none() {
        return Err(Error::Serve(format!(
            "refusing to serve on {addr} without a token: set one (OKBASE_MCP_TOKEN) or bind to 127.0.0.1"
        )));
    }
    let app = router(bundle, scopes, options, http);
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|e| Error::Serve(format!("bind {addr}: {e}")))?;
    eprintln!(
        "okbase MCP on http://{addr}{MCP_PATH}{}",
        if http.token.is_some() {
            " (bearer token required)"
        } else {
            ""
        }
    );
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
        .map_err(|e| Error::Serve(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::constant_time_eq;

    #[test]
    fn token_compare() {
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(!constant_time_eq(b"abc", b"abd") && !constant_time_eq(b"abc", b"abcd"));
    }
}
