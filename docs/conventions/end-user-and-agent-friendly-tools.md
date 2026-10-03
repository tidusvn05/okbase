# Conventions: building tools that end users and their agents can use well

These conventions come from designing okbase with its maintainer. They are written to be reused by
other projects: CLIs, libraries with a CLI, MCP servers, developer tools, anything a person may set
up through an AI agent. Each rule has a short "why" and an "in okbase" example. Treat MUST/SHOULD as
in RFC 2119.

---

## 1. Evidence before defaults

1. **Every default is backed by a measurement.** Thresholds, recommended models and limits come from
   spikes or evals that are kept in the repository (inputs, scripts, results). Changing a default needs
   new data. *Why:* users cannot judge defaults; they trust you to have measured. *okbase:* the full
   context limit of 30k tokens, the embedding model and the question counts each cite a spike.
2. **Advice cites its evidence in one line.** When the tool recommends something, it says why, with the
   number behind it ("lexical search reached 90–100% at 4.4M tokens, S4"). No unexplained "recommended".
3. **Report results honestly.** State misses, regressions and what was not tested, next to the gains.
   Never round a failed threshold into a pass. *okbase:* "kept 2/3 of the gain; missed my own 0.92 by
   one question; same-language dropped 3 points".
4. **Measure before switching.** Anything that replaces a working setup (a tuned model, a new index)
   passes a quality gate against the current one first, on data it was not trained on.
5. **Spend the user's money or quota only with consent**, and say the estimated cost first. Report
   overruns as soon as they happen.

## 2. Recommend the simplest setup that works

1. **Offer a ladder, from simplest to most complex**, and recommend climbing only when a measured
   signal says so (size, languages, data types, quality level). Say explicitly what is *not* needed.
   *okbase:* `advise`: full context → lexical tools → curate → SQL for tables → semantic search →
   fine-tuning.
2. **Organization beats machinery.** If cleaning up the user's own material helps more than adding a
   model, recommend that first.
3. **Heavy or risky features are opt-in modules**, never part of the default build or default path
   (models, Python environments, network services).
4. **Give one concrete command per step**, copy-pasteable, with the paths of the user's project.

## 3. Understand the user's real situation before advising

1. **Look before you assume.** The folder the user points at may be a software repository, an empty
   folder, a half-converted dataset, or contain the real target in a subfolder. Scan and classify
   first; never treat the whole thing as your format by default.
2. **Do not impose your structure.** Keep the user's layout and tools (documentation sites, editors,
   vaults). Adapt with profiles instead of asking them to reorganize.
3. **Grade conformance; do not use the weakest link.** "2 of 11 files need fixing" leads to a small
   in-place fix; "below standard" leads to an unnecessary conversion.
4. **Never touch what the user does not own**: dependency folders, build output, ignored files.
   Respect `.gitignore` and offer a tool-specific ignore file.
5. **Cover the start from nothing.** An empty folder needs a scaffold and a way to add the first
   content, not a list of features to enable.
6. **Turn every real situation into a test fixture**, and keep it passing.

## 4. Agents are first-class users

People increasingly ask an agent ("set this up for me") instead of reading docs. The tool must be
operable by an agent from a single sentence.

1. **One entry point.** A single command (okbase: `onboard`) computes the plan from the real state of
   the machine and the project: what is done, what to run next, and what to ask the user. Running it
   again shows progress; finished steps move to "done".
2. **Step kinds:** `run` (safe, do it), `ask` (a question for the user, verbatim, with the commands for
   each answer), `tell` (something only the user can do, e.g. restart a session).
3. **Discoverable from `--help`.** The first lines of `--help` point agents at the entry point and at
   an agent contract page (`help --agent`). A README section "For agents" and an `llms.txt` cover
   agents that start from the repository link.
4. **Verify at the end.** A `doctor` command checks everything, including a real round trip of the
   integration (start the server, list tools, call one), and gives the fix for each problem.
5. **Test with real agents.** Give agents only the user's sentence and the binary; log every call.
   Success criteria: they complete the setup, ask at every decision, and add zero consent flags on their
   own. Every confusion they hit is a bug in the tool, not in the agent.

## 5. The machine contract

1. **Every command accepts `--json`.** Read commands keep a stable schema (in okbase: identical to the
   MCP tool output); setup commands return what they changed and `next` (commands to run next).
2. **Errors are data.** With `--json`, errors are printed as `{"error": {code, message, hint, question,
   flag, next}}` with a stable, documented `code`.
3. **Exit codes carry meaning:** 0 success, 1 error, 2 usage, **3 the user must agree first**,
   **4 the command worked and found problems** (lint errors, a rejected input, a failed gate).
