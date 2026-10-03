# Changelog

All notable changes to okbase are listed here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and okbase uses
[Semantic Versioning](https://semver.org/) (see docs/releasing.md for what counts as a
breaking change). This file is generated from commit messages by git-cliff.

## [Unreleased]

### Features

- **core:** concept model, frontmatter round-trip and OKF validation (a6aa899)
- **standard:** levels L0-L2, foreign field mapping and tag vocabulary (ac7de54)
- **analyze:** folding, Japanese tokenization, stemming and language hints (102a3b6)
- **index:** incremental SQLite index with chunks and full-text search (e256e75)
- **query:** grep, get, list, query, catalog, links, stats with Scope (889680d)
- **lint:** rule engine, standard rules, SARIF output and --fix-safe (7ba9e02)
- **mcp:** stdio MCP server with capability-based tools; okbase facade (b13110a)
- **skills:** okbase-answer skill and agent install for Claude Code and Codex (aa7e99e)
- **cli:** okbase command-line interface (12ccafa)
- **data:** datasets module — CSV/TSV/XLSX as read-only SQL (242e5f0)
- **adopt:** turn plain markdown folders into OKF bundles (ad1d14e)
- **skills:** okbase-curate and okbase-adopt skills; unreviewed-generated lint (11948c3)
- **vocab:** okbase vocab — tag usage and vocabulary suggestion (48160be)
- **analyze:** download the Japanese dictionary on first use (3fb59c0)
- **embed:** opt-in embeddings crate — local ONNX models and API (b82ff0f)
- **search:** semantic search and retrieval behind the embed module (8876988)
- **mcp:** streamable HTTP transport and router() for hosts (21ffbdb)
- **embed:** EmbeddingGemma fp32 option, 2048-token inputs, license accepted once (59419e6)
- **cli:** okbase advise recommends a setup per bundle (d4a52e3)
- **embed:** custom ONNX models (custom:<name>) (8a18314)
- **tune:** agent-written questions for fine-tuning, embed eval (9261958)
- **tune:** train, export, eval gate, activate and rollback (6ce5ed4)
- **agent:** many bundles, shared servers, uninstall, status, clean (a7f7dab)
- **cli:** machine contract, okbase onboard, help --agent (903a1f5)
- **cli:** okbase doctor (ee9d65c)
- agent bootstrap (README for agents, llms.txt, okbase-setup skill) (6167059)
- **core:** respect .gitignore and skip dependency folders (U1) (11bc53a)
- docs-site and vault profiles; adopt keeps sites working (U2, U5, U6) (7ac51a3)
- okbase scan; onboard plans for the folder it is in (U3, U4) (1db3ab0)
- okbase init, okbase new and the okbase-author skill (U7) (1d129c6)
- **convert:** okbase-convert, a controlled wrapper over anydoc, pdf-inspector and htmd (I1) (90f8603)
- **index:** read PDF, Office and HTML documents directly (I2) (3bf8b0d)
- okbase import (plan, write, status, OCR by agents) (I3) (f9e9979)
- source documents in scan, onboard, doctor; okbase-import skill (I4) (7354629)
- **import:** ocr-next hands agents the scanned page image (0ea2323)
- **eval:** okbase-eval lexical replays real agents' tool calls (a3f4ff5)
- **convert:** export CCITT Group 4 scans as PNG (565455d)
- **mcp:** answer rules as server instructions; smaller tool list (54c79a7)

### Bug fixes

- **analyze:** calibrated token estimate; v0.3 retrieval evals (ea67a4b)
- **tune:** Vietnamese syllables, exact rollback, tune setup (d2b3b41)
- **mcp:** see edits during a session; Codex installs per project (c07acae)
- findings of the agent onboarding spike (S13) (be73fa6)
- findings of the real-folder agent runs (S13b) (844bc30)
- **mcp:** kb_query reads <field>_from/<field>_to as a range (21e45bc)
- **eval:** inherit okbase-mcp features from the workspace (471c898)
- findings of the readiness audit and the Codex spikes (b98cebf)
- keep LF line endings on checkout (Windows) (a463959)
- **cli:** run on a 16 MB stack (Windows main thread overflowed) (c99c73c)
- **skills:** compare Codex project paths loosely; Windows-safe test (cb2b76e)

### Refactoring

- [**breaking**] rename the project to okbase (Open Knowledge base) (f64eda3)

### Documentation

- **eval:** v0.1 acceptance results — 93% on bundle L (bcc0c70)
- English design document (translation of PLAN.md v2.1) (1c37792)
- **eval:** v0.2 eval results — S5 x20, S9, S8 (2704967)
- **spikes:** S11 EmbeddingGemma LoRA fine-tune results (0112c6d)
- S11 pipeline, retrain policy and trade-offs (8252022)
- plan for okbase advise and embedding fine-tuning (3fd2920)
- **spikes:** S11 phase 0, tuned model as ONNX Q4 through okbase (562cc27)
- usage for small, medium and large projects (e861399)
- plan for agent-driven onboarding (56be651)
- plan for real-world folders (software repos, empty, partial OKF, nested bundles) (76a354f)
- conventions for end-user and agent-friendly tools (ce24bf9)
- usage for real folders, profiles, ignore rules, CI and bots (U8) (7626a83)
- **spikes:** S10-lite, light document import with anydoc and htmd (0434d20)
- plan for document import (anydoc + htmd, direct reading, agent OCR) (85411e1)
- **spikes:** I5, agents on a folder of source documents (4ba28cb)
- **spikes:** S14 page-image benchmark; spike index and quick numbers (992b919)
- roadmap status, handoff status and commit references (394ed94)
- cite the CCITT G4 commit (bedbc52)
- default binary size target is 100 MB (a47e890)
- **spikes:** S7 and S13 with Codex (9fd98d3)
- privacy, troubleshooting, FAQ, environment variables; fix stale facts (0c268fa)
- **spikes:** S13 with Codex after the sandbox fix (548b958)
- all documentation in English (708751d)

### Build and packaging

- okbase-full no longer embeds the Japanese dictionary (cedaf9c)

### Continuous integration

- release workflow, curl installer, changelog and OSS templates (1b27525)
- newer release actions; dependabot ignores the MSRV toolchain and rcdom (4a0b1e4)

### Dependencies

- **deps:** raise minimums to the latest releases; accurate converter names (11af680)

### Other

- bootstrap workspace, licenses and CI (5fc83bc)
- add OKF sample and OpenClaw docs fixtures (4187c14)
- add synthetic business fixture from the biz-meta spike (8be25a5)
- add multilingual fixture from the embed-bench spike (e98647c)
- **eval:** v0.1 acceptance harness; SQLite busy timeout (6499968)
- ignore Python caches (37f287b)
- **eval:** v0.2 eval harness (S5 x20, S9 skill, S8 adopt) (38dc7c1)
- okbase-full in CI, build features in modules/dict status, README (45bcfae)
- **deny:** record ttf-parser (unmaintained) via pdf-inspector (d0eba82)
- raise MSRV to 1.89 for serde-saphyr 1.3 (376c416)
- **tune:** pin peft 0.21.2 (2795a79)
- **cli:** give each test its own install registry (Windows) (8f9b9c7)
- **tune:** platform-neutral path check; CI shows every failing test (3fb555e)


