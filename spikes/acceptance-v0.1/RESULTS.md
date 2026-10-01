# v0.1 acceptance results (HANDOFF T11)

Run on 2026-10-01 with Claude Code 2.1.284 (`--model sonnet`), the okf-scale
bundle **L** (OpenClaw docs, ~1M tokens, 301 files) and its 30 questions
(10 vi, 10 en, 10 ja). Same system prompt, questions and blind LLM judge
(sonnet) as the okf-scale spike's G2; the MCP server is
`okfkit mcp serve --stdio` (release build of commit 6499968).

| Config | Correct | Key facts | Turns (mean) | Tool calls (mean) | Cost (mean) |
|---|---|---|---|---|---|
| okf-scale G2 (spike, reference) | 28/30 (93%) | — | ~4.5 | 2.4 (XL) | $0.017–0.022 |
| **O** okfkit | **28/30 (93%)** | 96% | 4.6 | 2.5 | $0.029 |
| OS okfkit + okfkit-answer skill | 27/30 (90%) | 96% | 4.8 | 2.7 | $0.047 |

**Acceptance: met.** Config O reaches the 93% target.

By language (O): vi 10/10, ja 10/10, en 8/10 (G2 in the spike: vi 9/10 on L).

Missed questions:
- O: q22 (en; also missed by spike G2 and E), q28 (en; partial: default value right, explanation missing).
- OS: q22, q28, q6 (vi; hedged about whether a restart is needed; spike G2 also missed q6).
- Spike G2: q6, q22.

Notes:
- With n = 30 one question is 3.3 points; O vs OS (1 question) and O vs G2
  (same score, different misses) are **not** statistically different.
- The skill did not improve accuracy here and cost ~60% more per question
  (larger prompt, slightly more turns). This is a lookup-style bundle with no
  metadata; the skill's main claim (S5: `kb_query` for list/count/filter
  questions) is not exercised — neither config called `kb_query`. Measure the
  skill on the business bundle (spike S9) before changing its defaults.
- Cost per question is higher than the spike's G2 ($0.029 vs ~$0.02): the
  okfkit tool list is larger (6 tools with longer descriptions vs 3).

Files: `results/table.md`, `results/summary.json`, `results/judge.json`
(grades), `results/runs.jsonl.gz` (all runs), `results/run.log`.
