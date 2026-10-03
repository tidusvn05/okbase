# Spike: Multilingual embeddings (vi / en / ja) — results

Run date: 2026-09-30 · Machine: AMD EPYC 8 vCPU, 23GB RAM, no GPU · fastembed 7.1 (ONNX fp32), lindera 6.2 (IPADIC), SQLite FTS5

## Dataset

- `data/docs.json`: 30 documents of a hypothetical company (10 vi, 10 en, 10 ja). Content covers policies, guides, HR and device faults. The dataset deliberately contains pairs of close topics as distractors:
  - đổi trả ↔ hoàn tiền (returns ↔ refunds)
  - vệ sinh điều hòa ↔ thay フィルター (air-conditioner cleaning ↔ filter replacement)
  - hóa đơn VAT ↔ 領収書 (VAT invoice ↔ receipt)
  - tích điểm ↔ ポイント期限 (earning points ↔ point expiry)
  - nghỉ phép ↔ remote work (leave ↔ remote work)
- `data/queries.json`: 90 questions. Each document has 3 questions, in vi, en and ja, **phrased differently** from the document text.
  - 30 questions in the same language as the document (*same-lang*)
  - 60 questions in a different language (*cross-lang*)

Rerun: `cargo run --release -- <bm25|e5-small|e5-base|bge-m3>`. Detailed results, including the list of missed questions, are in `results/*.json`.

## Quality results (Recall@1 = top-1 correct)

| Method | R@1 | R@3 | MRR | Same-lang R@1 | **Cross-lang R@1** |
|---|---|---|---|---|---|
| BM25 (FTS5 + lindera + vi diacritics stripped) | 0.27 | 0.36 | 0.32 | 0.73 | **0.03** |
| multilingual-e5-small | 0.48 | 0.78 | 0.64 | 0.93 | **0.25** |
| multilingual-e5-base | 0.68 | 0.87 | 0.79 | 1.00 | **0.52** |
| **bge-m3** | **0.98** | **1.00** | **0.99** | **1.00** | **0.97** |

Hybrid, tried on all 3 models:

| Fusion | e5-small R@1 | e5-base R@1 | bge-m3 R@1 |
|---|---|---|---|
| Dense only | 0.48 | 0.68 | 0.98 |
| RRF (BM25 w=1.0) | 0.36 | 0.37 | 0.38 |
| RRF (BM25 w=0.1) | 0.37 | 0.42 | 0.46 |
| Convex (cos + 0.3·bm25) | 0.41 | 0.50 | 0.87 |
| Convex (cos + 0.1·bm25) | 0.43 | 0.62 | 0.98 |

## Resource results (measured with the model already in the cache)

| | BM25 | e5-small | e5-base | bge-m3 |
|---|---|---|---|---|
| Model size (fp32) | 0 (dictionary embedded in the binary) | 465MB | 1.1GB | 2.2GB |
| Load time | 8ms | 2.3s | 3.2s | 2.9s |
| RAM after load | ~39MB | ~1.0GB | ~1.6GB | ~1.7GB |
| Peak RAM while indexing (batch 32) | 39MB | 1.1GB | 2.0GB | 2.1GB |
| Indexing speed (chunks of ~150–250 tokens) | instant | 48 chunks/s | 16 chunks/s | **5 chunks/s** |
| Estimated first index of 10k chunks | <1s | ~3.5 min | ~10 min | **~35 min** |
| Query p50 / p95 | 0.4 / 0.7ms | 9 / 12ms | 23 / 27ms | 71 / 84ms |

The release binary is about 84MB (including onnxruntime and the IPADIC dictionary).

## Findings

