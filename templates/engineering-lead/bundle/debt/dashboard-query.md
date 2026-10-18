---
type: "Task"
title: "Validate the dashboard query"
description: "Track technical debt with an owner, evidence gap and measurable completion rule."
tags: ["debt","task","example"]
lang: en
status: draft
updated: "2026-10-18"
example: true
"due": "2026-10-16"
"owner": "leo"
"priority": "high"
"project_id": "atlas"
"task_status": "blocked"
---
# Validate the dashboard query

> Fictional example: replace this record with your own information before relying on it.

## Debt and impact
The dashboard query lacks a checked-in definition and a deterministic old-event test.
Owner Leo must capture the query, add the test and document its interpretation before this item can close. The item is blocked on a platform export.
## Completion
Review the actual query against the [RFC definition](../rfcs/retry-dashboard.md). A screenshot or verbal statement alone does not establish correctness.
The [delivery risk](../risks/monitoring-evidence.md) remains open while this task is blocked.
