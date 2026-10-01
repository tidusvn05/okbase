# v0.2 eval results (PLAN §13)

Run on 2026-10-01 with Claude Code 2.1.284 (`--model sonnet`) and a release
build of okfkit (commit 38dc7c1). 156 runs, $6.36 including judging.
Questions, prompts and judges are the spikes' own (biz-meta, okf-scale).

## S5 ×20 and S9 — biz-meta ×20 bundle (3,020 docs, sheets of ~10k rows, 48 questions)

- **K**: the spike's QD setup (its system prompt with tool hints, Read/Grep/Glob + `kb_query`,
  `data_tables`, `data_query`) with the MCP server replaced by `okfkit mcp serve --stdio`.
- **KS**: no tool hints in the system prompt; the three okfkit skills are installed instead.

| Category | Spike QD ×20 | **K** | KS |
|---|---|---|---|
| list (8) | 8 | 8 | 8 |
| version (8) | 8 | 8 | 7 |
| count (8) | 8 | 8 | 8 |
| facet (6) | 5 | 4 | 4 |
| sheet (10) | 10 | **9** | **10** |
| content (8) | 7 | 7 | 8 |
| **all (48)** | **46 (96%)** | **44 (92%)** | **45 (94%)** |
| `kb_query` used (runs) | 32 | 30 | 30 |
| `data_query` used on sheet questions | 10 | 10 | 9 |
| Skill invoked (runs) | — | — | **0** |
| mean cost | $0.032 | $0.041 | $0.061 |
| mean turns / wall time | — | 4.5 / 9 s | 4.8 / 11 s |

Findings:
- **Sheets:** every sheet question used `data_query` (K 10/10). K's one miss (q36)
  reported two candidate row counts (191 and 198) instead of committing to 198;
  KS answered all 10. The v0.2 criterion "sheet 10/10" is met by KS and missed by
  one ambiguous answer in K.
- **Overall** K is 2 questions below the spike's QD ×20 (n = 48: 1 question = 2.1
  points; not significant). The misses are the same kind as the spike's: facet
  questions over ~120 customers / 18 drafts (q28, q29; the spike also missed one
  facet question) and one content question citing other documents (q40).
- **Cost** is ~28% higher than the spike's server ($0.041 vs $0.032): okfkit lists
  more tools with longer descriptions. Worth trimming before v1.0 (with eval).
- **S9 (skill) is inconclusive.** Claude Code listed the okfkit skills (checked
  separately) but **never invoked them** in 48 runs; without any tool hints in the
  prompt, KS still used `kb_query` as often as K (30/48) and scored 45/48. The tool
  descriptions alone carry the strategy. The PLAN target (`kb_query` on ≥ 45/48)
  is not reached by either config. Next step: test the skill where it is actually
  loaded (e.g. `/okfkit-answer` invoked by the user, or the skill text in the
  system prompt) before changing tool descriptions or skill content.
- KS costs ~50% more than K: the skill listing (and the user's other installed
  skills) enlarge the prompt.

## S8 — adopt (okf-scale L: 287 OpenClaw pages, 30 questions vi/en/ja)

An agent with Read/Grep/Glob and the same neutral prompt, on the raw markdown files
of bundle L (**R**) and on the output of `okfkit adopt --out` (**A**, L1: 0 lint
errors).

| Metric | R (raw) | A (adopted) |
|---|---|---|
| correct | 29/30 | 29/30 |
| mean tool calls | 2.5 | 2.3 |
| mean cost | $0.024 | $0.024 |
| mean wall time | 6 s | 7 s |

**Criterion met: adopt does not lower accuracy.** Both miss q22, the question every
lexical configuration of the okf-scale spike misses. The raw OpenClaw docs already
carry good `title`/`summary` frontmatter, so little headroom was expected here; a
folder without frontmatter would show the benefit of adopt better.

Files: `results/table.md`, `results/judge-biz.json`, `results/judge.json`,
`results/runs.jsonl.gz`, `results/run.log`.
