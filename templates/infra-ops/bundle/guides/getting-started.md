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

This bundle was created from the `infra-ops` template. All domain records are fictional English examples (`example: true`, `status: draft`). Changing --langs configures languages for new documents and advice; it does not translate these examples.

## Try it

Run commands from this bundle's directory, or add `-b <path>` to each command.

```sh
okbase status
okbase catalog
okbase lint --level L2
okbase onboard
```

Ask your agent these questions; require a citation to the linked source, rather than guessing.

### Where does atlas-api run and what does it depend on?

Expected evidence: atlas-api runs on app-01 in production and depends on atlas-db on db-01.

Source: [services/api](../services/api.md).

### Which services share app-01?

Expected evidence: app-01 hosts atlas-api and atlas-worker in production.

Source: [servers/app-01](../servers/app-01.md).

### What is the previous working application version?

Expected evidence: the previous working application artifact is 1.4.1

Source: [runbooks/rollback-api](../runbooks/rollback-api.md).

### Is the cause of INC-07 confirmed?

Expected evidence: the causal mechanism remains unconfirmed

Source: [incidents/inc-07](../incidents/inc-07.md).

### What should the postmortem investigate next?

Expected evidence: compare connection-pool settings between 1.4.1 and 1.4.2

Source: [postmortems/inc-07](../postmortems/inc-07.md).

## Workflows

- [Trace infrastructure impact](../workflows/map-impact.md): Identify the affected environment, services, servers and owners before taking action.
- [Review a deployment and rollback plan](../workflows/review-deployment.md): Collect the target, artifact, policy and recovery boundary for an authorized operator.
- [Investigate and record an incident](../workflows/investigate-incident.md): Connect a service incident to timestamped evidence, hypotheses and follow-up work.

## Make it yours

1. Replace the domain examples with real records. Verify their facts and sources, remove `example: true`, and use `status: stable` only after approval. Remove examples you do not need and update their folder listings and links.
2. Inspect `_meta/types/` and `_meta/vocabulary.md`. Create a record with `okbase new --type <Type> "<Title>" --description "<what it answers>" --tags <existing-tag>`. Fill every required field reported by the command, then write its body and links.
3. Add new fields or types in `_meta/types/<Type>.md`. Declare a new canonical tag in `_meta/vocabulary.md` before using it. Keep `status` as draft/stable/deprecated; use `task_status` or `incident_status` for work in progress.
4. Re-run `okbase lint --level L2` and the relevant trial questions after edits. Reports must name their reporting period, evidence, and unresolved unknowns. Re-running init never updates an existing bundle; upgrading okbase does not rewrite these files.

## Several bundles and bot hosts

Connect another bundle with its own server name, for example `okbase -b ../operations agent install --name okbase-ops`. Query each bundle through its own tool prefix; cross-bundle references do not become local Markdown links.

For a bot, use MCP or the CLI's `--json` output, or the Rust library with a host-provided Scope. Put the scoped catalog in the bot's context. The host decides who may read which documents and provides authorized file-writing tools. MCP tools currently read knowledge; they do not write tasks, deploy services or run schedules. Credentials belong in the host's secret configuration, not knowledge records.
