---
type: "Investigation"
title: "INV-01: retry-age calculation"
description: "Separate a monitoring hypothesis from confirmed evidence and its follow-up task."
tags: ["investigation","monitoring","example"]
lang: en
status: draft
updated: "2026-10-18"
example: true
"finding_kind": "hypothesis"
"investigation_status": "investigating"
"owner": "atlas-team"
"project_id": "atlas"
---
# INV-01: retry-age calculation

> Fictional example: replace this record with your own information before relying on it.

## Question
Does the dashboard measure the age of the oldest pending retry or only recently retried events?
## Evidence and hypothesis
The exported query is missing. The hypothesis is that recent successful retries hide older pending events; this has not been confirmed.
## Next experiment
Inspect the query, then compare it with a sample containing one old pending event and one recent successful event. Record inputs, expected result and actual result before concluding.
The work is tracked by [AT-43](../tasks/at-43.md) and summarized in the [weekly report](../reports/week-42.md).
