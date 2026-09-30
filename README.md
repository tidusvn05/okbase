# okfkit

**Make markdown knowledge bases work well for AI agents.**

okfkit is an open-source Rust toolkit for knowledge bundles in the [Open Knowledge Format (OKF)](https://github.com/GoogleCloudPlatform/open-knowledge-format). It helps agents such as Claude Code, Codex, OpenCode, or any MCP client answer questions from a folder of markdown files quickly, cheaply and correctly, in Vietnamese, English and Japanese.

> Status: **design complete, implementation starting** (v0.1 in progress). okfkit is an independent community project and is not affiliated with Google.

## What it does

- **Works on existing OKF bundles** with zero configuration and read-only by default.
- **Adopts plain markdown folders** (docs sites, wikis, Obsidian vaults) into OKF: frontmatter, descriptions, `index.md`, tags. You can preview the diff before anything is written.
- **Agent tools** via MCP and CLI:
  - multilingual, accent-insensitive `grep`;
  - metadata `query` with filters, facets and sums;
  - section-level `get` and `list`;
  - a prompt-ready `catalog`;
  - read-only SQL over imported spreadsheets.
- **Agent Skills** that teach agents the most effective way to use the bundle. Install them with `okfkit agent install --claude` or `--codex`.
- **Lint** against okfkit's quality levels L0–L3, from OKF-valid up to curated.
- **Opt-in modules:**
  - multilingual semantic search (local EmbeddingGemma / bge-m3, or an API);
  - PDF/DOCX/HTML import;
  - Google Drive/Sheets/Notion sync;
  - write tools;
  - evaluation.

The defaults are backed by experiments (see [`spikes/`](spikes/README.md)). One example: lexical navigation with a strong `grep` matched embedding-based retrieval on bundles from 60k to 4.4M tokens, so embeddings are optional.

## Documentation
- Design and roadmap: [`docs/PLAN.md`](docs/PLAN.md) (Vietnamese; an English design doc will follow)
- Implementation handoff: [`docs/HANDOFF.md`](docs/HANDOFF.md)
- Contributing: [`CONTRIBUTING.md`](CONTRIBUTING.md) · Agents: [`AGENTS.md`](AGENTS.md)

## License

Dual-licensed under either of MIT or Apache-2.0, at your option.
