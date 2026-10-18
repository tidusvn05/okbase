---
type: "Environment"
title: "Production environment"
description: "Identify the Atlas production boundary and authoritative service and host records."
tags: ["environment","mapping","example"]
lang: en
status: draft
updated: "2026-10-18"
example: true
"environment": "production"
"owner": "atlas-team"
"project_id": "atlas"
---
# Production environment

> Fictional example: replace this record with your own information before relying on it.

## Boundary
Production runs the Atlas API and worker on app-01 and PostgreSQL on db-01.
The ingress endpoint in this fictional topology is api.atlas.example.invalid. Staging is a separate environment; production evidence must not be inferred from staging health.
## Ownership and records
The platform-team owns infrastructure. Read [app-01](../servers/app-01.md), [db-01](../servers/db-01.md), [API](../services/api.md), [worker](../services/worker.md), and [database](../services/database.md).
Use the [change policy](../policies/production-changes.md) before any external action.
