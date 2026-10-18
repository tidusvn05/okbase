---
type: "Experiment"
title: "Two-event retry-age experiment"
description: "Specify the small input set and expected observations needed to compare monitoring definitions."
tags: ["experiment","reproducibility","example"]
lang: en
status: draft
updated: "2026-10-18"
example: true
"owner": "atlas-team"
"project_id": "atlas"
---
# Two-event retry-age experiment

> Fictional example: replace this record with your own information before relying on it.

## Input and fixed clock
At a fixed clock of 12:00 UTC, event A has been pending since 10:00 UTC; event B was created at 11:59 UTC and delivered at 12:00 UTC.
## Expected comparison
Oldest pending event age should be 120 minutes; latest successful attempt age is zero minutes.
Run candidate queries against this exact fixture and preserve the query text, clock and result. The demo records the hand-calculated expectation; no live query was executed.
Read the [evidence record](../evidence/demo-result.md) and [limitations](../research/limitations.md).
