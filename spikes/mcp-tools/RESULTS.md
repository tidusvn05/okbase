# S15 — a smaller MCP tool list, and the answer rules as server instructions (2026-10-03)

Two open items from the v0.2 eval (`../acceptance-v0.2/RESULTS.md`):
1. okbase cost about 28% more per question than the spike's own server. Is the longer tool list
   the cause, and does trimming it change accuracy?
2. S9: agents never invoked the `okbase-answer` skill (0 of 48 runs). Do the skill's rules work
   when the MCP server sends them as instructions? Claude Code puts server instructions into the
   system prompt.

## Changes under test (commit after `bedbc52`)
- `kb_grep` and `kb_search` describe their `filter` argument as "the same fields as kb_query"
  instead of repeating the whole filter schema. The tool list for this bundle went from 6,986 to
  5,874 characters (−16%).
- The server instructions carry the rules of `okbase-answer`, about 810 characters, built from
  the tools that are on:
  - `kb_query` for list, count and filter questions;
  - `data_query` with aggregates for numbers in sheets;
  - `kb_grep` with alternation across languages;
  - status and `active_on`;
  - state only values that were read, and cite ids.
- Before, the instructions were 230 characters of general advice.

## Setup
- **Bundle:** biz-meta ×20 (3,020 documents, sheets of about 10k rows) and its 48 questions; the
  questions, prompt, structured output and judge are the spike's.
- **Agent:** Claude Code 2.1.284 (`--model sonnet`) with Read, Grep, Glob and every okbase tool;
  6 runs in parallel.
- **Configs:**
  - **K0:** okbase before the change, plus the spike's tool hints in the system prompt (the v0.2
    K setup, rerun so that all configs use the same model and day);
  - **K1:** okbase after the change, plus the same hints;
  - **I1:** okbase after the change, with **no hints**: only the server instructions.
- **Cost:** 144 runs, $6.29, plus the judge.

## Results (`results/table.md`)

| | K0 (before + hints) | K1 (trimmed + hints) | I1 (trimmed, instructions only) |
|---|---|---|---|
| correct | 46/48 | 45/48 | **46/48** |
| list / version / count / content | 8 / 8 / 8 / 8 | 7 / 8 / 8 / 8 | 8 / 8 / 8 / 8 |
| facet / sheet | 4/6, 10/10 | 4/6, 10/10 | 4/6, 10/10 |
| `kb_query` used (runs) | 31 | 32 | 32 |
| `data_query` on sheet questions | 10/10 | 9/10 | 10/10 |
| mean input tokens | 34,920 | 33,929 (−3%) | 34,592 |
| mean cost | $0.0431 | $0.0439 | $0.0440 |
| mean turns / wall time | 4.2 / 10.3 s | 4.1 / 11.1 s | 4.1 / 11.6 s |

Missed questions:
- q28 and q29 (facet questions over about 120 customers and 18 drafts) are missed by all three
  configs, as in v0.2 and the spike.
  - The agents report 127–128 customers, while the computed answer lists about 123.
  - For q29 they keep only the 4 "v3" drafts, which is a reading of the question.
- K1 also missed q5: a list of 77 deprecated products, whose `items` came back incomplete.

## Findings
1. **Server instructions replace the hints.**
   - With nothing in the system prompt, I1 matches K0: 46/48, with the same use of `kb_query`
     (32 vs 31) and of `data_query` (10/10).
   - In v0.2, the skill without hints (KS) reached 45/48 at +49% cost, and the skill was never
     invoked.
   - Answer to S9: the rules work when the server sends them; agents do not need to invoke a skill
     to follow them.
   - The skill stays useful for agents without MCP (CLI fallback) and for curating.
2. **Trimming the tool list does not move cost.**
   - Input tokens drop by about 1k per question (−3%), and cost is unchanged within the noise.
   - Accuracy is the same: one question apart, which is not significant with n = 48 (1 question
     = 2.1 points).
   - The cost per question is set by Claude Code's own prompt and the tool results, not by
     okbase's tool definitions.
   - The trim is kept: same results with less context.
3. **The v0.2 cost gap was not the tool list.**
   - K0 costs $0.043 today, against $0.041 in v0.2 and $0.032 for the spike's server.
   - The spike ran a different Claude Code version, so the remaining gap cannot be attributed to
     okbase. Comparing against the spike's server again would need it rerun on the same day.

## Not covered
- Codex, and agents other than Claude Code. Whether a client shows server instructions differs
  between clients, which is why the skill and the `AGENTS.md` block stay.
- The okf-scale L lookup suite (S4).

## Reproduce
```
cargo build --release -p okbase-cli          # the new binary; BASE_BIN=<bedbc52 build> for K0
python3 run.py run [--configs K0,K1,I1] [--limit N]
python3 run.py report
```
`work/` (states, base binary) and `runs/` are git-ignored; every run is in `results/runs.jsonl.gz`.
