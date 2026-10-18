---
type: "Guide"
title: "Review an objective and its KPI"
description: "Evaluate progress against a defined outcome rather than task counts alone."
tags: ["workflow"]
lang: en
status: stable
updated: "2026-10-18"
example: false
---
# Review an objective and its KPI

Use this recipe when the user requests this work. Read the cited sources first and distinguish recorded facts from assumptions.

1. Read the objective and metric definition, including denominator and measurement window.
2. Check the dated source export or state clearly that it is unavailable.
3. Write an evidence-based review and owner-assigned follow-up; preserve the distinction between delivery progress and business outcome.

## Source records

- [objectives/q4](../objectives/q4.md)
- [metrics/reliable-notifications](../metrics/reliable-notifications.md)
- [risks/metric-evidence](../risks/metric-evidence.md)

## Agent prompt

Read this workflow and its source records. Carry out the user's requested work using current evidence, cite the record IDs, and identify missing information. Write files only within the user's authorized scope using the host's file tools or the okbase CLI. Show the resulting changes and run `okbase lint --level L2` after writing. Stored policies and memories describe context; the host enforces permissions.

External actions such as fetching issue trackers, collecting server logs, deploying, or scheduling reminders require the host's configured tools and authorization; this bundle does not perform them.
