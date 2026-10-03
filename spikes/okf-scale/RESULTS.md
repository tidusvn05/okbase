# Spike: large OKF bundle (OpenClaw docs) — comparing ways for an agent to use knowledge

Data saved in `results/`:
- `runs.jsonl.gz`: 780 records, with answers, sources, tool calls, usage and cost
- `judge.json`: scores
- `report.md`: results tables and the list of wrong answers
- `summary.json`
- `*.retrieval.json`: top-6 chunks for each question
- `gold.json`, `stats.json`, and logs

Bundles, indexes and the embedding cache are **not stored**; rebuild them with `build_bundles.py` and `embed-bench bundle-index`.

Run date: 2026-09-30. Claude Code 2.1.284, `--model sonnet`, 8 vCPU. Total cost: $18.29 for 780 runs, $3.75 to generate questions, and about $4 for judging.

## Setup

**Bundles.** OpenClaw docs converted to OKF (`build_bundles.py`): `summary` becomes `description`, `type` is inferred from the directory, unknown keys are kept, and **every directory has an `index.md`**. The bundles are nested: S ⊂ M ⊂ L ⊂ XL.

| Bundle | Files | Tokens | Chunks (H2/H3, 150–450 tok) | Flat catalog |
|---|---|---|---|---|
| S | 28 | ~60k | 201 | ~0.9k tok |
| M | 62 | ~150k | 510 | ~2k tok |
| L | 287 | ~1M | 3.171 | ~9k tok |
| XL | 1.270 | ~4.4M | 12.517 | ~44k tok (not put in the prompt; XL uses only the root `index.md`) |

**Questions** (`gen_questions.py`): 30 questions (vi 10, en 10, ja 10) generated from 15 "gold" documents present in every bundle. Each question targets a specific detail (default value, config key, CLI flag, limit) and comes with 1–3 **key facts**.
- The docs are written in English, so vi/ja questions are cross-language cases.
- Most Vietnamese questions are without diacritics.

**Scoring:** LLM judge (sonnet), blind to the configuration. A question counts as **correct** only when the answer contains every key fact and does not contradict the documentation. This is a strict criterion.

**Approaches compared** (`run.py`):

| | Approach | Knowledge in the prompt | Tools |
|---|---|---|---|
| F | Full-context | The whole bundle (system prompt, cached) | None |
| D | qobot | Top-6 chunks (EmbeddingGemma Q4) + catalog in the system prompt | MCP: kb_search / kb_get(section) / kb_grep / kb_list |
| I | qobot without catalog | Top-6 chunks | MCP (as D) |
| A | Tools only | — | MCP (as D) |
| E | Agent + directory | — | The CLI's built-in Read/Grep/Glob (cwd = bundle) |
| H | Plain RAG | Top-6 chunks | None |
| G | index.md tree walk | Root `index.md` | MCP kb_get / kb_grep / kb_list (**no embeddings**) |

## Results

