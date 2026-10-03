# Spikes

The experiments and benchmarks behind every okbase default. Do not change a default without new
data. Each spike is a directory whose `RESULTS.md` is the source of truth. This file is the index and
a quick-reference table of numbers.

Early spikes ran inside qobot, the host application okbase was first built for; their records mention it.

## 1. Index

IDs follow `docs/design.md` §1. The date is the run date.

| ID | Date | Directory | Question | Main conclusion | Decision recorded in |
|---|---|---|---|---|---|
| S1 | 2026-09-30 | `embed-bench` rounds 1–2 | Which embedding model for vi/en/ja? | **EmbeddingGemma-300M Q4** (188 MB, ~0.5 GB RAM, R@1 0.85 / R@3 0.96). Fallback: bge-m3 int8 (MIT). The e5 family is weak on cross-language questions; Qwen3 and nomic are too slow on CPU | design §1, §5 |
| S2 | 2026-09-30 | `embed-bench` round 3 | BM25 threshold? Chunks per file? | **Do not mix BM25 into ranking** (gain 0–1 points). Short OKF concepts ≈ 1.4–2.2 chunks/file; long docs ≈ 10 chunks/file | design §1; AGENTS.md (rule 4) |
| S3 | 2026-09-30 | `embed-bench` rounds 4/4b | Catalog or tools? (100 short documents) | Same accuracy (~97%). **The catalog must be in the system prompt** (otherwise cost ×2.5). For a small bundle, loading the whole bundle is fastest and cheapest | design §1 |
| S4 | 2026-09-30 | `okf-scale` rounds 1–2 | Large bundle (OpenClaw docs, 60k → 4.4M tokens): which approach? | **G2 lexical (no embeddings) 100/100/93/90%**, on par with embeddings (D) and agent + grep (E). Vietnamese questions 85 → 95% thanks to a stronger `kb_grep`. Telling the agent to "double-check" via the prompt is **not effective** | design §1, §5 |
| S5 | 2026-09-30 | `biz-meta` | Metadata/tags and sheets: are tools needed? (151 → 3,020 documents) | ×1: the agent grepping frontmatter is enough. ×20: **`data_query` is required** (sheets 10/10, 9× cheaper); `kb_query` is 30–45% cheaper. View files do not scale | design §1, §5 |
| S6 | 2026-09-30 | `embed-bench` | Chunking and indexing speed | ~3 chunks/s on 8 CPUs; 4.4M tokens ≈ 64 minutes. Embedding runs in the background; lexical is ready immediately | design §1, §11 |
| v0.1 | 2026-10-01 | `acceptance-v0.1` | Does the real okbase reproduce S4's G2? | **28/30 (93%)**, target met. The skill does not raise accuracy on a lookup bundle; cost +60% | HANDOFF T11 |
| S8, S9 (v0.2) | 2026-10-01 | `acceptance-v0.2` | S5 ×20 with okbase; does adopt reduce accuracy; does the skill help | K 44/48, KS 45/48; sheets 9–10/10. Adopt does not reduce accuracy. **S9 inconclusive**: the agent never invoked the skill | design §13 |
| v0.3 | 2026-10-01 | `acceptance-v0.3` | Retrieval through okbase's real code path | S1 R@1 **0.857** (Gemma Q4), 0.750 (bge-m3); S4 top-6 29/30 and 30/30. Fixed the token estimate (S4 26 → 29/30) | design §13 |
| S10-lite | 2026-10-02 | `import-bench` | Lightweight and accurate PDF/Office/HTML import? | **anydoc + htmd**: accurate for Office and text PDFs (vi/ja); binary +~10 MB; 0.3–10 ms per small document. Still weak: two columns in mid-page, HTML tables without `<th>`, scanned pages | `docs/plans/import.md` |
| I5 | 2026-10-02 | `import-bench` | Real agent on a mixed PDF/DOCX/HTML folder | Answered 3/3, citing file and page. Asked for consent before OCR; added a consent flag on its own 0 times | `docs/plans/import.md` |
| S11 | 2026-10-01 → 02 | `embed-tune` | Does fine-tuning EmbeddingGemma (LoRA, the Unsloth approach) help? | fp32: R@1 0.853 → 0.943. **Q4 in okbase: 0.857 → 0.917**, S4 30/30. The agent + CLI workflow works (held-out 0.875 → 0.925) | `docs/plans/advise-tune.md` |
| S13 | 2026-10-02 | `onboarding` | Can an agent install okbase on its own from one sentence? | Claude 3/3 completed (35–51 s, ~50k tokens); added a consent flag on its own 0 times. Codex: see S13 (Codex) | `docs/plans/onboarding.md` |
| S13b | 2026-10-02 | `onboarding` | Real-world folders: software repo, empty folder, partially broken bundle | 3/3 understood the folder correctly and asked at the right points. The agent found 5 bugs, now fixed | `docs/plans/usecases.md` |
| S14 | 2026-10-02 | `page-image-bench` | Giving scanned pages to an agent: extract embedded images or render pages? | **Extract embedded images**: JPEG 1–3 ms (render 0.2–0.95 s); Flate is lossless and 2–4× faster than rendering. Same image tokens (~1.5k). CCITT G4: 86 ms, accurate (crate `fax`) | `docs/plans/import.md` §7 |
| S15 | 2026-10-03 | `mcp-tools` | Does trimming the tool list reduce cost? Can skill rules sent through the MCP server instructions replace hints in the prompt? | **Instructions can replace hints**: without hints still 46/48 (same as with hints), `kb_query`/`data_query` used equally. This resolves S9. Trimming tools: −3% tokens, cost unchanged, accuracy unchanged. $6.29 | `design.md` §13–14 |
| S7 | 2026-10-03 | `codex` | Lexical with Codex and small models | **gpt-6.1-sol 26/30 (87%)**, Claude Sonnet 28/30, no meaningful difference. **gpt-6-luna (small model) 21/30**: finds the right documents but answers are missing points; adding `kb_search` does not help (still 21/30), only cuts 24% of tokens. Keep lexical as the default for Codex too | `design.md` §13–14 |
| S13 (Codex) | 2026-10-03 | `codex` | Codex installs okbase on its own from one sentence | 12/12 runs found `onboard`, 0 times added the consent flag on its own, `doctor` passed 11/12. The Codex sandbox blocks writes to `.codex/`; fixed (`sandbox_blocked` and a step for the user in `onboard`): the rerun hands the user the exact command, 61–72 s per setup | `plans/onboarding.md` |

