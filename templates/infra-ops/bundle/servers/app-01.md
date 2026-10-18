---
type: "Server"
title: "Application server app-01"
description: "Locate the API and worker processes and the team responsible for the application host."
tags: ["server","mapping","example"]
lang: en
status: draft
updated: "2026-10-18"
example: true
"environment": "production"
"owner": "atlas-team"
"project_id": "atlas"
"server_id": "app-01"
---
# Application server app-01

> Fictional example: replace this record with your own information before relying on it.

## Hosted services
app-01 hosts atlas-api and atlas-worker in production.
The platform-team owns the host; the api-team owns application behavior. Host address and login details are omitted deliberately; use the real inventory and secret manager when adapting this record.
## Dependency and impact
Both services depend on [db-01](db-01.md). A restart affects API traffic and notification processing.
Read [the API service](../services/api.md), [worker service](../services/worker.md), and [change policy](../policies/production-changes.md).
