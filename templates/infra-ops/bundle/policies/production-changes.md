---
type: "Policy"
title: "Production change policy"
description: "State ownership, review and execution boundaries for infrastructure changes."
tags: ["policy","deployment","example"]
lang: en
status: draft
updated: "2026-10-18"
example: true
"owner": "atlas-team"
"project_id": "atlas"
---
# Production change policy

> Fictional example: replace this record with your own information before relying on it.

## Before a change
A production change requires a named change owner, a reviewed deployment plan, a known previous artifact and a rollback decision owner.
The fictional rollout requires approval from the platform on-call before execution. This record describes the team's process; the execution host must enforce actual permissions.
## Evidence and secrets
Record environment, service, version, timestamp, result and evidence location. Store only secret references in knowledge; keep credentials in the host's secret manager.
Follow the [deployment](../runbooks/deploy-api.md) and [rollback](../runbooks/rollback-api.md) procedures. Adapt this example policy to the real team's approved policy before use.
