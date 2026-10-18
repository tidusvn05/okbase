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

This bundle was created from the `research` template. All domain records are fictional English examples (`example: true`, `status: draft`). Changing --langs configures languages for new documents and advice; it does not translate these examples.

## Try it

Run commands from this bundle's directory, or add `-b <path>` to each command.

```sh
okbase status
okbase catalog
okbase lint --level L2
okbase onboard
```

Ask your agent these questions; require a citation to the linked source, rather than guessing.

### What research question is being investigated?

Expected evidence: Which retry-age definition reveals an old pending event

Source: [research/question](../research/question.md).

### What is the hypothesis?

Expected evidence: Measuring only the latest retry attempt can hide an older event

Source: [hypotheses/old-events](../hypotheses/old-events.md).

### What should the oldest pending age be in the fixture?

Expected evidence: Oldest pending event age should be 120 minutes

Source: [experiments/retry-age](../experiments/retry-age.md).

### Are these measured production results?

Expected evidence: hand-calculated expected results, not measured production data

Source: [evidence/demo-result](../evidence/demo-result.md).

### What must happen before accepting the recommendation?

Expected evidence: inspect a real query result and its limitations

Source: [decisions/monitoring-definition](../decisions/monitoring-definition.md).

## Workflows

- [Frame an investigation](../workflows/frame-investigation.md): Write a falsifiable question and keep assumptions separate from observations.
- [Record a reproducible experiment](../workflows/record-experiment.md): Preserve inputs, procedure, clock and actual output so the result can be checked.
- [Write an evidence-based recommendation](../workflows/write-recommendation.md): Connect a decision proposal to observations, alternatives and remaining uncertainty.

## Make it yours

1. Replace the domain examples with real records. Verify their facts and sources, remove `example: true`, and use `status: stable` only after approval. Remove examples you do not need and update their folder listings and links.
2. Inspect `_meta/types/` and `_meta/vocabulary.md`. Create a record with `okbase new --type <Type> "<Title>" --description "<what it answers>" --tags <existing-tag>`. Fill every required field reported by the command, then write its body and links.
3. Add new fields or types in `_meta/types/<Type>.md`. Declare a new canonical tag in `_meta/vocabulary.md` before using it. Keep `status` as draft/stable/deprecated; use `task_status` or `incident_status` for work in progress.
4. Re-run `okbase lint --level L2` and the relevant trial questions after edits. Reports must name their reporting period, evidence, and unresolved unknowns. Re-running init never updates an existing bundle; upgrading okbase does not rewrite these files.

## Several bundles and bot hosts

Connect another bundle with its own server name, for example `okbase -b ../operations agent install --name okbase-ops`. Query each bundle through its own tool prefix; cross-bundle references do not become local Markdown links.

For a bot, use MCP or the CLI's `--json` output, or the Rust library with a host-provided Scope. Put the scoped catalog in the bot's context. The host decides who may read which documents and provides authorized file-writing tools. MCP tools currently read knowledge; they do not write tasks, deploy services or run schedules. Credentials belong in the host's secret configuration, not knowledge records.
