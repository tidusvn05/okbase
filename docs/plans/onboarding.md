# Plan: onboarding through agents (agent-friendly setup)

Status: **O0–O5 done** with Claude (2026-10-02) and Codex (2026-10-03, `spikes/codex`); results in §6. The Codex sandbox blocking writes to `.codex/` has been handled (`sandbox_blocked`, a user step in `onboard`). Supplements [`../design.md`](../design.md) and [`advise-tune.md`](advise-tune.md).

## 0. Goals

The user only has to tell the agent (Claude Code, Codex, …): **"set up okbase for this folder"**.
The agent does every step itself: install, analyze, configure, verify. The agent only asks the user
about decisions that belong to them: licenses, large downloads, sending documents to an LLM, editing or deleting files.

For that, the CLI must be an **interface for agents**, not only for people:
1. Every command tells the agent the result, the next step, and where it must ask the user.
2. Output, exit codes and errors are stable so the agent can handle them programmatically.
3. Never wait for interactive input; every write, delete or download requires an explicit flag.

This also covers 3 open items from the MCP lifecycle review (§2).

## 1. Current state (review 2026-10-02)

| | Current state | Problem for agents |
|---|---|---|
| Interactive prompts | None (only `tune submit -` reads stdin) | Good |
| `--json` | 31 places, including every read command, `tune`, `clean`, `agent status` | Write commands such as `agent install/uninstall`, `embed enable/disable`, `adopt`, `dict install` print text only |
| Errors | Text on stderr (`error:` / `hint:`) | No error codes for the agent to branch on |
| Exit codes | Every error is 1; `lint` with errors is also 1; a rejected `tune submit` is also 1 | No distinction between "findings", "user consent required" and "invalid arguments" |
| Next step | Present in `tune` (`next`) and `advise` | Missing in other commands |
| Starting point | README for people | The agent does not know where to start, nor how to install okbase |

## 2. Open items from the MCP lifecycle review (do first, P0)

1. **Stale index within an MCP session.** The stdio and HTTP servers only sync the index at startup; documents edited mid-session (including by the agent itself following `okbase-curate`) are not seen.
   - **Fix:** before each tool call, if the last sync was more than 2 seconds ago, sync incrementally (check only file modification times, reindex changed files). The HTTP server does the same, with a lock to avoid overlapping syncs.
   - **Test:** after editing a file mid-session, `kb_grep` sees it immediately.
2. **Codex scope.** Codex MCP config is currently always global (`~/.codex/config.toml`). Verify whether Codex 0.159 supports a per-project `.codex/config.toml`.
   - If yes: `--codex` installs into the project, `--user` installs globally, like Claude Code.
   - If no: keep the current behavior and document it clearly.
3. **Lifecycle and scope documentation** in `docs/usage.md`:
   - stdio: the agent starts and stops the server, one process per session, N sessions × model RAM.
   - HTTP: run manually or via systemd/Docker, Ctrl-C for a clean stop.
   - A project / user scope table for each agent.

## 3. Design

### 3.1 Single entry point: `okbase onboard`

The agent only needs to remember **one command**. `okbase onboard` reads the actual state of the machine and the bundle and prints
a **setup plan for the agent**, updated after each run:

```
$ okbase onboard            # in the bundle directory (or -b DIR)
okbase onboarding: /home/u/kb (287 docs, ~1.1M tokens, en 92% / vi 8%, level L1)

Done
  ✓ okbase 0.3.0 (okbase-full: embeddings, fine-tuning)
  ✓ index up to date

To do (run in order; stop at every ASK and wait for the user's answer)
  1. [run]  okbase agent install --claude                       writes .mcp.json, .claude/skills
  2. [ask]  "41 documents have no description. May I write descriptions and an index.md
            into /home/u/kb? (files change; review with git diff)"
            yes → follow the okbase-curate skill    no → skip
  3. [ask]  "People ask in Vietnamese but the documents are English. Semantic search needs the
            EmbeddingGemma model (188 MB, Gemma Terms of Use: https://ai.google.dev/gemma/terms).
            Do you accept the license and the download? (MIT alternative: bge-m3, 570 MB, weaker)"
            yes → okbase embed enable --accept-license && okbase embed index
            alt → okbase embed enable --model bge-m3-int8 && okbase embed index
  4. [run]  okbase doctor                                       verify
  5. [tell] "Restart the agent session (or /mcp) to load the okbase tools."

Rules for agents: never pass --accept-license, --yes, --write, --force or --replace unless the user
agreed to that step; never edit documents without asking.
```

