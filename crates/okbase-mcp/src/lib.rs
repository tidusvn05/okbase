//! MCP server for okbase bundles (stdio in v0.1).
//!
//! Tools are registered by capability and named `<prefix>_<tool>` (default
//! prefix `kb`). Each call returns the compact text form validated in the spikes
//! as content, and the typed result as `structuredContent` (the same JSON as the
//! CLI's `--json`).

#[cfg(feature = "http")]
mod http_server;
pub mod tools;

#[cfg(feature = "http")]
pub use http_server::{HttpOptions, router, serve_http};

use std::sync::Arc;

use okbase::{Bundle, Scope};
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, ErrorData,
    Implementation, ListToolsResult, PaginatedRequestParams, ServerCapabilities, ServerConfig,
    Tool,
};
use rmcp::service::{MaybeSendFuture, RequestContext, RoleServer};
use rmcp::{ServerHandler, ServiceExt};
use serde_json::{Map, Value, json};

pub use tools::{BundleFacts, ToolDef, all_tools};

/// Decides what each caller may see. The host implements it; okbase only enforces the result.
pub trait ScopeProvider: Send + Sync {
    /// The scope for the current request.
    fn scope(&self, request: &RequestInfo<'_>) -> Scope;
}

impl ScopeProvider for Scope {
    fn scope(&self, _request: &RequestInfo<'_>) -> Scope {
        self.clone()
    }
}

impl<F: Fn(&RequestInfo<'_>) -> Scope + Send + Sync> ScopeProvider for F {
    fn scope(&self, request: &RequestInfo<'_>) -> Scope {
        self(request)
    }
}

/// What okbase knows about the caller of a tool, for [`ScopeProvider`].
#[derive(Debug, Clone, Copy, Default)]
pub struct RequestInfo<'a> {
    /// HTTP request headers (MCP over HTTP); `None` over stdio.
    pub headers: Option<&'a http::HeaderMap>,
}

impl RequestInfo<'_> {
    /// A header value as text.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers?.get(name)?.to_str().ok()
    }

    /// The token of an `Authorization: Bearer <token>` header.
    pub fn bearer_token(&self) -> Option<&str> {
        self.header("authorization")?
            .strip_prefix("Bearer ")
            .map(str::trim)
    }
}

/// Server options.
#[derive(Debug, Clone)]
pub struct ServerOptions {
    /// Tool name prefix (`kb` → `kb_grep`).
    pub prefix: String,
    /// Tools to hide, by unprefixed name (`catalog`, `links`, …).
    pub disable: Vec<String>,
}

impl Default for ServerOptions {
    fn default() -> Self {
        ServerOptions {
            prefix: "kb".into(),
            disable: Vec::new(),
        }
    }
}

/// Errors from running the server.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// The MCP session could not start or ended with an error.
    #[error("MCP server: {0}")]
    Serve(String),
}

/// The okbase MCP server.
#[derive(Clone)]
pub struct KbServer {
    bundle: Bundle,
    scopes: Arc<dyn ScopeProvider>,
    tools: Arc<Vec<ToolDef>>,
    prefix: String,
}

impl KbServer {
    /// Builds a server for `bundle`. The tool list follows the bundle's capabilities.
    pub fn new(bundle: Bundle, scopes: Arc<dyn ScopeProvider>, options: &ServerOptions) -> Self {
        let facts = BundleFacts::from_bundle(&bundle, &scopes.scope(&RequestInfo::default()));
        let caps = bundle.capabilities();
        let tools = all_tools(&facts)
            .into_iter()
            .filter(|t| caps.has(t.capability) && !options.disable.iter().any(|d| d == t.name))
            .collect();
        KbServer {
            bundle,
            scopes,
            tools: Arc::new(tools),
            prefix: options.prefix.clone(),
        }
    }

