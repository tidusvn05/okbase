---
type: "API"
title: "Create issue API contract"
description: "Specify issue creation, idempotency, errors and compatibility expectations."
tags: ["api","contract","example"]
lang: en
status: draft
updated: "2026-10-18"
example: true
"owner": "atlas-team"
"project_id": "atlas"
---
# Create issue API contract

> Fictional example: replace this record with your own information before relying on it.

## POST /v1/issues
Required fields: project_id, title. The authenticated principal must have write access to the project. A successful response is HTTP 201 with issue_id and version.
An Idempotency-Key is required for retried create requests. Repeating the same key and payload returns the original issue; repeating a key with a different payload returns HTTP 409.
## Validation and compatibility
An empty title returns HTTP 422. Unknown project IDs return HTTP 404. Adding an optional response field is compatible; renaming issue_id needs a versioned migration.
Persist using the [issue data model](../data-model/issues.md) and add cases to the [test plan](../development/testing.md).
