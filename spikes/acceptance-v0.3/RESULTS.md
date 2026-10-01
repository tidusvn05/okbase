# v0.3 retrieval evals (PLAN §13)

Run on 2026-10-01, CPU only (8 vCPU), release build, `examples/retrieval_eval.rs` through the
okfkit facade (the same path MCP `kb_search` and hosts use). Models from the user cache;
EmbeddingGemma used after the maintainer accepted the Gemma Terms of Use.

| Eval | Metric | Target | Spike | EmbeddingGemma 300M Q4 | bge-m3 int8 |
|---|---|---|---|---|---|
| **S1** v2 (300 vi/en/ja questions, 100 docs) | R@1 | ≥ 0.84 | 0.847 (Gemma Q4) | **0.857** | 0.750 (spike 0.780) |
| | R@3 / MRR | — | 0.960 / 0.905 | 0.953 / 0.909 | 0.920 / 0.839 |
| | same-lang / cross-lang R@1 | — | 0.93 / 0.805 | 0.94 / 0.815 | 0.88 / 0.685 |
| **S4** okf-scale L (30 questions, 287 docs, 3,230 chunks) | gold in top-6 chunks | ≥ 97% | 30/30 | **29/30** (96.7%) | **30/30** |
| | gold first | — | 23 | 23 | 22 |
| | query latency p50 / p90 | ≤ 60 ms (PLAN §11) | 38 ms | 58 / 67 ms | 77 / 111 ms |
| | first embedding of L | ≈ 3 chunks/s | 3.2 | 2.6 chunks/s | 1.9 chunks/s |

## Findings

- **S1 met** with EmbeddingGemma Q4 (0.857, slightly above the spike). bge-m3 int8 stays the MIT
  alternative without license acceptance, at a clear cost on cross-language questions.
- **S4: one question short with EmbeddingGemma** (29/30). The miss, q23 (ja, gold
  `help/faq/what-is-openclaw`), was borderline in the spike too: the gold document entered the
  top-6 at rank 5 (score 0.548) through a neighbouring FAQ chunk. Different chunk boundaries push it
  out. bge-m3 int8 reaches 30/30. With n = 30, one question is 3.3 points; not tuned for.
- **Token estimate fixed before these numbers.** The first run gave S4 26/30: the model-free token
  estimate (spike per-word rates) undercounted code-heavy OpenClaw chunks by ~30%, so ~32% of the
  chunks exceeded the 512-token model limit and were truncated. `estimate_tokens` is now a linear
  model fitted on 2,752 chunks against the Gemma tokenizer (median error +1%); chunk sizes again
  match the spike's intent (≤ 450 real tokens). S4 went from 26/30 to 29/30, S1 was unchanged.
- Latency on L is around the 60 ms target (brute-force over 3,230 vectors of 768/1024 dims; the
  query embedding dominates). First embedding is slower than the spike's 3.2 chunks/s, partly
  because more, smaller chunks are embedded (3,230 vs the spike's count at the old sizes).

Files: `results/s1-*.json`, `results/s4-*.json`, `results/run.log`; run with `run.sh`.