    /// `kb_<tool>`; `data_*` tools keep their own names.
    fn full_name(&self, name: &str) -> String {
        if self.prefix.is_empty() || name.starts_with("data_") {
            name.to_owned()
        } else {
            format!("{}_{name}", self.prefix)
        }
    }

    /// The tools as listed by `tools/list`.
    pub fn list(&self) -> Vec<Tool> {
        self.tools
            .iter()
            .map(|t| {
                let schema = t.input_schema.as_object().cloned().unwrap_or_default();
                let mut tool = Tool::new(
                    self.full_name(t.name),
                    t.description.clone(),
                    Arc::new(schema),
                );
                let mut ann = rmcp::model::ToolAnnotations::default();
                ann.read_only_hint = Some(true);
                ann.open_world_hint = Some(false);
                tool.annotations = Some(ann);
                tool
            })
            .collect()
    }

    /// Runs one tool call without request information (as over stdio). Blocking.
    pub fn call(&self, name: &str, args: Map<String, Value>) -> Result<(String, Value), String> {
        self.call_as(name, args, &RequestInfo::default())
    }

    /// Runs one tool call for a request (blocking; the index is SQLite). Returns `(text, structured JSON)`.
    pub fn call_as(
        &self,
        name: &str,
        args: Map<String, Value>,
        request: &RequestInfo<'_>,
    ) -> Result<(String, Value), String> {
        let short = if self.prefix.is_empty() || name.starts_with("data_") {
            name
        } else {
            name.strip_prefix(&format!("{}_", self.prefix))
                .unwrap_or("")
        };
        if !self.tools.iter().any(|t| t.name == short) {
            return Err(format!("unknown tool: {name}"));
        }
        // Documents may change during a session (an agent curating the bundle, an editor):
        // re-sync cheaply when the last sync is a little old.
        self.bundle
            .refresh(okbase::REFRESH_INTERVAL)
            .map_err(|e| format!("updating the index: {e}"))?;
        let scope = self.scopes.scope(request);
        let args = Value::Object(args);
        let err = |e: okbase::Error| e.to_string();
        match short {
            "data_tables" => {
                let r = self.bundle.data_tables(&scope).map_err(err)?;
                Ok((r.to_text(), to_json(&r)))
            }
            "data_query" => {
                let sql = args
                    .get("sql")
                    .and_then(Value::as_str)
                    .ok_or("missing `sql`")?;
                let r = self
                    .bundle
                    .data_query(sql, &okbase::DataLimits::default(), &scope)
                    .map_err(err)?;
                Ok((r.to_text(), to_json(&r)))
            }
            "search" => {
                let req: okbase::SearchRequest = from_args(args)?;
                let r = self.bundle.search(&req, &scope).map_err(err)?;
                Ok((r.to_text(), to_json(&r)))
            }
            "catalog" => {
                let r = self
                    .bundle
                    .catalog(&okbase::CatalogOptions::default(), &scope)
                    .map_err(err)?;
                Ok((r.content.clone(), to_json(&r)))
            }
            "list" => {
                let dir = args.get("dir").and_then(Value::as_str).unwrap_or(".");
                let r = self.bundle.list(dir, &scope).map_err(err)?;
                Ok((r.content.clone(), to_json(&r)))
            }
            "grep" => {
                let req: okbase::GrepRequest = from_args(args)?;
                let r = self.bundle.grep(&req, &scope).map_err(err)?;
                Ok((r.to_text(req.files_only), to_json(&r)))
            }
            "get" => {
                let req: okbase::GetRequest = from_args(args)?;
                let r = self.bundle.get(&req, &scope).map_err(err)?;
                Ok((r.to_text(), to_json(&r)))
            }
            "query" => {
                let req: okbase::QueryRequest = from_args(move_unknown_to_fields(args))?;
                let r = self.bundle.query(&req, &scope).map_err(err)?;
                Ok((r.to_text(), to_json(&r)))
            }
            "links" => {
                let id = args
                    .get("id")
                    .and_then(Value::as_str)
                    .ok_or("missing `id`")?;
                let r = self.bundle.links(id, &scope).map_err(err)?;
                Ok((links_text(&r), to_json(&r)))
            }
            _ => Err(format!("unknown tool: {name}")),
        }
    }
}

