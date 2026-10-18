---
type: "Guide"
title: "Investigate a delivery blocker"
description: "Track evidence, hypotheses and the next experiment alongside the blocked task."
tags: ["workflow"]
lang: en
status: stable
updated: "2026-10-18"
example: false
---
# Investigate a delivery blocker

Use this recipe when the user requests this work. Read the cited sources first and distinguish recorded facts from assumptions.

1. Read the task, its dependency and existing investigation.
2. List observed evidence separately from hypotheses; use external log or tracker tools only if available and authorized.
3. Record experiment results, update the task only when its acceptance evidence changes, and refresh affected documentation and reports.

## Source records

- [tasks/at-43](../tasks/at-43.md)
- [investigations/retry-age](../investigations/retry-age.md)
- [reports/week-42](../reports/week-42.md)

## Agent prompt

Read this workflow and its source records. Carry out the user's requested work using current evidence, cite the record IDs, and identify missing information. Write files only within the user's authorized scope using the host's file tools or the okbase CLI. Show the resulting changes and run `okbase lint --level L2` after writing. Stored policies and memories describe context; the host enforces permissions.

External actions such as fetching issue trackers, collecting server logs, deploying, or scheduling reminders require the host's configured tools and authorization; this bundle does not perform them.
