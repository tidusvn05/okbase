# okbase

**Open Knowledge base: make markdown knowledge work well for AI agents.**

[![CI](https://github.com/tidusvn05/okbase/actions/workflows/ci.yml/badge.svg)](https://github.com/tidusvn05/okbase/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/tidusvn05/okbase?include_prereleases)](https://github.com/tidusvn05/okbase/releases)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)](#license)

okbase is an open-source Rust toolkit for knowledge bundles in the [Open Knowledge Format (OKF)](https://github.com/GoogleCloudPlatform/open-knowledge-format). It helps agents such as Claude Code, Codex, OpenCode, or any MCP client answer questions from a folder of markdown files quickly, cheaply and correctly, in Vietnamese, English and Japanese.

> Status: **pre-release**. The v0.1–v0.3 milestones are implemented; the first release (0.1.0) is being prepared. okbase is an independent community project and is not affiliated with Google.

## Install

Linux and macOS (x86_64 and arm64):

```sh
curl -fsSL https://raw.githubusercontent.com/tidusvn05/okbase/main/install.sh | sh
```

Add `-s -- --full` for **okbase-full** (semantic search and fine-tuning), `-s -- --version v0.1.0`
for a given release. The script downloads the release archive from GitHub, checks it against the
release's `SHA256SUMS` and installs `okbase` into `~/.local/bin` (`--dir` to change). Read it
first if you prefer: [`install.sh`](install.sh).

Windows (PowerShell):

```powershell
irm https://raw.githubusercontent.com/tidusvn05/okbase/main/install.ps1 | iex
```

From source (Rust 1.89+): `cargo install --locked --path crates/okbase-cli` in a clone
(`--features full` for okbase-full). Archives for every platform are on the
[releases page](https://github.com/tidusvn05/okbase/releases).

## For agents

If you are an AI agent asked to set up okbase, do this (the user only answers your questions):

1. Install it if `okbase --version` fails:
   `curl -fsSL https://raw.githubusercontent.com/tidusvn05/okbase/main/install.sh | sh` (on Windows,
   `irm https://raw.githubusercontent.com/tidusvn05/okbase/main/install.ps1 | iex`). Before the first
   release, build it from a clone instead: `cargo install --locked --path crates/okbase-cli`.
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

## What the experiments show

Every default comes from a measured experiment with real agents ([`spikes/README.md`](spikes/README.md)
has all of them, with data and scripts). The ones that shape how okbase works:

| Question | Result |
|---|---|
| Do agents need embeddings to find answers in a large bundle? | **No.** Lexical tools with a strong `grep` answered 100 / 100 / 93 / 90% on bundles of 60k, 150k, 1M and 4.4M tokens, on par with embedding retrieval (S4). okbase itself: 28/30 on the 1M-token bundle with Claude (v0.1). |
| Does it work with other agents? | Codex (gpt-6.1-sol) 26/30 on the same bundle. A small model (gpt-6-luna) 21/30: it finds the documents but leaves details out, and semantic search does not change that, so use a strong model for answering (S7). |
| Spreadsheets with ~10k rows? | Without SQL the agent gives up; with okbase's `data_query` it answers 10/10, 9× cheaper (S5). Metadata questions through `kb_query` cost 30–45% less. |
| Questions in another language than the documents? | Lexical search finds 3.5% of cross-language answers; EmbeddingGemma Q4 reaches R@1 0.857 on 300 vi/en/ja questions, 0.917 after fine-tuning on the bundle (S1, S11). |
| Can an agent set okbase up alone? | From one sentence: Claude 3/3 and Codex 12/12 runs found `okbase onboard` and never added a consent flag the user had not given; all but one ended with a passing `okbase doctor` (S13). |
| Do agents follow okbase's answering rules? | They never invoked the skill, but with the same rules sent as MCP server instructions they score 46/48 without any hints in the prompt (S9, S15). |
| Scanned PDFs? | okbase hands the agent the page's embedded image: 1–3 ms for a JPEG scan against 0.2–0.95 s to render the page, pixel-exact, with the same image tokens (S14). |

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
- Plans behind each feature: [`docs/plans/`](docs/plans/); the original v0.1 handoff: [`docs/HANDOFF.md`](docs/HANDOFF.md)
- Releases: [`docs/releasing.md`](docs/releasing.md) · [`CHANGELOG.md`](CHANGELOG.md)
- Contributing: [`CONTRIBUTING.md`](CONTRIBUTING.md) · Agents: [`AGENTS.md`](AGENTS.md)

## License

Dual-licensed under either of MIT or Apache-2.0, at your option. See [`THIRD_PARTY.md`](THIRD_PARTY.md) for bundled and downloaded third-party material.
