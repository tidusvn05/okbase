---
type: "Guide"
title: "Triage and escalate a case"
description: "Collect the smallest useful evidence set and route the issue to its owner."
tags: ["workflow"]
lang: en
status: stable
updated: "2026-10-18"
example: false
---
# Triage and escalate a case

Use this recipe when the user requests this work. Read the cited sources first and distinguish recorded facts from assumptions.

1. Read the intake procedure and sample case.
2. Gather the event identity, timezone and redacted evidence using configured tools.
3. Update the authorized case record, name the escalation owner and next update, and preserve unverified hypotheses as such.

## Source records

- [procedures/triage-case](../procedures/triage-case.md)
- [cases/cs-17](../cases/cs-17.md)
- [guides/missing-notification](../guides/missing-notification.md)

## Agent prompt

Read this workflow and its source records. Carry out the user's requested work using current evidence, cite the record IDs, and identify missing information. Write files only within the user's authorized scope using the host's file tools or the okbase CLI. Show the resulting changes and run `okbase lint --level L2` after writing. Stored policies and memories describe context; the host enforces permissions.

External actions such as fetching issue trackers, collecting server logs, deploying, or scheduling reminders require the host's configured tools and authorization; this bundle does not perform them.