## 2. Quick-reference numbers

Machine: AMD EPYC 8 vCPU, 23 GB RAM, no GPU; release build. For details and measurement conditions
see the "Source" column.

| Item | Numbers | Source |
|---|---|---|
| Retrieval quality (S1, 300 vi/en/ja questions) | Gemma Q4 R@1 0.857; tuned Q4 0.917; bge-m3 int8 0.750 | `acceptance-v0.3`, `embed-tune` |
| Large bundle, lexical (S4, 30 questions) | 93% (28/30) with Claude Sonnet, ~$0.029/question, 4.6 turns | `acceptance-v0.1` |
| Embedding a whole bundle | ~3 chunks/s (8 CPUs); 4.4M tokens ≈ 64 minutes | `embed-bench` |
| Fine-tune (CPU) | 320 pairs × 2 epochs: 627 s; export 20 s; eval 70 s; venv 1.7 GB | `embed-tune` |
| Document conversion | 0.3–10 ms/small document; 172 ms for a 15-page paper | `import-bench` |
| Extracting scanned-page images | JPEG 1–3 ms; Flate 50–90 ms (A4 300 dpi); PDFium render 150 dpi ~0.2 s, 300 dpi ~0.6–0.95 s | `page-image-bench` |
| Binary size | anydoc +~10 MB; htmd +1.5 MB; page image +0 crates | `import-bench`, `page-image-bench` |
| Cost per question (biz ×20, Claude Sonnet, Claude Code 2.1.284) | ~$0.043; ~34k input tokens, 4.1–4.2 turns | `mcp-tools` |
| Agent self-install | 35–51 s, 51–56k tokens per run (Claude) | `onboarding` |
| Cost of generating questions for tuning | ~$3 for ~1k pairs (Claude Sonnet) | `embed-tune` |

## 3. Not done yet (needs user consent or data)

| ID | Task | Why not yet |
|---|---|---|
| S10 | ~20 real PDFs and 5 real sheets | Needs the user's real documents |
| S12 | Codex generating questions for tuning | Uses quota |
| S14 follow-up | Pages composed of several images; real scanned files | Not started (CCITT G4 is done) |

## 4. Conventions for new spikes

- **Directory:** `spikes/<name>/`.
  - `RESULTS.md`: question, setup (machine, versions, samples), results, findings, what was not
    measured, how to rerun.
  - Scripts to build the data and run.
  - `results/`: small machine-readable results (`.json`, `.jsonl.gz`).
- **Do not commit:** rebuildable data, models, caches, generated files. List them in the spike
  directory's or the repo's `.gitignore`.
- **Language:** all spike documents are in English.
- **Index:** add a row to the table in §1. If there are measurements of time, size or cost, add them
  to §2.
- **Decisions:** record the conclusion in the corresponding plan in `docs/`, with the spike ID.
- **Cost:** record the amount spent and the number of runs when a spike uses a real agent. Do not run
  agents that use quota without the user's consent.

## 5. What is not stored (rebuildable)

- Models and caches: `.fastembed_cache`, `.models`, `.embcache`, `embed-tune/models`.
- Corpora: `.corpora`, `.okf-samples`.
- OKF bundles and indexes: `okf-scale/bundles`.
- Document samples: `import-bench/samples`, `page-image-bench/samples`.
- Per-run prompts, the `work/` and `runs/` directories.

The spikes were moved here from the qobot repo on 2026-10-01; older records in `results/` may still
contain `qobot/spikes/...` paths.

## 6. Notes on data (open source)

- The business bundle (`biz-meta`), the multilingual question set (`embed-bench/data`) and the
  document samples (`import-bench`, `page-image-bench`) are **synthetic data**, written by an LLM or
  generated from templates. They are not real data from any company.
- The questions in `okf-scale/questions.json` were generated by an LLM from the OpenClaw docs (MIT).
  The source corpus is not committed; `okf-scale/build_bundles.py` downloads and rebuilds it.
- Google's OKF samples (Apache-2.0) are cloned into `.okf-samples/` at run time and not committed. The
  arXiv paper 1706.03762 in `import-bench` is downloaded at run time and not committed.
- `results/*.jsonl.gz` contain paths on the machine that ran the spike and LLM-generated answers; use
  them for analysis only.
