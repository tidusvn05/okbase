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

This bundle was created from the `product-management` template. All domain records are fictional English examples (`example: true`, `status: draft`). Changing --langs configures languages for new documents and advice; it does not translate these examples.

## Try it

Run commands from this bundle's directory, or add `-b <path>` to each command.

```sh
okbase status
okbase catalog
okbase lint --level L2
okbase onboard
```

Ask your agent these questions; require a citation to the linked source, rather than guessing.

### Who is Atlas for?

Expected evidence: small engineering teams that lose track of issue ownership during handoffs

Source: [product/vision](../product/vision.md).

### How much interview evidence is available?

Expected evidence: Three fictional team leads

Source: [research/handoff-interviews](../research/handoff-interviews.md).

### What must happen when an assignment event is replayed?

Expected evidence: Replaying an assignment event must not send a second notification.

Source: [requirements/notification-reliability](../requirements/notification-reliability.md).

### Why were digest emails deferred?

Expected evidence: protect the M1 notification reliability scope

Source: [decisions/defer-digest](../decisions/defer-digest.md).

### What acceptance evidence is missing?

Expected evidence: retry-age monitoring evidence is still missing

Source: [reports/m1-review](../reports/m1-review.md).

## Workflows

- [Write a source-backed PRD](../workflows/write-prd.md): Turn a user problem into testable acceptance criteria and explicit exclusions.
- [Review product priorities](../workflows/review-priorities.md): Expose the tradeoff and evidence behind a roadmap change.
- [Assess product acceptance](../workflows/assess-release.md): Check release claims against requirements and the evidence cutoff.

## Make it yours

1. Replace the domain examples with real records. Verify their facts and sources, remove `example: true`, and use `status: stable` only after approval. Remove examples you do not need and update their folder listings and links.
2. Inspect `_meta/types/` and `_meta/vocabulary.md`. Create a record with `okbase new --type <Type> "<Title>" --description "<what it answers>" --tags <existing-tag>`. Fill every required field reported by the command, then write its body and links.
3. Add new fields or types in `_meta/types/<Type>.md`. Declare a new canonical tag in `_meta/vocabulary.md` before using it. Keep `status` as draft/stable/deprecated; use `task_status` or `incident_status` for work in progress.
4. Re-run `okbase lint --level L2` and the relevant trial questions after edits. Reports must name their reporting period, evidence, and unresolved unknowns. Re-running init never updates an existing bundle; upgrading okbase does not rewrite these files.

## Several bundles and bot hosts

Connect another bundle with its own server name, for example `okbase -b ../operations agent install --name okbase-ops`. Query each bundle through its own tool prefix; cross-bundle references do not become local Markdown links.

For a bot, use MCP or the CLI's `--json` output, or the Rust library with a host-provided Scope. Put the scoped catalog in the bot's context. The host decides who may read which documents and provides authorized file-writing tools. MCP tools currently read knowledge; they do not write tasks, deploy services or run schedules. Credentials belong in the host's secret configuration, not knowledge records.
