# okbase — Implementation handoff

> This is the historical v0.1 handoff. Current status is in `docs/design.md` §13.

> Date: 2026-10-01 · Handed over by: the design and spike session (Claude Code)
> Audience: the agent or developer who will implement okbase. Read this whole file before writing code.

## 1. What you are receiving

| Document | Role |
|---|---|
| `docs/design.md` (v2.1) | **Source of truth** for the design: L0–L3 standard, architecture, modules/profiles, tools, skills, roadmap, OSS (§17) |
| `docs/HANDOFF.md` (this file) | How to start, task order, completion criteria, what not to do |
| `AGENTS.md` (repo root, English) | Day-to-day working conventions for every agent/contributor |
| `spikes/` | Experimental evidence and **reusable sample code**. `spikes/README.md` has a summary table |

Status (2026-10-03): v0.1–v0.3 are done and accepted; the additional plans (advise/tune, real-world use cases, onboarding, import) are implemented. See `docs/design.md` §13 (Status). The repo has commit history but is **not pushed** yet (no GitHub organization). This file is kept as the original v0.1 handoff; the rules in §5 still apply.

The project is **open source** (MIT OR Apache-2.0). Everything public (README, rustdoc, CLI help, commit messages, CONTRIBUTING) is written in **English**.

## 2. The design in 10 lines

1. okbase = CLI + MCP server + Rust library that helps agents work with markdown bundles in **OKF v0.2**.
2. **Lexical by default, no model:** catalog + `grep` v2 + `query` (metadata) + `get`/`list`. Spike S4: accuracy on par with embedding.
3. **Embedding is an opt-in module** (`embed-local`: EmbeddingGemma-300M Q4 or bge-m3 int8; `embed-api`), arriving in v0.3.
4. **Tabular data needs SQL** (`data` module, v0.2). Spike S5: without SQL the agent gives up on a ~10k-row sheet.
5. **okbase standard L0–L3** + `okbase lint --level` + `okbase adopt` (plain markdown → OKF).
6. **Skills** (`okbase-answer`, `-curate`, `-adopt`, `-import`, `-author`), installed with `okbase agent install --claude|--codex`.
7. **Read-only by default.** Round-trip keeps unknown keys, order and comments. The index can live outside the bundle.
8. **User-agnostic:** every read operation takes a `Scope` passed in by the host (qobot is the first host).
9. Extension through the **capability registry**, profiles (`minimal`/`standard`/`full`), `_meta/types`, `_meta/vocabulary.md`, `okbase-<x>` CLI plugins, Rust traits.
10. Every default is tied to a spike. **Changing a default requires data** (eval).

## 3. Sample code from the spikes, worth reusing

| Task | Look at | Notes |
|---|---|---|
| H2/H3 chunker (skips code fences, merges < min, splits > max) | `spikes/embed-bench/src/bundle.rs` → `chunk_doc` | Uses the Gemma tokenizer to count tokens; the core has no model, so it needs an approximate counter (§6 T4) |
| **grep v2** (regex, diacritic folding, frontmatter, path glob, context, files_only, hints when empty) | `bundle.rs` → `grep`, `fold`, `glob_re` | Measured: G to G2 gains 3–7 points, Vietnamese questions 85% → 95% |
| `get` by section, truncation with a list of headings | `bundle.rs` → `serve()`, `kb_get` branch | |
| **kb_query** (filter, `active_on`, facets, sum, sort) | `spikes/biz-meta/mcp_meta.py` → `match`, `kb_query` | Semantics validated over 336 runs |
| **data_query** (SQLite `query_only`, SELECT only, row limit) | `mcp_meta.py` → `data_query`, `data_tables` | Add a timeout (progress handler) |
| Minimal MCP stdio (JSON-RPC) | `bundle.rs` → `serve()` | Production uses **rmcp** |
| Analyzer: NFKC, Vietnamese diacritic folding, lindera | `spikes/embed-bench/src/main.rs` → `Analyzer`, `fold_latin` | lindera 6: `load_dictionary("embedded://ipadic")` |
| Embedding (v0.3) | `main.rs` → `load()`; `bundle.rs` → `index()` (cache by hash, sorted by length) | fastembed must have default features off and use rustls |
| Eval with real agent CLIs | `spikes/okf-scale/run.py`, `report.py`; `spikes/biz-meta/run.py`, `report.py` | Template for `okbase-eval` |
| Converting docs with foreign frontmatter (Mintlify `summary`) to OKF | `spikes/okf-scale/build_bundles.py` | Template for `adopt` |

**Note:** the spike code was written for quick measurement, not to production standards (unwrap, no tests). Copy the **logic and semantics**, not the code verbatim.