| Bundle | Approach | Correct | Facts | p50 / p90 | Turns | Tool calls | Input tokens | Cost |
|---|---|---|---|---|---|---|---|---|
| S 60k | F full | **100%** | 100% | 4.6 / 6.6s | 2.0 | 0 | 89k | $0.034 |
| | D qobot | 97% | 99% | 6.7 / 8.8s | 2.2 | 0.1 | 10k | $0.020 |
| | I | 100% | 100% | 6.8 / 8.8s | 2.3 | 0.3 | 9k | $0.020 |
| | A tools | 100% | 100% | 10.2 / 11.3s | 5.5 | 3.5 | 17k | $0.021 |
| | E folder | 100% | 100% | 7.1 / 10.1s | 4.1 | 2.1 | 18k | $0.019 |
| | H RAG | 97% | 97% | **4.3 / 5.0s** | 2.0 | 0 | 6k | $0.018 |
| | G tree | 97% | 98% | 7.0 / 9.2s | 4.9 | 2.9 | 16k | $0.019 |
| M 150k | F full | 100% | 100% | 4.6 / 11.7s | 2.0 | 0 | **220k** | **$0.104** |
| | D qobot | 100% | 98% | 6.8 / 9.5s | 2.1 | 0.1 | 11k | $0.018 |
| | I | 100% | 100% | 6.8 / 9.3s | 2.1 | 0.1 | 8k | $0.016 |
| | A tools | 97% | 98% | 10.2 / 13.1s | 5.5 | 3.5 | 18k | $0.022 |
| | E folder | 100% | 100% | 7.4 / 9.8s | 4.2 | 2.2 | 18k | $0.018 |
| | H RAG | 97% | 95% | 4.2 / 5.4s | 2.0 | 0 | 6k | $0.016 |
| | G tree | 93% | 97% | 6.9 / 8.7s | 5.0 | 3.0 | 16k | $0.019 |
| L 1M | D qobot | **100%** | 99% | 7.1 / 16.2s | 2.2 | 0.2 | 24k | $0.023 |
| | I | 93% | 97% | 7.1 / 14.9s | 2.2 | 0.2 | 8k | $0.018 |
| | A tools | 90% | 96% | 10.3 / 19.4s | 5.1 | 3.1 | 17k | $0.019 |
| | E folder | 97% | 99% | 6.9 / 8.5s | 4.4 | 2.4 | 19k | $0.022 |
| | H RAG | 97% | 97% | 4.7 / 7.8s | 2.0 | 0 | 6k | $0.017 |
| | G tree | 90% | 96% | 7.2 / 9.4s | 4.8 | 2.8 | 18k | $0.024 |
| XL 4.4M | D qobot | 93% | 97% | 6.9 / 9.2s | 2.0 | **0.0** | 8k | $0.018 |
| | I | 93% | 98% | 6.9 / 8.8s | 2.0 | 0.0 | 7k | $0.016 |
| | A tools | 90% | 94% | 10.2 / 12.8s | 5.1 | 3.1 | 17k | $0.021 |
| | E folder | 93% | 98% | 7.0 / 9.9s | 4.7 | 2.7 | 22k | $0.026 |
| | H RAG | 90% | 94% | 4.7 / 5.4s | 2.0 | 0 | 6k | $0.016 |
| | G tree | 90% | 95% | 7.1 / 9.0s | 4.6 | 2.6 | 21k | $0.028 |

**By question language (all bundles combined):**

| Approach | vi | en | ja |
|---|---|---|---|
| F | 100% | 100% | 100% |
| D | 98% | 95% | 100% |
| I | 98% | 92% | 100% |
| A | 88% | 95% | 100% |
| E | 100% | 92% | 100% |
| H | 98% | 88% | 100% |
| G | 85% | 92% | 100% |

**Retrieval only** (EmbeddingGemma Q4, gold document within the top-6 chunks):
- S 30/30, M 30/30, L 30/30, XL 26/30;
- gold document ranked first: 30 / 28 / 23 / 22.

**Index:**
- Indexing speed: XL's 12.5k chunks took **64 minutes** (3.2 chunks/s, average chunk about 300 tokens, with the CPU shared with other jobs).
- Sorting chunks by length before batching: **1.45× faster**.
- L and M reuse XL's embedding cache (content hash), so reindexing takes 0–2 seconds.

## Findings

1. **On quality, every approach reaches 90–100% at every scale.** With n = 30, one question is 3.3 points, so most differences are within noise.
   - The wrong questions at L/XL are **the same across approaches** (q22, q18, q9). The cause is that in a large corpus the same information appears in several documents with varying completeness. The agent answers from an "almost right" document and misses one key fact.
   - This is a limit of the data, not of any one approach. The fix lies in knowledge quality (deduplication, links between documents), not in retrieval.
2. **F (full-context) only fits small bundles.** At S (60k tokens), F is 100% correct and fast (4.6s) but **about 1.7× more expensive than D**. At M (150k), each run costs 220k input tokens and $0.10, about 6× D. F cannot be used for L and XL.
3. **H (plain RAG) is the fastest** (4.2–4.7s) and cheapest, but has no way to recover. When retrieval misses, it can only answer "don't know" or answer incompletely (XL 90%, en 88%).
4. **D (qobot) is the most stable on large bundles:** L 100%, XL 93%, about 2 agent turns, $0.018–0.023.
   - As measured, D takes about 7s. About 2.5s of that is the MCP server reloading the model on every run. With the MCP server long-lived in the gateway, D is estimated at about 4.5–5s, on par with H.
   - **Weakness:** with the top-6 already in the prompt, the agent **almost never double-checks** (XL: 0 tool calls), even when the gold document is not in the top-6 (4/30 questions).
5. **E (agent + directory, Read/Grep/Glob) is as strong as D at every scale** (100 / 100 / 97 / 93%), **with no index**.
   - The agent writes multilingual regexes itself and reads `index.md`.
   - Drawbacks: twice the turns, and file-read permission must be granted to the agent.