1. **Embeddings are required for a 3-language bot.** BM25 finds the right document for only 3% of cross-language questions, and still misses 27% of same-language questions when the user phrases things differently from the document.
2. **bge-m3 is clearly ahead**, especially on cross-language questions: 97%, against 52% for e5-base and 25% for e5-small. e5-small is not good enough for vi↔ja.
3. **The cost of bge-m3 is acceptable on a server.** Queries take about 70ms and RAM is about 2GB (shared by all bots). The only weakness is the slow first index (5 chunks/s), so indexing must run in the background and incrementally index changes.
4. **"Equal-weight" hybrid makes results worse.**
   - For Vietnamese, BM25 matches common words such as "không", "được", "có". For Japanese, it matches generic nouns such as 家電.
   - When scores are normalized by the per-query max, a weak match also becomes 1.0. This pushes wrong same-language documents above correct cross-language documents.
   - RRF is even worse, because on a small set the score gaps between ranks are very small.
   
   → **BM25 should not be a ranking source on equal footing with embeddings.** Use it for:
   - (a) exact matching of identifiers such as error codes `E2`, `HTTP 429`, SKUs or proper names, through an `aliases` table or rare tokens (high IDF);
   - (b) a fallback mode when embeddings are disabled;
   - (c) filtering or searching in the CLI (`qobot kb search --lexical`).
5. **The tokenizers work correctly.** Lindera segments Japanese correctly; Vietnamese diacritic stripping works. Improving BM25 further would need vi/ja stopword removal and English stemming; this spike did not do that.
6. **Build:** `native-tls` must be disabled and `rustls` used (`default-features = false`) to avoid depending on OpenSSL.

## Limits of the spike

- Small data (30 documents, 90 questions), written by me. With a few thousand documents, the scores of all four methods will be lower and the gaps between them may change. **Needs a rerun on real data.**
- Each document is one chunk; splitting by heading was not tried.
- Single run, 8 vCPU; thread count was not limited to simulate a small machine.
- The int8 (quantized) version of bge-m3, usually about 4 times smaller and 2–3 times faster, was not tried, nor was a reranker.

## Recommendation for qobot

| Profile | Condition | Configuration |
|---|---|---|
| **standard** (default) | Server with ≥ 4GB free RAM | bge-m3 + BM25 used for identifier matching (with a threshold) |
| **lite** | Weak machine (<2GB) | e5-small or embeddings disabled; the agent translates the query into 3 languages and calls `kb.search` several times |
| **api** | No local model wanted | `EmbeddingProvider` calls an API (Voyage/OpenAI/Gemini multilingual) |

Next spikes to run:
- (1) bge-m3 int8;
- (2) BM25 with a threshold for identifiers;
- (3) chunking by heading;
- (4) a run on about 500 of your real documents.

---

# Round 2 — Hard dataset (v2) and 12 models

Run date: 2026-09-30. Rerun with `./download.sh && ./run-v2.sh`. Log in `results/run-v2.log`, details in `results/v2/*.json`.

**Dataset v2** (`data/v2/`, data-generation source in `data/v2/src/`):
- 100 documents (33 vi, 33 en, 34 ja), split into **20 groups of near-identical topics**. Within a group, documents differ by only one detail (device, sales channel, country, leave type…) and are deliberately written in different languages. For example, the warranty group has 6 documents and the E1–E6 error-code group covers 5 devices.
- 300 questions, 3 per document (vi, en, ja). About 20% of the Vietnamese questions are without diacritics and use slang (bh, ship, pass, WFH), and some questions are keywords only.

## Quality (sorted by R@1)