## 4. Settled technical conventions

| Item | Value |
|---|---|
| Rust | edition 2024, stable; **MSRV 1.89** (serde-saphyr ≥ 1.2; rmcp 3.x needs 1.88); workspace `resolver = "3"` |
| Lint | `clippy -D warnings`; `unsafe_code = "forbid"` in every crate unless a reason is written down |
| Errors | `thiserror` in libraries, `anyhow` only in the CLI |
| Async | tokio; the core can read synchronously (sync API + async wrapper), because the CLI and grep do not need async |
| YAML | **serde-saphyr** (not serde_yaml, serde_yml). **A round-trip that keeps comments cannot be done with serde** → store the raw frontmatter text and edit it per key at the text level (see T1) |
| SQLite | rusqlite 0.40 (`bundled`), FTS5 |
| Markdown | pulldown-cmark (headings, links); the body is never re-rendered |
| Japanese | lindera 6 (`embed-ipadic`); Vietnamese: unicode-normalization |
| CLI | clap 4 (derive); every read command has `--json` |
| MCP | rmcp 3.5 (pinned minor), stdio in v0.1, streamable HTTP in v0.3 |
| TLS | **rustls** only (no OpenSSL) |
| Logging | tracing; no telemetry |
| MCP tool names | `kb_catalog`, `kb_list`, `kb_grep`, `kb_get`, `kb_query`, `kb_links`, `data_tables`, `data_query`, `kb_search` (embed), `kb_write` (write); prefix can be changed |

Crate versions are as checked on 2026-09-30. Use the latest compatible versions when you start, and record them in `Cargo.lock`.

## 5. Do not

1. **Do not put embedding, ONNX or models into the core** or into the default `okbase` build.
2. **Do not write to the bundle** unless the user asked explicitly (`--write`, `--fix-safe`, `write` module).
3. **Do not break round-trip:** parse → write with no changes must produce a byte-identical file.
4. **Do not fuse BM25 with vectors** in ranking (spike S2).
5. **Do not generate full views** (a table of every document by tag); generate only the vocabulary and facets (S5).
6. **Do not decide per-user permissions yourself**; only enforce the `Scope` passed in.
7. **Do not change defaults** (mode, chunk size, tool output, skill content) without an eval that justifies it.
8. Do not commit caches, models, corpora or built bundles (`.gitignore` already covers them).
9. Do not use names or logos that suggest a Google product.

## 6. v0.1 tasks (2-week target), in order

Each task has completion criteria. Work sequentially T0 → T9; T10 and T11 can run in parallel once T5 is done.

**T0 — Set up the OSS repo**
- `git init`, workspace, empty crates: `okbase-core`, `okbase-standard`, `okbase-analyze`, `okbase-index`, `okbase-query`, `okbase-lint`, `okbase-mcp`, `okbase-skills`, `okbase` (facade), `okbase-cli`.
- `LICENSE-MIT`, `LICENSE-APACHE` (official text, verbatim), `CODE_OF_CONDUCT.md` (Contributor Covenant 2.1, verbatim), update `CONTRIBUTING.md`, `SECURITY.md`, `THIRD_PARTY.md`.
- CI (GitHub Actions): fmt, clippy, test on 3 OSes, cargo-deny, MSRV.
- ✅ CI green on the empty workspace; `cargo deny check` passes.

**T1 — `okbase-core`: model and round-trip**
- `Concept { id, path, frontmatter: Frontmatter, body }`. `Frontmatter` keeps the **raw text** and the parsed form (serde-saphyr → ordered `Value`).
- `set(key, value)` / `remove(key)` edit the text in exactly the block of that top-level key (keeping comments, order and formatting of other keys). New keys are appended at the end.
- Validate OKF v0.2 (L0); extract links (plain markdown + wikilinks); ID = path without `.md`.
- ✅ Byte-identical round-trip on the 4 official OKF bundles and the OpenClaw docs fixture; a test that `set` on one key changes only that key's lines; property test with random frontmatter.

**T2 — `okbase-standard`**
- Check levels L0–L2 (design §2); map foreign frontmatter on read (`summary`/`excerpt` → description, `categories` → tags, `sidebar_label` → title); read `_meta/vocabulary.md` (canonical tags + synonyms).
- ✅ Reports the correct level for the fixtures (official OKF bundles ≈ L1; original OpenClaw docs: L0 fails but are readable through mapping).

**T3 — `okbase-analyze`**
- `fold()` (NFKC, diacritic folding, `đ`→`d`, lowercase); Japanese tokenization for FTS (lindera); English stemming; language detection (lingua, vi/en/ja only, hints only).
- ✅ Unit tests for the examples from the spikes (`đổi trả` ~ `doi tra`, `ＡＢＣ` → `abc`, `食べた` → `食べる`).

