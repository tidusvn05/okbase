# okbase

**Open Knowledge base: make markdown knowledge work well for AI agents.**

[![CI](https://github.com/tidusvn05/okbase/actions/workflows/ci.yml/badge.svg)](https://github.com/tidusvn05/okbase/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/tidusvn05/okbase?include_prereleases)](https://github.com/tidusvn05/okbase/releases)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)](#license)

Give Claude Code, Codex or any MCP client a folder of markdown, and get accurate, cited answers
in any language. okbase reads [Open Knowledge Format](https://github.com/GoogleCloudPlatform/open-knowledge-format)
bundles, docs sites, wikis and PDFs; it is written in Rust and is read-only by default.

> okbase is a community project, not affiliated with Google.

[See the benchmark tables](#what-the-experiments-show): answer accuracy, retrieval quality,
cost and agent setup, with links to the recorded results.

## Get started: paste this to your agent

Open your agent in the folder that holds your knowledge and paste:

```text
Set up okbase (https://github.com/tidusvn05/okbase) so you can answer questions from the
knowledge in this folder.

1. If `okbase --version` fails, install it:
   curl -fsSL https://raw.githubusercontent.com/tidusvn05/okbase/main/install.sh | sh
   (Windows: irm https://raw.githubusercontent.com/tidusvn05/okbase/main/install.ps1 | iex)
   If it finds no release (before 0.1.0), install from source instead (Rust 1.89+, a few minutes):
   cargo install --locked --git https://github.com/tidusvn05/okbase okbase-cli
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

## Start with a template

Try a complete fictional knowledge base before writing your first documents:

```sh
okbase templates
okbase -b ./project-kb init --template software-project
okbase -b ./project-kb get guides/getting-started
okbase -b ./project-kb onboard
```

Ten offline templates cover software projects, personal and project assistants, infrastructure
operations, product management, engineering leadership, executive work, team handbooks, customer
support and research. Each has five trial questions, three workflow guides and linked example
records. `init --print` previews all files; after creation, replace examples and extend ordinary
Markdown, type schemas and tags. [Choose and customize a template](docs/templates.md).

## Install it yourself

| Platform | Command |
|---|---|
| Linux, macOS | `curl -fsSL https://raw.githubusercontent.com/tidusvn05/okbase/main/install.sh \| sh` |
| … with semantic search | add `-s -- --full` |
| Windows | `irm https://raw.githubusercontent.com/tidusvn05/okbase/main/install.ps1 \| iex` |
| crates.io (Rust 1.89+) | `cargo install --locked okbase-cli` |
| From Git (Rust 1.89+) | `cargo install --locked --git https://github.com/tidusvn05/okbase okbase-cli` |

Each binary download is checked against the release's `SHA256SUMS`. More: [install, update, remove](docs/usage.md#install).

### Install with Cargo

With Rust 1.89 or newer, install the CLI from [crates.io](https://crates.io/crates/okbase-cli):

```sh
cargo install --locked okbase-cli
okbase --version
```

The package is `okbase-cli`; the installed command is `okbase`. For semantic search and
fine-tuning, install with the optional `full` features:

```sh
cargo install --locked okbase-cli --features full
```

To update, rerun the same install command, keeping `--features full` if you use it.
For Rust applications, add the [library](https://crates.io/crates/okbase):

```sh
cargo add okbase
```

## What it does

| | |
|---|---|
| **Reads** | OKF bundles, plain markdown (docs sites, wikis, Obsidian), PDF, Word, PowerPoint, HTML; scans are handed to the agent to transcribe |
| **Agent tools** | MCP and CLI: `kb_grep`, `kb_query` (metadata, counts), `kb_get`, `kb_catalog`, `data_query` (SQL over CSV/XLSX), optional `kb_search` |
| **Shows it** | `okbase view`: a local web viewer with the link graph, rendered documents, search with facets, and the lint report |
| **Organizes** | `adopt` plain markdown into OKF, `lint` levels L0–L3, `init` and `new` for new knowledge bases |
| **Sets itself up** | `onboard` plans the setup for an agent; `doctor` checks it; skills for Claude Code and Codex |
| **Stays safe** | read-only unless you pass a write flag; no telemetry; the host decides who sees what (`Scope`) |

## What the experiments show

The tables below summarize recorded runs from **2026-09-30 to 2026-10-03**. The experiments
compare approaches; acceptance runs check the actual okbase implementation. Each report includes
the setup, scoring, misses and scripts or commands to reproduce it.

**Answer accuracy as the knowledge base grows.** S4 used OpenClaw documentation and 30 questions
in Vietnamese, English and Japanese per configuration, with Claude Sonnet answering. A correct
answer had to include every required fact.

| Bundle size | Lexical tools, no embeddings (G2) | Embedding-assisted tools (D) | Agent + Read/Grep/Glob (E) |
|---|---|---|---|
| 60k tokens | **100%** | 97% | 100% |
| 150k tokens | **100%** | 100% | 100% |
| 1M tokens | **93%** | 100% | 97% |
| 4.4M tokens | **90%** | 93% | 93% |

This supports the lexical default on these bundles. With 30 questions, one answer changes the
score by 3.3 percentage points. [Full comparison, costs and limitations (S4)](spikes/okf-scale/RESULTS.md#round-2--upgrading-kb_grep-and-a-verification-rule).

**Answering through okbase's MCP server.** These runs exercise the shipped tools, including
metadata queries and SQL over sheets.

| Test | Scope | Recorded result | Report |
|---|---|---|---|
| Lexical answers, Claude Sonnet | ~1M tokens; 30 vi/en/ja questions | **28/30 (93%)**; mean $0.029/question | [v0.1 acceptance](spikes/acceptance-v0.1/RESULTS.md) |
| Lexical answers, Codex gpt-6.1-sol | Same 30 questions | **26/30 (87%)**; adding semantic search also scored 26/30 | [S7](spikes/codex/RESULTS.md#s7--lexical-okbase-with-codex-and-a-small-model-s7py) |
| Lexical answers, Codex gpt-6-luna | Same 30 questions | **21/30 (70%)**; adding semantic search also scored 21/30 | [S7](spikes/codex/RESULTS.md#s7--lexical-okbase-with-codex-and-a-small-model-s7py) |
| Metadata, content and sheets, Claude Sonnet | 3,020 documents; sheets of ~10k rows; 48 questions | **46/48** overall, **10/10** sheet questions; mean $0.044/question using server instructions only | [S15](spikes/mcp-tools/RESULTS.md#results-resultstablemd) |

**When query tools help.** S5 compared three approaches on the same synthetic business bundle:
**3,020 documents**, sheets of up to ~10k rows, and 48 questions answered by Claude Sonnet.
These runs used the experimental server, before okbase's acceptance runs above.

| Approach | Correct answers | Sheet questions | Mean cost/question | Median time |
|---|---|---|---|---|
| Directory + Read/Grep/Glob | 92% | 9/10 | $0.056 | 10.3 s |
| Directory + generated metadata views | 94% | 8/10 | $0.070 | 9.6 s |
| Directory + metadata queries + SQL | **96%** | **10/10** | **$0.032** | **7.3 s** |

On sheet questions alone, mean cost fell from **$0.105 to $0.012** with query tools.
[Results by question category, including the smaller 151-document bundle (S5)](spikes/biz-meta/RESULTS.md#results).

**Retrieval and agent setup.** Retrieval scores measure whether the right document was found;
they are separate from answer accuracy.

| Test | Scope | Recorded result | Report |
|---|---|---|---|
| Semantic retrieval | 300 vi/en/ja questions, 100 documents | EmbeddingGemma Q4 **R@1 0.857**; bge-m3 int8 **0.750** (R@1: correct document ranked first) | [v0.3 acceptance](spikes/acceptance-v0.3/RESULTS.md) |
| Retrieval on long documentation | 30 questions, 287 documents | Correct document in top 6: Gemma Q4 **29/30**, bge-m3 int8 **30/30**; Gemma missed one question | [v0.3 acceptance](spikes/acceptance-v0.3/RESULTS.md) |
| Fine-tuning through the CLI | 320 training pairs; 80 held-out questions | R@1 **0.875 → 0.925**; cross-language R@1 stayed at **0.810** | [S11 workflow acceptance](spikes/embed-tune/RESULTS.md#acceptance-the-whole-workflow-with-real-agents-2026-10-02) |
| Claude setup from one sentence | Small, large and multilingual folders | **3/3** ended with `doctor` OK; **0** unapproved consent flags | [S13](spikes/onboarding/RESULTS.md#results) |
| Codex setup from one sentence | 6 folder scenarios × 2 models | **12/12** found `onboard`; **11/12** ended with `doctor` OK; **0** unapproved consent flags; sandbox registration needed a fix | [S13 Codex, including fix and rerun](spikes/codex/RESULTS.md#s13-with-codex--setup-from-one-sentence-s13py) |

These are small, recorded evaluations on the versions named in each report. Questions were
LLM-generated; the business bundle and multilingual retrieval data are synthetic. They establish
what was tested, rather than guaranteeing the same scores on every knowledge base.

More measurements: [embedding models, RAM and latency](spikes/embed-bench/RESULTS.md),
[PDF/Office/HTML import](spikes/import-bench/RESULTS.md),
[scanned-page image extraction](spikes/page-image-bench/RESULTS.md).
The [experiment index](spikes/README.md) links all reports and lists work still to be tested.

## Builds

| Build | Adds | Size |
|---|---|---|
| `okbase` | everything above (lexical search) | ~34 MB |
| `okbase-full` | local semantic search, [fine-tuning on your bundle](docs/usage.md#fine-tuning-on-your-bundle-cross-language--100-documents) | ~60 MB |

Models and language dictionaries are downloaded on first use, never bundled. What leaves your
machine and when: [privacy](docs/usage.md#privacy-what-leaves-the-machine).

## License

MIT OR Apache-2.0, at your option. Third-party material: [`THIRD_PARTY.md`](THIRD_PARTY.md).
