---
type: "Guide"
title: "Getting started"
description: "Try this knowledge base, follow its workflows and replace the fictional examples."
tags: ["getting-started"]
lang: en
status: stable
updated: "2026-10-18"
example: false
---
# Getting started

This bundle was created from the `project-assistant` template. All domain records are fictional English examples (`example: true`, `status: draft`). Changing --langs configures languages for new documents and advice; it does not translate these examples.

## Try it

Run commands from this bundle's directory, or add `-b <path>` to each command.

```sh
okbase status
okbase catalog
okbase lint --level L2
okbase onboard
```

Ask your agent these questions; require a citation to the linked source, rather than guessing.

### What does M1 deliver?

Expected evidence: Atlas M1 delivers reliable issue notifications.

Source: [project/context](../project/context.md).

### Which task is complete?

Expected evidence: AT-42 is done

Source: [tasks/at-42](../tasks/at-42.md).

### Why is AT-43 blocked?

Expected evidence: pending the instrumentation export from the platform-team

Source: [tasks/at-43](../tasks/at-43.md).

### Are digest emails an M1 blocker?

Expected evidence: Digest emails are deferred until after M1

Source: [decisions/m1-scope](../decisions/m1-scope.md).

### How many tasks are done and blocked in the report?

Expected evidence: One task is done and one task is blocked

Source: [reports/week-42](../reports/week-42.md).

## Workflows

- [Turn a meeting into tracked work](../workflows/meeting-to-tasks.md): Create linked decisions and owner-assigned tasks without losing the source meeting.
- [Prepare a project status report](../workflows/weekly-report.md): Generate a source-backed report from current task states and decisions.
- [Investigate a delivery blocker](../workflows/investigate-blocker.md): Track evidence, hypotheses and the next experiment alongside the blocked task.

## Make it yours

1. Replace the domain examples with real records. Verify their facts and sources, remove `example: true`, and use `status: stable` only after approval. Remove examples you do not need and update their folder listings and links.
2. Inspect `_meta/types/` and `_meta/vocabulary.md`. Create a record with `okbase new --type <Type> "<Title>" --description "<what it answers>" --tags <existing-tag>`. Fill every required field reported by the command, then write its body and links.
3. Add new fields or types in `_meta/types/<Type>.md`. Declare a new canonical tag in `_meta/vocabulary.md` before using it. Keep `status` as draft/stable/deprecated; use `task_status` or `incident_status` for work in progress.
4. Re-run `okbase lint --level L2` and the relevant trial questions after edits. Reports must name their reporting period, evidence, and unresolved unknowns. Re-running init never updates an existing bundle; upgrading okbase does not rewrite these files.

## Several bundles and bot hosts

Connect another bundle with its own server name, for example `okbase -b ../operations agent install --name okbase-ops`. Query each bundle through its own tool prefix; cross-bundle references do not become local Markdown links.

For a bot, use MCP or the CLI's `--json` output, or the Rust library with a host-provided Scope. Put the scoped catalog in the bot's context. The host decides who may read which documents and provides authorized file-writing tools. MCP tools currently read knowledge; they do not write tasks, deploy services or run schedules. Credentials belong in the host's secret configuration, not knowledge records.
