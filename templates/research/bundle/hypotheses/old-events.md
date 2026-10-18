---
type: "Hypothesis"
title: "Recent retries can hide old pending events"
description: "State a falsifiable hypothesis separately from observed experiment results."
tags: ["hypothesis","monitoring","example"]
lang: en
status: draft
updated: "2026-10-18"
example: true
"owner": "atlas-team"
"project_id": "atlas"
---
# Recent retries can hide old pending events

> Fictional example: replace this record with your own information before relying on it.

## Hypothesis
Measuring only the latest retry attempt can hide an older event that is still pending.
Falsify this by testing a query that claims to measure backlog age against both an old pending event and a recent successfully delivered event.
## Evidence boundary
A plausible mechanism is not a confirmed production incident cause. Read the [experiment protocol](../experiments/retry-age.md) and [results](../evidence/demo-result.md) before drawing a conclusion.
