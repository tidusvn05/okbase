# okbase — Design

> This is the okbase design document: the source of truth for the design. The evidence behind every default is in `spikes/` (summary table in `spikes/README.md`). Current status is in §13; decisions made after v2.1 are in Appendix B. Detailed sub-plans live in `docs/plans/`.

> Version: 2.1 (2026-10-01) · Status: **Final for implementation** · **Open-source project** (MIT OR Apache-2.0) · Original v0.1 handoff: `docs/HANDOFF.md`
> Changes since 1.0: embedding became **opt-in** (lexical by default); added the **okbase standard** (levels L0–L3) and the **adopt** flow (plain markdown → OKF); added **Agent Skills**; **modules + profiles + extension points** design.

## 0. Summary

**okbase** helps AI agents (Claude Code, Codex, OpenCode, qobot, any MCP client) work with **a folder of markdown documents** quickly, cheaply and accurately, using the **OKF (Open Knowledge Format)**.

Four main uses:

| You have… | okbase does | Command |
|---|---|---|
| **An existing OKF bundle** | Works immediately, no configuration, no file changes | `okbase mcp serve` / `okbase agent install` |
| **A plain markdown folder** (docs, wiki, Obsidian, docs site) | Converts it to the okbase standard: adds frontmatter, `index.md`, descriptions, tags; diff can be previewed | `okbase adopt` |
| **PDFs, Sheets, Drive…** | Imports them as per-section markdown and SQL datasets | `okbase import` (module) |
| **An agent that must use it efficiently** | Skills + instructions + MCP tools designed from measured results | `okbase agent install` |

**Simple defaults:** no model, no network, no configuration; **lexical** mode (catalog + `grep` v2 + `query` + `get`). Heavy features (embedding, import, source sync, writing, eval) are **opt-in modules**.

---

## 1. Evidence from spikes → decisions

