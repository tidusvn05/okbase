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
okfkit agent install --claude         # or --codex: MCP tools + skills, for when it grows
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
okfkit agent install --claude         # this project: .mcp.json + .claude/skills
okfkit agent install --codex          # Codex: ~/.codex/config.toml + an AGENTS.md block
okfkit agent install --claude --print # show the changes first
```

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
server per audience, with `--allow`/`--deny` deciding what that audience can read.

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
| `agent install` | agent configuration and skills (`--print` to preview) |
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
