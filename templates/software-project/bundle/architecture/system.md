---
type: "Reference"
title: "Atlas system architecture"
description: "Trace request and notification paths across the web client, API, database and worker."
tags: ["architecture","dependencies","example"]
lang: en
status: draft
updated: "2026-10-18"
example: true
"owner": "atlas-team"
"project_id": "atlas"
---
# Atlas system architecture

> Fictional example: replace this record with your own information before relying on it.

## Request path
The request path is web client -> Atlas API -> PostgreSQL. Successful writes add an outbox row in the same transaction; the worker reads the outbox and sends notifications.
The API is stateless. PostgreSQL stores durable issues and outbox records; Redis holds disposable rate-limit counters only.
## Failure boundaries
A notification failure must not roll back an accepted issue. The worker retries with the event ID as its idempotency key.
See [the storage ADR](../decisions/postgresql.md), [the data model](../data-model/issues.md), and [API error behavior](../api/issues.md).