| # | Spike | Key result | Decision for okbase |
|---|---|---|---|
| S1 | 12 embedding models, 300 questions vi/en/ja (`embed-bench`) | EmbeddingGemma Q4 is best (R@1 0.85, R@3 0.96, 188MB, ~0.5GB RAM); BM25 on cross-language questions only 3.5% | If embedding is enabled, use EmbeddingGemma Q4 (or bge-m3 int8, MIT). **Pre-retrieval for cross-language questions needs embedding** |
| S2 | BM25 threshold | Fusing BM25 and embedding gives no benefit (0 to +1 point) | No fusion. Lexical is used for `grep` |
| S3 | Catalog vs. tool, with the real Claude CLI | Same accuracy; the catalog must be in the system prompt (otherwise cost ×2.5) | `catalog()` is the main product; guide hosts to put the catalog in the system prompt |
| S4 | OpenClaw docs 60k → 4.4M tokens, 9 approaches | **Lexical (G2) 100/100/93/90% ≈ embedding (D) 97/100/100/93%**; agent + Read/Grep is on par too; embedding needs fewer turns (~2 vs. ~4.5); prompting the agent to "double-check" does not help; remaining errors come from duplicated documents | **Lexical by default**; `grep` must be as strong as the CLI's Grep; embedding is opt-in to optimize speed; lint detects duplicates |
| S5 | Metadata/tags + sheets, 151 → 3,020 documents (`biz-meta`) | Small: agent grepping frontmatter is enough. Large: `query` keeps lists complete, 30–45% cheaper. **Sheet ~10k rows: without SQL the agent gives up; with SQL 10/10, 9× cheaper** | `query` is core; `data` is a module (auto-enabled when CSV/XLSX is present); no full views are generated; catalog includes tag vocabulary and facets |
| S6 | Chunking and indexing speed | ~3 chunks/s/8 CPUs; 4.4M tokens ≈ 64 minutes | Embedding runs in the background, cached by hash; lexical is ready immediately |
| S11 | Fine-tuning EmbeddingGemma with LoRA (Unsloth's approach) on synthetic questions (`spikes/embed-tune`) | In-domain: S1 R@1 0.853 → 0.943, cross-language 0.815 → 0.95; no forgetting of other domains; S4 saturated (27 → 28/30). CPU ~25 minutes of training, ~$3/1k pairs; every model change requires re-embedding the whole bundle | Do not put training into okbase. Candidate: allow loading a user-supplied ONNX model (opt-in), **after** checking that ONNX/Q4 keeps the improvement |

**Central lesson: good bundle organization is the deciding factor.** A strong agent with only Read/Grep already does well on a bundle with `index.md`, clear `description`s and consistent metadata. So okbase focuses on three things:
1. **help organize well** (standard, adopt, lint);
2. **help agents exploit that organization** (catalog, skills, tools);
3. add heavy machinery only when needed: SQL for tabular data, embedding for speed and pre-retrieval.

**Measured after this plan was written** (results in §13 and `spikes/README.md`):
- S7: lexical mode with **Codex** and small models: gpt-6.1-sol 26/30 (Claude 28/30); the small
  gpt-6-luna 21/30, and semantic search does not close the gap.
- S8: adopt does not lower accuracy on plain markdown (29/30 before and after).
- S9: agents never invoked the skills; S15 shows the same rules work as MCP server instructions.

---

## 2. okbase standard (okbase profile of OKF)

okbase does not create a new format. **okbase standard = OKF v0.2 + conventions that help agents work well**, split into levels. Every level is valid OKF.

| Level | Name | Requirements | Benefit (per spikes) |
|---|---|---|---|
| **L0** | OKF | Every `.md` (except index/log) has frontmatter with `type` | Readable by any OKF tool |
| **L1** | Navigable | L0 + every document has `title` and **`description`** (1 sentence); **`index.md` in every folder**; stable IDs | Catalog and `index.md` are meaningful, so agents find the right file with Read/Grep or `kb_list` (S3/S4) |
| **L2** | Structured | L1 + `tags` from a vocabulary (`_meta/vocabulary.md`), `lang`, `status` (stable/deprecated/draft) with `supersedes`, `updated`; custom fields per the type's schema | Accurate `query`, picks the current version, filterable when the bundle is large (S5) |
| **L3** | Curated | L2 + no near-duplicate content (or linked to a canonical page); `sources`/`verified` for important documents; no broken links; nothing past `stale_after` | Reduces the remaining error class at scale: information spread over many documents (S4) |

`okbase lint --level L2` reports which level the bundle is at and what is missing. `okbase adopt` brings plain markdown to L1 automatically, and to L2 with agent assistance.

### 2.1 Recommended folder layout (not mandatory)

```
my-bundle/
├── okbase.toml         # optional — defaults are used if absent
├── index.md, log.md    # OKF (generated/updated by okbase)
├── knowledge/…         # curated knowledge
├── sources/…           # imports (PDF by section, web…), with provenance
├── data/               # datasets: *.csv|*.xlsx + <name>.md (type: Dataset, auto-generated schema)
├── _meta/
│   ├── vocabulary.md   # tag vocabulary + vi/en/ja synonyms + facets
│   └── types/<Type>.md # (optional) field schema per type: kind, required, allowed values
└── .okbase/            # (gitignored) index, cache — or placed outside the bundle (§3.1)
```

### 2.2 Frontmatter

| Group | Fields |
|---|---|
| OKF required | `type` |
| okbase L1 | `title`, `description` |
| okbase L2 | `tags`, `lang`, `status`, `supersedes`, `updated`, `aliases`, `audience`, `effective_from`, `effective_to` |
| OKF v0.2 (L3) | `sources[]`, `generated{by,at}`, `verified[]`, `stale_after`, `resource` |
| Custom | Any field; types declared in `_meta/types/<Type>.md` or `okbase.toml [fields]` |

Write rule: **round-trip keeps unknown keys, key order, comments and body.** okbase edits only the field it was asked to edit.

---

## 3. Usage flows

### 3.1 Existing OKF bundle (no configuration, no file changes)

```
cd existing-bundle
okbase status                  # level L0–L3, document/token count, recommended mode, hints
okbase agent install --claude  # register MCP + install skills for Claude Code (or --codex)
```

- **Read-only by default:** okbase does not write to the bundle without an explicit write command (`adopt --write`, `index --write-index-md`, `lint --fix`).
- **The index can live outside the bundle:** `state_dir = "auto"` uses `.okbase/` if the bundle is writable, otherwise `~/.cache/okbase/bundles/<hash>/`. This makes read-only bundles usable (other people's repos, mounted folders).
- Accepts non-conforming bundles: if `description` is missing, use the first sentence of the body; if `index.md` is missing, build a virtual catalog in memory. No bundle is rejected (per OKF conformance rules).
- Also recognizes common non-OKF frontmatter: `summary` (Mintlify/OpenClaw), `excerpt`, `tags`/`categories` (Jekyll/Hugo), `sidebar_label` (Docusaurus). These fields are mapped on read; files are not modified.

### 3.2 Plain markdown → okbase standard (`okbase adopt`)

```
okbase adopt ./docs --plan            # report: what will be added, and where (no writes)
okbase adopt ./docs --out ./docs-okf  # write to a new folder (safe)
okbase adopt ./docs --write           # edit in place (requires a clean git tree or --force), with log.md
okbase adopt ./docs --level L2 --with-agent claude   # ask an agent to write missing descriptions/tags
```

| Step | Automatic (no LLM) | With agent assistance (optional) |
|---|---|---|
| 1. Detect | Site kind (Obsidian, Docusaurus, Hugo, Mintlify, MkDocs, plain folder); read and map existing frontmatter | — |
| 2. `type` | Inferred from folder/file name/heading by rules (`faq/` → FAQ, `adr-*` → Decision…); default `Document` | Suggests a more accurate type |
| 3. `title` | H1 → existing frontmatter → file name | — |
| 4. `description` | First meaningful sentence (skipping admonitions, badges, code); marked `generated: {by: okbase-heuristic}` | **Writes a 1-sentence description** (most important for the catalog) |
| 5. Links | Convert wikilinks `[[x]]` and relative links to OKF form; report broken links | — |
| 6. Structure | Generate `index.md` in every folder; `log.md`; suggest splitting overly long files (> N tokens) by H2 | Proposes a folder structure |
| 7. L2 | `lang` (detection), `updated` (git log/mtime), tags from folders | Tags from the vocabulary, `status`/`supersedes` when multiple versions are detected |
| 8. Report | Diff, lint before/after, L level reached | — |

- The agent-assisted parts of steps 4 and 7 are done through the **`okbase-curate` skill** (§6) or the `adopt --with-agent` command. That command calls a CLI agent through the optional `agent-bridge` module.
- Every machine-generated value records `generated{by,at}` (OKF v0.2), so later readers know it needs verification (`verified`).

### 3.3 Importing source documents (`import` module)

- PDF, DOCX, HTML → `sources/<name>/<nn>-<section>.md`: split by heading or every N pages, with `resource`, `pages`, `generated`; tables become markdown tables; OCR is optional.
- XLSX, CSV, Google Sheets → dataset (`data` module) + `data/<name>.md`.
- Incrementally synced sources (`source-*` modules): folders, Google Drive/Sheets, Notion.
- **Import does not modify `knowledge/`.** Distilling `sources/` into concepts is the agent's job, via the `okbase-curate` skill (`okbase distill --plan` lists what needs to be done).

### 3.4 Agents using the bundle (daily)

```
okbase agent install --claude | --codex | --opencode | --print
```
- Registers the MCP server (`okbase mcp serve --stdio`, or HTTP for long-running use).
- Installs **skills** (§6) into the CLI's skill folder; for CLIs without skill support, generates an equivalent `AGENTS.md` snippet.
- Prints the catalog snippet and instructions to put in the system prompt, for users who build their own agent.

---

## 4. Architecture

### 4.1 Principles
1. **Small core, no heavy dependencies:** no ONNX, no network, fast builds. Everything heavy is a module.
2. **Files are the source of truth.** The index is rebuildable and can live outside the bundle.
3. **User-agnostic:** every read operation takes a `Scope` passed in by the host.
4. **Output for agents:** compact, with IDs for citing sources, hints when there are no results, and a clear notice when truncated.
5. **Extend through traits, capabilities and profiles**, not by changing the core.
6. **Measurable:** built-in eval, and every default is tied to a spike.

### 4.2 Diagram

```
   ┌────────────── Interfaces ─────────────────────────────────────────────────┐
   │ CLI `okbase` (+ plugin `okbase-<x>`)   MCP (stdio/HTTP)   Rust API   Skills│
   └──────────┬─────────────────────────────┬──────────────────┬───────────────┘
              ▼                             ▼                  ▼
   ┌──────────────────────── Facade `okbase::Bundle` ───────────────────────────┐
   │ Capability registry: which tools/commands exist depends on enabled modules │
   └──────────┬──────────────────────────────────────────────────────────────────┘
   ┌──────────▼──────────── CORE (always on) ──────────────────────────────────┐
   │ core: parse/write round-trip, validate, links · standard: L0–L3, schema    │
   │ index: metadata, tags, links, aliases, chunks, FTS (analyzer vi/en/ja)     │
   │ read: grep v2 · get · list · query(filter/facet/sum) · catalog · stats     │
   │ maint: lint (rule engine) · adopt (heuristic) · index.md/log.md/vocabulary │
   └───────┬───────────────┬──────────────┬───────────────┬────────────────────┘
   ┌───────▼─────┐ ┌───────▼──────┐ ┌─────▼───────┐ ┌─────▼──────────────────┐
   │ data        │ │ embed-local  │ │ import-*    │ │ write · watch · eval   │
   │ CSV/XLSX →  │ │ embed-api    │ │ pdf/docx/   │ │ source-gdrive/notion…  │
   │ SQLite, SQL │ │ search,      │ │ html/xlsx   │ │ agent-bridge (CLI call)│
   │ (duckdb)    │ │ retrieve     │ │             │ │ ann (usearch)          │
   └─────────────┘ └──────────────┘ └─────────────┘ └────────────────────────┘
        (MODULE: Cargo feature at build time + enabled/disabled by config at runtime)
```

### 4.3 Crates

| Crate | Core / module | Contents |
|---|---|---|
| `okbase-core` | core | Concept, Frontmatter (keeps order, unknown keys, comments), parse/write, OKF validation, links, IDs |
| `okbase-standard` | core | Levels L0–L3, type schemas (`_meta/types`), tag vocabulary, mapping of foreign frontmatter (summary→description…) |
| `okbase-analyze` | core | Analyzer trait: NFKC, Vietnamese diacritic folding, lindera (ja, default-on feature), English stemming, language detection |
| `okbase-index` | core | SQLite schema, incremental indexer, chunker, catalog, facets, index.md/log.md generation |
| `okbase-query` | core | `grep` v2, `get`, `list`, `query`, `catalog`, `stats`, `recommend_mode` |
| `okbase-lint` | core | Rule engine + standard rules (L0–L3); `LintRule` trait |
| `okbase-adopt` | core | Site detection, mapping, title/description/type heuristics, file splitting, plan/diff |
| `okbase-data` | `data` module | Dataset → SQLite (DuckDB as a sub-feature), `data.query` with safety limits, schema doc |
| `okbase-embed` | `embed-local` / `embed-api` module | Embedder trait, fastembed (EmbeddingGemma Q4 / bge-m3 int8), HTTP client; cache by hash; batching by length |
| `okbase-search` | module (needs embed) | Dense search, MMR, `retrieve`, semantic duplicate lint, ANN (`ann` feature) |
| `okbase-import` | `import-*` module | Converter trait + pdf/docx/html/xlsx/csv |
| `okbase-source` | `source-*` module | Source trait + fs/gdrive/gsheets/notion |
| `okbase-mcp` | core (stdio) / `http` module | Capability-based tool registry, rmcp; `router()` for hosts to embed |
| `okbase-skills` | core | Skills and instructions embedded in the binary; `agent install` for claude/codex/opencode |
| `okbase-bridge` | `agent-bridge` module | Calls CLI agents (via agent-core) for adopt/curate/eval |
| `okbase-eval` | `eval` module | Runs question sets (retrieval or agent), scores, reports cost and latency (from spikes) |
| `okbase` | facade | `Bundle`, `Scope`, `Capabilities`, re-exports |
| `okbase-cli` | binary | `okbase`, discovers `okbase-<x>` plugins on PATH |

Release builds:
- **`okbase`**: core + data + mcp-http + import (PDF, Office, HTML; CSV/XLSX through data). No ONNX; about 34MB (2026-10-03). Language dictionaries (currently the Japanese IPADIC) are **not embedded** in any build, `okbase-full` included: they are downloaded on first use, or installed ahead of time with `okbase dict install`. Dictionaries for languages added later will work the same way.
- **`okbase-full`**: adds embed-local, import-pdf/docx/html, source-*, eval.

### 4.4 Configuration: simple defaults, extend gradually

Without `okbase.toml`, defaults are used. The fullest file only enables what is needed:

```toml
# okbase.toml — every section is optional
profile = "standard"            # minimal | standard | full  (default module set)

[bundle]
languages = ["vi", "en", "ja"]
state_dir = "auto"              # auto | ".okbase" | "~/.cache/okbase/..."

[modules]                       # overrides the profile
data = "auto"                   # auto: enabled when *.csv/*.xlsx exist in data/
embed = "off"                   # off | local | api
import = ["pdf", "xlsx"]
sources = []                    # ["gdrive", "notion"]
write = false                   # write tools for agents
watch = false

[embed]                         # read only when modules.embed != off
model = "embeddinggemma-300m-q4"    # | bge-m3-int8 | api:<provider>/<model>

[standard]
target_level = "L2"             # lint against this level
vocabulary = "_meta/vocabulary.md"

[tools]                         # MCP/skills: rename, hide, limit
prefix = "kb"
disable = []
limits = { grep_lines = 40, get_tokens = 4000, data_rows = 100 }
```

| Profile | Modules enabled | Use when |
|---|---|---|
| `minimal` | Core | CI, small bundles, weak machines |
| `standard` (default) | Core + data (auto) + import csv/xlsx | Most use with Claude Code/Codex |
| `full` | All modules, including embed-local | Large multilingual bundles that need fast pre-retrieval; qobot-style hosts |

### 4.5 Extension points

| To add | How | Rebuild needed? |
|---|---|---|
| A new document type with its own fields | `_meta/types/<Type>.md` (schema: field, kind, required, enum) → understood by lint, query, adopt | No |
| Tags, synonyms, facets | `_meta/vocabulary.md` | No |
| Type-inference rules for adopt | `okbase.toml [adopt.rules]` (glob → type/tags) | No |
| Custom lint rules | Declarative rules (TOML: field X required when type is Y, forbidden regex…) | No |
| New CLI commands | External plugin `okbase-<name>` on PATH (git-style), takes `--bundle` and communicates via JSON | No |
| Tools for agents | External MCP server (configured by the agent), or a "command" tool declared in `okbase.toml [tools.custom]` (runs a command, JSON in/out) | No |
| Project-specific skills | `_meta/skills/<name>/SKILL.md`, installed along by `agent install` | No |
| Converter, Source, Embedder, Analyzer, LintRule, ToolProvider | Implement the trait in Rust, register via the registry (feature) | Yes |
| Safe in-process hooks | WASM (extism): `on_index_doc`, `on_lint`, `on_tool_output` (later phase) | No |

**Capability registry:** each module declares capabilities (`read.grep`, `data.sql`, `embed.search`, `import.pdf`…). The CLI, MCP and skills show only what is available. Example: without embed, `kb_search` does not appear, and the skill directs the agent to grep/list.

### 4.6 Library API (for hosts such as qobot)

```rust
let bundle = Bundle::open(dir, OpenOptions::default()
    .profile(Profile::Standard)
    .embedder(shared_embedder.clone())      // optional; None = lexical
    .state_dir(StateDir::Auto))?;
bundle.sync(SyncMode::Incremental).await?;
let caps = bundle.capabilities();            // which modules are available
let scope = Scope::all().deny("memory/people/**").filter(MetaFilter::not_audience("private"));

bundle.catalog(&CatalogOptions::default(), &scope)?;         // for the system prompt (+ vocabulary/facets)
bundle.recommend_mode(&scope);                               // Full | Retrieval | Lexical
bundle.retrieve(q, TokenBudget(3000), &scope).await?;       // only with embed
bundle.grep(&req, &scope)?; bundle.query(&mq, &scope)?; bundle.get(&id, &sel, &scope)?;
bundle.data()?.query(sql, Limits::default())?;               // when the data module is enabled
bundle.lint(&LintConfig::level(Level::L2))?;
bundle.write(ConceptWrite { .. }).await?;                    // when the write module is enabled, validate + index/log
okbase_mcp::router(bundle.clone(), scope_provider);          // mount into the host's axum
bundle.watch();                                              // Stream<ChangeEvent>
```

### 4.7 Index

| Table | Always present | Contents |
|---|---|---|
| `docs`, `doc_fields`, `doc_tags`, `aliases`, `links` | ✅ | Typed metadata, original and normalized tags, links and backlinks |
| `chunks`, `chunks_fts` | ✅ | Chunks by H2/H3 (150–450 tokens, `title > heading` prefix), FTS via Analyzer |
| `chunk_vecs` | embed module | f32 vectors loaded into RAM (brute force; ANN above ~100k chunks) |
| `datasets.sqlite` | data module | One table per dataset + `_schema` |

- Incremental indexing by hash. Lexical is ready immediately; embedding runs in the background, with progress, and a global hash-keyed cache (`~/.cache/okbase/emb`).
- While embedding is not finished, `recommend_mode()` returns `Lexical`.

---

## 5. Agent tools (MCP / CLI `--json`)

| Tool | Module | Description (based on spikes) |
|---|---|---|
| `kb_catalog` | core | Catalog (≤ N tokens) + tag vocabulary and facets; hosts should put it in the system prompt instead of letting the agent call it |
| `kb_list` | core | A folder's `index.md` |
| `kb_grep` | core | Regex/alternation, case- and diacritic-insensitive, searches frontmatter too, `path` glob, `context`, `files_only`, `filter`; hints when empty (S4: G improved to G2) |
| `kb_get` | core | By `section`/`lines`/`max_tokens`; reports truncation with a list of headings |
| `kb_query` | core | Metadata filters, `active_on`, `facets`, `sum_field`, `sort`, `count_only` (S5) |
| `kb_links` | core | Links and backlinks |
| `data_tables`, `data_query` | data | Read-only SQL (S5: required for sheets) |
| `kb_search` | embed | Multilingual semantic search, with `filter` |
| `kb_write`, `kb_propose` | write | Validated writes, updates index/log (off by default) |

Output is identical across MCP and `--json`. Tool names can be changed with `[tools].prefix`.

---

## 6. Agent Skills

Skills (AgentSkills standard, `SKILL.md`) are **the cheapest way to make agents use okbase correctly**. A skill is loaded only when needed, so it does not cost standing tokens like long instructions in the system prompt. Skills are embedded in the binary and installed with `okbase agent install`.

| Skill | When the agent uses it | Main content (per spikes) |
|---|---|---|
| **`okbase-answer`** | Answering questions from the bundle | Read the catalog or `index.md` first. **List, count, filter questions: use `kb_query`** (S5). **Numbers: use `data_query`**, do not add up by hand (S5). Codes or exact strings: `kb_grep` with alternation, English terms and synonyms (S4). Meaning-based questions: `kb_search` if available, otherwise `kb_grep` + `kb_list`. Long documents: `kb_get` by `section`. Prefer `status: stable`, check `supersedes`. Multi-part questions: cover every part. **Always cite ids** |
| **`okbase-curate`** | Maintaining and upgrading the bundle | Run `okbase lint --level L2`, fix in order of impact: missing or weak descriptions → index.md → vocabulary tags → status/supersedes → merge duplicate content into a canonical page. Write specific 1-sentence descriptions. Record `generated`. Do not delete content |
| **`okbase-adopt`** | Converting a markdown folder to the standard | Run `okbase adopt --plan`, review, then `--out` or `--write`; write descriptions and tags for files the heuristic marked as weak; re-run lint |
| **`okbase-import`** | Bringing in PDFs/Sheets | `okbase import`, check `sources/` and datasets, use `distill --plan` to create concepts with `sources[]` |
| **`okbase-author`** | Writing new documents | Use `okbase new --type …` (per schema), frontmatter complete to L2, link to related pages, update the vocabulary if a tag is new |

- Skills **adapt to capabilities**: the instructions for `kb_search` and `data_query` appear only when the corresponding module is enabled (generated at `agent install` time).
- **Fallback without MCP:** skills instruct the agent to call `okbase … --json` through the shell, so they also work in CLIs without MCP configured.
- Projects can add their own skills in `_meta/skills/`.
- **To measure (S9):** compare agents with and without skills on correct tool usage rate (target `kb_query` ≥ 45/48 vs. 32/48), accuracy, number of turns and cost.

---

## 7. CLI

```
# Use immediately
okbase status                         # L level, document/token count, modules, recommended mode, next-step hints
okbase agent install --claude|--codex|--opencode [--skills-only|--mcp-only]
okbase mcp serve [--stdio | --http :7331 --token …]

# Read
okbase grep 'E2|429' [--path 'sop/**'] [-C 1] [--files-only]
okbase get <id> [--section …] | okbase list [dir]
okbase query 'type=Policy tag:billing status!=deprecated active_on=2026-10-01' [--facets tags] [--sum contract_value]
okbase catalog [--max-tokens 10000]
okbase data tables | okbase data sql "select …"              # data module
okbase search "refund policy" | okbase retrieve "…" --budget 3000  # embed module

# Organize
okbase lint [--level L2] [--fix-safe] [--format text|json|sarif]
okbase adopt <dir> --plan | --out <dir> | --write [--level L2 --with-agent claude]
okbase index [--full] [--write-index-md] [--watch]
okbase vocab [--suggest] | okbase new --type <Type> <id> | okbase validate --okf

# Modules
okbase embed enable [--model …] | okbase embed status
okbase import <file|dir> | okbase source add|sync …
okbase eval run questions.json [--mode retrieval|agent --cli claude]
okbase modules                        # modules in the build + currently enabled
```

---

## 8. Multilingual support

- Core: NFKC, Vietnamese diacritic folding (grep/FTS), lindera (Japanese), English stemming.
- Language detection (lingua) is only used for hints: Vietnamese without diacritics is only about 90% correct.
- Multilingual tag vocabulary, including synonyms.
- Without embedding, cross-language questions rely on the agent translating keywords itself (guided by the skill). S4 shows this is good enough with Claude.
- With embedding, EmbeddingGemma handles cross-language matching directly, enabling pre-retrieval.

---

## 9. Lint (rules by level)

| Level | Rules |
|---|---|
| L0 | Frontmatter parses; has `type` |
| L1 | `title`/`description` present and specific enough (length, does not repeat the title); `index.md` in every folder; stable IDs |
| L2 | Tags belong to the vocabulary (or a cross-language synonym is reported); fields have the right kinds per the type's schema; `status`/`supersedes` consistent; no multiple `stable` documents for the same topic/scope; valid `effective_*`; datasets have a schema doc |
| L3 | Near-duplicate content (needs embed; otherwise shingle heuristics); broken links; orphans; past `stale_after`; important documents missing `sources`/`verified` |
| General | Overly long documents (suggest splitting); imported PDFs with blank pages or broken tables |

Output text/JSON/SARIF; `--fix-safe` fixes only what is safe (generate index.md, normalize tags, add `lang`).

---

## 10. Security

- **Read-only by default**; every write operation needs an explicit command or module.
- Path normalization, path traversal blocked.
- SQL: `query_only`, SELECT only, timeout, row limit, no ATTACH.
- MCP HTTP: token, binds to localhost by default; the host provides a `ScopeProvider`.
- Output includes `id`/`resource` so hosts can mark content as *untrusted*.
- Source secrets come from env/keyring, never from `okbase.toml`.
- `agent-bridge` runs only when the user invokes it, and prints the exact CLI command it will run.

---

## 11. Performance targets (8 CPUs, per spikes)

| Operation | Target |
|---|---|
| `okbase status` / first lexical index of 1k documents | ≤ 5s |
| `grep` over 4.4M tokens | ≤ 500ms |
| `query` over 3k documents | ≤ 20ms |
| `data.query` aggregate over 10k rows | ≤ 50ms |
| `search` (embed module, ≤ 20k chunks) | p50 ≤ 60ms |
| First embedding | ≈ 3 chunks/s (EmbeddingGemma Q4), in the background |
| `okbase` binary (no ONNX) | ≤ 100MB (raised from 25MB on 2026-10-03; currently ~34MB) |

---

## 12. Testing and evaluation

- Golden round-trip: the official OKF bundles (acme_retail, ga4, stackoverflow, crypto_bitcoin) must have diff = 0; tests for preserving comments and unknown keys.
- **Adopt fixtures:** OpenClaw docs (Mintlify), a sample Obsidian vault, a Docusaurus docs folder, and a "dirty" markdown folder. Check that `adopt --plan` is stable (snapshot) and that lint after adopt reaches L1.
- Snapshots of tool output and of skills generated per capability.
- `okbase-eval lexical` (done, 2026-10-03): replays the agents' real tool calls from spikes S3/S5 on `fixtures/business` and `fixtures/multilingual` and compares them with the answers; takes a few seconds, needs no model; runs in `cargo test` and fails when a previously passing case regresses.
- `okbase-eval` (remaining, from spikes): retrieval mode (R@k, currently `examples/retrieval_eval.rs`) and agent mode (claude/codex, scored by key facts or precomputed answers, cost, latency). Fixtures: multilingual v2 (S1), OpenClaw S/M/L (S4), business ×1/×20 (S5).
- CI: fmt, clippy, test, deny; fast lexical eval on small fixtures.

---

## 13. Roadmap

Estimates for 1 full-time developer.

**Status (2026-10-03):**

| Milestone | Status | Evidence |
|---|---|---|
| v0.1 | ✅ Done. G2 reproduced 28/30 (93%) | `spikes/acceptance-v0.1` |
| v0.2 | ✅ Done. S5 ×20: 44–45/48, sheet 9–10/10; S8 passed (adopt does not reduce accuracy). S9: agents did not invoke the skills; **S15** shows MCP server instructions can replace them (46/48 without hints) | `spikes/acceptance-v0.2` |
| v0.3 | ✅ Done. S1 R@1 0.857; S4 top-6: 29/30 (Gemma), 30/30 (bge-m3) | `spikes/acceptance-v0.3` |
| advise + fine-tune | ✅ Done (phases 0–5). The agent runs the whole workflow; Q4 keeps the improvement | `docs/plans/advise-tune.md`, `spikes/embed-tune` |
| Real-world use cases | ✅ Done (U1–U8) | `docs/plans/usecases.md`, S13b |
| Onboarding through an agent | ✅ Done (O1–O5). Claude 3/3; Codex 12/12 found `onboard` and did not add consent flags on its own. The Codex sandbox blocks `.codex/`: handled with `sandbox_blocked` and a step for the user | `docs/plans/onboarding.md`, S13, `spikes/codex` |
| v0.4 import | ✅ Conversion part done (I1–I5, S14). Remaining: Google Drive/Sheets connectors, `distill`, S10 with real documents | `docs/plans/import.md` |
| Eval | `okbase-eval lexical` runs in `cargo test`. Agent mode is still scripts in `spikes/` | §12 |
| v0.5 → v1.0 | Not started | |

### v0.1 — Works immediately with existing bundles (2 weeks)
- core (round-trip), standard (L0–L2, foreign frontmatter mapping), index (lexical), query (grep v2, get, list, query, catalog, stats), lint L0–L2, MCP stdio, CLI.
- **`okbase-answer` skill** + `agent install --claude/--codex`.
- ✅ Criteria:
  - round-trip of the official OKF bundles has diff = 0;
  - reproduce S4's G2 (≥ 93% at size L) with Claude via skill + MCP;
  - `okbase status` runs on a read-only bundle (external state_dir).

### v0.2 — Adopt + data (2 weeks)
- `adopt` (site detection, heuristics, `--plan/--out/--write`), index.md/log.md generation, vocab, `okbase-adopt`/`okbase-curate` skills.
- `data` module (CSV/XLSX → SQLite, `data.query`, schema doc).
- **Spike S8:** plain markdown docs → adopt → compare agent accuracy before and after.
- **Spike S9:** with skills vs. without skills.
- ✅ Criteria: adopting the original OpenClaw docs reaches L1 with no manual edits; reproduce S5 ×20 (sheet 10/10); S8 shows adopt does not reduce (expected: improves) accuracy.

### v0.3 — Embed module (opt-in) (2 weeks)
- `embed-local` (EmbeddingGemma Q4, bge-m3 int8), `embed-api`, cache, background indexing, `search`, `retrieve`, `recommend_mode`, MCP HTTP, `router()` for hosts.
- `okbase-full` build.
- ✅ Criteria: reproduce S1 v2 (R@1 ≥ 0.84); S4 retrieval ≥ 97% at size L; the default `okbase` binary still has no ONNX.

### New proposal: `okbase advise` + embedding fine-tuning
- See `docs/plans/advise-tune.md` (order relative to v0.4 not decided; phase 0 is the ONNX/Q4 technical gate).

### New proposal: real-world use cases
- See `docs/plans/usecases.md`: a software repo with `docs/`, an empty folder, non-conforming OKF, a bundle in a subfolder; a folder scanner, ignore rules, `adopt` that is safe on docs sites, a `docs-site` profile.

### New proposal: onboarding through an agent
- See `docs/plans/onboarding.md`: `okbase onboard`, the machine contract (JSON, error codes, exit codes), the consent catalog, `doctor`, bootstrap for agents; plus fixing the index during an MCP session (P0).

### v0.4 — Import + sources (2 weeks)
- **Conversion part:** see `docs/plans/import.md` (anydoc + htmd, direct reading and full conversion, OCR through the agent).
- import-pdf/docx/html, source fs/gdrive/gsheets, `okbase-import` skill, `distill --plan`.
- **Spike S10:** about 20 real PDFs and 5 real sheets.
- ✅ Criteria: the agent correctly answers lookup and aggregation questions on imported data.

### v0.5 → v1.0 — Extension and stabilization (2–3 weeks)
- `okbase-<x>` CLI plugins, command tools, declarative lint rules, type schemas, project skills, complete eval, watch, ANN, DuckDB.
- **Spike S7:** lexical with Codex and small models, to choose the default profile per CLI.
- API stabilization (semver), documentation, integration examples.

**About 10–11 weeks in total to v1.0.** v0.1 (week 2) is already usable daily with Claude Code/Codex on existing bundles.

---

## 14. Risks and open decisions

| Risk / question | Mitigation |
|---|---|
| Lexical has only been measured with Claude Sonnet | Measured (S7): Codex gpt-6.1-sol 87% vs. Claude 93%; small models 70%, and adding embedding does not improve them. Keep lexical as the default; recommend a strong model for answering |
| Adopt heuristics write poor descriptions | Mark as `generated`; `okbase-curate` skill lets the agent rewrite them; S8 measures the impact |
| Agents do not use the skills | Measured (S9, S15): agents do not invoke the skills, but rules sent as MCP server instructions are followed. Skills are kept for setups without MCP and for curation work |
| The name "OKF" is a Google spec | Product name `okbase`; state clearly "community tool" |
| The OKF spec changes | Core keeps unknown keys; the okbase standard is a separate layer (`okbase-standard`) |
| Gemma license | Relevant only when embed-local is enabled; bge-m3 int8 (MIT) as an alternative |
| PDF quality | Converter is a trait; S10 |
| Self-fine-tuned models: the improvement is lost when quantized to Q4; synthetic questions differ from agents' real questions; internal data is sent to an LLM | S11: check ONNX/Q4 before building the feature; evaluate with human-written questions; vector cache keyed by model hash |
| rmcp changes quickly | Pin minor |

**Decided:** license MIT OR Apache-2.0 (§17); publish to crates.io from v0.1 (0.x releases); keep the L0–L3 level names and skill names as above (can change before v1.0).

---

## 15. Relationship to other projects

- **qobot:** depends on the `okbase` crate (git tag); uses `Bundle`, `catalog`, `recommend_mode`, `retrieve` (when embed is enabled), `router` with `ScopeProvider`, `write`, `watch`. qobot chooses the profile (usually `full` for a multilingual chat bot that needs speed). qobot does not access okbase's internal index.
- **agent-core:** used only in the `agent-bridge` module (adopt/curate/eval calling CLI agents).
- **Claude Code / Codex / OpenCode:** via `okbase agent install` (MCP + skills).

## 16. Immediate tasks (week 1)

1. `git init`, workspace, CI, license.
2. `okbase-core` round-trip (keeping comments, unknown keys) plus fixtures from the official OKF bundles and OpenClaw docs.
3. `okbase-index` lexical + `okbase-query` (reuse the chunker and grep v2 from `spikes/embed-bench/src/bundle.rs`, and `kb_query` from `spikes/biz-meta/mcp_meta.py`).
4. CLI `status/grep/get/list/query/catalog` + MCP stdio + a draft `okbase-answer` skill; try it with Claude Code on the OpenClaw S fixture.

---

## 17. Open source

| Item | Decision |
|---|---|
| License | **MIT OR Apache-2.0** (dual, per Rust convention); `LICENSE-MIT`, `LICENSE-APACHE` copied verbatim from the official sources |
| Name | `okbase` (available on crates.io and GitHub, checked 2026-10-01). README states clearly: *independent community project, not affiliated with Google*; "OKF" is a Google Cloud spec |
| Documentation language | README, rustdoc, CLI help, CONTRIBUTING: **English**. Design documents (`docs/design.md`, `docs/HANDOFF.md`, `docs/plans/`) are in English as well. vi/ja documentation is welcome |
| Governance | The lead maintainer decides (BDFL) until v1.0; major changes (format, public API, defaults) need a short issue/RFC in `docs/rfcs/` with data (spike/eval) |
| Contributions | PR + review; no CLA; `Signed-off-by` (DCO) encouraged; `CONTRIBUTING.md` covers build, test, eval; Code of Conduct: Contributor Covenant 2.1 |
| Security | `SECURITY.md`: private reports via GitHub Security Advisories; no public issues for vulnerabilities |
| Versioning | SemVer; 0.x may break the API but must be recorded in the CHANGELOG; the **on-disk format** (okbase frontmatter, `_meta/*`) stabilizes earlier than the API (from v0.2) |
| Releases | Tag `vX.Y.Z` → CI builds binaries (Linux x86_64/aarch64, macOS arm64/x86_64, Windows x86_64) with cargo-dist, publishes to crates.io, CHANGELOG generated from commits (conventional commits), GitHub Release. Two editions: `okbase` and `okbase-full` |
| CI | GitHub Actions: fmt, clippy `-D warnings`, test (Linux/macOS/Windows), cargo-deny (license allowlist + advisories), MSRV check, fast lexical eval on small fixtures |
| Third-party dependencies | cargo-deny allowlist: MIT, Apache-2.0, BSD-2/3, ISC, Unicode-3.0, Zlib, MPL-2.0 (only if unmodified). Check the license of the lindera IPADIC dictionary and record it in `THIRD_PARTY.md` |
| Models | **No models are bundled in the repo or the binary.** `okbase embed enable` downloads the model to the user cache, prints the model's license and asks for confirmation for models with their own terms (Gemma); bge-m3 (MIT) is the option that needs no confirmation |
| Fixture data | Google's OKF samples (Apache-2.0) and OpenClaw docs (MIT): only small subsets go into `fixtures/`, with NOTICE/attribution files; large corpora are downloaded by script, not committed |
| Spikes | `spikes/` is committed (scripts, questions, `results/`, ~7MB); caches, models, corpora and built bundles are in `.gitignore`. Records in `results/` contain local machine paths and are synthetic or LLM-generated data; this is noted in `spikes/README.md` |
| Telemetry | None |
| Platform support | Linux, macOS, Windows (core); embed-local module: Linux/macOS x86_64 + arm64, Windows x86_64 (per onnxruntime) |

---

## Appendix A. On-disk formats defined by okbase

These formats are part of the okbase standard and are stable from v0.2 (maintainer decision, 2026-10-01). Both are ordinary OKF documents: the data lives in the frontmatter, the body is free prose for people.

### A.1 Tag vocabulary — `_meta/vocabulary.md`

```markdown
---
type: Vocabulary
title: Tag vocabulary
terms:
  refund:                                   # canonical tag
    synonyms: [hoàn tiền, đổi trả, 返金, returns]
    facet: topic                            # optional dimension shown in catalog facets
    description: Refunds, returns and exchanges.
  shipping: [giao hàng, 配送]               # shorthand: a list of synonyms
  hr: {}                                    # a tag without synonyms
---
```

- Tags and synonyms are compared after normalization (trim, lowercase, runs of whitespace, `_` and `-` → `-`); the index also matches accent-insensitively.
- A synonym may belong to one term only; conflicts make the vocabulary invalid (`invalid-vocabulary`).
- Unknown keys inside a term are allowed. `okbase vocab --suggest` drafts this file from the tags in use.

### A.2 Type schemas — `_meta/types/<Type>.md`

```markdown
---
type: Type Schema
title: Policy
applies_to: Policy          # optional; defaults to the file name
fields:
  owner: {type: string, required: true}
  region: {type: string, enum: [VN, JP]}
  effective_from: {type: date}
  contract_value: number    # shorthand: just the type
---
```

- Field types: `string`, `number`, `integer`, `bool`, `date` (`YYYY-MM-DD`, optionally with a time), `list`, `map`, `any`.
- `okbase lint --level L2` reports `schema-missing-field`, `schema-wrong-type` and `schema-not-allowed`.

### A.3 Provenance of machine-made values

`okbase adopt` and `okbase vocab --suggest` record what they wrote in OKF's `generated` field, with an extra `fields` list: `generated: {by: okbase-adopt/0.1.0, at: "2026-10-01", fields: [type, description]}`. Lint reports listed title/description/type fields as `unreviewed-generated` until the document has a `verified` entry.

## Appendix B. Decisions made after v2.1

| Date | Decision | Why |
|---|---|---|
| 2026-10-01 | The IPADIC dictionary (~58 MB) is no longer embedded in the default build. It is downloaded on first use (md5-verified, rustls), built into the user cache, and loaded from there; `OKBASE_OFFLINE=1` disables downloads (Japanese then falls back to character bigrams); `okbase dict install` prepares offline machines; feature `ja-embedded` restores embedding. The index records the tokenization mode and rebuilds when it changes. | Binary target ≤ 25 MB (§11): 64 MB → ~21 MB. |
| 2026-10-01 | Dataset reads take a `Scope` like every other read: `Bundle::data_tables(scope)` / `data_query(sql, limits, scope)` instead of `bundle.data()?.query(sql, limits)` (§4.6). | Hard rule: every read API takes a host `Scope`. |
| 2026-10-01 | `ScopeProvider::scope` takes the request (`RequestInfo`: HTTP headers, `None` over stdio), and closures implement it, so hosts can scope per caller. `okbase_mcp::router(bundle, scopes, &ServerOptions, &HttpOptions)` returns an axum `Router` serving `/mcp`; `okbase mcp serve --http` binds 127.0.0.1:7331 by default and refuses non-loopback addresses without a bearer token (`OKBASE_MCP_TOKEN`). | §4.6, §10. |
| 2026-10-01 | `estimate_tokens` is a linear model fitted against the EmbeddingGemma tokenizer (words, Vietnamese syllables, CJK, punctuation, digits, line breaks) instead of the spike's per-word rates. | The per-word rates undercounted code-heavy text by ~30%; chunks exceeded the 512-token model limit (v0.3 S4 eval). |
| 2026-10-01 | MSRV stays 1.88; `serde-saphyr` is pinned to 1.1, and okbase double-quotes strings that YAML 1.1 parsers would read as booleans, numbers or dates. | serde-saphyr ≥ 1.2 needs Rust 1.89. |
| 2026-10-02 | MSRV raised to 1.89 to use `serde-saphyr` 1.3 (maintenance and YAML 1.1 string fixes). okbase keeps double-quoting strings that YAML 1.1 parsers would misread. | Maintainer decision; replaces the 2026-10-01 pin. |
| 2026-10-03 | The size target for the default `okbase` binary (no ONNX) is ≤ 100 MB instead of ≤ 25 MB. Document import (anydoc, about 10 MB) stays in the default build; the binary is ~34 MB. | Maintainer decision: import out of the box is worth the size. |
| 2026-10-03 | Language resources (the Japanese IPADIC dictionary now, others later) are external: downloaded on first use into the user cache, or installed ahead of time with `okbase dict install`; no build embeds them, `okbase-full` included. Feature `ja-embedded` stays only for custom builds that must work offline from a single file. | Maintainer decision: with more languages, embedding every dictionary would not scale. |
| 2026-10-03 | `embed enable --api-url` needs `--send-documents` (a new consent: every chunk and query goes to that service). `embed enable` refuses a module the build lacks before writing `okbase.toml`. | Readiness audit: the API path sent documents out without the user being asked. |
| 2026-10-03 | Inside Codex (`CODEX_THREAD_ID`), connecting Codex is a step for the user: Codex's sandbox keeps `.codex/` read-only, so `agent install --codex` stops before writing with `sandbox_blocked` (exit 3) and the command to run in the user's terminal. `CODEX_HOME` is honoured. | S13 with Codex: 12/12 runs hit the read-only `.codex/` and improvised. |