fn to_json<T: serde::Serialize>(v: &T) -> Value {
    serde_json::to_value(v).unwrap_or(Value::Null)
}

fn from_args<T: serde::de::DeserializeOwned>(v: Value) -> Result<T, String> {
    serde_json::from_value(v).map_err(|e| format!("invalid arguments: {e}"))
}

/// Keys of `kb_query` other than the documented ones are treated as field filters
/// (`{"region": ["VN"]}` → `fields.region`), as in the biz-meta spike's tool. `<field>_from` and
/// `<field>_to` are range ends (`effective_to_from` → `ranges.effective_to.from`), like
/// `updated_from`/`updated_to`: agents write them that way, and as field filters they would
/// silently match nothing.
fn move_unknown_to_fields(args: Value) -> Value {
    const KNOWN: [&str; 20] = [
        "type",
        "tags_all",
        "tags_any",
        "status",
        "status_not",
        "lang",
        "fields",
        "path",
        "text",
        "updated_from",
        "updated_to",
        "ranges",
        "active_on",
        "sort",
        "limit",
        "count_only",
        "facets",
        "sum_field",
        "filter",
        "id",
    ];
    let Value::Object(mut map) = args else {
        return args;
    };
    let unknown: Vec<String> = map
        .keys()
        .filter(|k| !KNOWN.contains(&k.as_str()))
        .cloned()
        .collect();
    for key in unknown {
        let v = map.remove(&key).expect("present");
        let end = [("_from", "from"), ("_to", "to")]
            .into_iter()
            .find_map(|(suffix, end)| Some((key.strip_suffix(suffix)?, end)));
        if let (Some((field, end)), Some(bound)) = (end, scalar_text(&v))
            && !field.is_empty()
        {
            let ranges = map.entry("ranges").or_insert_with(|| json!({}));
            if let Some(r) = ranges.as_object_mut() {
                let range = r.entry(field).or_insert_with(|| json!({}));
                if let Some(range) = range.as_object_mut() {
                    range.insert(end.to_owned(), Value::String(bound));
                }
            }
            continue;
        }
        let values: Vec<Value> = match v {
            Value::Array(items) => items,
            Value::Null => continue,
            other => vec![other],
        };
        let values: Vec<Value> = values
            .into_iter()
            .map(|x| {
                x.as_str().map_or_else(
                    || Value::String(x.to_string()),
                    |s| Value::String(s.to_owned()),
                )
            })
            .collect();
        let fields = map.entry("fields").or_insert_with(|| json!({}));
        if let Some(f) = fields.as_object_mut() {
            f.insert(key, Value::Array(values));
        }
    }
    Value::Object(map)
}

