# okbase

**Make markdown knowledge bases work well for AI agents.**

okbase is an open-source Rust toolkit for knowledge bundles in the [Open Knowledge Format (OKF)](https://github.com/GoogleCloudPlatform/open-knowledge-format). It helps agents such as Claude Code, Codex, OpenCode, or any MCP client answer questions from a folder of markdown files quickly, cheaply and correctly, in Vietnamese, English and Japanese.

> Status: **pre-release**. The v0.1–v0.3 milestones are implemented; crates are versioned `0.1.0-dev` and not yet published. okbase is an independent community project and is not affiliated with Google.

## For agents

If you are an AI agent asked to set up okbase, do this (the user only answers your questions):

1. Install it if `okbase --version` fails. Until it is published, build it from a clone of this
   repository with Rust 1.89+: `cargo install --locked --path crates/okbase-cli` (add
   `--features full` for semantic search and fine-tuning). After publication:
   `cargo install --locked okbase-cli`.
2. In the knowledge folder (or with `-b <folder>`), run **`okbase onboard`** and follow it: do the
   first step, run `okbase onboard` again, repeat. At every ASK step, ask the user and wait.
3. Read **`okbase help --agent`** once: JSON output, exit codes (3 = ask the user, 4 = findings),
   error codes, and what you must never decide alone (`--accept-license`, `--yes`, `--write`,
   `--force`, `--replace`, `--send-documents`).
4. Finish with `okbase doctor`, then tell the user to restart the agent session.
5. Inside Codex, its sandbox keeps `.codex/` read-only: ask the user to run
   `okbase agent install --codex` in their own terminal.

A machine-readable summary is in [`llms.txt`](llms.txt).

## Quick start

```sh
cd my-bundle                          # a folder of markdown files (OKF or not)
okbase status                         # size, level (L0–L3), recommended mode
okbase advise                         # how to use okbase here, simplest setup first
okbase agent install --claude         # register the MCP server and skills (or --codex)
```

Plain markdown (docs sites, wikis, Obsidian vaults) first:

```sh
okbase adopt ./docs --out ./docs-okf  # adds frontmatter and index.md; the source is not touched
okbase -b ./docs-okf lint --level L1
```

## What it does

- **Works on existing OKF bundles** with zero configuration, read-only by default.
- **Reads PDF, Word, PowerPoint, OpenDocument, EPUB and HTML directly** (page-cited, scans listed for an agent to transcribe), and imports them as markdown when you want to edit.
- **Agent tools** over MCP (stdio or HTTP) and the CLI (`--json` everywhere):
  - `kb_grep`: multilingual, accent-insensitive regex search;
  - `kb_query`: metadata filters, facets and sums;
  - `kb_get` / `kb_list`: section-level reads and directory listings;
  - `kb_catalog`: a prompt-ready catalog; `kb_links`: links and backlinks;
  - `data_tables` / `data_query`: read-only SQL over the bundle's CSV/TSV/XLSX sheets;
  - `kb_search`: semantic search (opt-in embeddings).
- **Agent Skills** (`okbase-answer`, `okbase-curate`, `okbase-adopt`, `okbase-author`, `okbase-import`, `okbase-tune`, `okbase-setup`) that teach agents the most effective way to use the bundle.
- **Standard and lint:** quality levels L0–L3, text/JSON/SARIF output, `--fix-safe`; `okbase vocab` for a tag vocabulary.
- **Every read takes a `Scope`** from the host (path rules, metadata filters, per HTTP caller); okbase never decides permissions itself.

The defaults are backed by experiments (see [`spikes/`](spikes/README.md)). One example: lexical navigation with a strong `grep` matched embedding-based retrieval on bundles from 60k to 4.4M tokens, so embeddings are optional.

## Builds

| Build | Contents | Size |
|---|---|---|
| `okbase` (default) | Everything above except local embeddings; Japanese dictionary downloaded on first use (`okbase dict install` for offline machines, `OKBASE_OFFLINE=1` to disable) | ~34 MB |
| `okbase-full` (`--features full`) | Adds local embedding models (EmbeddingGemma 300M Q4, bge-m3 int8, your own ONNX models) and fine-tuning; the dictionary stays external | ~60 MB |

Models are never bundled: `okbase embed enable` records the choice (models with their own terms, such as Gemma, need `--accept-license`) and the first `okbase embed index` downloads the model into the user cache.

```sh
okbase embed enable --model bge-m3-int8   # MIT; or --accept-license for EmbeddingGemma
okbase embed index
okbase search "chính sách đổi trả"
okbase mcp serve --http                    # http://127.0.0.1:7331/mcp (token required off loopback)
```

### Fine-tuning embeddings on a bundle

When people ask in another language than the documents, a model tuned on the bundle helps most
(spike S11: R@1 0.857 → 0.917, cross-language 0.815 → 0.92, after ONNX Q4 export). okbase never
calls an LLM: your agent writes the questions, okbase checks them against a fixed standard,
trains a LoRA adapter in a private Python environment (created on request), exports it as an
ONNX model and switches only when it beats the base model. Ask your agent to "tune embeddings
for this bundle" (skill `okbase-tune`), or follow `okbase embed tune guide`:

```sh
okbase embed tune init --langs vi,ja,en     # sample passages, hold out 15% of documents
okbase embed tune next                      # a batch for the agent; then: okbase embed tune submit <n> -
okbase embed tune check && okbase embed tune train --yes   # or --backend colab for a free GPU
okbase embed tune export && okbase embed tune eval         # base vs tuned, with a quality gate
okbase embed tune activate --write          # undo: okbase embed tune rollback --write
```

Tuned models stay in your user cache (`okbase embed models`); a model derived from EmbeddingGemma
is under the Gemma Terms of Use.

## Documentation

- Usage for small, medium and large projects (matches `okbase advise`): [`docs/usage.md`](docs/usage.md)
  - [Privacy: what leaves the machine](docs/usage.md#privacy-what-leaves-the-machine) · [Troubleshooting](docs/usage.md#troubleshooting) · [FAQ](docs/usage.md#faq) · [Environment variables](docs/usage.md#environment-variables)
- Design: [`docs/design.md`](docs/design.md); the evidence behind every default: [`spikes/README.md`](spikes/README.md)
- Maintainers' working notes (Vietnamese): [`docs/PLAN.md`](docs/PLAN.md), [`docs/HANDOFF.md`](docs/HANDOFF.md)
- Contributing: [`CONTRIBUTING.md`](CONTRIBUTING.md) · Agents: [`AGENTS.md`](AGENTS.md)

## License

Dual-licensed under either of MIT or Apache-2.0, at your option. See [`THIRD_PARTY.md`](THIRD_PARTY.md) for bundled and downloaded third-party material.
