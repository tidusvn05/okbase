---
type: "Runbook"
title: "Release and rollback checklist"
description: "Prepare a reviewed rollout with compatibility evidence, health checks and a rollback boundary."
tags: ["release","rollback","example"]
lang: en
status: draft
updated: "2026-10-18"
example: true
"owner": "atlas-team"
"project_id": "atlas"
---
# Release and rollback checklist

> Fictional example: replace this record with your own information before relying on it.

## Before rollout
Identify the release commit and owner, complete the [test plan](../development/testing.md), review schema compatibility, and name the previous working artifact. Confirm the actual deployment commands with the platform owner.
## After rollout
Check create-issue success rate and outbox lag. Roll back if create-issue errors exceed the agreed release threshold for five minutes; record the real threshold before using this checklist.
A rollback reuses the previous application artifact and keeps additive schema changes. Do not reverse a destructive migration without a separate recovery plan.
Update [architecture documentation](../architecture/system.md) when topology changes, and record the result and evidence in the release report.
