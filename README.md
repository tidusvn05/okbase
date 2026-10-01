# okfkit

**Make markdown knowledge bases work well for AI agents.**

okfkit is an open-source Rust toolkit for knowledge bundles in the [Open Knowledge Format (OKF)](https://github.com/GoogleCloudPlatform/open-knowledge-format). It helps agents such as Claude Code, Codex, OpenCode, or any MCP client answer questions from a folder of markdown files quickly, cheaply and correctly, in Vietnamese, English and Japanese.

> Status: **pre-release** (v0.1–v0.3 implemented; not yet published). okfkit is an independent community project and is not affiliated with Google.

## Quick start

```sh
cd my-bundle                          # a folder of markdown files (OKF or not)
okfkit status                         # size, level (L0–L3), recommended mode
okfkit advise                         # how to use okfkit here, simplest setup first
okfkit agent install --claude         # register the MCP server and skills (or --codex)
```

Plain markdown (docs sites, wikis, Obsidian vaults) first:

```sh
okfkit adopt ./docs --out ./docs-okf  # adds frontmatter and index.md; the source is not touched
okfkit -b ./docs-okf lint --level L1
```

## What it does

- **Works on existing OKF bundles** with zero configuration, read-only by default.
- **Agent tools** over MCP (stdio or HTTP) and the CLI (`--json` everywhere):
  - `kb_grep`: multilingual, accent-insensitive regex search;
  - `kb_query`: metadata filters, facets and sums;
  - `kb_get` / `kb_list`: section-level reads and directory listings;
  - `kb_catalog`: a prompt-ready catalog; `kb_links`: links and backlinks;
  - `data_tables` / `data_query`: read-only SQL over the bundle's CSV/TSV/XLSX sheets;
  - `kb_search`: semantic search (opt-in embeddings).
- **Agent Skills** (`okfkit-answer`, `okfkit-curate`, `okfkit-adopt`, `okfkit-tune`) that teach agents the most effective way to use the bundle.
- **Standard and lint:** quality levels L0–L3, text/JSON/SARIF output, `--fix-safe`; `okfkit vocab` for a tag vocabulary.
- **Every read takes a `Scope`** from the host (path rules, metadata filters, per HTTP caller); okfkit never decides permissions itself.

The defaults are backed by experiments (see [`spikes/`](spikes/README.md)). One example: lexical navigation with a strong `grep` matched embedding-based retrieval on bundles from 60k to 4.4M tokens, so embeddings are optional.

## Builds

| Build | Contents | Size |
|---|---|---|
| `okfkit` (default) | Everything above except local embeddings; Japanese dictionary downloaded on first use (`okfkit dict install` for offline machines, `OKFKIT_OFFLINE=1` to disable) | ~23 MB |
| `okfkit-full` (`--features full`) | Adds local embedding models (EmbeddingGemma 300M Q4, bge-m3 int8) and embeds the Japanese dictionary | ~96 MB |

Models are never bundled: `okfkit embed enable` records the choice (models with their own terms, such as Gemma, need `--accept-license`) and the first `okfkit embed index` downloads the model into the user cache.

```sh
okfkit embed enable --model bge-m3-int8   # MIT; or --accept-license for EmbeddingGemma
okfkit embed index
okfkit search "chính sách đổi trả"
okfkit mcp serve --http                    # http://127.0.0.1:7331/mcp (token required off loopback)
```

## Documentation

- Design: [`docs/design.md`](docs/design.md) (English translation of [`docs/PLAN.md`](docs/PLAN.md), Vietnamese)
- Implementation handoff: [`docs/HANDOFF.md`](docs/HANDOFF.md); acceptance results: `spikes/acceptance-v0.1/`, `-v0.2/`, `-v0.3/`
- Contributing: [`CONTRIBUTING.md`](CONTRIBUTING.md) · Agents: [`AGENTS.md`](AGENTS.md)

## License

Dual-licensed under either of MIT or Apache-2.0, at your option. See [`THIRD_PARTY.md`](THIRD_PARTY.md) for bundled and downloaded third-party material.
