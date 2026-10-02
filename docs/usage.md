# Using okfkit: small, medium and large projects

okfkit makes a folder of markdown knowledge (OKF bundle, docs site, wiki, Obsidian vault)
easy for AI agents to use. How much machinery you need depends on the size of the folder,
the languages involved and who uses it. **`okfkit advise` measures your folder and tells you
which of the setups below fits**; this page explains each of them.

```sh
cd my-knowledge
okfkit advise                       # add --user-langs vi,ja if people ask in other languages
okfkit advise --for claude|codex|team|host --json   # the same, for scripts and agents
```

`advise` only reads. Every step it prints says why, citing the measurement behind it (the
spikes in `spikes/`), and lists the commands to run.

## The easy way: let your agent do it

Tell Claude Code or Codex: **"set up okfkit for this folder"** (with a link to this repository if
okfkit is not installed yet). The agent runs `okfkit onboard`, which plans every step below from
the real state of your machine and folder; it asks you only where the decision is yours
(licenses, downloads, editing documents, sending text to its model provider, deleting data), then
checks the result with `okfkit doctor`. `okfkit onboard --goal remove` plans the removal the same
way. What agents follow: the [README section for agents](../README.md#for-agents), `llms.txt`, and
`okfkit help --agent`.

The rest of this page describes the same steps for people.

## Install

Not yet published; build from this repository (Rust 1.88+):

```sh
cargo install --path crates/okfkit-cli                   # okfkit: lexical tools, ~23 MB
cargo install --path crates/okfkit-cli --features full   # okfkit-full: + embeddings, fine-tuning, ~96 MB
```

Start with the default build. You only need `okfkit-full` when `advise` recommends semantic
search or fine-tuning.

## Which setup?

| | Small | Medium | Large |
|---|---|---|---|
| Size (estimated tokens; `okfkit status`) | ≤ 30k (roughly up to 100 short docs) | 30k – 1M (hundreds of docs) | > 1M (thousands of docs) |
| How the agent reads | whole bundle in context | catalog in the prompt + tools | catalog + tools, optionally semantic search |
| Build | `okfkit` | `okfkit` (`okfkit-full` if cross-language) | `okfkit-full` recommended |
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
okfkit status                         # size, languages, level, recommended mode (Full)
okfkit lint --level L1                # fix errors: missing titles, broken links, invalid YAML
okfkit catalog                        # one line per document: put it in the system prompt
okfkit agent install                  # MCP tools + skills for Claude Code and/or Codex (whichever is installed)
```

- Point the agent at the folder ("read the files in `knowledge/`") or paste the documents into
  the prompt; the catalog tells it what exists.
- Plain markdown that is not OKF yet? `okfkit adopt ./docs --out ./docs-okf` adds the frontmatter
  and `index.md` without touching the source.
- No index, no models, no configuration needed. okfkit keeps its index in `.okfkit/` (or the
  user cache when the folder is read-only) and never edits your files unless you pass a write flag.

**Move on when** `okfkit status` reports mode `Lexical` (the bundle passed ~30k tokens).

---

## Medium projects (30k – 1M tokens)

**Setup: catalog in the system prompt plus the okfkit tools.** Lexical tools answered 90–100% of
questions on bundles up to 4.4M tokens in the okf-scale spike, with no model at all.

### 1. Connect the agent

```sh
okfkit agent install                  # every agent found (claude/codex on PATH, ~/.claude, ~/.codex)
okfkit agent install --claude         # only Claude Code: .mcp.json + .claude/skills in this project
okfkit agent install --codex          # only Codex: ~/.codex/config.toml + an AGENTS.md block
okfkit agent install --print          # show the changes first
```

Why one command per agent product: each keeps its configuration in a different place and
format, and okfkit writes the right one. See [Usage scenarios](#usage-scenarios) for several
bundles, several projects, a shared server, and removing everything again.

The agent gets these tools (also available on the CLI with `--json`):

| Tool | Use it for | CLI |
|---|---|---|
| `kb_catalog` | what exists (keep it in the system prompt: without it, costs were ×2.5 in S3) | `okfkit catalog` |
| `kb_grep` | exact terms, codes, synonyms in any language (`refund\|đổi trả`) | `okfkit grep 'refund\|đổi trả' -l` |
| `kb_query` | lists, counts, filters, sums over frontmatter | `okfkit query --type Contract --facet region` |
| `kb_get` / `kb_list` | read a document, a section or a line range; browse folders | `okfkit get policies/refunds -s "Exceptions"` |
| `kb_links` | links and backlinks | `okfkit links metrics/revenue` |
| `data_tables` / `data_query` | read-only SQL over spreadsheets | `okfkit data sql "SELECT region, SUM(revenue) FROM sales_2026 GROUP BY 1"` |

The `okfkit-answer` skill teaches the agent when to use which tool (lists → `kb_query`,
numbers → `data_query`, exact terms → `kb_grep`, cite the document ids).

### 2. Organize the bundle (the biggest lever)

Agents pick documents by their `description` and `index.md`; a well-organized bundle matters more
than any model (PLAN §1). `advise` lists this step when the level is below L2, more than 10% of
documents lack a description, or there is no `index.md`.

```sh
okfkit lint --level L2                 # what to fix, in order of impact
okfkit lint --level L2 --fix-safe      # create missing index.md files (writes)
okfkit vocab --suggest                 # a tag vocabulary from the tags in use
okfkit vocab --suggest --write         # save it as _meta/vocabulary.md
```

Ask your agent to "curate the bundle" (skill `okfkit-curate`): it rewrites weak descriptions,
fills tags from the vocabulary and merges duplicate pages.

### 3. If people ask in other languages: semantic search

```sh
okfkit advise --user-langs vi,ja       # recommends this as a [next] step when it applies
okfkit embed enable --accept-license   # EmbeddingGemma 300M Q4 (Gemma terms; okfkit-full)
okfkit embed enable --model bge-m3-int8   # or MIT-licensed, weaker on cross-language (S1 0.78 vs 0.85)
okfkit embed index                     # ~3 chunks/s on 8 CPUs; cached, only changes later
okfkit search "chính sách đổi trả"     # the agent gets kb_search
```

Models download once into the user cache; nothing is bundled, nothing leaves the machine.

---

## Large projects (> 1M tokens), teams and applications

Everything above, plus:

### Semantic search for fewer turns

Above ~1M tokens lexical search stays accurate, but embeddings roughly halve the agent's turns
(~2 vs ~4.5 in S4) and let an application pre-retrieve context. `advise` marks this step `maybe`
for single-language bundles and `next` for cross-language ones.

```sh
okfkit embed enable --accept-license && okfkit embed index
okfkit retrieve "how do I rotate the API key?" --budget 3000   # best sections within a token budget
```

### Fine-tuning on your bundle (cross-language, ≥ ~100 documents)

When people mostly ask in another language than the documents, a model tuned on the bundle
helped most in spike S11 (R@1 0.857 → 0.917, cross-language 0.815 → 0.92, as ONNX Q4). okfkit
never calls an LLM: your agent writes the questions, okfkit checks them against a fixed standard,
trains, and switches models only if the tuned one measurably wins.

Ask your agent: **"tune embeddings for this bundle"** (skill `okfkit-tune`). The steps it follows:

```sh
okfkit embed tune init --langs vi,ja,en     # sample passages, hold out 15% of documents
okfkit embed tune setup --yes               # private Python environment (~1.7 GB), once
okfkit embed tune next                      # a batch; the agent answers with: tune submit <n> -
okfkit embed tune check                     # ≥ 300 training pairs required
okfkit embed tune train                     # ~10–25 min on a CPU; --backend colab for a free GPU
okfkit embed tune export                    # ONNX Q4, installed as custom:<bundle>-<run>
okfkit embed tune eval                      # base vs tuned + quality gate
okfkit embed tune activate --write          # undo: okfkit embed tune rollback --write
```

Before starting, decide whether passages may be sent to your agent's model provider (they are,
while it writes questions). Add `_meta/eval/questions.jsonl` with ~50 questions written by people
for a trustworthy gate. You don't retrain after ordinary edits; start a new run when a new domain
or language becomes a large part of the bundle. Details: `okfkit embed tune guide`.

### Serving a team

```sh
export OKFKIT_MCP_TOKEN=$(openssl rand -hex 32)
okfkit mcp serve --http 0.0.0.0:7331 --allow-host kb.example.com
okfkit mcp serve --http --deny 'internal/**'          # hide part of the bundle
```

Off loopback a bearer token is required; clients send `Authorization: Bearer <token>`. Run one
server per audience, with `--allow`/`--deny` deciding what that audience can read. Each member
connects their agent without a local copy of the bundle:

```sh
export KB_TOKEN=…                                       # from the server's admin
okfkit agent install --url https://kb.example.com/mcp --token-env KB_TOKEN
```

The token stays in the environment variable; the agent configuration only names it.

### Embedding okfkit in an application

The host decides who may read what; every read takes a `Scope` (see
`crates/okfkit/examples/host_usage.rs`):

```rust
use okfkit::{Bundle, CatalogOptions, GrepRequest, OpenOptions, Scope};

let bundle = Bundle::open(path, OpenOptions::default())?;   // once; cheap to clone, thread-safe
bundle.sync()?;                                             // incremental; call after changes
let scope = Scope::all().deny("internal/**")?;              // per user or tenant
let catalog = bundle.catalog(&CatalogOptions::default(), &scope)?;   // → system prompt
let hits = bundle.grep(&GrepRequest { pattern: "refund|đổi trả".into(), ..Default::default() }, &scope)?;
```

To expose the same tools over MCP inside your web server, mount
`okfkit_mcp::router(bundle, scope_provider, &ServerOptions, &HttpOptions)` (axum) and implement
`ScopeProvider` to map each request (its headers) to a `Scope`.

---

## Usage scenarios

Every situation below has one supported way to handle it. Install commands are safe to run
again (they converge on the same files), and `--print` shows any change before it is made.

### Agents and projects

| Scenario | Do this | Notes |
|---|---|---|
| One project, one bundle | `okfkit -b ./kb agent install` | Installs for every agent found; `--claude` / `--codex` to pick |
| Claude Code and Codex on the same project | `okfkit -b ./kb agent install` (or both flags) | One command, both agents |
| One bundle in every project (a personal or company knowledge base) | `okfkit -b ~/kb agent install --user` | Claude Code and Codex user configuration; skills and instructions in `~/.claude/skills`, `~/.codex/AGENTS.md` |
| Several bundles in one project (e.g. policies + API docs) | first: `okfkit -b ./policies agent install`; next: `okfkit -b ./docs agent install --name okfkit-docs` | Each bundle gets its own server, tools (`kb_*`, `docs_*`), skills (`okfkit-answer`, `okfkit-answer-docs`) and AGENTS.md block |
| Several projects, each with its own bundle | in each project: `okfkit -b <its bundle> agent install` | Each project has its own configuration (Codex: trust the project when asked). Only `--user` installs share one namespace: there, give each bundle its own `--name`; okfkit refuses to overwrite a name that serves another bundle |
| Point an existing name at another bundle | `okfkit -b ./new agent install --replace` | Explicit, never silent |
| A team on one shared server | admin: `okfkit mcp serve --http …`; members: `okfkit agent install --url https://kb.example.com/mcp --token-env KB_TOKEN` | Members need no copy of the bundle; only the answering skill is installed |
| An application (chatbot, internal tool) | the library (`okfkit::Bundle`) or `okfkit_mcp::router()` | The application decides each user's `Scope` |
| CI: keep the bundle healthy | `okfkit lint --level L2 --format sarif` | Exits 1 on errors; no agent needed |

### Where `agent install` writes, and how long the server runs

| | Default (this project only) | `--user` (every project) |
|---|---|---|
| Claude Code: MCP server | `<project>/.mcp.json` (can be committed; Claude Code asks once to approve it) | `claude mcp add --scope user` (`~/.claude.json`) |
| Claude Code: skills | `<project>/.claude/skills/` | `~/.claude/skills/` |
| Codex: MCP server | `<project>/.codex/config.toml`, read once the project is trusted in Codex (it asks on first run; okfkit does not trust it for you) | `~/.codex/config.toml` |
| Codex: instructions | `<project>/AGENTS.md` (an okfkit block) | `~/.codex/AGENTS.md` |

**Local server (stdio), the default: its lifetime follows the agent.** The agent starts
`okfkit --bundle <path> mcp serve --stdio` when a session starts and it exits when the session
ends; there is nothing to start or stop by hand.
- Several sessions or projects at once: one small process per session, each serving its own bundle.
  Sessions on the same bundle share its index safely (SQLite WAL).
- Documents edited during a session (by you or by the agent) are picked up on the next tool call
  (the index re-syncs when it is older than 2 s; search embeds up to 64 new chunks on the fly).
- With embeddings, each process loads the model on its first `kb_search` (~0.5 GB for
  EmbeddingGemma Q4): N parallel sessions use N times that memory. Many sessions on one large
  bundle are better served by one shared server.

**Shared server (HTTP): you run it.** `okfkit mcp serve --http …` is a long-running process: start it
by hand, or as a systemd / launchd service or a container; Ctrl-C (SIGINT) stops it gracefully.
One process serves every client, and the model is loaded once. Agents only connect to it
(`agent install --url`).

### Changes over time

| Scenario | Do this |
|---|---|
| Documents edited or added | nothing: indexes and vectors update incrementally on the next read |
| The bundle or the okfkit binary moved, or okfkit was upgraded to another path | `okfkit agent status` reports it (`FAIL … no longer exists`); run `okfkit agent install` again from the new place (`--replace` if asked) |
| Check what is installed where | `okfkit agent status` (`--json` for scripts) |
| Offline or air-gapped machine | `okfkit dict install` and `okfkit embed models pull <id>` while online; then `OKFKIT_OFFLINE=1` |
| The folder is read-only (a mounted share) | nothing: the index goes to the user cache automatically (`--state-dir cache` to force it) |

### Trying okfkit, then stopping

| Goal | Do this | What stays |
|---|---|---|
| Stop using it for one project | `okfkit agent uninstall` (add `--name` for a second bundle) | Removes only okfkit's MCP entry, skills and AGENTS.md block; files that held nothing else are deleted |
| Turn off semantic search only | `okfkit embed disable` | Vectors stay cached; `embed enable` brings them back instantly |
| Undo a fine-tuned model | `okfkit embed tune rollback --write` | The previous `okfkit.toml`, byte for byte |
| Free disk space | `okfkit clean` (shows sizes) → `okfkit clean --index --yes` / `--all --yes` | Documents are never touched; everything deleted is rebuilt or re-downloaded on demand |
| Remove okfkit completely | `okfkit agent uninstall --all`, `okfkit clean --all --yes`, then `cargo uninstall okfkit-cli` | Nothing; `~/.config/okfkit/installs.json` is empty and can be deleted |

---

## Reference

### Thresholds behind `advise`

| Rule | Value | Evidence |
|---|---|---|
| Whole bundle in context | ≤ 30,000 estimated tokens | okf-scale spike: full context best up to ~30k |
| A language counts | ≥ 5% of the bundle's tokens | — |
| Semantic search for size alone | > 1,000,000 tokens | S4: fewer turns, same accuracy |
| Semantic search for language | people's languages ≠ document languages, or a mixed bundle | S1: BM25 3.5% cross-language, embeddings 0.85 R@1 |
| Curate first | level < L2, > 10% without description, or no `index.md` | PLAN §1 |
| Fine-tuning | cross-language and ≥ 100 documents | S11 |

### What writes to disk

okfkit is read-only by default. These commands write, and only where stated:

| Command | Writes |
|---|---|
| any read command | its index in `<bundle>/.okfkit/` (or the user cache; `--state-dir` to choose) |
| `adopt --out DIR` / `adopt --write` | a new folder / the folder in place (clean git tree required) |
| `lint --fix-safe`, `vocab --suggest --write` | missing `index.md` files, `_meta/vocabulary.md` |
| `embed enable/disable`, `tune activate/rollback --write` | `okfkit.toml` (only the `[embed]` keys) |
| `agent install` / `agent uninstall` | agent configuration, skills, AGENTS.md block (`--print` to preview); a record in `~/.config/okfkit/installs.json` |
| `clean --yes` | deletes okfkit's own index or user cache (never documents) |
| `embed index`, `embed models add`, `tune …` | the user cache (models, vectors, runs, Python environment) |

### Everyday commands

```sh
okfkit status                     # summary
okfkit advise                     # what to do next
okfkit lint --level L2            # quality
okfkit grep 'pattern' -l          # find documents
okfkit query --tag x --facet type # filter and count
okfkit get <id> -s "Section"      # read
okfkit modules                    # which optional modules this build has
```
