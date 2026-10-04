# Web viewer: `okbase view`

Status: implemented (2026-10-04).

## Why

okbase is used through agents (MCP, skills) and the CLI. People who maintain a bundle also need to
*see* it: how documents link to each other, which ones are isolated or have broken links, what the
lint report says, and what a document looks like rendered. `okbase view` serves a read-only viewer
for that on a loopback address.

## What it shows

| View | Content | Data |
|---|---|---|
| Overview | document and token counts, broken links, OKF level, documents by type and language, lint grouped by rule | `stats`, `capabilities`, `lint` |
| Graph | the link graph, colored by type, sized by degree; filters by type, tag and path; hover shows neighbours, click opens | `graph` (new) |
| Document | rendered markdown, table of contents, frontmatter, links, backlinks, the 1-hop neighbourhood | `get`, `links`, `graph` |
| Search | regex search with context lines, facets (type, tags, status, language) as filters | `grep`, `query` |

## Design

- **Crate `okbase-web`.** `router(bundle, scopes, &ViewOptions) -> axum::Router` and `serve(...)`.
  The JSON API wraps `Bundle` reads; its bodies are the same structs as the CLI's `--json`
  (`QueryRequest`, `GraphRequest`, `GrepRequest` in; `QueryResult`, `GraphResult`, … out). Reads run
  on blocking threads and call `Bundle::refresh(REFRESH_INTERVAL)` first, like the MCP server.
- **New read API `graph`** in `okbase-query` (`okbase graph --json`): nodes with in/out degree and a
  count of broken links, merged edges, an optional `Filter`, and a node limit (2000 by default,
  keeping the most linked documents). Hidden and filtered documents are neither nodes nor edge
  ends; a link to a hidden document is not counted as broken.
- **Scope.** Every request reads with the scope the host's `ScopeProvider` returns (the CLI passes
  `--allow`/`--deny`). The lint report is computed for the whole bundle (cached 10 s) and filtered to
  visible files per request.
- **Rendering on the server.** `pulldown-cmark` renders the body. Raw HTML in documents is shown as
  text. Links use the targets the index resolved: a visible document becomes `#/doc/<id>`, a missing
  or hidden one becomes an inert `#/missing`. Relative images are served from `/files/` (image types
  only, inside the bundle, scope-checked, sandboxed). External images become links, so the browser
  never contacts other hosts.
- **Safety.** Read-only (hard rule 2). Loopback addresses only, because the viewer has no
  authentication; hosts that need more mount `router()` behind their own. A Host-header allowlist
  blocks DNS rebinding. Strict CSP (`default-src 'self'`), `nosniff`, `no-referrer`.
- **Frontend.** Svelte 5 + Vite + TypeScript in `crates/okbase-web/web/`, a hash router, no UI
  library. The graph uses sigma.js (WebGL) with graphology and ForceAtlas2 (synchronous up to 300
  nodes, a web worker above). Light and dark themes; categorical colors follow the type (fixed
  slots by bundle-wide frequency, "Other" past seven).
- **No Node to build okbase.** `web/dist/` is committed; `build.rs` embeds it with `include_bytes!`.
  CI rebuilds it and fails if the committed copy differs. Licenses of the bundled JavaScript
  (all MIT) are in `web/dist/LICENSES.txt`, served at `/LICENSES.txt`.

## Not in this version

- An MCP `graph` tool: tool descriptions are agent prompts and need eval data first.
- Authentication and non-loopback serving.
- Editing (the `write` module does not exist yet).
- Semantic search in the viewer (the `embed` module); the search view is lexical.