/// A string or number as text (range bounds); `None` for other values.
fn scalar_text(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

fn links_text(r: &okbase::LinksResult) -> String {
    let mut out = format!("# Links of {}\n", r.id);
    out.push_str("\n## Outgoing\n");
    for l in &r.outgoing {
        let target = l.id.as_deref().unwrap_or(&l.raw);
        out.push_str(&format!(
            "- {target}{} — {}\n",
            if l.exists { "" } else { " (missing)" },
            l.text
        ));
    }
    out.push_str("\n## Backlinks\n");
    for l in &r.backlinks {
        out.push_str(&format!(
            "- {} — {}\n",
            l.id.as_deref().unwrap_or(""),
            l.text
        ));
    }
    out
}

impl KbServer {
    /// How to answer from the bundle: the rules of the `okbase-answer` skill, sent with the
    /// server so every agent gets them (agents rarely invoke skills: spike S9).
    pub fn instructions(&self) -> String {
        let has = |name: &str| self.tools.iter().any(|t| t.name == name);
        let p = &self.prefix;
        let tool = |name: &str| self.full_name(name);
        let root = self.bundle.root();
        let root = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_owned());
        let mut out = format!(
            "Tools for the markdown knowledge bundle at {}.\n",
            root.display()
        );
        if has("catalog") {
            out.push_str(&format!(
                "- Orient with {} (or {}) unless the catalog is in your context.\n",
                tool("catalog"),
                tool("list")
            ));
        }
        if has("query") {
            out.push_str(&format!(
                "- List, count, \"which documents\", filters by type, tag, status, date or field: {p}_query \
                 (facets, count_only, sum_field). Do not count or add up by reading documents.\n"
            ));
        }
        if has("data_query") {
            out.push_str(
                "- Numbers in spreadsheets: data_tables, then data_query with SQL aggregates. \
                 Never add numbers up yourself.\n",
            );
        }
        if has("grep") {
            out.push_str(&format!(
                "- Exact terms (keys, codes, names): {p}_grep with an alternation of spellings, synonyms and \
                 the documents' languages; files_only first.\n"
            ));
        }
        if has("search") {
            out.push_str(&format!("- Questions about meaning: {p}_search.\n"));
        }
        if has("get") {
            out.push_str(&format!(
                "- Read with {p}_get (section for long documents). "
            ));
        } else {
            out.push_str("- ");
        }
        out.push_str(
            "status: stable is in force, deprecated is superseded, draft is not approved; \
             active_on for \"in force on a date\".\n\
             - State only values you have read, cover every part of the question, and cite document ids.",
        );
        out
    }
}

impl ServerHandler for KbServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("okbase", env!("CARGO_PKG_VERSION")))
            .with_instructions(self.instructions())
    }

    fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListToolsResult, ErrorData>> + MaybeSendFuture + '_ {
        std::future::ready(Ok(ListToolsResult::with_all_items(self.list())))
    }

    fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<CallToolResponse, ErrorData>> + MaybeSendFuture + '_ {
        let server = self.clone();
        // Over HTTP, rmcp hands us the request parts so the host can scope per caller.
        let headers = context
            .extensions
            .get::<http::request::Parts>()
            .map(|p| p.headers.clone());
        async move {
            let name = request.name.to_string();
            let args = request.arguments.unwrap_or_default();
            let outcome = tokio::task::spawn_blocking(move || {
                server.call_as(
                    &name,
                    args,
                    &RequestInfo {
                        headers: headers.as_ref(),
                    },
                )
            })
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;
            let result = match outcome {
                Ok((text, structured)) => {
                    let mut r = CallToolResult::success(vec![ContentBlock::text(text)]);
                    r.structured_content = Some(structured);
                    r
                }
                // Tool-level errors go back to the agent so it can correct the call.
                Err(message) => CallToolResult::error(vec![ContentBlock::text(message)]),
            };
            Ok(result.into())
        }
    }
}

/// Serves the bundle over stdio until the client disconnects.
pub async fn serve_stdio(
    bundle: Bundle,
    scopes: Arc<dyn ScopeProvider>,
    options: &ServerOptions,
) -> Result<(), Error> {
    let server = KbServer::new(bundle, scopes, options);
    let running = server
        .serve(rmcp::transport::stdio())
        .await
        .map_err(|e| Error::Serve(e.to_string()))?;
    running
        .waiting()
        .await
        .map_err(|e| Error::Serve(e.to_string()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    #[test]
    fn field_from_and_to_become_a_range() {
        let args = json!({"type": ["Contract"], "effective_to_from": "2026-10-01",
            "effective_to_to": "2027-06-30", "region": "VN"});
        assert_eq!(
            super::move_unknown_to_fields(args),
            json!({"type": ["Contract"], "fields": {"region": ["VN"]},
                "ranges": {"effective_to": {"from": "2026-10-01", "to": "2027-06-30"}}})
        );
    }
}
