---
type: "Guide"
title: "Prepare an executive briefing"
description: "Connect strategic outcomes to dated evidence and a concrete decision request."
tags: ["workflow"]
lang: en
status: stable
updated: "2026-10-18"
example: false
---
# Prepare an executive briefing

Use this recipe when the user requests this work. Read the cited sources first and distinguish recorded facts from assumptions.

1. Read strategy, objectives, initiatives and the previous report.
2. Check KPI definitions and actual evidence; list unknown values explicitly.
3. Draft the briefing with reporting dates, source links, risks and the decision required; do not invent financial or customer outcomes.

## Source records

- [strategy/direction](../strategy/direction.md)
- [metrics/reliable-notifications](../metrics/reliable-notifications.md)
- [reports/week-42](../reports/week-42.md)

## Agent prompt

Read this workflow and its source records. Carry out the user's requested work using current evidence, cite the record IDs, and identify missing information. Write files only within the user's authorized scope using the host's file tools or the okbase CLI. Show the resulting changes and run `okbase lint --level L2` after writing. Stored policies and memories describe context; the host enforces permissions.

External actions such as fetching issue trackers, collecting server logs, deploying, or scheduling reminders require the host's configured tools and authorization; this bundle does not perform them.
