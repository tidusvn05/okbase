---
type: "Guide"
title: "API and worker test plan"
description: "Cover transaction, idempotency and notification behavior before accepting an Atlas change."
tags: ["testing","quality","example"]
lang: en
status: draft
updated: "2026-10-18"
example: true
"owner": "atlas-team"
"project_id": "atlas"
---
# API and worker test plan

> Fictional example: replace this record with your own information before relying on it.

## Required checks
Test the issue insert and outbox insert as one transaction, including a forced rollback. Test notification retries and duplicate event delivery.
The compatibility test must cover HTTP 409 for an idempotency key reused with a different payload.
## Evidence
Record the commit, command and result in the PR; a green local unit test does not establish production health. For a new migration, test both the previous and new application version against the expanded schema.
Use the [API contract](../api/issues.md) as the oracle and the [release checklist](../delivery/release.md) for rollout gates.
