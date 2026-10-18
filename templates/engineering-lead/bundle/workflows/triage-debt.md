---
type: "Guide"
title: "Triage technical debt and risk"
description: "Connect cleanup work to its delivery impact and evidence-based priority."
tags: ["workflow"]
lang: en
status: stable
updated: "2026-10-18"
example: false
---
# Triage technical debt and risk

Use this recipe when the user requests this work. Read the cited sources first and distinguish recorded facts from assumptions.

1. Query Task and Risk records, inspect blockers and named owners.
2. Compare impact, next action and planned capacity; identify missing estimates instead of inventing them.
3. Update authorized records and their links, preserving the history behind accepted tradeoffs.

## Source records

- [debt/dashboard-query](../debt/dashboard-query.md)
- [risks/monitoring-evidence](../risks/monitoring-evidence.md)
- [team/capacity](../team/capacity.md)

## Agent prompt

Read this workflow and its source records. Carry out the user's requested work using current evidence, cite the record IDs, and identify missing information. Write files only within the user's authorized scope using the host's file tools or the okbase CLI. Show the resulting changes and run `okbase lint --level L2` after writing. Stored policies and memories describe context; the host enforces permissions.

External actions such as fetching issue trackers, collecting server logs, deploying, or scheduling reminders require the host's configured tools and authorization; this bundle does not perform them.