| Model | R@1 | R@3 | MRR | Same-lang R@1 | Cross-lang R@1 |
|---|---|---|---|---|---|
| gemma-q4 **+ reranker** bge-v2-m3 | 0.887 | 0.973 | 0.927 | 0.95 | 0.855 |
| gte-mb-int8 **+ reranker** | 0.883 | 0.967 | 0.923 | 0.94 | 0.855 |
| **EmbeddingGemma-300M** (fp32) | **0.863** | 0.957 | 0.913 | 0.94 | **0.825** |
| EmbeddingGemma Q (int8, dynamic) | 0.860 | 0.957 | 0.911 | 0.93 | 0.825 |
| **EmbeddingGemma Q4** | **0.847** | **0.960** | 0.905 | 0.93 | 0.805 |
| e5-small + reranker | 0.833 | 0.900 | 0.865 | 0.95 | 0.775 |
| gte-multilingual-base (fp32) | 0.807 | 0.920 | 0.865 | 0.87 | 0.775 |
| bge-m3 (fp32) | 0.780 | 0.940 | 0.861 | 0.89 | 0.725 |
| bge-m3 int8 | 0.780 | 0.933 | 0.856 | 0.89 | 0.725 |
| gte-multilingual-base int8 | 0.767 | 0.917 | 0.847 | 0.82 | 0.740 |
| Qwen3-Embedding-0.6B | 0.753 | 0.923 | 0.846 | 0.90 | 0.680 |
| nomic-embed-text-v2-moe | 0.703 | 0.933 | 0.823 | 0.88 | 0.615 |
| multilingual-e5-large | 0.483 | 0.730 | 0.635 | 0.90 | 0.275 |
| multilingual-e5-base | 0.460 | 0.697 | 0.601 | 0.88 | 0.250 |
| multilingual-e5-small | 0.390 | 0.563 | 0.506 | 0.91 | 0.130 |
| BM25 (FTS5 + lindera) | 0.333 | 0.377 | 0.363 | **0.93** | 0.035 |

## Resources (8 vCPU; the "2 CPU" column is measured with `THREADS=2`)

| Model | Size | RAM after load | Peak RAM | Query p50 (8 CPU / 2 CPU) | Index chunks/s (8 / 2 CPU) |
|---|---|---|---|---|---|
| **gemma-q4** | **188MB** | **~460MB** | **~600MB** | 38 / 49ms | 7.5 / 4.2 |
| gemma (fp32) | 1.2GB | ~770MB | ~980MB | 39ms | 10 |
| gemma-q (int8 dynamic) | 295MB | ~370MB | 1.7GB ¹ | 106ms | 7.7 |
| bge-m3 int8 | 558MB | ~1.1GB | ~1.7GB | 37 / 46ms | 6.2 / 3.2 |
| bge-m3 | 2.2GB | ~1.7GB | ~2.0GB | 77 / 93ms | 3.8 / 1.9 |
| gte-mb int8 | 341MB | ~900MB | ~1.25GB | 16 / 20ms | 13.6 / 8.8 |
| gte-mb | 1.2GB | ~1.8GB | ~3.2GB | 31ms | 8.7 |
| e5-small | 465MB | ~1.0GB | ~1.1GB | 9 / 12ms | 41 / 22 |
| Qwen3-0.6B (candle) | 1.1GB | ~2.4GB | ~3.5GB | **439ms** | **0.7** |
| nomic-v2-moe (candle) | 1.8GB | ~2.1GB | ~3.7GB | 190ms | 2.6 |
| + reranker bge-v2-m3 (top-20) | +2.2GB | | +~2GB | **+5–6 s/question** | — |

¹ Dynamically quantized models do not allow batching, so fastembed must embed all texts in a single pass; that is why peak RAM is high.

## Round 2 findings

1. **The round 1 results no longer hold.** On the hard set, bge-m3 drops from 0.98 to 0.78. **EmbeddingGemma leads** (0.86) and is best on cross-language questions (0.83).
2. **EmbeddingGemma Q4 has the best quality-to-cost ratio:**
   - it loses only 1.6 R@1 points against fp32, and R@3 still reaches 0.96;
   - the file is 188MB, RAM about 0.5GB (1/3–1/4 of bge-m3);
   - queries take 38–49ms, even when limited to 2 CPUs.