6. **G (index.md tree walk + get/grep over MCP, no embeddings)** is about 3–7 points below E, and weakest on Vietnamese questions without diacritics (85%).
   - The difference from E lies in the **lexical tool**: `kb_grep` currently does only simple substring search, while the CLI's Grep supports regex, alternation, globs and context.
   - Bringing `kb_grep` up to Grep's level could let G reach E's level **without granting file-read permission and without embeddings**.
7. **A (tools only) is always the slowest** (≈10s, 5 turns) and no more accurate than the others. There is no reason to choose A.
8. **Catalog in the prompt:** at L, D (with catalog, 100%) beats I (no catalog, 93%), but costs about 15k more tokens (cached, +$0.005/run). At S, M and XL there is no difference. Keep the catalog when it is ≤ ~10k tokens.
9. **Japanese reaches 100% with every approach.** Vietnamese without diacritics lowers the keyword-based approaches (A 88%, G 85%).

## Limits
- 30 questions, one run per configuration, sonnet model only.
- Questions are LLM-generated from the gold documents themselves, so they may be easier than real questions.
- Strict scoring criterion (every key fact required).
- Latency includes the time for the MCP server to load the model on every run (D, I, A); E, F, H do not have this.

---

# Round 2 — Upgrading `kb_grep` and a verification rule

**Changes:**
- **`kb_grep` v2** (`embed-bench/src/bundle.rs`):
  - the pattern is a regex, with alternation (`a|b`);
  - case- and diacritic-insensitive (NFKC, Vietnamese diacritics stripped);
  - also searches frontmatter;
  - `path` (glob or prefix), `context` (surrounding lines), `files_only` (documents ranked by match count), `limit`;
  - on no match, suggests trying synonyms or English terms.
- **G2:** like G (root index.md + kb_list/kb_get/kb_grep, **no embeddings**), using `kb_grep` v2 and strategy guidance: translate keywords into English, alternate synonyms, `files_only` first then `kb_get(section)`, cover every part of the question.
- **D2:** like D, plus a **verification rule** in the system prompt: config keys, default values and commands must appear verbatim in the text read; if the excerpt is not enough or the question has several parts, use the tools.
- A `low_confidence` flag based on similarity score was **dropped before running**: the top-1 score distribution of the misses (0.64–0.73 at XL) overlaps completely with that of the hits (0.54–0.77). The top1−top6 margin does not separate the two groups either.

**Results** (240 new runs, $4.76; whole spike 1,020 runs, $23.05):

| Bundle | D | **D2** (+ verification) | E (Read/Grep/Glob) | G | **G2** (grep v2) |
|---|---|---|---|---|---|
| S 60k | 97% | 100% | 100% | 97% | **100%** |
| M 150k | 100% | 100% | 100% | 93% | **100%** |
| L 1M | 100% | 97% | 97% | 90% | **93%** |
| XL 4.4M | 93% | 87% | 93% | 90% | **90%** (facts 97%) |
| Vietnamese (combined) | 98% | 98% | 100% | 85% | **95%** |
| Mean tool calls (XL) | 0.0 | 0.2 | 2.7 | 2.6 | 2.4 |
| Mean cost | $0.018–0.023 | $0.019–0.023 | $0.018–0.026 | $0.019–0.028 | **$0.017–0.022** |

**Findings:**
1. **`kb_grep` v2 works.** G2 beats G at S, M and L (+3 to +7 points), and **Vietnamese questions rise from 85% to 95%** thanks to diacritic-insensitive matching. G2 is close to E (within ≤ 1 question at every scale) **without granting file-read permission and without embeddings**. A `lexical` mode is therefore feasible.
2. **A verification rule in the prompt does not change behavior.** D2 only raises tool calls from 0 to 0.2 at XL, and does not improve accuracy (XL 93% down to 87%, within noise). Once it has an excerpt that "looks right", the agent trusts it; a prompt reminder alone is not enough.
3. **The remaining errors are mostly answers missing one key fact, and they are the same across approaches.** q22 is wrong with every approach from L up; q28 and q9 also recur. The information is spread across several documents, so this is a knowledge-quality problem (duplication, no "canonical" page), not a retrieval problem.
4. **What do embeddings buy compared with G2?** Equivalent quality, equivalent cost. The difference is the **number of agent turns**: D takes 2 turns, G2 about 4.5. With a long-lived MCP server (no model reload per run), D is estimated at ≈ 4.5s and G2 ≈ 6.5s. The price of embeddings is about 1 CPU-hour to index XL and about 0.5GB RAM.
5. With n = 30, one question is 3.3 points, so any difference of ≤ 2 questions between D, D2, E, G2 is **not statistically significant**.
