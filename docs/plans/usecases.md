# Plan: real-world situations (software repos, empty folders, non-conforming OKF, bundles in subfolders…)

Status: **U1–U8 implemented** (U9 = import, v0.4) (2026-10-02; the 4 questions in §4 settled as proposed). Evidence: running the current build on 4 sample
folders (§1). Related: [`onboarding.md`](onboarding.md), `docs/usage.md`.

## 0. Short conclusion

okbase currently **assumes the given folder is a bundle**. That assumption holds for "clean" knowledge
folders, but is wrong in most real-world situations:
- a software repo with `docs/`;
- an empty folder;
- OKF with only a few broken files;
- a bundle in a subfolder of the repo.

In those cases `onboard`/`advise` give wrong advice, and `adopt` can **mistakenly edit third-party library
files** or **break a docs site**. A **folder-understanding** layer is needed in front of all advice, plus
two safety changes in `adopt` and the file walker.

## 1. Experiments (2026-10-02, current build)

| Sample folder | What okbase currently does | Problem |
|---|---|---|
| **A. Software repo** (`README`, `CHANGELOG`, `src/`, `node_modules/`, `target/`, `mkdocs.yml`, `docs/` with 4 pages) run at the repo root | Treats **the whole repo** as the bundle (6 documents, including `node_modules/leftpad/README.md`); recommends "create an OKF copy of the whole folder" | Does not skip vendor/build folders; does not recognize that the knowledge lives in `docs/` |
| A, `adopt .` (plan only) | Would **edit** `node_modules/leftpad/README.md`, create `node_modules/index.md`, `target/index.md`; **rename `docs/index.md` → `overview.md`** even though it recognized MkDocs | Edits third-party library files; breaks the docs site's home page (MkDocs/Docusaurus use `index.md`) |
| A, `-b docs adopt` | Does not recognize MkDocs (`mkdocs.yml` is in the parent folder) → "Plain"; also renames `index.md` | Only looks for site markers in the folder itself |
| **B. Empty folder** | "0 docs, level **L2**"; recommends installing the agent and asks "May I improve descriptions…" | Meaningless: nothing to install or curate; no entry point for "start a knowledge base" |
| **C. OKF with 2/11 broken files** (1 missing frontmatter, 1 invalid YAML) | Level "below L0" → asks "**the folder is plain markdown**, create an OKF copy?" | Wrong: 9/11 files already conform; only 2 files need fixing in place |
| **D. Repo with `knowledge/` (conforming OKF) for a bot**, run at the root | Treats the repo root as the bundle; `README.md` drags the level down; recommends adopting the whole repo | Does not detect the conforming bundle in `knowledge/` |

## 2. Real-world situations and solutions

### 2.1 Software repo with `docs/`

**Need:** coding agents (Claude Code, Codex) answer from and update the project's docs; the docs must still
render on the site (MkDocs, Docusaurus, Hugo, Mintlify) and live in git alongside the code.

**Proposed solution:**
1. **Do not take the whole repo as the bundle.** Run at the repo root, `onboard` recognizes a software repo (`.git`
   plus `Cargo.toml`/`package.json`/`pyproject.toml`/`go.mod`…) and proposes knowledge folders
   (`docs/`, `doc/`, `documentation/`, `website/docs/`, `wiki/`, OKF subfolders), with document counts and
   levels. With one clear candidate it is used directly (`-b docs`); with several candidates the user is asked.
   MCP is still installed at the **repo root** (project = repo), the bundle is `docs/`.
2. **Keep the original layout**; do not reorganize to the "standard" folder structure. The okbase standard (design §2.1) is only a
   recommendation; the docs site's structure is correct for that project.
3. **Three ways to use it, chosen by who owns the docs:**

   | Approach | When | Effect |
   |---|---|---|
   | **a. Use as is** (default) | Docs already have titles and a clear structure | Nothing is modified; grep/get/list/catalog work on plain markdown; catalog takes the title and first sentence |
   | **b. Add metadata in place, site-compatible** (recommended for projects that want more accurate agents) | Docs lack descriptions; the team reviews via PRs | `adopt --write` in **site mode**: only *adds* `title`/`description`/`type` to frontmatter (MkDocs, Docusaurus and Hugo ignore unknown keys, and `description` is also used for SEO); **does not rename `index.md`**, does not create listing `index.md` files |
   | c. Separate OKF copy (`adopt --out`) | Archiving, or docs you are not allowed to edit | Warns that it will drift from the original |

