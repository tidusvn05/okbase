# AGENTS.md — working on okbase

okbase is an open-source (MIT OR Apache-2.0) Rust toolkit that helps AI agents work with markdown knowledge bundles in the Open Knowledge Format (OKF v0.2). It ships a CLI (`okbase`), an MCP server, a Rust library (`okbase::Bundle`), and Agent Skills.

## Name
- The project is **okbase**, short for **Open Knowledge base**. Write it in lowercase (`okbase`), as
  the binary, crates (`okbase`, `okbase-cli`, `okbase-*`), skills (`okbase-answer`, …), MCP server
  name, `okbase.toml`, `.okbase/`, `.okbaseignore` and `OKBASE_*` variables do.
- It was called **okfkit** until 2026-10-03. Do not use the old name in new text; git history keeps
  it, and `cliff.toml` rewrites it in the changelog.
- **OKF** (Open Knowledge Format) is the file format okbase reads, a separate specification by
  Google. Keep that name for the format; okbase is not affiliated with Google.
- Repository: https://github.com/tidusvn05/okbase. Installs come from GitHub releases
  (`install.sh` / `install.ps1`).

## Read first
- `docs/design.md` — design source of truth (section numbers are referenced below); later plans in `docs/plans/`.
- `docs/HANDOFF.md` — the original v0.1 handoff (task order, acceptance criteria, hard rules).
- `docs/releasing.md` — versioning, what is public surface, how to cut a release.
- `spikes/README.md` — the experiments behind every default. Do not change a default without new eval data.
- `docs/runbooks/fix-issues.md` — how to take open GitHub issues, group related ones, fix them and open one pull request per group.

## Product conventions
`docs/conventions/end-user-and-agent-friendly-tools.md` applies to every user-facing change: evidence
before defaults, the simplest-first ladder, agents as first-class users (`onboard`, `doctor`, the machine
contract, exit codes 3/4), the consent catalog, safety and reversibility, the full lifecycle.

## Hard rules
1. Core stays lexical and model-free. Embeddings (ONNX/fastembed) live only in the opt-in `embed-*` modules/features.
2. Read-only by default. Never write to a bundle unless the user passed an explicit write flag or enabled the `write` module.
3. Frontmatter round-trip must be byte-identical when nothing changed; edits touch only the targeted key (keep order, comments, unknown keys).
4. No BM25/vector score fusion in ranking. No full metadata "view" pages (vocabulary + facets only).
5. Every read API takes a `Scope` from the host; okbase never decides user permissions.
6. TLS via rustls only. No telemetry.
7. All text in the repository is in English: README, docs (including `docs/design.md`, `docs/plans/`, spike RESULTS), rustdoc, CLI help, errors, commits. Multilingual test data in `fixtures/` is the exception.

## Conventions
- Rust edition 2024, stable, MSRV 1.89, workspace resolver 3.
- `thiserror` in libraries, `anyhow` only in `okbase-cli`. `#![forbid(unsafe_code)]` unless justified.
- YAML via `serde-saphyr` (never `serde_yaml`/`serde_yml`); SQLite via `rusqlite` (bundled, FTS5); MCP via `rmcp` (pinned minor).
- Every read command supports `--json`; its schema equals the MCP tool output.
- Tool descriptions and skill text are prompts for agents: keep them precise, test them with snapshots, and back changes with eval.
- Conventional commits (`feat:`, `fix:`, `docs:`, …); CHANGELOG is generated from them.

## Verify before finishing a change
```
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo deny check
```
For changes to search/grep/query/catalog/data/MCP tools, also run the fast lexical eval and report the numbers in the PR:
```
cargo run --release -p okbase-eval -- lexical
```
It replays real agents' tool calls on `fixtures/` (seconds, no model) and exits 1 on a regression; `cargo test` runs it too. A change that makes a case pass is an improvement: rebuild the baselines with `python3 fixtures/eval/mine.py`.

## Layout
- `crates/okbase-*` — see `docs/design.md` §4.3 for responsibilities and core vs. module split.
- `fixtures/` — small test bundles with licenses/NOTICE (OKF official samples: Apache-2.0; OpenClaw docs: MIT).
- `spikes/` — experiments; heavy caches and generated bundles are git-ignored.
