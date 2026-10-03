# Spike: metadata/tags — are metadata-filter and sheet-query tools needed?

Run date: 2026-09-30. Claude Code 2.1.284 (`--model sonnet`). Total cost: $11.84 for 336 runs, $4.29 to generate content, and about $1.5 for judging.

Data saved in `results/`: `runs.jsonl.gz` (336 runs), `judge.json`, `report.md` (tables and the list of wrong answers), and logs.

Rerun:
```
python3 specs.py && python3 build.py
SCALE=20 python3 specs.py && SUF=-x20 python3 build.py
python3 run.py
python3 run.py --suffix=-x20 --configs E,V,QD
python3 report.py
```

## Setup

**Business bundle "Hikari Home"** (`specs.py`, `build.py`):
- 151 vi/en/ja documents with full OKF frontmatter: `type, title, description, tags` (half inline `[a, b]`, half block list), `status, lang, department, updated`, plus `region, customer, version, supersedes, effective_from/to, contract_value, currency, model, meeting_date`.
- **Policies have several versions:** v1 deprecated, v2 stable, v3 draft.
- 3 sheets as CSV (`data/`), plus markdown-table versions in `sources/sheets/` (as after an import).
- Content written by an LLM from the spec; every number in the spec is checked to be present in the text.

**×20:** 2,869 more documents with the same schema (contracts with 200 customers, meeting minutes, SOPs for 40 branches, TH/SG/KR policies…), 3,020 documents in total. Larger sheets: inventory 4,800 rows, sales 9,806 rows, price 1,200 rows.

**48 questions** (vi 18, en 17, ja 13). **Answers are computed by code** from the spec, independent of any LLM:

| Group | Questions | Examples |
|---|---|---|
| list | 8 | "hợp đồng với Sakura Trading còn hiệu lực", "SOP kho đang draft", "tài liệu tag security cập nhật 2026" |
| version | 8 | "theo chính sách đổi trả **hiện hành** ở Nhật, trả trong bao nhiêu ngày" (older and draft versions have different values) |
| count | 8 | "bao nhiêu hợp đồng hết hạn trong 2026", "tổng giá trị hợp đồng ACME còn hiệu lực" |
| facet | 6 | "phòng ban nào có tài liệu tag privacy", "khách hàng nào có nhiều hợp đồng hiệu lực nhất" |
| sheet | 10 | "tổng tồn kho máy lọc khí ở kho HCM", "tháng nào JP có doanh thu cao nhất" |
| content | 8 | Ordinary content questions (control group) |

**Approaches compared** (`run.py`). Every approach has Read/Grep/Glob on the bundle directory:
- **E:** only the directory layout + `index.md`.
- **V:** E + auto-generated `_views/` (table pages by tag, type, status, department, lang, customer, `contracts-all`, `recent`).
- **Q:** E + MCP `kb_query` (frontmatter filtering combining several conditions, `active_on`, date ranges, facets, sum).
- **QD:** Q + MCP `data_tables` / `data_query` (read-only SQL on sheets loaded into SQLite).

**Scoring:**
- list group: automatic comparison of ID sets (correct on an exact match), with precision/recall.
- Other groups: LLM judge against the precomputed answer, blind to the configuration.

## Results

| Group | E | V | Q | QD | **E ×20** | **V ×20** | **QD ×20** |
|---|---|---|---|---|---|---|---|
| list (8) | 8 | 8 | 8 | 8 | 7 (recall 89%) | 8 | 8 |
| version (8) | 8 | 8 | 8 | 8 | 8 | 8 | 8 |
| count (8) | 8 | 8 | 8 | 8 | 8 | 8 | 8 |
| facet (6) | 6 | 6 | 6 | 6 | 4 | 5 | 5 |
| sheet (10) | 10 | 9 | 10 | 10 | 9 | 8 | **10** |
| content (8) | 8 | 8 | 8 | 8 | 8 | 8 | 7 |
| **Total** | **100%** | 98% | **100%** | **100%** | 92% | 94% | **96%** |
| Mean cost | $0.024 | $0.026 | $0.022 | **$0.017** | $0.056 | $0.070 | **$0.032** |
| p50 / p90 | 7.9 / 10.8s | 7.4 / 10.6s | 7.5 / 11.7s | **6.7 / 8.6s** | 10.3 / 26.4s | 9.6 / 36.6s | **7.3 / 13.9s** |
| Mean input tokens | 18k | 18k | 20k | 20k | 33k | **44k** | 25k |

