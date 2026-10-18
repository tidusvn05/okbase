---
type: "Policy"
title: "Memory capture and correction policy"
description: "Decide what to retain, how to cite it and how to correct outdated assistant memory."
tags: ["memory","policy","example"]
lang: en
status: draft
updated: "2026-10-18"
example: true
"owner": "atlas-team"
"project_id": "atlas"
---
# Memory capture and correction policy

> Fictional example: replace this record with your own information before relying on it.

## Capture
Store durable preferences as Memory records with memory_kind: preference and observed_at. Store project facts separately, with their source and observation date. Keep temporary working context in Journal records.
Only retain information within the user's requested scope. Use links to authoritative records instead of copying entire conversations.
## Correction and privacy
A changed preference replaces the previous active preference; mark the old record deprecated and link supersedes in the new record. Review personal context before sharing it with a team or another bot. The host must enforce the audience's read scope.
## Action boundary
A saved memory is context, not permission to send messages, spend money or run a deployment. Read the [preferences](../profile/preferences.md) and [handoff](../handoffs/next-session.md) within the current user's authorization.