**T4 — `okbase-index`**
- SQLite schema v1 (design §4.7, except `chunk_vecs`): `docs`, `doc_fields`, `doc_tags`, `aliases`, `links`, `chunks`, `chunks_fts`, `meta(schema_version)`.
- Incremental indexing by hash; `state_dir = auto` (`.okbase/` if the bundle is writable, otherwise `~/.cache/okbase/bundles/<hash>`).
- **Approximate token counter** without a model. From the spikes: en ≈ 1.24 tokens/word, vi ≈ 1.28 tokens/syllable, ja ≈ 0.53 tokens/character. State clearly that this is an estimate.
- ✅ First index of a 1k-document bundle ≤ 5s; editing 1 file reindexes only that file; deleting `.okbase/` and reindexing gives an identical result.

**T5 — `okbase-query`**
- Port `grep` v2, `get`, `list` (index.md, or a virtual catalog if missing), `query` (per `mcp_meta.py`, using the typed tables), `catalog` (≤ N tokens: flat + vocabulary + facets; larger: root index.md + facets), `stats`, `recommend_mode` (Full if ≤ ~30k tokens; otherwise Lexical).
- Every function takes `&Scope`.
- ✅ Output snapshots; performance per design §11 (grep over 4.4M tokens ≤ 500ms, query over 3k documents ≤ 20ms); tests that Scope blocks the right paths and filters.

**T6 — `okbase-lint`**
- Rule engine + L0–L2 rules (design §9); text/JSON/SARIF output; `--fix-safe` (generate index.md, normalize tags).
- ✅ Catches the errors planted in the `fixtures/lint/` fixture.

**T7 — `okbase-mcp` (stdio)**
- rmcp; capability-based tool registry; tool names and descriptions per design §5. Descriptions are "prompts" for agents, so write them carefully, based on the spikes.
- ✅ Works with `claude --mcp-config` and Codex; snapshot of `tools/list`.

**T8 — `okbase-skills` + `agent install`**
- `okbase-answer` skill (templated by capability, with a fallback to the CLI `--json`); `okbase agent install --claude` (register MCP + write the skill into the project or user skill folder), `--codex` (MCP config + `AGENTS.md` snippet), `--print`.
- ✅ Installs on a clean machine; the agent can list the tools and load the skill.

**T9 — `okbase-cli`**
- `status`, `index`, `grep`, `get`, `list`, `query`, `catalog`, `lint`, `mcp serve --stdio`, `agent install`, `modules`; `--json` for every read command; error messages with hints.
- ✅ CLI tests with `assert_cmd` + snapshots.

**T10 — Fixtures**
- `fixtures/okf-official/` (subset of Google's samples, Apache-2.0, with NOTICE).
- `fixtures/openclaw-s/` (about 30 OpenClaw docs files, MIT, with NOTICE).
- `fixtures/multilingual/` (the v2 set from `spikes/embed-bench/data/v2`).
- `fixtures/business/` (×1 from `spikes/biz-meta`).
- `fixtures/lint/` (planted errors).

**T11 — v0.1 acceptance by eval** (uses CLI quota, run by hand)
- Reuse `spikes/okf-scale/run.py` with the G2 configuration, but point MCP at `okbase mcp serve --stdio`, with and without the skill.
- ✅ Bundle L: ≥ 93% correct (equal to the spike's G2). Record the results in `spikes/acceptance-v0.1/`.

## 7. After v0.1

Per design §13:
- v0.2: adopt + data; spikes S8 (adopt) and S9 (skills).
- v0.3: opt-in embed, MCP HTTP, `router()`.
- v0.4: import/sources; spike S10.
- v1.0: extension, stabilization; spike S7 (Codex, small models).

Before wide public release (v0.2), translate the design plan into English (`docs/design.md`). (Done: `docs/design.md` is now the only design document.)

## 8. Downstream user: qobot

qobot (a host application; see its own design notes) uses only the public facade: `Bundle::open`, `capabilities`, `catalog`, `recommend_mode`, `retrieve` (v0.3), `grep/get/query`, `data()`, `write`, `watch`, `okbase_mcp::router(bundle, ScopeProvider)`. Keep these signatures stable, and announce changes in the CHANGELOG.

## 9. Open questions (defaults chosen, can change)

| Question | Default |
|---|---|
| GitHub organization/account to host the repo | None yet. Ask the maintainer before pushing |
| Copyright holder in LICENSE | "The okbase contributors" |
| Names of levels L0–L3, skill names | As in the design; can change before v1.0 |
| Publish to crates.io from v0.1 | Yes (0.x) |
