---
type: "Decision"
title: "Use PostgreSQL for issues and the outbox"
description: "Explain the storage choice, rejected alternative and conditions for reconsideration."
tags: ["architecture","decision","example"]
lang: en
status: draft
updated: "2026-10-18"
example: true
"decision_status": "accepted"
"owner": "atlas-team"
"project_id": "atlas"
---
# Use PostgreSQL for issues and the outbox

> Fictional example: replace this record with your own information before relying on it.

## Decision
PostgreSQL keeps the issue write and outbox insert in one transaction.
We chose it over a document store because a lost outbox event would leave users with accepted issues but missing notifications. A queue-only design would introduce a second durable-write boundary.
## Reconsider when
Revisit this decision if the measured write workload exceeds the capacity of the current primary after query and index tuning. Do not migrate based on an unmeasured scaling concern.
The [architecture](../architecture/system.md) and [data model](../data-model/issues.md) depend on this decision.
