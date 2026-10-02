# AGENTS.md — working on okfkit

okfkit is an open-source (MIT OR Apache-2.0) Rust toolkit that helps AI agents work with markdown knowledge bundles in the Open Knowledge Format (OKF v0.2). It ships a CLI (`okfkit`), an MCP server, a Rust library (`okfkit::Bundle`), and Agent Skills.

## Read first
- `docs/PLAN.md` — design source of truth (written in Vietnamese; section numbers are referenced below).
- `docs/HANDOFF.md` — task order for v0.1, acceptance criteria, reusable spike code, hard rules.
- `spikes/README.md` — the experiments behind every default. Do not change a default without new eval data.

## Product conventions
`docs/conventions/end-user-and-agent-friendly-tools.md` applies to every user-facing change: evidence
before defaults, the simplest-first ladder, agents as first-class users (`onboard`, `doctor`, the machine
contract, exit codes 3/4), the consent catalog, safety and reversibility, the full lifecycle.

## Hard rules
1. Core stays lexical and model-free. Embeddings (ONNX/fastembed) live only in the opt-in `embed-*` modules/features.
2. Read-only by default. Never write to a bundle unless the user passed an explicit write flag or enabled the `write` module.
3. Frontmatter round-trip must be byte-identical when nothing changed; edits touch only the targeted key (keep order, comments, unknown keys).
4. No BM25/vector score fusion in ranking. No full metadata "view" pages (vocabulary + facets only).
5. Every read API takes a `Scope` from the host; okfkit never decides user permissions.
6. TLS via rustls only. No telemetry.
7. Public-facing text (README, rustdoc, CLI help, errors, commits) in English.

## Conventions
- Rust edition 2024, stable, MSRV 1.89, workspace resolver 3.
- `thiserror` in libraries, `anyhow` only in `okfkit-cli`. `#![forbid(unsafe_code)]` unless justified.
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
cargo run --release -p okfkit-eval -- lexical
```
It replays real agents' tool calls on `fixtures/` (seconds, no model) and exits 1 on a regression; `cargo test` runs it too. A change that makes a case pass is an improvement: rebuild the baselines with `python3 fixtures/eval/mine.py`.

## Layout
- `crates/okfkit-*` — see `docs/PLAN.md` §4.3 for responsibilities and core vs. module split.
- `fixtures/` — small test bundles with licenses/NOTICE (OKF official samples: Apache-2.0; OpenClaw docs: MIT).
- `spikes/` — experiments; heavy caches and generated bundles are git-ignored.
