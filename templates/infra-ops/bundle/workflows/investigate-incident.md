---
type: "Guide"
title: "Investigate and record an incident"
description: "Connect a service incident to timestamped evidence, hypotheses and follow-up work."
tags: ["workflow"]
lang: en
status: stable
updated: "2026-10-18"
example: false
---
# Investigate and record an incident

Use this recipe when the user requests this work. Read the cited sources first and distinguish recorded facts from assumptions.

1. Read service dependencies, the latest deployment and the incident timeline.
2. Collect authorized redacted logs and metrics; separate observations from causal hypotheses.
3. Update the incident and postmortem with source timestamps, verified outcomes and owner-assigned follow-up actions.

## Source records

- [incidents/inc-07](../incidents/inc-07.md)
- [postmortems/inc-07](../postmortems/inc-07.md)
- [services/database](../services/database.md)
- [deployments/api-1-4-2](../deployments/api-1-4-2.md)

## Agent prompt

Read this workflow and its source records. Carry out the user's requested work using current evidence, cite the record IDs, and identify missing information. Write files only within the user's authorized scope using the host's file tools or the okbase CLI. Show the resulting changes and run `okbase lint --level L2` after writing. Stored policies and memories describe context; the host enforces permissions.

External actions such as fetching issue trackers, collecting server logs, deploying, or scheduling reminders require the host's configured tools and authorization; this bundle does not perform them.
