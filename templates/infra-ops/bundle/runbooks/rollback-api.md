---
type: "Runbook"
title: "Roll back the Atlas API"
description: "Explain the application rollback boundary and evidence required to call recovery complete."
tags: ["runbook","rollback","example"]
lang: en
status: draft
updated: "2026-10-18"
example: true
"owner": "atlas-team"
"project_id": "atlas"
---
# Roll back the Atlas API

> Fictional example: replace this record with your own information before relying on it.

## Boundary
For the example 1.4.2 incident, the previous working application artifact is 1.4.1.
An application rollback preserves additive database migrations. Do not reverse a destructive migration by guessing a down command.
## Verification
The decision owner authorizes rollback through the deployment tooling. Capture the actual artifact, environment and timestamp, then verify error rate and dependency health before declaring recovery.
Use [INC-07](../incidents/inc-07.md) and the [postmortem](../postmortems/inc-07.md) as examples of recorded observations, not permission for a new action.
