//! JSON API handlers. Each one reads with the caller's scope on a blocking thread.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::Json;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use okbase::{
    CatalogOptions, GetRequest, GetResult, GraphRequest, GrepRequest, LinksResult, QueryRequest,
    Scope,
};
use serde::Serialize;
use serde_json::json;

use crate::render::{Heading, render};
use crate::{AppState, RequestInfo};

/// How long a lint report of the whole bundle is reused.
const LINT_TTL: Duration = Duration::from_secs(10);
/// Token budget for a document shown in the viewer (whole documents in practice).
const DOC_TOKENS: usize = 1_000_000;
/// Bundle files the viewer serves (images referenced by documents).
const FILE_TYPES: [&str; 8] = ["png", "jpg", "jpeg", "gif", "webp", "avif", "svg", "ico"];

pub(crate) struct ApiError(StatusCode, String);

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(json!({ "error": self.1 }))).into_response()
    }
}

impl From<okbase::Error> for ApiError {
    fn from(e: okbase::Error) -> Self {
        let status = match &e {
            okbase::Error::Query(okbase_query::Error::NotFound(_)) => StatusCode::NOT_FOUND,
            okbase::Error::Query(okbase_query::Error::InvalidArgument(_)) => {
                StatusCode::BAD_REQUEST
            }
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        ApiError(status, e.to_string())
    }
}

/// Runs `f` on a blocking thread with a fresh index and the caller's scope.
async fn read<T: Serialize + Send + 'static>(
    state: Arc<AppState>,
    headers: HeaderMap,
    f: impl FnOnce(&AppState, &Scope) -> Result<T, ApiError> + Send + 'static,
) -> Result<Json<T>, ApiError> {
    tokio::task::spawn_blocking(move || {
        state.bundle.refresh(okbase::REFRESH_INTERVAL)?;
        let scope = state.scopes.scope(&RequestInfo {
            headers: Some(&headers),
        });
        f(&state, &scope).map(Json)
    })
    .await
    .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
}

#[derive(Serialize)]
pub(crate) struct Overview {
    /// Folder name of the bundle.
    name: String,
    stats: okbase::Stats,
    capabilities: okbase::Capabilities,
}

pub(crate) async fn overview(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<Overview>, ApiError> {
    read(state, headers, |s, scope| {
        Ok(Overview {
            name: s
                .bundle
                .root()
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            stats: s.bundle.stats(scope)?,
            capabilities: s.bundle.capabilities(),
        })
    })
    .await
}

pub(crate) async fn query(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<QueryRequest>,
) -> Result<Json<okbase::QueryResult>, ApiError> {
    read(state, headers, move |s, scope| {
        Ok(s.bundle.query(&req, scope)?)
    })
    .await
}

pub(crate) async fn graph(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<GraphRequest>,
) -> Result<Json<okbase::GraphResult>, ApiError> {
    read(state, headers, move |s, scope| {
        Ok(s.bundle.graph(&req, scope)?)
    })
    .await
}

pub(crate) async fn grep(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<GrepRequest>,
) -> Result<Json<okbase::GrepResult>, ApiError> {
    read(state, headers, move |s, scope| {
        Ok(s.bundle.grep(&req, scope)?)
    })
    .await
}

pub(crate) async fn catalog(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<okbase::CatalogResult>, ApiError> {
    read(state, headers, |s, scope| {
        Ok(s.bundle.catalog(&CatalogOptions::default(), scope)?)
    })
    .await
}

/// A document as the viewer shows it.
#[derive(Serialize)]
pub(crate) struct DocView {
    #[serde(flatten)]
    doc: GetResult,
    /// The body rendered to HTML.
    html: String,
    /// Headings of the body, with their anchors.
    toc: Vec<Heading>,
    links: LinksResult,
}

pub(crate) async fn doc(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<DocView>, ApiError> {
    read(state, headers, move |s, scope| {
        let doc = s.bundle.get(
            &GetRequest {
                id,
                max_tokens: Some(DOC_TOKENS),
                ..Default::default()
            },
            scope,
        )?;
        let links = s.bundle.links(&doc.id, scope)?;
        let (html, toc) = render(&doc.content, &doc.path, &links);
        Ok(DocView {
            doc,
            html,
            toc,
            links,
        })
    })
    .await
}

pub(crate) async fn lint(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<okbase::LintReport>, ApiError> {
    read(state, headers, |s, scope| {
        let report = {
            let mut cached = s.lint.lock().unwrap_or_else(|e| e.into_inner());
            match &*cached {
                Some((at, r)) if at.elapsed() < LINT_TTL => r.clone(),
                _ => {
                    let r = s
                        .bundle
                        .lint(&okbase::LintConfig::level(okbase::Level::L2))?;
                    *cached = Some((Instant::now(), r.clone()));
                    r
                }
            }
        };
        if scope.is_unrestricted() {
            return Ok(report);
        }
        // Keep diagnostics of visible files only; metadata filters need the visible set.
        let visible: Option<HashSet<String>> = if scope.has_filters() {
            let all = QueryRequest {
                limit: Some(usize::MAX),
                ..Default::default()
            };
            Some(
                s.bundle
                    .query(&all, scope)?
                    .docs
                    .into_iter()
                    .map(|d| d.id)
                    .collect(),
            )
        } else {
            None
        };
        let mut report = report;
        report.diagnostics.retain(|d| {
            let id = d.path.strip_suffix(".md").unwrap_or(&d.path);
            scope.permits_path(id) && visible.as_ref().is_none_or(|v| v.contains(id))
        });
        report.errors = report
            .diagnostics
            .iter()
            .filter(|d| d.severity == okbase::Severity::Error)
            .count();
        report.warnings = report.diagnostics.len() - report.errors;
        Ok(report)
    })
    .await
}

/// An image (or icon) file of the bundle, for documents that embed one.
pub(crate) async fn file(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(path): Path<String>,
) -> Response {
    let bad = || StatusCode::NOT_FOUND.into_response();
    let ext = path
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default();
    if !FILE_TYPES.contains(&ext.as_str())
        || path
            .split('/')
            .any(|p| p.is_empty() || p.starts_with('.') || p.contains('\\'))
    {
        return bad();
    }
    let result = tokio::task::spawn_blocking(move || -> Option<Vec<u8>> {
        let scope = state.scopes.scope(&RequestInfo {
            headers: Some(&headers),
        });
        if !scope.permits_path(&path) {
            return None;
        }
        let root = state.bundle.root().canonicalize().ok()?;
        let file = root.join(&path).canonicalize().ok()?;
        if !file.starts_with(&root) || !file.is_file() {
            return None;
        }
        std::fs::read(file).ok()
    })
    .await;
    match result {
        Ok(Some(bytes)) => (
            [
                (
                    header::CONTENT_TYPE,
                    crate::content_type(&format!("x.{ext}")),
                ),
                (header::CACHE_CONTROL, "no-cache"),
                (
                    header::CONTENT_SECURITY_POLICY,
                    "sandbox; default-src 'none'; style-src 'unsafe-inline'",
                ),
            ],
            bytes,
        )
            .into_response(),
        _ => bad(),
    }
}

pub(crate) async fn not_found() -> ApiError {
    ApiError(StatusCode::NOT_FOUND, "unknown API route".into())
}
