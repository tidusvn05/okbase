---
type: "Policy"
title: "Engineering change review standard"
description: "Define the review evidence needed for compatibility, operational impact and document updates."
tags: ["standards","quality","example"]
lang: en
status: draft
updated: "2026-10-18"
example: true
"owner": "atlas-team"
"project_id": "atlas"
---
# Engineering change review standard

> Fictional example: replace this record with your own information before relying on it.

## Review standard
A behavior-changing PR must link its requirement or decision, include test evidence and identify any documentation that needs updating.
Changes to retry behavior need an idempotency case and a failure case. Production-facing changes also name an operational owner and rollback boundary.
## Record decisions
Use the [RFC](../rfcs/retry-dashboard.md) for an unresolved design tradeoff and the [debt record](../debt/dashboard-query.md) for accepted cleanup work. An accepted RFC does not itself authorize a deployment.