4. **Never wait for input.** No interactive prompts, ever; a test runs every command with stdin closed.
5. **Preview everything that changes state** (`--print`, or a dry run by default for deletions).
6. **Idempotent setup.** Running an install twice converges on the same files.
7. **Stable and tested.** JSON fields, error codes and agent-facing text are covered by snapshot tests.

## 6. Consent: what only the user decides

1. **Keep an explicit consent catalog**, embedded in the tool and shown to agents. Typical entries:
   accepting a license; downloading large files; sending the user's content to a third party
   (including the agent's own model provider); editing the user's files; applying a change to every
   project (user/global scope); overwriting something that serves another purpose; deleting data;
   bypassing a quality gate.
2. **Each consent has one flag** (`--accept-license`, `--yes`, `--write`, `--replace`, `--force`…). The
   tool exits with code 3 without it, and the error carries the exact question to ask (name, URL, size,
   alternatives).
3. **Agents never add consent flags on their own.** State this rule in every agent-facing text.
4. **Never accept a license on the user's behalf**, and never redistribute derived artifacts whose
   license you cannot grant.
5. **Project-local, reversible changes may be done after telling the user**; global, destructive or
   outward-facing ones are asked.

## 7. Safety and reversibility

1. **Read-only by default.** Writing needs an explicit flag or an explicit command.
2. **Edit minimally.** Touch only the targeted keys; keep order, comments and unknown fields;
   round-trips are byte-identical when nothing changed.
3. **Never overwrite silently.** If a name or slot is taken by something else, refuse, say what is
   there, and offer the two ways out (a new name, or an explicit replace).
4. **Every change has an undo.** Rollback restores the previous state byte for byte when the file was
   not edited since; uninstall removes only what the tool added and deletes files that held nothing
   else.
5. **Show before deleting.** Cleanup lists what and how much, then deletes only with consent; it never
   touches the user's content.
6. **Record what you installed** (where, for whom, what files) so status, repair and uninstall work
   later, even after moves and upgrades.

## 8. Complete lifecycle and scope

Design every integration for its whole life, not just the install:

| Stage | Must exist |
|---|---|
| Try | works with zero configuration on the user's existing material |
| Install | per project by default; per user only on request; several instances side by side (names, prefixes) |
| Use | changes to the user's content are picked up without restarting |
| Share | one shared server for a team, clients connect by URL; tokens stay in environment variables |
| Check | `status` / `doctor` detect moved projects, missing binaries, broken configuration |
| Repair / upgrade | re-running install fixes it; conflicts are explicit |
| Stop | uninstall per project, per name, or everything |
| Remove | uninstall all, clean caches (with sizes), uninstall the program |

1. **Be explicit about scope**: for each integrated host, document where configuration goes for
   "this project" and "every project", and verify it against the real host version.
2. **Tie process lifetime to the host when possible** (a stdio server started and stopped by the
   agent); document how to run and stop long-lived servers otherwise.
3. **Several projects and several instances are normal.** Name things per instance and detect
   collisions across projects.

## 9. One source of truth for instructions

1. **Agent instructions live in the binary** (guides, prompts, rules) and are rendered into every
   place agents read them: help pages, skills, AGENTS.md blocks, `llms.txt`.
2. **A test keeps them identical** on the rules that matter (entry point, consent flags).
3. **Short and staged.** Agents get a short instruction at each step (the next batch, the next
   command), not a long manual up front. A full guide exists for agents without skills.
4. **Usage docs are organized by the user's situation** (small / medium / large project, and a
   scenario table: several projects, shared servers, stopping, removing), not by feature list.

## 10. When the tool asks agents to produce data

(For example, writing questions to fine-tune a model.)

1. **Publish a standard**: unit of work, quantities, kinds, languages, length, what not to do, and the
   output format. Version it.
2. **Hand out small batches with a short prompt** and accept them all-or-nothing, with errors listed
   per line so the agent can fix and resubmit.
3. **Validate mechanically**, measure the gain on held-out data split by source (never by item), and
   prefer a small human-written evaluation set over generated ones.
4. **Calibrate validation on real agent output.** Rules that reject good input (e.g. counting words in
   a syllable-based language) are bugs.

## 11. Planning and communication

1. **Recommend; do not survey.** When asking the user to decide, give the recommended option first and
   say why; list only the decisions that are truly theirs.
2. **Plans are phased, each phase with an acceptance criterion**, and record what changed from the plan
   and why after implementation.
3. **Speak the user's language** in conversation; keep the project's public artifacts in the project's
   language.
4. **Save decisions** where future sessions and contributors will find them (plans, memory), so they
   are not asked again.