- **Has `--json`:** each step has `id`, `kind` (`run` / `ask` / `tell`), `command`, `writes` (paths), `question` (suggested question for the user), `options` (answer → corresponding command), `done_when` (command that checks completion), `why` (reason, citing a spike).
- **Built on existing parts:** `advise` says what should be done, `agent status` / `embed status` / `lint` say what has been done, and the consent catalog (§3.3) says which steps must be asked.
- **Repeatable:** rerun after each step; completed steps move to the "Done" section. The agent keeps looping "onboard → do the first unfinished step" until the list is empty.
- **Options:** `--for claude|codex|team|host`, `--user-langs`, `--private` (as in `advise`), and `--goal` (`answer` | `curate` | `serve-team`) to narrow the plan.

### 3.2 Machine contract (applies to every command)

| Item | Convention |
|---|---|
| `--json` | Every command, including write commands. Write commands return `{"changes": [{action, path, why}], "next": [...]}` |
| `next` | Every JSON result has `next`: a list of suggested next commands (may be empty) |
| Errors | With `--json`, errors are printed to stdout as `{"error": {"code", "message", "hint", "next"}}`. `code` is fixed and documented (e.g. `bundle_not_found`, `name_conflict`, `license_required`, `consent_required`, `not_built`, `invalid_argument`) |
| Exit codes | 0 success · 1 error · 2 usage error (clap) · **3 user consent required** (missing `--yes` / `--accept-license` / `--write`) · **4 findings** (`lint` has errors, `tune submit` rejected, gate failed) |
| Non-interactive | Guarantee: no command waits for TTY input. A test runs every command with stdin closed |
| Preview | Every write command has `--print` (or dry-runs by default, like `clean`) |
| Stability | JSON field names and error codes are snapshot-tested; a change means a version change |

### 3.3 Consent catalog: what the agent must ask

Embedded in the binary (`okbase onboard --json` returns it; the skill and guide quote from it):

| Action | Flag requiring user consent | The suggested question must state |
|---|---|---|
| Accept a model license (Gemma) | `--accept-license` | License name, URL, size, the MIT alternative |
| Download a model, dictionary, Python environment | `--yes` (tune setup/train), `embed index` | Size, storage location |
| Send document passages to an LLM provider (tune) | (a step the agent performs itself) | Whether the documents may be sent |
| Edit documents (`adopt --write`, `lint --fix-safe`, `vocab --write`, curate) | `--write` / file edits | Which files change; recommend git |
| Write agent configuration | `agent install` (no flag needed) | Tell the user beforehand; ask if `--user` |
| Overwrite another server with the same name | `--replace` | Where it currently points, where it will point |
| Delete data | `clean --yes`, `agent uninstall --all` | Size, what will be lost |
| Bypass the quality gate | `tune activate --force` | Gate numbers |

If the flag is missing, the command exits with **code 3**, with `error.code = consent_required` and a `question` field containing the question to ask. The agent only has to relay that question to the user.

### 3.4 Verification: `okbase doctor`

Combines `agent status`, `embed status`, `dict status`, index freshness and build features into **one pass/fail report**, with `--json`, each failure paired with a fix command. Adds a **real MCP probe**: it runs `okbase mcp serve --stdio` for each installed bundle and sends `initialize` → `tools/list` → `kb_catalog`. This lets the agent prove the setup works before telling the user to restart.

### 3.5 How the agent finds okbase (bootstrap)

1. **okbase not installed:** the README has a short **"For agents"** section, plus an `llms.txt` file at the repo root, saying how to install (release binary or `cargo install`), then run `okbase onboard` and follow it. The user only has to paste the repo link to the agent.
2. **okbase installed, skills not installed:** `okbase --help` has the first line "Agents: start with `okbase onboard`". "Not set up" errors also suggest `onboard`.
3. **Already set up:** the `okbase-*` skills handle daily work. A new skill **`okbase-setup`**: when the user asks to add a bundle, enable a feature, move machines or uninstall, it runs `okbase onboard` (or `--goal`), follows the consent catalog and confirms with `doctor`.
4. **Uninstalling also goes through the agent:** `okbase onboard --goal remove` prints an uninstall plan: `agent uninstall --all`, then `clean --all --yes` (ask the user), then the command to remove the binary.

