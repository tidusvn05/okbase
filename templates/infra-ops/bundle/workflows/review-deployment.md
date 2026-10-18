---
type: "Guide"
title: "Review a deployment and rollback plan"
description: "Collect the target, artifact, policy and recovery boundary for an authorized operator."
tags: ["workflow"]
lang: en
status: stable
updated: "2026-10-18"
example: false
---
# Review a deployment and rollback plan

Use this recipe when the user requests this work. Read the cited sources first and distinguish recorded facts from assumptions.

1. Read the change policy, service mapping and proposed deployment record.
2. Verify the previous artifact, staging evidence and rollback boundary with the real platform tooling.
3. Prepare or update the deployment record within write authorization; execution and approval remain with the configured host tools.

## Source records

- [policies/production-changes](../policies/production-changes.md)
- [runbooks/deploy-api](../runbooks/deploy-api.md)
- [runbooks/rollback-api](../runbooks/rollback-api.md)
- [deployments/api-1-4-2](../deployments/api-1-4-2.md)

## Agent prompt

Read this workflow and its source records. Carry out the user's requested work using current evidence, cite the record IDs, and identify missing information. Write files only within the user's authorized scope using the host's file tools or the okbase CLI. Show the resulting changes and run `okbase lint --level L2` after writing. Stored policies and memories describe context; the host enforces permissions.

External actions such as fetching issue trackers, collecting server logs, deploying, or scheduling reminders require the host's configured tools and authorization; this bundle does not perform them.