3. **The int8 version of bge-m3 loses no quality**, and is 2× faster and 1/4 the size of fp32. This is the fallback option with an MIT license.
4. **Qwen3 and nomic running through candle on CPU are too slow** (0.7–2.6 chunks/s, queries 190–440ms) with no better quality. Rejected.
5. **The e5 family is weak on cross-language questions** (0.13–0.28). Rejected.
6. **A reranker adds 3–4 points** (gemma-q4 from 0.847 to 0.887), but costs **5–6 seconds per question** and about 2GB more RAM on CPU. Not suitable for realtime chat. Only worth considering for background tasks (reports, automation) or when a GPU is available.
7. **BM25 complements embeddings:** on same-language questions BM25 reaches 0.93, higher than any embedding; on cross-language questions it is near 0. Worth experimenting further with a thresholded combination: trust BM25 only when the match score is high and the query is in the same language as the document. This could exceed 0.9 without a reranker.
8. **R@3 is about 0.96** with gemma. The bot should give the agent 3–5 documents and let the LLM pick the right one among near-identical documents.

## Recommendation (replaces the round 1 recommendation)

| Profile | Model | Notes |
|---|---|---|
| **standard** (default) | **EmbeddingGemma-300M Q4** | ~0.5GB RAM, runs on a 2-CPU VPS. The Gemma terms need checking for commercial use |
| standard-mit | bge-m3 int8 | When an MIT license is required |
| quality | EmbeddingGemma fp32 or Q4, plus a reranker for background tasks | Or use a GPU |
| lite | BM25 + the agent translates the query | Very weak machines |
| api | Embeddings through an API | No RAM cost |

---

# Round 3 — BM25 threshold and number of chunks per file

## A. BM25 threshold (`DATA=v2 MODEL=<m> embed-bench gate`, logs in `results/gate-*.log`)

**Method:**
- `final = cos(q,d) + β·g(q,d)`, where the gate `g` was tried in 4 families:
  - absolute threshold `bm25 ≥ T`;
  - confident top-1: `bm25 ≥ T` and top1/top2 ≥ R;
  - IDF-weighted token coverage ≥ C (hard and soft forms);
  - each family above also has a "same language only" variant (language detection with lingua).
- Parameters β, T, R, C were chosen with **2-fold cross-validation**, split by target document; the reported number is R@1 on the half of the data not used to choose the parameters (held-out).

**BM25 top-1 precision by score threshold** (FTS5 −bm25, on 100 documents):

| T | ≥ 4 | ≥ 13 | ≥ 16.5 | ≥ 21 | ≥ 24 | ≥ 27 |
|---|---|---|---|---|---|---|
| Precision | 33% | 48% | 55% | 64% | 74% | 81% |
| Share of queries above threshold | 100% | 60% | 40% | 20% | 10% | 5% |

**R@1 after applying the gate:**

| Gate | gemma-q4 | bge-m3-int8 | gte-mb-int8 |
|---|---|---|---|
| Dense, no BM25 | **84.7%** | **77.3%** | **77.7%** |
| Best of all gates (held-out) | 85.0% | 77.7% | 79.0% |
| Theoretical ceiling (dense correct OR BM25 correct) | 86.7% | 79.7% | 82.7% |
| Queries with identifiers (21 questions: E2, 429…): dense / BM25 | 90.5% / 33.3% | 81.0% / 33.3% | 85.7% / 33.3% |

**Findings:**
1. **BM25 adds almost nothing to dense.** The theoretical ceiling is only 2–5 points above dense. The best gate adds +0.3 to +1.3 points, i.e. 1–4 out of 300 questions, within noise; many gates score below dense when measured held-out.
2. Reason: BM25's 93% same-lang in round 2 comes from questions that **dense also gets right** (gemma-q4 same-lang = 93%). The two methods are right on the same questions and do not compensate for each other.
3. **Identifiers do not rescue BM25 either.** Codes like E2 appear in many near-identical documents (one error-code table per device). What distinguishes these documents is the device name, and the device name is often in a different language from the question. Dense reaches 90%, BM25 only 33%.
4. **An absolute threshold does not transfer to another corpus.** The bm25 score depends on the number of documents (IDF) and document length, so there is no shared "threshold number".
5. **lingua** detects 100% of Japanese and English questions correctly, but only **90% of Vietnamese questions** (it fails on questions without diacritics or that are too short). So the "same language" gate is unreliable to begin with.

