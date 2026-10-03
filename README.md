# okbase

**Open Knowledge base: make markdown knowledge work well for AI agents.**

[![CI](https://github.com/tidusvn05/okbase/actions/workflows/ci.yml/badge.svg)](https://github.com/tidusvn05/okbase/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/tidusvn05/okbase?include_prereleases)](https://github.com/tidusvn05/okbase/releases)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)](#license)

Give Claude Code, Codex or any MCP client a folder of markdown, and get accurate, cited answers
in any language. okbase reads [Open Knowledge Format](https://github.com/GoogleCloudPlatform/open-knowledge-format)
bundles, docs sites, wikis and PDFs; it is written in Rust and is read-only by default.

> **Pre-release:** the first release (0.1.0) is being prepared. okbase is a community project, not
> affiliated with Google.

## Get started: paste this to your agent

Open your agent in the folder that holds your knowledge and paste:

```text
Set up okbase (https://github.com/tidusvn05/okbase) so you can answer questions from the
knowledge in this folder.

1. If `okbase --version` fails, install it:
   curl -fsSL https://raw.githubusercontent.com/tidusvn05/okbase/main/install.sh | sh
   (Windows: irm https://raw.githubusercontent.com/tidusvn05/okbase/main/install.ps1 | iex)
   If you cannot run it (no network or a sandbox), give me the command and wait.
2. Run `okbase onboard` here and follow it until no steps are left (or only ones I declined): run
   its `run` steps, ask me its `ask` questions and wait for my answers, and never add a consent
   flag I did not approve.
3. Finish with `okbase doctor`. Then tell me briefly what you set up, how to use it now, and
   what okbase recommends for later.
```

The agent asks you before anything that is yours to decide (`--accept-license`, `--yes`,
`--write`, `--force`, `--replace`, `--send-documents`). For another goal, change the first sentence:

| Goal | First sentence |
|---|---|
| Documents in a subfolder | Set up okbase so you can answer questions from the knowledge in `docs/`. |
| A new knowledge base | Start a knowledge base about *&lt;topic&gt;* in this folder with okbase, and write the first document about *&lt;subject&gt;*. |
| Better answers | Use okbase to improve this knowledge base so agents answer better (`okbase onboard --goal curate`). |
| Remove okbase | Remove okbase from this project (`okbase onboard --goal remove`). |

Agents: [`llms.txt`](llms.txt) and `okbase help --agent` have the full contract.

## Install it yourself

| Platform | Command |
|---|---|
| Linux, macOS | `curl -fsSL https://raw.githubusercontent.com/tidusvn05/okbase/main/install.sh \| sh` |
| … with semantic search | add `-s -- --full` |
| Windows | `irm https://raw.githubusercontent.com/tidusvn05/okbase/main/install.ps1 \| iex` |
| From source (Rust 1.89+) | `cargo install --locked --path crates/okbase-cli` |

Each download is checked against the release's `SHA256SUMS`. More: [install, update, remove](docs/usage.md#install).

## What it does

| | |
|---|---|
| **Reads** | OKF bundles, plain markdown (docs sites, wikis, Obsidian), PDF, Word, PowerPoint, HTML; scans are handed to the agent to transcribe |
| **Agent tools** | MCP and CLI: `kb_grep`, `kb_query` (metadata, counts), `kb_get`, `kb_catalog`, `data_query` (SQL over CSV/XLSX), optional `kb_search` |
| **Organizes** | `adopt` plain markdown into OKF, `lint` levels L0–L3, `init` and `new` for new knowledge bases |
| **Sets itself up** | `onboard` plans the setup for an agent; `doctor` checks it; skills for Claude Code and Codex |
| **Stays safe** | read-only unless you pass a write flag; no telemetry; the host decides who sees what (`Scope`) |

## What the experiments show

Every default is backed by a measurement with real agents. Details and data: [`spikes/README.md`](spikes/README.md).

| Question | Answer | Spike |
|---|---|---|
| Embeddings needed for a large bundle? | **No**: lexical tools 90–100% from 60k to 4.4M tokens | S4 |
| Accuracy on a 1M-token bundle | Claude 28/30, Codex 26/30 | v0.1, S7 |
| Small models? | 21/30: use a strong model for answering | S7 |
| Spreadsheets of ~10k rows | 10/10 with `data_query`, 9× cheaper (without it, agents give up) | S5 |
| Asking in another language | R@1 0.857 with EmbeddingGemma, 0.917 after fine-tuning | S1, S11 |
| Agent sets okbase up alone | Claude 3/3, Codex 12/12; never added a consent flag on its own | S13, S16 |
| Scanned PDF pages | 1–3 ms to hand over the image (rendering: 0.2–0.95 s) | S14 |

## Builds

| Build | Adds | Size |
|---|---|---|
| `okbase` | everything above (lexical search) | ~34 MB |
| `okbase-full` | local semantic search, [fine-tuning on your bundle](docs/usage.md#fine-tuning-on-your-bundle-cross-language--100-documents) | ~60 MB |

Models and language dictionaries are downloaded on first use, never bundled. What leaves your
machine and when: [privacy](docs/usage.md#privacy-what-leaves-the-machine).

## License

MIT OR Apache-2.0, at your option. Third-party material: [`THIRD_PARTY.md`](THIRD_PARTY.md).
