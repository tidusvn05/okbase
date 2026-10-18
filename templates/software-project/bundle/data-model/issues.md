---
type: "Reference"
title: "Issue and outbox data model"
description: "Define durable issue fields and the outbox relationship used for notifications."
tags: ["data-model","architecture","example"]
lang: en
status: draft
updated: "2026-10-18"
example: true
"owner": "atlas-team"
"project_id": "atlas"
---
# Issue and outbox data model

> Fictional example: replace this record with your own information before relying on it.

## Records
An issue has issue_id, project_id, title, version, created_at and updated_at. issue_id is immutable; updates increment version for optimistic concurrency.
An outbox event has event_id, issue_id, event_type, payload and delivered_at. Both the issue and its creation event commit together.
## Change rules
Never reuse a deleted issue ID. Schema changes use expand-and-contract migrations: introduce a compatible field before removing its predecessor.
This follows the [storage decision](../decisions/postgresql.md) and supports the [create contract](../api/issues.md).