4. **`docs-site` profile** (`okbase.toml [bundle] profile = "docs-site"`, enabled automatically when a site is detected):
   `index.md` is treated as a content page (the folder's home page); okbase generates folder listings
   at read time (`list`, catalog) without writing files; lint does not require a listing `index.md`. The level is still measured as
   before, to show the metadata level.
5. **Large docs** (several hundred pages or more): `advise` already recommends search and semantic search based on size
   and language; it now applies to the right docs folder instead of the whole repo.
6. **CI:** recommend a step `okbase -b docs lint --level L1 --format sarif` so docs PRs do not break metadata.

### 2.2 Folder with nothing yet (starting a new knowledge base)

**Solution:** `onboard` recognizes an empty folder (or one with no `.md`) and switches to an **initialization** flow:
1. **Ask:** what the knowledge base is about, who uses it (people or a bot), which languages questions come in; whether existing
   documents are available somewhere (PDF, DOCX, wiki exports).
2. **`okbase init`** (new) creates a skeleton:
   - `index.md` with `okf_version`;
   - `_meta/vocabulary.md` (tags by domain);
   - `_meta/types/` for a few common document types (Policy, Guide, FAQ, Reference) or based on the answers in step 1;
   - `okbase.toml`.
3. **Add content:**
   - `okbase new --type Guide "Title"` (new; already planned in design §6) creates a document with frontmatter matching the schema;
   - the **`okbase-author`** skill lets the agent write new L2-conforming documents;
   - existing PDF/DOCX documents use **import (v0.4)**. Before v0.4, `onboard` says so clearly and suggests a temporary conversion with external tools.
4. Agent installation and the other steps come only once there is content.

### 2.3 OKF exists but does not conform

**Solution:** classify by the **share of conforming documents**, not only by the whole bundle's level (the level is the "weakest link"):

| Condition | Recommendation |
|---|---|
| ≥ 80% of files reach L0 | "N files need fixing": list them; `adopt --write --only <files missing frontmatter>` to fill in what is missing in place; for broken YAML the agent fixes it by hand at the error location lint reports; then curate up to L2 |
| 20–80% | As above, plus ask the user whether to normalize the whole folder in place (`adopt --write`, reviewed with git diff) |
| < 20% | Treat as plain markdown (the current adopt flow) |

In addition, `lint --fix-safe` already creates missing `index.md` files and fixes synonym tags; the plan should state this explicitly.

### 2.4 Repo with an OKF subfolder as knowledge for a bot

**Need:** an application (a customer-support bot) reads `knowledge/`; developers use agents to answer
questions and update `knowledge/`; CI maintains quality.

**Solution:**
1. **Detection:** when scanning the repo root, a subfolder whose `index.md` contains `okf_version`, or where most files have
   `type`, is recognized as a bundle → `-b knowledge`. The repo root is not a bundle.
2. **Developers' agents:** `okbase -b knowledge agent install` (project = repo root). The skills
   `okbase-curate` / `okbase-author` help update the knowledge via PRs.
3. **The bot at runtime** (`advise --for host` already exists; needs a concrete recipe per bot language):
   - Rust: the `okbase::Bundle` library with a per-user `Scope`;
   - other languages (Python, Node…): `okbase mcp serve --http` with a token (MCP clients exist for common languages), or call the CLI with `--json`;
   - the catalog goes in the bot's system prompt (S3).
4. **CI:** lint `knowledge/` on every PR (SARIF); optionally run `embed tune eval` periodically if tuned.

### 2.5 Other situations to support

| Situation | Current state | Proposal |
|---|---|---|
| **Monorepo** with several doc sets (`services/*/docs`) | Must run each set manually | The scanner lists all candidates; install each set with its own `--name` (already supported), or merge them into one bundle with `--allow` |
| Personal **Obsidian vault** | `adopt` recognizes Obsidian; creating `index.md` in every folder clutters the vault | `vault` profile: do not create `index.md`, keep wikilinks; recommend using as is plus metadata in place; no fine-tuning if private |
| **Exports from Confluence / Notion / Google Docs** (HTML, PDF, DOCX) | Cannot be read (`.md` only) | Import in v0.4; before that, `onboard` reports the number of non-markdown files and how to convert them temporarily |
| **Read-only shared drive** (NAS) | The index moves to the cache automatically (already exists) | Serve the team with `mcp serve --http` |
| **Sensitive documents** | `advise` has `--private` | The scanner asks about sensitivity; when private: local models only, no fine-tuning with a cloud LLM |
| **Folder with spreadsheets** | The data module is enabled automatically (already exists) | Keep as is |
| **Very large** (> 10k documents) | `advise` recommends embeddings based on size | Measure further (ANN/MRL in v0.5+) |

## 3. Features to build

| # | Feature | Priority | Why |
|---|---|---|---|
| U1 | **Ignore rules for the file walker:** respect `.gitignore` (the `ignore` library, as in ripgrep), `.okbaseignore`, and a default list (`node_modules`, `target`, `vendor`, `dist`, `build`, `site`, `_build`, `.venv`, `__pycache__`) | **P0** (safety) | It can currently index and **edit** third-party library files |
| U2 | **`adopt` safe for docs sites:** look for site markers from the parent folders up to the repo root; for MkDocs/Docusaurus/Hugo/Mintlify/Obsidian do not rename `index.md`, do not create listing `index.md` files; only add frontmatter | **P0** | It currently breaks the site's home page |
| U3 | **Folder scanner** (`okbase scan`, also the first step of `onboard`): classify the folder (empty / software repo / docs site / OKF / partial OKF / plain markdown / non-markdown files) and list bundle candidates with document count, level, conformance share | **P1** | Foundation for all correct advice |
| U4 | **`onboard` and `advise` by classification:** choose or ask for the bundle; initialization flow for empty folders; "fix N files" advice for partial OKF; bot recipe (`--for host`) | **P1** | Replaces the wrong advice in §1 |
| U5 | **`docs-site` / `vault` profiles** in `okbase.toml`: `index.md` is content, folder listings generated at read time, matching lint | **P1** | Use in place without conflicting with the site |
| U6 | **`adopt --only <glob>`** (fix exactly the files that are missing things) and report the conformance share | P1 | Partial OKF |
| U7 | **`okbase init`**, **`okbase new --type`**, skill **`okbase-author`** | P2 | Start a new base; add conforming documents |
| U8 | **CI recipe** (GitHub Actions: lint SARIF) and **bot recipe** (Python/Node over HTTP MCP) in `docs/usage.md` | P2 | Repos with a bot |
| U9 | **Import PDF/DOCX/HTML** | v0.4 (already on the roadmap) | Wiki exports, office documents |

**Verification:** turn the 4 sample folders in §1 (plus a monorepo, an Obsidian vault and a folder with PDFs) into
**scenario fixtures**. Tests assert that `onboard` gives the right candidates and the right questions, and that
`adopt --write` does not touch ignored files or the site's `index.md` pages. Then rerun S13
with agents on these folders.

## 4. Questions to settle

1. Is the default for software repo docs **(a) use as is**, with the suggestion **(b) add metadata
   in place** via PR? (proposal: yes; do not propose an `--out` copy for actively maintained docs)
2. The `docs-site` profile lets `index.md` be a content page, which deviates from OKF §3.1 (`index.md` is reserved
   for listings). Accept it as an okbase profile, documented clearly? (proposal: yes; pure OKF bundles
   are not affected)
3. Respect `.gitignore` by default? (proposal: yes; `.okbaseignore` to add or remove)
4. Order of work: U1 + U2 (safety) immediately, then U3–U6, then U7–U8, with U9 following v0.4? (proposal: yes)

## 5. Implementation (2026-10-02)

| # | Commit | Notes |
|---|---|---|
| U1 | `11bc53a` | `.gitignore` (including parent folders), `.okbaseignore`, skipping dependency folders. The default list is kept narrow (`node_modules`, `__pycache__`…); `build/`, `vendor/`, `target/` are left to `.gitignore`, since they may be real knowledge folders |
| U2, U5, U6 | `7ac51a3` | `docs-site`/`vault` profiles (detected also in parent folders up to the repo root; overridable in `okbase.toml`); `adopt` keeps `index.md`, does not create listings or logs; `adopt --only` |
| U3, U4 | `1db3ab0` | `okbase scan`; `onboard` chooses or asks for the bundle, with flows for empty folders, PDFs, "fix N files" and site metadata; `onboard` commands keep `-b` |
| U7 | `1d129c6` | `okbase init`, `okbase new`, skill `okbase-author` |
| U8 | `7626a83` | `docs/usage.md`: real-world folders, profiles, ignore rules, CI, bot |

Verified by a scenario test (`onboard_understands_real_folders`): a repo with MkDocs and `node_modules`, an empty folder, OKF with 1 broken file, a bot repo with `knowledge/`.
