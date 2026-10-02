# okfkit

**Make markdown knowledge bases work well for AI agents.**

okfkit is an open-source Rust toolkit for knowledge bundles in the [Open Knowledge Format (OKF)](https://github.com/GoogleCloudPlatform/open-knowledge-format). It helps agents such as Claude Code, Codex, OpenCode, or any MCP client answer questions from a folder of markdown files quickly, cheaply and correctly, in Vietnamese, English and Japanese.

> Status: **pre-release** (v0.1–v0.3 implemented; not yet published). okfkit is an independent community project and is not affiliated with Google.

## For agents

If you are an AI agent asked to set up okfkit, do this (the user only answers your questions):

1. Install it if `okfkit --version` fails. Until it is published, build it from a clone of this
   repository: `cargo install --path crates/okfkit-cli` (add `--features full` for semantic search
   and fine-tuning). After publication: `cargo install okfkit-cli`.
2. In the knowledge folder (or with `-b <folder>`), run **`okfkit onboard`** and follow it: do the
   first step, run `okfkit onboard` again, repeat. At every ASK step, ask the user and wait.
3. Read **`okfkit help --agent`** once: JSON output, exit codes (3 = ask the user, 4 = findings),
   error codes, and what you must never decide alone (`--accept-license`, `--yes`, `--write`,
   `--force`, `--replace`).
4. Finish with `okfkit doctor`, then tell the user to restart the agent session.

A machine-readable summary is in [`llms.txt`](llms.txt).

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
| `okfkit-full` (`--features full`) | Adds local embedding models (EmbeddingGemma 300M Q4, bge-m3 int8, your own ONNX models), fine-tuning, and embeds the Japanese dictionary | ~96 MB |

Models are never bundled: `okfkit embed enable` records the choice (models with their own terms, such as Gemma, need `--accept-license`) and the first `okfkit embed index` downloads the model into the user cache.

```sh
okfkit embed enable --model bge-m3-int8   # MIT; or --accept-license for EmbeddingGemma
okfkit embed index
okfkit search "chính sách đổi trả"
okfkit mcp serve --http                    # http://127.0.0.1:7331/mcp (token required off loopback)
```

### Fine-tuning embeddings on a bundle

When people ask in another language than the documents, a model tuned on the bundle helps most
(spike S11: R@1 0.857 → 0.917, cross-language 0.815 → 0.92, after ONNX Q4 export). okfkit never
calls an LLM: your agent writes the questions, okfkit checks them against a fixed standard,
trains a LoRA adapter in a private Python environment (created on request), exports it as an
ONNX model and switches only when it beats the base model. Ask your agent to "tune embeddings
for this bundle" (skill `okfkit-tune`), or follow `okfkit embed tune guide`:

```sh
okfkit embed tune init --langs vi,ja,en     # sample passages, hold out 15% of documents
okfkit embed tune next                      # a batch for the agent; then: okfkit embed tune submit <n> -
okfkit embed tune check && okfkit embed tune train --yes   # or --backend colab for a free GPU
okfkit embed tune export && okfkit embed tune eval         # base vs tuned, with a quality gate
okfkit embed tune activate --write          # undo: okfkit embed tune rollback --write
```

Tuned models stay in your user cache (`okfkit embed models`); a model derived from EmbeddingGemma
is under the Gemma Terms of Use.

## Documentation

- Usage for small, medium and large projects (matches `okfkit advise`): [`docs/usage.md`](docs/usage.md)
- Design: [`docs/design.md`](docs/design.md) (English translation of [`docs/PLAN.md`](docs/PLAN.md), Vietnamese)
- Implementation handoff: [`docs/HANDOFF.md`](docs/HANDOFF.md); acceptance results: `spikes/acceptance-v0.1/`, `-v0.2/`, `-v0.3/`
- Contributing: [`CONTRIBUTING.md`](CONTRIBUTING.md) · Agents: [`AGENTS.md`](AGENTS.md)

## License

Dual-licensed under either of MIT or Apache-2.0, at your option. See [`THIRD_PARTY.md`](THIRD_PARTY.md) for bundled and downloaded third-party material.