**Conclusion: remove BM25 from the default ranking.** Keep FTS5 only for:
- (a) exact lookup tools (`kb.grep`, `kb search --lexical`) for agents and people;
- (b) exact matching of `aliases` or title;
- (c) a fallback while embeddings are not ready.

The `boost_lexical` option stays in the configuration (off by default). Turn it on only when `retrieval_log` on real data (for example a corpus with SKUs or per-document codes) proves it helps.

## B. Number of chunks per file (`chunkstat`, EmbeddingGemma tokenizer, rule §5.5: H2/H3, 200–600 tokens)

| Corpus | Files | Tokens/file (mean / p50 / p90) | Chunks/file (mean / p90) | **Files ≈ 1,000 chunks** |
|---|---|---|---|---|
| OKF samples (Google): acme_retail, ga4, stackoverflow, crypto_bitcoin | 53 | ~480–690 / 410–600 / 580–1.190 | 1.4–2.2 / 2–3 | **~450–720** |
| OpenClaw docs (real technical documentation markdown) | 1.314 | 4.235 / 2.056 / 6.970 | 10.5 / 19 | **~96** |
| — concepts / channels / gateway / tools | 78–158 | 2.700–3.200 / 1.900–2.600 | 7.9–9.2 / 14–20 | ~110–125 |
| v2 test set (short documents) | 100 | 113 | 1.0 | 1.000 |

Token density (EmbeddingGemma tokenizer):
- en ≈ 1.24 tokens/word;
- vi ≈ 1.28 tokens/syllable;
- ja ≈ 0.53 tokens/character.

A 600-token chunk is about 480 English words, 470 Vietnamese syllables, or 1,100 Japanese characters.

---

# Round 4 — End-to-end with a real agent CLI: catalog in the prompt or tools?

Run date: 2026-09-30. Claude Code 2.1.284, `--model sonnet`. All built-in tools disabled (`--tools ""`); the agent can only use the `kb` MCP server (Rust, `embed-bench mcp`) with the tools `kb_search` (EmbeddingGemma Q4), `kb_grep`, `kb_get`, `kb_list`. The OKF bundle is built from data/v2 (100 documents + `index.md`).

Rerun: `DATA=v2 embed-bench e2e-prep && python3 e2e_run.py && python3 e2e_report.py`. Data saved in `results/e2e/` (`runs.jsonl.gz` 180 runs A–F, `judge.json`, `report.md`, `summary.json`, logs).

**30 questions**, 10 per language:
- 20 "hard" questions: dense top-1 wrong;
- 10 "easy" questions: dense top-1 right.

**Scoring:** LLM judge (sonnet), blind to the configuration (item IDs shuffled), compared against the answer document.

| Configuration | Correct answer | Correct source | Time p50 / p90 | Agent turns | Mean tool calls | No tool call | Mean cost |
|---|---|---|---|---|---|---|---|
| A. Tools only | 97% | 97% | 9.5s / 12.3s | 4.8 | 2.7 | 0% | $0.017 |
| B. Catalog (in user message) + tools | 97% | 97% | 8.4s / 11.0s | 3.5 | 1.4 | 0% | $0.043 |
| C. Top-5 + catalog (user message) + tools | 97% | 97% | 6.8s / 9.8s | 2.3 | 0.2 | 83% | $0.043 |
| **D. Top-5 + catalog in system prompt + tools** | **97%** | 97% | **6.2s / 8.4s** | **2.2** | **0.2** | **87%** | **$0.017** |

Total experiment cost: $3.60.