**Cost and p50 / p90 by group at ×20:**

| Group | E | V | QD |
|---|---|---|---|
| list | $0.063 · 15 / 26s | $0.093 · 16 / 31s | $0.063 · 14 / 23s |
| count | $0.054 · 11 / 20s | $0.067 · 10 / 22s | **$0.037 · 8 / 10s** |
| facet | $0.056 · 12 / 24s | $0.083 · 9 / 22s | **$0.046 · 10 / 17s** |
| sheet | $0.105 · 22 / **60s** | $0.122 · 37 / **60s** | **$0.012 · 6 / 7s** |
| version / content | ≈ $0.022 · 7s | ≈ $0.022 · 8s | ≈ $0.020 · 7s |

**Does the agent use the tools on its own?**
- `data_query`: used on 10/10 sheet questions (both ×1 and ×20).
- `kb_query`: ×1 used in 63/96 runs (Q and QD); ×20 used in 32/48 runs.
- `_views/`: ×1 used on 17/48 questions; ×20 used on 15/48 questions.

**Wrong answers worth noting:**
- q29 (×20) is wrong with **all 3 approaches**. The error is in the dataset: the "distractor" TH/SG/KR policies have status draft and so slipped into the reference answer, although they are not "new versions awaiting approval".
- q41 (QD×20): the 40 branch SOPs share names with the base SOP, so the question becomes ambiguous.
- q15 was misjudged on the first pass and was rejudged.

## Findings

1. **At small scale (151 documents, sheets ≤ 500 rows), the hypothesis "grep does not handle tags well" is false.** E reaches 100% in every group: the agent greps frontmatter itself (`^(status|effective_to|customer):`), intersects the results itself, and adds up a few dozen numbers itself. The tools only make it **cheaper**: `kb_query` is about 20% cheaper on list/count/facet; `data_query` is 64% cheaper on sheet questions.
2. **At ×20 (3,020 documents, sheets of about 10k rows), the hypothesis is partly true:**
   - **Sheets are the clearest breaking point.** E and V take 22–37s (p90 60s), cost $0.10–0.12 per question, and sometimes **give up**: the agent says it has no calculation tool to add up 4,900 rows. QD reaches 10/10, $0.012, 6s, i.e. **about 9× cheaper and about 4× faster**.
   - **Lists come back incomplete when the result set is large.** E missed 41/45 SOPs on one question (recall 89%). V and QD reach 100%.
   - **Count/facet:** similar accuracy, but QD is 30–45% cheaper and about 2× faster at p90.
   - Version and content questions show no difference between approaches.
3. **View files (`_views/`) do not scale.** At ×20, each view page is a table with hundreds of rows, so V **uses the most tokens** (44k input per run, $0.070) and is the slowest at p90. The agent also opens views on only about 1/3 of questions. Full views should not be the default; keep only a small page listing the tag vocabulary with counts.
4. **`kb_query` is valuable, but the agent does not always use it** (32/48 at ×20): sometimes it still greps. This could be improved with a clearer tool description, or with a catalog containing the list of tags and facets so the agent knows when to filter.
5. **Organization is still the foundation.** q41 shows that when there are several variants with the same name (branch SOPs), the question becomes ambiguous. Good metadata (`branch`, `scope`) plus metadata filtering is how to tell them apart.

## Conclusion for qobot
- **`data_query` (sheet → SQLite, read-only SQL): required.** This is the largest benefit, and directory organization cannot replace it.
- **`kb_query` (frontmatter filtering, facets, sum, `active_on`): recommended**, with the same `filter` parameters for `kb_search`/`kb_grep`. The benefit grows with bundle size: complete results for large lists, cheaper and faster for count/facet.
- **Views:** generate only the tag and facet vocabulary page (small), not full tables.
- **Limits:** synthetic data; the content of the ×20 documents is generated from templates; one run per configuration; 48 questions.
