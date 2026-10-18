---
type: "Service"
title: "Atlas PostgreSQL service"
description: "Describe the durable dependency shared by the API and notification worker."
tags: ["service","database","example"]
lang: en
status: draft
updated: "2026-10-18"
example: true
"environment": "production"
"owner": "atlas-team"
"project_id": "atlas"
"server_id": "db-01"
"service_id": "atlas-db"
---
# Atlas PostgreSQL service

> Fictional example: replace this record with your own information before relying on it.

## Mapping
atlas-db runs on db-01 and stores issues plus the outbox.
The platform-team owns recovery. API and worker availability depend on database connectivity, but only captured evidence can establish whether a particular incident was caused by it.
## Changes
Application rollback keeps additive schema changes. Any destructive migration needs its own approved recovery procedure and verified backups.
See [db-01](../servers/db-01.md) and the [production change policy](../policies/production-changes.md).
