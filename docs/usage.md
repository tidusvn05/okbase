# Using okbase: small, medium and large projects

okbase makes a folder of markdown knowledge (OKF bundle, docs site, wiki, Obsidian vault)
easy for AI agents to use. How much machinery you need depends on the size of the folder,
the languages involved and who uses it. **`okbase advise` measures your folder and tells you
which of the setups below fits**; this page explains each of them.

```sh
cd my-knowledge
okbase advise                       # add --user-langs vi,ja if people ask in other languages
okbase advise --for claude|codex|team|host --json   # the same, for scripts and agents
```

`advise` only reads. Every step it prints says why, citing the measurement behind it (the
spikes in `spikes/`), and lists the commands to run.

## The easy way: let your agent do it

Paste the prompt from the README ([Get started](../README.md#get-started-paste-this-to-your-agent))
into Claude Code, Codex or another agent with a shell, in the folder that holds your knowledge.
The agent installs okbase, runs `okbase onboard`, which plans every step below from the real
state of your machine and folder, and asks you only where the decision is yours (licenses,
downloads, editing documents, sending text to a service, deleting data). It checks the result
with `okbase doctor` and ends with what is set up, how to use it, and what okbase recommends for
later. The same prompt with another first sentence starts a new knowledge base, improves one
(`--goal curate`) or removes okbase (`--goal remove`). What agents follow: `llms.txt` and
`okbase help --agent`.

The rest of this page describes the same steps for people.

## Install

```sh
curl -fsSL https://raw.githubusercontent.com/tidusvn05/okbase/main/install.sh | sh                # okbase: lexical tools, ~34 MB
curl -fsSL https://raw.githubusercontent.com/tidusvn05/okbase/main/install.sh | sh -s -- --full  # okbase-full: + embeddings, fine-tuning, ~60 MB
```

On Windows: `irm https://raw.githubusercontent.com/tidusvn05/okbase/main/install.ps1 | iex`
(`$env:OKBASE_FULL=1` first for okbase-full). The installers verify each download against the
release's `SHA256SUMS` and write only the `okbase` binary (`~/.local/bin`, or
`%LOCALAPPDATA%\okbase\bin` on Windows). From source, with Rust 1.89 or newer (the only way
before the first release, when the installers report "no release found"):
`cargo install --locked --git https://github.com/tidusvn05/okbase okbase-cli [--features full]`,
or `cargo install --locked --path crates/okbase-cli` in a clone.

To update, run the installer again. To remove okbase, see
[Trying okbase, then stopping](#trying-okbase-then-stopping).

Start with the default build. You only need `okbase-full` when `advise` recommends semantic
search or fine-tuning. Neither build embeds language dictionaries or models: they are downloaded
on first use (see [Privacy](#privacy-what-leaves-the-machine)).

Platforms: releases are built for Linux (x86_64, arm64; glibc 2.35+), macOS (Intel, Apple
silicon) and Windows (x86_64); see [docs/releasing.md](releasing.md).

## Which setup?

| | Small | Medium | Large |
|---|---|---|---|
| Size (estimated tokens; `okbase status`) | ≤ 30k (roughly up to 100 short docs) | 30k – 1M (hundreds of docs) | > 1M (thousands of docs) |
| How the agent reads | whole bundle in context | catalog in the prompt + tools | catalog + tools, optionally semantic search |
| Build | `okbase` | `okbase` (`okbase-full` if cross-language) | `okbase-full` recommended |
| Typical users | one person, one agent | a person or a small team | a team or an application |

Two things move you up a column regardless of size:
- **Cross-language questions** (people ask in Vietnamese, documents are in English…): lexical
  search cannot match across languages (3.5% in spike S1), so add semantic search (Medium+).
- **Spreadsheets** (CSV, TSV, XLSX): the SQL tools turn on automatically at every size.

The steps below are cumulative: a medium project also does what a small one does.

---

## Small projects (≤ ~30k tokens)

**Setup: put the whole bundle in the agent's context.** In the okf-scale spike this was the
fastest and cheapest way to answer correctly, up to about 30k tokens.

```sh
okbase status                         # size, languages, level, recommended mode (Full)
okbase lint --level L1                # fix errors: missing titles, broken links, invalid YAML
okbase catalog                        # one line per document: put it in the system prompt
okbase agent install                  # MCP tools + skills for Claude Code and/or Codex (whichever is installed)
```

- Point the agent at the folder ("read the files in `knowledge/`") or paste the documents into
  the prompt; the catalog tells it what exists.
- Plain markdown that is not OKF yet? `okbase adopt ./docs --out ./docs-okf` adds the frontmatter
  and `index.md` without touching the source.
- No index, no models, no configuration needed. okbase keeps its index in `.okbase/` (or the
  user cache when the folder is read-only) and never edits your files unless you pass a write flag.

**Move on when** `okbase status` reports mode `Lexical` (the bundle passed ~30k tokens).

---

## Medium projects (30k – 1M tokens)

**Setup: catalog in the system prompt plus the okbase tools.** Lexical tools answered 90–100% of
questions on bundles up to 4.4M tokens in the okf-scale spike, with no model at all.

### 1. Connect the agent

```sh
okbase agent install                  # every agent found (claude/codex on PATH, ~/.claude, ~/.codex)
okbase agent install --claude         # only Claude Code: .mcp.json + .claude/skills in this project
okbase agent install --codex          # only Codex: ~/.codex/config.toml + an AGENTS.md block
okbase agent install --print          # show the changes first
```

Why one command per agent product: each keeps its configuration in a different place and
format, and okbase writes the right one. See [Usage scenarios](#usage-scenarios) for several
bundles, several projects, a shared server, and removing everything again.

The agent gets these tools (also available on the CLI with `--json`):

| Tool | Use it for | CLI |
|---|---|---|
| `kb_catalog` | what exists (keep it in the system prompt: without it, costs were ×2.5 in S3) | `okbase catalog` |
| `kb_grep` | exact terms, codes, synonyms in any language (`refund\|đổi trả`) | `okbase grep 'refund\|đổi trả' -l` |
| `kb_query` | lists, counts, filters, sums over frontmatter | `okbase query --type Contract --facet region` |
| `kb_get` / `kb_list` | read a document, a section or a line range; browse folders | `okbase get policies/refunds -s "Exceptions"` |
| `kb_links` | links and backlinks | `okbase links metrics/revenue` |
| `data_tables` / `data_query` | read-only SQL over spreadsheets | `okbase data sql "SELECT region, SUM(revenue) FROM sales_2026 GROUP BY 1"` |

The `okbase-answer` skill teaches the agent when to use which tool (lists → `kb_query`,
numbers → `data_query`, exact terms → `kb_grep`, cite the document ids).

### 2. Organize the bundle (the biggest lever)

Agents pick documents by their `description` and `index.md`; a well-organized bundle matters more
than any model (design §1). `advise` lists this step when the level is below L2, more than 10% of
documents lack a description, or there is no `index.md`.

```sh
okbase lint --level L2                 # what to fix, in order of impact
okbase lint --level L2 --fix-safe      # create missing index.md files (writes)
okbase vocab --suggest                 # a tag vocabulary from the tags in use
okbase vocab --suggest --write         # save it as _meta/vocabulary.md
```

Ask your agent to "curate the bundle" (skill `okbase-curate`): it rewrites weak descriptions,
fills tags from the vocabulary and merges duplicate pages.

### 3. If people ask in other languages: semantic search

```sh
okbase advise --user-langs vi,ja       # recommends this as a [next] step when it applies
okbase embed enable --accept-license   # EmbeddingGemma 300M Q4 (Gemma terms; okbase-full)
okbase embed enable --model bge-m3-int8   # or MIT-licensed, weaker on cross-language (S1 0.78 vs 0.85)
okbase embed index                     # ~3 chunks/s on 8 CPUs; cached, only changes later
okbase search "chính sách đổi trả"     # the agent gets kb_search
```

Local models download once into the user cache (nothing is bundled), and your documents stay on
the machine. An OpenAI-compatible API (`embed enable --api-url … --send-documents`) sends every
chunk and query to that service instead; see [Privacy](#privacy-what-leaves-the-machine).

---

## Large projects (> 1M tokens), teams and applications

Everything above, plus:

### Semantic search for fewer turns

Above ~1M tokens lexical search stays accurate, but embeddings roughly halve the agent's turns
(~2 vs ~4.5 in S4) and let an application pre-retrieve context. `advise` marks this step `maybe`
for single-language bundles and `next` for cross-language ones.

```sh
okbase embed enable --accept-license && okbase embed index
okbase retrieve "how do I rotate the API key?" --budget 3000   # best sections within a token budget
```

### Fine-tuning on your bundle (cross-language, ≥ ~100 documents)

When people mostly ask in another language than the documents, a model tuned on the bundle
helped most in spike S11 (R@1 0.857 → 0.917, cross-language 0.815 → 0.92, as ONNX Q4). okbase
never calls an LLM: your agent writes the questions, okbase checks them against a fixed standard,
trains, and switches models only if the tuned one measurably wins.

Ask your agent: **"tune embeddings for this bundle"** (skill `okbase-tune`). The steps it follows:

```sh
okbase embed tune init --langs vi,ja,en     # sample passages, hold out 15% of documents
okbase embed tune setup --yes               # private Python environment (~1.7 GB), once
okbase embed tune next                      # a batch; the agent answers with: tune submit <n> -
okbase embed tune check                     # ≥ 300 training pairs required
okbase embed tune train                     # ~10–25 min on a CPU; --backend colab for a free GPU
okbase embed tune export                    # ONNX Q4, installed as custom:<bundle>-<run>
okbase embed tune eval                      # base vs tuned + quality gate
okbase embed tune activate --write          # undo: okbase embed tune rollback --write
```

Before starting, decide whether passages may be sent to your agent's model provider (they are,
while it writes questions). Add `_meta/eval/questions.jsonl` with ~50 questions written by people
for a trustworthy gate. You don't retrain after ordinary edits; start a new run when a new domain
or language becomes a large part of the bundle. Details: `okbase embed tune guide`.

### Serving a team

```sh
export OKBASE_MCP_TOKEN=$(openssl rand -hex 32)
okbase mcp serve --http 0.0.0.0:7331 --allow-host kb.example.com
okbase mcp serve --http --deny 'internal/**'          # hide part of the bundle
```

Off loopback a bearer token is required; clients send `Authorization: Bearer <token>`. Run one
server per audience, with `--allow`/`--deny` deciding what that audience can read. Each member
connects their agent without a local copy of the bundle:

```sh
export KB_TOKEN=…                                       # from the server's admin
okbase agent install --url https://kb.example.com/mcp --token-env KB_TOKEN
```

The token stays in the environment variable; the agent configuration only names it.

### Embedding okbase in an application

The host decides who may read what; every read takes a `Scope` (see
`crates/okbase/examples/host_usage.rs`):

```rust
use okbase::{Bundle, CatalogOptions, GrepRequest, OpenOptions, Scope};

let bundle = Bundle::open(path, OpenOptions::default())?;   // once; cheap to clone, thread-safe
bundle.sync()?;                                             // incremental; call after changes
let scope = Scope::all().deny("internal/**")?;              // per user or tenant
let catalog = bundle.catalog(&CatalogOptions::default(), &scope)?;   // → system prompt
let hits = bundle.grep(&GrepRequest { pattern: "refund|đổi trả".into(), ..Default::default() }, &scope)?;
```

To expose the same tools over MCP inside your web server, mount
`okbase_mcp::router(bundle, scope_provider, &ServerOptions, &HttpOptions)` (axum) and implement
`ScopeProvider` to map each request (its headers) to a `Scope`.

---

## Real folders: where your knowledge actually is

`okbase scan` (and `okbase onboard`, which starts with it) looks at the folder before advising:

```sh
okbase scan          # empty? software repository? docs site? OKF, partly OKF? PDFs?
```

| Your folder | What okbase does | You run (or your agent does) |
|---|---|---|
| **A software repository with `docs/`** (MkDocs, Docusaurus, Hugo, Mintlify or plain) | Uses `docs/` as the bundle, not the repository; keeps your layout; under a docs site, `index.md` stays your page (profile `docs-site`) | from the repository root: `okbase -b docs agent install`; optionally `okbase -b docs adopt --write` to add `title`/`description` in place (review the PR) |
| **An empty folder** | Starts a knowledge base | `okbase init --title "…" --langs vi,en`, then `okbase new --type Policy "…" --description "…"` (or the `okbase-author` skill) |
| **OKF with a few broken files** | Lists the files to fix; nothing else changes | `okbase adopt --only a.md --only b.md --write`, then `okbase lint --level L1` for what needs a hand fix (invalid YAML is reported with its line) |
| **A repository whose `knowledge/` feeds a bot** | Uses `knowledge/`; the rest of the repository is ignored | developers: `okbase -b knowledge agent install`; the bot: see below; CI: see below |
| **An Obsidian vault** | Profile `vault`: notes stay as they are, no `index.md` files are created | `okbase agent install`; `adopt --write` only adds frontmatter |
| **PDF / Word / PowerPoint / HTML** (alone or next to markdown) | Reads them **directly**: searchable as they are, nothing written; pages without text (scans, images) are listed | nothing to do; `okbase import ocr-next` lets an agent transcribe scans; `okbase import --write` makes markdown copies to edit |

Several candidates (a monorepo with many `docs/` folders) → `onboard` asks which one; install
each with its own `--name`.

### Source documents (PDF, Office, HTML)

okbase converts PDF (page by page), Word (`.docx`, `.doc`), PowerPoint, OpenDocument, RTF, EPUB,
HTML, `.txt`/`.rst`/`.adoc` and images while indexing, with
[anydoc](https://github.com/firecrawl/anydoc), pdf-inspector and htmd: no models, no services,
nothing written to your folder. Spreadsheets (`.csv`, `.xlsx`, `.xls`, `.ods`…) are tables for SQL
instead (`okbase data tables`).

```sh
okbase import                  # what each file becomes; pages without text; files that cannot be read
okbase import ocr-next         # a page without text, for your agent to read and transcribe
okbase import ocr-submit manual.pdf --page 3 -   # the transcription (stored in okbase's state, not your folder)
okbase import --write          # markdown copies in sources/ to edit or curate (re-runs keep your edits)
okbase new --type Policy "Returns" --description "…" --source manual.pdf   # a clean document distilled from a source
```

Ids keep the extension (`manuals/printer.pdf`) and PDF pages are marked `<!-- page N -->`, so answers
can cite the file and the page. Multi-column PDF pages are listed because their reading order may
be wrong. Scanned pages are never sent to an OCR service; a multimodal agent transcribes them only
if you agree.

### Profiles

| Profile | `index.md` is | Chosen when |
|---|---|---|
| `okf` | a directory listing (OKF) | default; always when the root `index.md` declares `okf_version` |
| `docs-site` | a content page; listings are built when reading | `mkdocs.yml`, `docusaurus.config.*`, `hugo.toml`, `mint.json`/`docs.json` in the folder or a parent up to the repository root |
| `vault` | a content page; no listing files | `.obsidian/` |

Override in the bundle's `okbase.toml`:

```toml
[bundle]
profile = "docs-site"   # okf | docs-site | vault
```

### What okbase never reads (or edits)

Hidden files and folders, `AGENTS.md`/`CLAUDE.md`/`GEMINI.md` at the root (they are instructions
for agents), dependency folders (`node_modules`, `__pycache__`, `site-packages`…), and anything your
`.gitignore` ignores (including the repository's, above the bundle). Add `.okbaseignore` (same
syntax, `!` re-includes) for the rest:

```gitignore
drafts/
!drafts/approved.md
```

### Keep a bundle healthy in CI

```yaml
# .github/workflows/knowledge.yml
on: pull_request
jobs:
  lint:
    runs-on: ubuntu-latest
    permissions: { security-events: write }
    steps:
      - uses: actions/checkout@v4
      - run: cargo install --locked okbase-cli # after publication; until then build from a clone
      - run: okbase -b knowledge lint --level L1 --format sarif > okbase.sarif || test $? -eq 4
      - uses: github/codeql-action/upload-sarif@v3
        with: { sarif_file: okbase.sarif }
```

Exit code 4 means "lint found problems"; the SARIF upload shows them on the pull request.

### A bot that answers from the bundle

Keep the catalog in the bot's system prompt (S3) and give its model the okbase tools:

- **Rust:** the library (`okbase::Bundle`, a `Scope` per user), see
  [Embedding okbase in an application](#embedding-okbase-in-an-application).
- **Python, Node, others:** run `OKBASE_MCP_TOKEN=… okbase -b knowledge mcp serve --http 127.0.0.1:7331`
  next to the bot and connect with an MCP client library (MCP Python / TypeScript SDK) to
  `http://127.0.0.1:7331/mcp` with `Authorization: Bearer …`; or call the CLI with `--json`
  (`okbase -b knowledge grep "refund" --files-only --json`) for simple cases.
- The catalog for the prompt: `okbase -b knowledge catalog` (or the `kb_catalog` tool) at startup.

## Usage scenarios

Every situation below has one supported way to handle it. Install commands are safe to run
again (they converge on the same files), and `--print` shows any change before it is made.

### Agents and projects

| Scenario | Do this | Notes |
|---|---|---|
| One project, one bundle | `okbase -b ./kb agent install` | Installs for every agent found; `--claude` / `--codex` to pick |
| Claude Code and Codex on the same project | `okbase -b ./kb agent install` (or both flags) | One command, both agents |
| One bundle in every project (a personal or company knowledge base) | `okbase -b ~/kb agent install --user` | Claude Code and Codex user configuration; skills and instructions in `~/.claude/skills`, `~/.codex/AGENTS.md` |
| Several bundles in one project (e.g. policies + API docs) | first: `okbase -b ./policies agent install`; next: `okbase -b ./docs agent install --name okbase-docs` | Each bundle gets its own server, tools (`kb_*`, `docs_*`), skills (`okbase-answer`, `okbase-answer-docs`) and AGENTS.md block |
| Several projects, each with its own bundle | in each project: `okbase -b <its bundle> agent install` | Each project has its own configuration (Codex: trust the project when asked). Only `--user` installs share one namespace: there, give each bundle its own `--name`; okbase refuses to overwrite a name that serves another bundle |
| Point an existing name at another bundle | `okbase -b ./new agent install --replace` | Explicit, never silent |
| A team on one shared server | admin: `okbase mcp serve --http …`; members: `okbase agent install --url https://kb.example.com/mcp --token-env KB_TOKEN` | Members need no copy of the bundle; only the answering skill is installed |
| An application (chatbot, internal tool) | the library (`okbase::Bundle`) or `okbase_mcp::router()` | The application decides each user's `Scope` |
| CI: keep the bundle healthy | `okbase lint --level L2 --format sarif` | Exits 4 on errors; no agent needed |

### Where `agent install` writes, and how long the server runs

| | Default (this project only) | `--user` (every project) |
|---|---|---|
| Claude Code: MCP server | `<project>/.mcp.json` (Claude Code asks once to approve it; it holds absolute paths to the binary and the bundle, so do not commit it for other machines) | `claude mcp add --scope user` (`~/.claude.json`) |
| Claude Code: skills | `<project>/.claude/skills/` | `~/.claude/skills/` |
| Codex: MCP server | `<project>/.codex/config.toml`, read once the project is trusted in Codex (it asks on first run; okbase does not trust it for you) | `$CODEX_HOME/config.toml` (`~/.codex` by default) |
| Codex: instructions | `<project>/AGENTS.md` (an okbase block) | `$CODEX_HOME/AGENTS.md` |

Codex's sandbox keeps `.codex/` read-only, so a Codex agent cannot connect okbase to Codex itself:
`okbase onboard` then shows a step for you, and `okbase agent install --codex` stops with
`sandbox_blocked` before writing anything. Run `okbase agent install --codex` in your own terminal
(spike S13 with Codex).

**Local server (stdio), the default: its lifetime follows the agent.** The agent starts
`okbase --bundle <path> mcp serve --stdio` when a session starts and it exits when the session
ends; there is nothing to start or stop by hand.
- Several sessions or projects at once: one small process per session, each serving its own bundle.
  Sessions on the same bundle share its index safely (SQLite WAL).
- Documents edited during a session (by you or by the agent) are picked up on the next tool call
  (the index re-syncs when it is older than 2 s; search embeds up to 64 new chunks on the fly).
- With embeddings, each process loads the model on its first `kb_search` (~0.5 GB for
  EmbeddingGemma Q4): N parallel sessions use N times that memory. Many sessions on one large
  bundle are better served by one shared server.

**Shared server (HTTP): you run it.** `okbase mcp serve --http …` is a long-running process: start it
by hand, or as a systemd / launchd service or a container; Ctrl-C (SIGINT) stops it gracefully.
One process serves every client, and the model is loaded once. Agents only connect to it
(`agent install --url`).

### Changes over time

| Scenario | Do this |
|---|---|
| Documents edited or added | nothing: indexes and vectors update incrementally on the next read |
| The bundle or the okbase binary moved, or okbase was upgraded to another path | `okbase agent status` reports it (`FAIL … no longer exists`); run `okbase agent install` again from the new place (`--replace` if asked) |
| Check what is installed where | `okbase agent status` (`--json` for scripts) |
| Offline or air-gapped machine | `okbase dict install` and `okbase embed models pull <id>` while online; then `OKBASE_OFFLINE=1` |
| The folder is read-only (a mounted share) | nothing: the index goes to the user cache automatically (`--state-dir cache` to force it) |

### Trying okbase, then stopping

| Goal | Do this | What stays |
|---|---|---|
| Stop using it for one project | `okbase agent uninstall` (add `--name` for a second bundle) | Removes only okbase's MCP entry, skills and AGENTS.md block; files that held nothing else are deleted |
| Turn off semantic search only | `okbase embed disable` | Vectors stay cached; `embed enable` brings them back instantly |
| Undo a fine-tuned model | `okbase embed tune rollback --write` | The previous `okbase.toml`, byte for byte |
| Free disk space | `okbase clean` (shows sizes) → `okbase clean --index --yes` / `--all --yes` | Documents are never touched; everything deleted is rebuilt or re-downloaded on demand |
| Remove okbase completely | `okbase agent uninstall --all`, `okbase clean --all --yes`, then delete the `okbase` binary (`~/.local/bin/okbase`, or `cargo uninstall okbase-cli` for a source install) | Nothing; `~/.config/okbase/installs.json` is empty and can be deleted |

---

## Privacy: what leaves the machine

okbase has no telemetry and never phones home. These are the only network uses, each one visible:

| What | When | Sent to / downloaded from | To avoid it |
|---|---|---|---|
| Japanese dictionary (IPADIC, about 13 MB) | the first time Japanese text is indexed; okbase prints a notice | download from `Lindera.dev` into the user cache | `OKBASE_OFFLINE=1` (Japanese falls back to character bigrams), or `okbase dict install` on a machine with network and copy the cache |
| Local embedding models | `okbase embed index` after `embed enable` | download from Hugging Face into the user cache | stay lexical (the default) |
| Embeddings API | only after `embed enable --api-url … --send-documents` | **every document chunk and search query** goes to that service | use a local model |
| Training environment | `okbase embed tune setup --yes` | packages from PyPI and the PyTorch index | do not fine-tune |
| ONNX Runtime | when building `okbase-full` | the `ort` crate downloads its binaries | use the default build |

What your agent reads through okbase's tools (search results, documents, page images for OCR,
passages for tune questions) goes to the agent's model provider, as with any file the agent opens.
That is why `onboard` asks you before OCR and before writing tune questions.

---

## Troubleshooting

| Symptom | Cause and fix |
|---|---|
| The agent does not see the `kb_*` tools | Restart the agent session after `agent install`; then `okbase doctor` (it starts the MCP server for real) |
| Codex does not load the server | Trust the project when Codex asks (project config is read only for trusted projects), or install with `--user` |
| `sandbox_blocked` (exit 3) | Codex's sandbox keeps `.codex/` read-only: run the command in `next` in your own terminal |
| `not_built` | The feature needs `okbase-full`: `cargo install --locked --path crates/okbase-cli --features full` in a clone |
| `license_required` (exit 3) | Accept the model's license with `--accept-license`, or use `bge-m3-int8` (MIT) |
| `name_conflict` (exit 3) | That server name serves another bundle: pass `--name`, or `--replace` to repoint it |
| `doctor` reports a moved bundle or a missing binary | `okbase onboard` plans the repair (`agent install --replace`) |
| Japanese search is weaker offline | The dictionary is not installed yet: `okbase dict status`, then `okbase dict install` |
| A PDF page has no text | It is a scan: `okbase import ocr-next` hands its image to an agent that reads images |
| `lint` fails a CI job | It exits 4 when the bundle has errors; `okbase lint --level L1` is the usual first target |
| Odd results after an upgrade | `okbase clean --index --yes`; the next command rebuilds the index |
| Exit 141 after `okbase … \| head` | Normal: the pipe was closed early (128 + SIGPIPE) |

---

## FAQ

**Does okbase change my documents?** Not unless you pass a write flag; see
[What writes to disk](#what-writes-to-disk). `agent install` writes agent configuration only, and
`agent uninstall` removes exactly what it wrote.

**Do I need semantic search?** Usually not. Lexical tools reach about 93% on a 1M-token bundle with
Claude (S4) and 87% with Codex gpt-6.1-sol (S7). Add it when people ask in a language the documents
are not written in.

**Which agents work?** Claude Code and Codex are set up by `agent install`; any MCP client can run
`okbase mcp serve` (see [Other MCP clients](#other-mcp-clients)). For answering, use a strong model:
in S7 the small gpt-6-luna scored 70% against 87% for gpt-6.1-sol, and semantic search did not
close that gap.

**Several bundles in one project?** `okbase -b <other> agent install --name okbase-<short>`; each
gets its own tool prefix.

**Why did `AGENTS.md` change?** okbase adds one marked block (`okbase:begin` … `okbase:end`) with
usage notes for agents; `agent uninstall` removes only that block.

---

## Reference

### Other MCP clients

Any client that starts stdio servers can use okbase:

```json
{"mcpServers": {"okbase": {"command": "/path/to/okbase", "args": ["--bundle", "/path/to/bundle", "mcp", "serve", "--stdio"]}}}
```

`--prefix` changes the tool prefix (`kb`), `--disable <tool>` hides a tool, and `--allow`/`--deny`
limit which documents are served. Over HTTP: `okbase mcp serve --http` (loopback; a bearer token
from `OKBASE_MCP_TOKEN` is required on other addresses).

### Environment variables

| Variable | Meaning |
|---|---|
| `OKBASE_BUNDLE` | Default for `--bundle` |
| `OKBASE_STATE_DIR` | Default for `--state-dir` (`auto`, `cache` or a directory) |
| `OKBASE_OFFLINE` | `1`: never download (the dictionary falls back to bigrams) |
| `OKBASE_MODELS_DIR` | Where embedding models are stored |
| `OKBASE_EMB_CACHE` | The vector cache file |
| `OKBASE_DICT_DIR` | Where the Japanese dictionary is built |
| `OKBASE_CONFIG_DIR` | Where okbase keeps its install records (`installs.json`) |
| `OKBASE_MCP_TOKEN` | Bearer token for `mcp serve --http` (the variable name can be changed with `--token-env`) |
| `OKBASE_PYTHON` | The Python used to create the training environment |
| `CODEX_HOME` | Codex's directory for `--user` installs (`~/.codex` by default) |

### Thresholds behind `advise`

| Rule | Value | Evidence |
|---|---|---|
| Whole bundle in context | ≤ 30,000 estimated tokens | okf-scale spike: full context best up to ~30k |
| A language counts | ≥ 5% of the bundle's tokens | — |
| Semantic search for size alone | > 1,000,000 tokens | S4: fewer turns, same accuracy |
| Semantic search for language | people's languages ≠ document languages, or a mixed bundle | S1: BM25 3.5% cross-language, embeddings 0.85 R@1 |
| Curate first | level < L2, > 10% without description, or no `index.md` | S3–S5 (`spikes/README.md`) |
| Fine-tuning | cross-language and ≥ 100 documents | S11 |

### What writes to disk

okbase is read-only by default. These commands write, and only where stated:

| Command | Writes |
|---|---|
| any read command | its index in `<bundle>/.okbase/` (or the user cache; `--state-dir` to choose) |
| `adopt --out DIR` / `adopt --write` | a new folder / the folder in place (the files it changes must be committed in git first) |
| `lint --fix-safe`, `vocab --suggest --write` | missing `index.md` files, `_meta/vocabulary.md` |
| `embed enable/disable`, `tune activate/rollback --write` | `okbase.toml` (only the `[embed]` keys) |
| `agent install` / `agent uninstall` | agent configuration, skills, AGENTS.md block (`--print` to preview); a record in `~/.config/okbase/installs.json` |
| `clean --yes` | deletes okbase's own index or user cache (never documents) |
| `init`, `new` | a new bundle skeleton; one new document plus its folder listing (never overwrites) |
| `import --write` | markdown copies of source documents in `sources/` (never overwrites edited ones without `--force`) |
| `import ocr-submit` | a page transcription in okbase's state directory (not in the bundle) |
| `embed index`, `embed models add`, `tune …` | the user cache (models, vectors, runs, Python environment) |

### Everyday commands

```sh
okbase status                     # summary
okbase advise                     # what to do next
okbase lint --level L2            # quality
okbase grep 'pattern' -l          # find documents
okbase query --tag x --facet type # filter and count
okbase get <id> -s "Section"      # read
okbase modules                    # which optional modules this build has
```

Less frequent commands (each has `--help`):
- `okbase index`: update the index.
- `okbase dict status` / `dict install`: the Japanese dictionary.
- `okbase embed status` / `embed eval` / `embed models`: semantic search and its models.
- `okbase embed tune status` / `tune runs` / `tune import`: fine-tuning runs.
- `okbase agent status`: which agents are connected to which bundles.
- `okbase agent install --project DIR`: connect another project folder.