### 3.6 Agent guidance content: one source, many places

Written once, in the binary:
- **Consent rules** (§3.3) and the **command table** (generated from the clap definitions): `okbase help --agent` prints a compact page (< 2k tokens) with the machine contract, the consent catalog, the main commands and examples.
- **Shown again in:** `onboard` output, the `okbase-setup` skill, the AGENTS.md block for Codex, `llms.txt`.
- **Snapshot tests** keep the copies from drifting apart.

## 4. Roadmap

| Phase | Content | Done when |
|---|---|---|
| **O0** (P0) | §2: index auto-sync within MCP sessions; Codex scope; lifecycle docs | Editing a file mid-session is seen by tools immediately; the scope table is verified on Codex 0.159 |
| **O1** | Machine contract §3.2: JSON for write commands, `next`, error codes, exit codes 3/4, non-interactive test | JSON snapshots for every command; a test running every command with stdin closed does not hang |
| **O2** | `okbase onboard` (text + JSON), consent catalog §3.3, `help --agent` | Correct plans on fixtures (small/medium/large, with tables, multilingual); on rerun, completed steps disappear |
| **O3** | `okbase doctor` with a real MCP probe | Detects a moved bundle, a missing binary, an unaccepted license, an MCP server that fails to start |
| **O4** | Bootstrap §3.5: "For agents" section in the README, `llms.txt`, skill `okbase-setup`, `onboard --goal remove` | An agent with only the repo link can install and set up by itself |
| **O5** | **Spike S13:** Claude Code and Codex on a clean HOME, with only the sentence "set up okbase for this folder" | ≥ 9/10 runs complete; 100% of consent steps are asked (0 times adding `--accept-license`/`--yes`/`--write` on its own); measure turns, time, cost |

Estimate: O0 about 1 day; O1–O3 about 1 week; O4–O5 about 3–4 days.

## 5. Decisions (made 2026-10-02)

1. Entry point: **`okbase onboard`**.
2. Exit codes 3 (consent required) and 4 (findings) are adopted before v1.0; `lint` with errors moves from 1 to 4.
3. Project-level `agent install`: the agent only informs the user beforehand, no need to ask; `--user` must be asked.
4. No binaries released yet (in development). Later the source will be opened and published to crates.io, so bootstrap
   uses `cargo install okbase-cli` (after publishing) and for now `cargo install --git <repo>` / `--path`.

## 6. Implementation

| Phase | Commit | Notes |
|---|---|---|
| O0 | `c07acae` | Index auto-syncs before each tool call (if more than 2 seconds old); search auto-embeds up to 64 new chunks. Codex 0.159 reads `<project>/.codex/config.toml` for trusted projects (verified), so `--codex` now installs per project |
| O1–O2 | `903a1f5` | Error codes, JSON errors, exit codes 3/4, JSON for write commands; `okbase onboard` (run/ask/tell, `--goal`); `help --agent`; consent catalog |
| O3 | `ee9d65c` | `okbase doctor`, with a real MCP call |
| O4 | `6167059` | README "For agents", `llms.txt`, skill `okbase-setup`, AGENTS.md block pointing to `onboard`; a test keeps the guidance copies consistent |
| O5 | `be73fa6` | S13 with Claude: 3/3 set up on their own, 0 times adding consent flags on its own (`spikes/onboarding/RESULTS.md`). Fixed 3 bugs found by the agent |
| O5 (Codex) | `b98cebf` (fix) | S13 with Codex (gpt-6.1-sol, gpt-6-luna; 6 folders): 12/12 found `onboard`, 0 times adding consent flags on its own. Codex's workspace-write sandbox blocks writes to `.codex/`: `agent install --codex` only reported "Permission denied" with no hint (`spikes/codex/RESULTS.md`) |

Deviation from the plan: the `next` field is present in setup commands, errors and the `onboard/doctor/tune` commands. Read
commands keep the MCP tool's JSON (AGENTS.md rule: `--json` schema = MCP tool output).
