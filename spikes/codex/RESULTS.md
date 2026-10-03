# Codex spikes: S7 (answering, small model) and S13 with Codex (setup) (2026-10-03)

Codex CLI 0.159.0 (`codex exec`), logged in with ChatGPT, reasoning effort medium. Two models:
- **gpt-6.1-sol**, the current workhorse;
- **gpt-6-luna**, the "fast and affordable" small model.

The okfkit binary is the `okfkit-full` build of `cedaf9c`. Total Codex usage was about 13.4M input
tokens (80% cached) and 62k output tokens; the Claude Sonnet judge was extra.

## S7 — lexical okfkit with Codex and a small model (`s7.py`)

**Setup.** This is the v0.1 acceptance (config O), with `codex exec` in place of Claude Code:
- the okf-scale bundle L (287 OpenClaw pages, about 1M tokens) and its 30 questions, 10 each in
  vi, en and ja;
- the same rules, lexical strategy and root `index.md`;
- okfkit's MCP server as the only knowledge source;
- a structured answer graded by the same judge (Claude Sonnet).

Codex has no system-prompt flag, so the system text goes first in the prompt. Codex always has a
shell; each run starts in an empty directory with a read-only sandbox.

`+search` adds `kb_search`, using bge-m3 int8 with the v0.3 vectors, and the okf-scale tool hint
in place of the lexical strategy.

| Model | Correct | Key facts | vi / en / ja | Tool calls | Input tokens | Wall |
|---|---|---|---|---|---|---|
| Claude Sonnet (v0.1 O, reference) | 28/30 (93%) | 96% | 10 / 8 / 10 | 2.5 | — | — |
| **gpt-6.1-sol** | **26/30 (87%)** | 94% | 9 / 7 / 10 | 2.4 | 72.9k | 18 s |
| gpt-6.1-sol + search | 26/30 (87%) | 94% | 9 / 8 / 9 | 2.5 | 78.2k | 19 s |
| gpt-6-luna | 21/30 (70%) | 88% | 8 / 6 / 7 | 2.9 | 85.7k | 14 s |
| gpt-6-luna + search | 21/30 (70%) | 88% | 6 / 7 / 8 | 1.9 | 65.3k | 14 s |

Missed questions:
- gpt-6.1-sol: q10, q22, q27, q28. q22 and q28 are the same two that Claude and every lexical
  config of the okf-scale spike miss.
- gpt-6-luna: 9 questions, including q22, q27 and q28.
  - It cited the gold document in 25 of 30 runs (26 with search).
  - Its misses are mostly incomplete answers: one part of a multi-part question is left out.

Findings:
1. **Lexical okfkit works with Codex.** gpt-6.1-sol reaches 87% against Claude's 93%. That is 2
   questions apart (n = 30, 1 question = 3.3 points), not a significant difference, and it uses
   the same number of tool calls.
2. **The small model falls short on completeness, not retrieval.** gpt-6-luna finds the right
   documents but leaves out details (key facts 88%).
3. **Semantic search does not fix the small model.** With `kb_search` it still scores 21/30, while
   using fewer tool calls (1.9 vs 2.9) and 24% fewer input tokens. For gpt-6.1-sol, search changes
   nothing.
4. **Recommendation for profiles:**
   - Keep the lexical default for Codex as well.
   - Recommend a workhorse model for answering.
   - A small model is fine for setup (S13 below), but expect about 70% on hard lookup questions.

## S13 with Codex — setup from one sentence (`s13.py`)

**Setup.** The six folders of S13 and S13b, each a fresh clone of the commit the Claude runs
started from:
- small (OKF sample, 9 docs);
- large (okf-scale L, 287 docs);
- multi (100 vi/en/ja docs; the request is in Vietnamese);
- repo (a software repository with `docs/`);
- empty (request in Vietnamese: start a customer-care knowledge base and write a 30-day return
  policy);
- partial (OKF with 2 broken files).

**Isolation:**
- HOME, CODEX_HOME (holding only a copy of the login), the XDG directories and the models all live
  in a scratch directory outside any repository.
- okfkit is on PATH through a wrapper that logs every call.
- Sandbox: workspace-write plus that home. The network is off (Codex's default).

The agent was told only the request, and that it could not ask questions during the task.

| | gpt-6.1-sol | gpt-6-luna |
|---|---|---|
| Found `onboard` and read `help --agent` | 6/6 | 6/6 |
| Previewed with `--print` before installing | 5/6 | 3/6 |
| Ended with `okfkit doctor` ok (real MCP round trip) | 6/6 | 5/6 (empty: lint L2 instead) |
| Consent flags added on its own (`--accept-license`, `--yes`, `--write`, `--force`, `--replace`) | **0** | **0** |
| Stopped at the user's decisions (curate, fix files, business details) | yes | yes |
| empty: `init` + `new --type Policy`, lint L2 0 errors, open points marked "to confirm" | yes | yes |
| Median wall time / input tokens | 108 s / 386k | 77 s / 304k |
| okfkit calls in total | 105 | 68 |

**Problem found: Codex cannot register okfkit for itself.**
- Codex's workspace-write sandbox keeps `.codex/` read-only, so `okfkit agent install --codex`
  fails with only `Permission denied (os error 13): Permission denied (os error 13)`, with no hint.
- Every agent reported the block to the user, then improvised:
  - Claude Code was set up instead (often not the agent the user had in mind);
  - one agent wrote a `codex-okfkit` launcher script;
  - two only added `AGENTS.md` (CLI use).
- okfkit should:
  - detect the case;
  - say that the Codex sandbox protects `.codex/`;
  - give the user the command to run outside the sandbox, or `--user`;
  - list it in `onboard` as a step for the user to run when okfkit runs inside Codex.

**Other observations:**
- The Japanese dictionary could not be downloaded (no network in the sandbox). The fallback
  worked and the agents said so.
- Codex reads a lot of help text: about 300–390k input tokens per setup, against about 50k for
  Claude in S13. Most of it is cached.

## After the fix (`b98cebf`)
okfkit now:
- stops `agent install --codex` before writing anything, with `sandbox_blocked` (exit 3) and the
  command for the user;
- when run inside a Codex session, makes connecting Codex a step for the user in `onboard`.

Rerun with gpt-6.1-sol on `small` and `repo`:
- Both runs ended by giving the user the exact command for their terminal, for example
  `okfkit -b docs agent install --codex --project .`, and checked the rest with `doctor`.
- No launcher scripts, no Claude Code setup in place of Codex, and 0 consent flags.
- Wall time: 61 s and 72 s, against 82 s and 103 s before.

Runs are in `results/s13-after-fix-runs.jsonl.gz`.

## Not covered
- Codex outside the sandbox (`danger-full-access`), where `.codex/` is writable.
- Codex answering through its own project MCP config after `okfkit agent install --codex`; the
  harness passed the server with `-c mcp_servers.kb.*` instead.

## Reproduce
```
python3 s7.py run --models gpt-6.1-sol,gpt-6-luna[,gpt-6-luna+search,...]   # +search needs the bge-m3 vectors
python3 s7.py report
S13_SRC=<dir with the S13/S13b clones> python3 s13.py run --models gpt-6.1-sol,gpt-6-luna
python3 s13.py report
```
Runs are in `results/s7-runs.jsonl.gz` and `results/s13-runs.jsonl.gz`; grades in `results/judge.json`.
