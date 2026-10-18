---
type: "Service"
title: "Atlas notification worker"
description: "Locate the asynchronous worker and its dependency on the outbox database."
tags: ["service","dependencies","example"]
lang: en
status: draft
updated: "2026-10-18"
example: true
"environment": "production"
"owner": "atlas-team"
"project_id": "atlas"
"server_id": "app-01"
"service_id": "atlas-worker"
---
# Atlas notification worker

> Fictional example: replace this record with your own information before relying on it.

## Mapping
atlas-worker runs on app-01 and reads the PostgreSQL outbox on db-01.
The api-team owns retry correctness. Pending retry age and delivery failures are the health signals; process uptime alone is insufficient.
## Investigation
A slow worker does not prove the [API](api.md) is failing. Inspect [database health](database.md), outbox age and the current deployment separately. Use timestamped, redacted evidence and follow the [change policy](../policies/production-changes.md).
