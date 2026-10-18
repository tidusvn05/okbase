---
type: "Requirement"
title: "Notification reliability PRD"
description: "Specify user outcomes, acceptance criteria and exclusions for the Atlas notification milestone."
tags: ["requirements","acceptance","example"]
lang: en
status: draft
updated: "2026-10-18"
example: true
"owner": "atlas-team"
"project_id": "atlas"
---
# Notification reliability PRD

> Fictional example: replace this record with your own information before relying on it.

## Problem and outcome
An issue assignee must receive one notification for one assignment event, even if the worker retries delivery.
## Acceptance criteria
Replaying an assignment event must not send a second notification. Pending retry age must be visible to the operating team. The owner can trace a notification back to its source issue and event.
## Out of scope
Digest emails and channel preferences are out of scope for M1.
Sources: [research](../research/handoff-interviews.md), [product vision](../product/vision.md), and [scope decision](../decisions/defer-digest.md).
