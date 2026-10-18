---
type: "Service"
title: "Atlas API service"
description: "Map the API to its production host, data dependency and deployment record."
tags: ["service","mapping","example"]
lang: en
status: draft
updated: "2026-10-18"
example: true
"environment": "production"
"owner": "atlas-team"
"project_id": "atlas"
"server_id": "app-01"
"service_id": "atlas-api"
---
# Atlas API service

> Fictional example: replace this record with your own information before relying on it.

## Mapping
atlas-api runs on app-01 in production and depends on atlas-db on db-01.
The api-team owns API correctness; the platform-team owns rollout execution. The worker is asynchronous and must not block successful create responses.
## Operational evidence
The current example rollout is [API 1.4.2](../deployments/api-1-4-2.md). Read [INC-07](../incidents/inc-07.md) before drawing conclusions about its health.
Follow the [deployment runbook](../runbooks/deploy-api.md) and [rollback runbook](../runbooks/rollback-api.md).