**Findings:**
1. **Accuracy is the same for all 4 approaches** (97%, the single wrong question is the same in all 4). The wrong question is q56, and it is a dataset error, not an agent error: a Japanese question about bulk purchases, the agent answered with the Japan policy ("same warranty as retail customers"), while the reference answer is the Vietnam policy. So in practice all 4 approaches reach 100%.
   - Notably, the agent **corrects cases that dense ranked wrong**. On the 20 hard questions, dense top-1 is wrong but answers are still 95–100% correct, because the agent reads several documents in the top-5 or searches further.
2. **The differences are in speed and cost:**
   - A (tools only) needs on average 2.7 tool calls and 4.8 agent turns, so it is the slowest.
   - C/D have documents in the prompt already, so 83–87% of answers come immediately without a tool call.
3. **Where the catalog is placed determines cost.**
   - In B/C, the catalog (~9k tokens) sits in the same block as the question. The question changes every run, so the catalog is rewritten to the cache on **every run**, raising cost by about 2.5×.
   - In D, the catalog is in the system prompt (the stable part) so it is read from cache; cost **equals A** while being the fastest.
4. With a catalog available, the agent uses `kb_get` to read documents by id (B: 39 gets, only 3 searches). Without a catalog, the agent must search first (A: 42 searches, 16 greps). **No configuration called `kb_list`.**
5. **The floor of about 6 seconds** includes CLI startup, MCP server startup (each run reloads the embedding model, about 2.5s) and 2 model turns. In qobot, the MCP server will run **long-lived inside the gateway** (streamable HTTP, model preloaded), removing about 2–3 seconds of this.

**Limits:**
- Small bundle: 100 short documents, catalog about 9k tokens.
- Only 30 questions, one run per configuration.
- Only one model tried (sonnet).
- With long documents or a large bundle, A will need more tool calls, so the gap to D may grow.

## Round 4b — Comparison with 2 "traditional" approaches (without qobot)

- **E. Agent + bundle directory:** `cwd` is the bundle directory; the agent only uses the built-in file tools (Read/Grep/Glob). No qobot tools, no pre-retrieved documents, no catalog in the prompt.
- **F. Whole bundle in the system prompt, no tools:** 100 documents, about 27k tokens. The bundle is cached: on average each run reads 25k tokens from cache and writes only 1.6k new.

| Configuration | Correct answer | Time p50 / p90 | Agent turns | Mean tool calls | Mean cost |
|---|---|---|---|---|---|
| A. qobot tools only | 97% | 9.5s / 12.3s | 4.8 | 2.7 | $0.017 |
| D. Top-5 + catalog (system) + qobot tools | 97% | 6.2s / 8.4s | 2.2 | 0.2 | $0.017 |
| **E. Agent + directory (Read/Grep/Glob)** | 97% | 6.6s / 8.3s | 4.3 | 2.3 (Read 36, Grep 32) | $0.032 |
| **F. Whole bundle in the prompt** | 97% | **4.1s / 5.0s** | 2.0 | 0 | **$0.014** |

Total cost of rounds 4 + 4b: $4.99.

**Findings:**
1. **Accuracy is still the same** (the single wrong question is still q56, a data error). At 100 short documents, every approach answers correctly.
2. **F is the fastest and cheapest** for a small bundle: no tool-call round trips, and the bundle is cached.
3. **E still finds cross-language documents with grep alone.** The agent writes multilingual patterns itself, for example `air conditioner|điều hòa|エアコン`. The LLM compensates for the weakness of keyword search. But E takes more turns, costs about 2× D/F, and requires enabling file-reading tools.
4. **F's cost and latency grow linearly with bundle size** (the estimates below are not measured):
   - 100–200 short OKF concepts are about 50–140k tokens; 100–200 long documents are about 400–800k tokens, beyond capacity or too expensive;
   - each time the cache expires (1-hour TTL), the first run must rewrite the whole bundle to the cache;
   - accuracy with a long context and many near-identical documents was not measured.
   
   → F only fits small bundles.
