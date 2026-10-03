---
name: okbase-setup
description: Set up, repair, extend or remove okbase for the knowledge bundle at {{bundle}} (connect agents, enable semantic search, add another bundle, fix broken tools after a move or upgrade, uninstall). Use when the user asks to install, configure, fix or remove okbase, or when the okbase tools fail.
---

# Setting up okbase

okbase plans the setup from the real state of the machine; you carry it out and the user decides.

1. **Plan.** Run `okbase -b {{bundle}} onboard --json` (or `--goal curate`, `--goal remove`). Read `steps` in order; `done` lists what is already in place.
2. **Do the first step.** `run`: run its commands. `ask`: ask the user its `question` verbatim and wait; run only the commands of the option they choose. `tell`: tell the user.
3. **Repeat** step 1 until `steps` is empty. Finished steps move to `done`.
4. **Consent rules.** Never add `--accept-license`, `--yes`, `--write`, `--force`, `--replace` or `--send-documents` on your own. A command that exits with code 3 needs the user's consent: relay its `error.question`, and on yes run `error.next`. Never edit documents without asking.
5. **Verify and report.** `okbase -b {{bundle}} doctor` must report `ok: true` (it starts the MCP server for real). Then tell the user in a few lines what is set up, how to use it now (`report.use_now` of `onboard --json`: usually restart the agent session once) and the optional upgrades (`report.optional`), which you do only if asked.

Inside Codex, its sandbox keeps `.codex/` read-only: okbase cannot connect Codex from there (`onboard` shows a `tell` step; `agent install --codex` exits 3 with `sandbox_blocked`). Ask the user to run `okbase agent install --codex` in their own terminal; meanwhile use the okbase CLI.

Another bundle in the same project: `okbase -b <other folder> agent install --name okbase-<short name>` (its tools get their own prefix). The full contract: `okbase help --agent`.
