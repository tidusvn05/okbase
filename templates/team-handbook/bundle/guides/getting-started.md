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

This bundle was created from the `team-handbook` template. All domain records are fictional English examples (`example: true`, `status: draft`). Changing --langs configures languages for new documents and advice; it does not translate these examples.

## Try it

Run commands from this bundle's directory, or add `-b <path>` to each command.

```sh
okbase status
okbase catalog
okbase lint --level L2
okbase onboard
```

Ask your agent these questions; require a citation to the linked source, rather than guessing.

### What is the team mission?

Expected evidence: reliable issue handoffs for small engineering teams

Source: [team/charter](../team/charter.md).

### Who owns monitoring exports?

Expected evidence: deployment and monitoring exports go to the platform-team

Source: [team/ownership](../team/ownership.md).

### What should a new member read first?

Expected evidence: Start by reading the team charter

Source: [onboarding/first-week](../onboarding/first-week.md).

### When does a discussion become an accepted decision?

Expected evidence: when its owner records the outcome and rationale

Source: [policies/communication](../policies/communication.md).

### What must an access request contain?

Expected evidence: the project, required role, business reason and approving owner

Source: [policies/access](../policies/access.md).

## Workflows

- [Onboard a team member](../workflows/onboard-member.md): Build context and route setup requests to the right owner.
- [Capture a working agreement](../workflows/capture-working-agreement.md): Turn a discussion into a durable, reviewable policy or decision.
- [Route a blocked request](../workflows/route-request.md): Find the owner and provide enough evidence to unblock a teammate.

## Make it yours

1. Replace the domain examples with real records. Verify their facts and sources, remove `example: true`, and use `status: stable` only after approval. Remove examples you do not need and update their folder listings and links.
2. Inspect `_meta/types/` and `_meta/vocabulary.md`. Create a record with `okbase new --type <Type> "<Title>" --description "<what it answers>" --tags <existing-tag>`. Fill every required field reported by the command, then write its body and links.
3. Add new fields or types in `_meta/types/<Type>.md`. Declare a new canonical tag in `_meta/vocabulary.md` before using it. Keep `status` as draft/stable/deprecated; use `task_status` or `incident_status` for work in progress.
4. Re-run `okbase lint --level L2` and the relevant trial questions after edits. Reports must name their reporting period, evidence, and unresolved unknowns. Re-running init never updates an existing bundle; upgrading okbase does not rewrite these files.

## Several bundles and bot hosts

Connect another bundle with its own server name, for example `okbase -b ../operations agent install --name okbase-ops`. Query each bundle through its own tool prefix; cross-bundle references do not become local Markdown links.

For a bot, use MCP or the CLI's `--json` output, or the Rust library with a host-provided Scope. Put the scoped catalog in the bot's context. The host decides who may read which documents and provides authorized file-writing tools. MCP tools currently read knowledge; they do not write tasks, deploy services or run schedules. Credentials belong in the host's secret configuration, not knowledge records.
