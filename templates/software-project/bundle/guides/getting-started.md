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

This bundle was created from the `software-project` template. All domain records are fictional English examples (`example: true`, `status: draft`). Changing --langs configures languages for new documents and advice; it does not translate these examples.

## Try it

Run commands from this bundle's directory, or add `-b <path>` to each command.

```sh
okbase status
okbase catalog
okbase lint --level L2
okbase onboard
```

Ask your agent these questions; require a citation to the linked source, rather than guessing.

### Why was PostgreSQL chosen?

Expected evidence: PostgreSQL keeps the issue write and outbox insert in one transaction.

Source: [decisions/postgresql](../decisions/postgresql.md).

### What happens when an idempotency key is reused with a different payload?

Expected evidence: repeating a key with a different payload returns HTTP 409

Source: [api/issues](../api/issues.md).

### Which data can Redis lose without losing issues?

Expected evidence: Redis holds disposable rate-limit counters only.

Source: [architecture/system](../architecture/system.md).

### What must the compatibility test cover?

Expected evidence: HTTP 409 for an idempotency key reused with a different payload

Source: [development/testing](../development/testing.md).

### What schema changes survive an application rollback?

Expected evidence: keeps additive schema changes

Source: [delivery/release](../delivery/release.md).

## Workflows

- [Onboard a developer](../workflows/onboard-developer.md): Build an accurate mental model before modifying the software.
- [Change an API safely](../workflows/change-contract.md): Connect a behavior change to its decision, tests and documentation.
- [Prepare a release review](../workflows/prepare-release.md): Collect compatibility evidence and a concrete rollback boundary.

## Make it yours

1. Replace the domain examples with real records. Verify their facts and sources, remove `example: true`, and use `status: stable` only after approval. Remove examples you do not need and update their folder listings and links.
2. Inspect `_meta/types/` and `_meta/vocabulary.md`. Create a record with `okbase new --type <Type> "<Title>" --description "<what it answers>" --tags <existing-tag>`. Fill every required field reported by the command, then write its body and links.
3. Add new fields or types in `_meta/types/<Type>.md`. Declare a new canonical tag in `_meta/vocabulary.md` before using it. Keep `status` as draft/stable/deprecated; use `task_status` or `incident_status` for work in progress.
4. Re-run `okbase lint --level L2` and the relevant trial questions after edits. Reports must name their reporting period, evidence, and unresolved unknowns. Re-running init never updates an existing bundle; upgrading okbase does not rewrite these files.

## Several bundles and bot hosts

Connect another bundle with its own server name, for example `okbase -b ../operations agent install --name okbase-ops`. Query each bundle through its own tool prefix; cross-bundle references do not become local Markdown links.

For a bot, use MCP or the CLI's `--json` output, or the Rust library with a host-provided Scope. Put the scoped catalog in the bot's context. The host decides who may read which documents and provides authorized file-writing tools. MCP tools currently read knowledge; they do not write tasks, deploy services or run schedules. Credentials belong in the host's secret configuration, not knowledge records.
