//! MCP server for okfkit bundles (stdio in v0.1).
//!
//! Tools are registered by capability and named `<prefix>_<tool>` (default
//! prefix `kb`). Each call returns the compact text form validated in the spikes
//! as content, and the typed result as `structuredContent` (the same JSON as the
//! CLI's `--json`).

pub mod tools;

use std::sync::Arc;

use okfkit::{Bundle, Scope};
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, ErrorData,
    Implementation, ListToolsResult, PaginatedRequestParams, ServerCapabilities, ServerConfig,
    Tool,
};
use rmcp::service::{MaybeSendFuture, RequestContext, RoleServer};
use rmcp::{ServerHandler, ServiceExt};
use serde_json::{Map, Value, json};

pub use tools::{BundleFacts, ToolDef, all_tools};

/// Decides what each caller may see. The host implements it; okfkit only enforces the result.
pub trait ScopeProvider: Send + Sync {
    /// The scope for the current request.
    fn scope(&self) -> Scope;
}

impl ScopeProvider for Scope {
    fn scope(&self) -> Scope {
        self.clone()
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

/// The okfkit MCP server.
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
        let facts = BundleFacts::from_bundle(&bundle, &scopes.scope());
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

    /// Runs one tool call (blocking; the index is SQLite). Returns `(text, structured JSON)`.
    pub fn call(&self, name: &str, args: Map<String, Value>) -> Result<(String, Value), String> {
        let short = if self.prefix.is_empty() || name.starts_with("data_") {
            name
        } else {
            name.strip_prefix(&format!("{}_", self.prefix))
                .unwrap_or("")
        };
        if !self.tools.iter().any(|t| t.name == short) {
            return Err(format!("unknown tool: {name}"));
        }
        let scope = self.scopes.scope();
        let args = Value::Object(args);
        let err = |e: okfkit::Error| e.to_string();
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
                    .data_query(sql, &okfkit::DataLimits::default(), &scope)
                    .map_err(err)?;
                Ok((r.to_text(), to_json(&r)))
            }
            "catalog" => {
                let r = self
                    .bundle
                    .catalog(&okfkit::CatalogOptions::default(), &scope)
                    .map_err(err)?;
                Ok((r.content.clone(), to_json(&r)))
            }
            "list" => {
                let dir = args.get("dir").and_then(Value::as_str).unwrap_or(".");
                let r = self.bundle.list(dir, &scope).map_err(err)?;
                Ok((r.content.clone(), to_json(&r)))
            }
            "grep" => {
                let req: okfkit::GrepRequest = from_args(args)?;
                let r = self.bundle.grep(&req, &scope).map_err(err)?;
                Ok((r.to_text(req.files_only), to_json(&r)))
            }
            "get" => {
                let req: okfkit::GetRequest = from_args(args)?;
                let r = self.bundle.get(&req, &scope).map_err(err)?;
                Ok((r.to_text(), to_json(&r)))
            }
            "query" => {
                let req: okfkit::QueryRequest = from_args(move_unknown_to_fields(args))?;
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
/// (`{"region": ["VN"]}` → `fields.region`), as in the biz-meta spike's tool.
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

fn links_text(r: &okfkit::LinksResult) -> String {
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

impl ServerHandler for KbServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("okfkit", env!("CARGO_PKG_VERSION")))
            .with_instructions(format!(
                "Tools for the markdown knowledge bundle at {}. Start from {p}_catalog (or {p}_list), \
                 use {p}_query to list/count/filter by metadata, {p}_grep for exact terms, and {p}_get to read. \
                 Cite document ids in answers.",
                self.bundle.root().display(),
                p = self.prefix
            ))
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
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<CallToolResponse, ErrorData>> + MaybeSendFuture + '_ {
        let server = self.clone();
        async move {
            let name = request.name.to_string();
            let args = request.arguments.unwrap_or_default();
            let outcome = tokio::task::spawn_blocking(move || server.call(&name, args))
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
