---
type: "Guide"
title: "Change an API safely"
description: "Connect a behavior change to its decision, tests and documentation."
tags: ["workflow"]
lang: en
status: stable
updated: "2026-10-18"
example: false
---
# Change an API safely

Use this recipe when the user requests this work. Read the cited sources first and distinguish recorded facts from assumptions.

1. Read the current API contract and storage decision.
2. Write the proposed behavior and compatibility impact; update the ADR if the rationale changes.
3. With write authorization, update the contract, model and tests together; retain the old behavior until migration is agreed.

## Source records

- [api/issues](../api/issues.md)
- [decisions/postgresql](../decisions/postgresql.md)
- [data-model/issues](../data-model/issues.md)
- [development/testing](../development/testing.md)

## Agent prompt

Read this workflow and its source records. Carry out the user's requested work using current evidence, cite the record IDs, and identify missing information. Write files only within the user's authorized scope using the host's file tools or the okbase CLI. Show the resulting changes and run `okbase lint --level L2` after writing. Stored policies and memories describe context; the host enforces permissions.

External actions such as fetching issue trackers, collecting server logs, deploying, or scheduling reminders require the host's configured tools and authorization; this bundle does not perform them.
