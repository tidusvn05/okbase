---
type: "Guide"
title: "Review and correct memory"
description: "Keep lasting facts accurate without turning every journal entry into memory."
tags: ["workflow"]
lang: en
status: stable
updated: "2026-10-18"
example: false
---
# Review and correct memory

Use this recipe when the user requests this work. Read the cited sources first and distinguish recorded facts from assumptions.

1. Review the goal and journal records against their original sources.
2. Propose corrections and distinguish confirmed facts from unverified observations.
3. Apply authorized corrections using deprecated/supersedes links where records are replaced, then refresh the handoff.

## Source records

- [reports/weekly-review](../reports/weekly-review.md)
- [policies/memory](../policies/memory.md)
- [goals/current-quarter](../goals/current-quarter.md)

## Agent prompt

Read this workflow and its source records. Carry out the user's requested work using current evidence, cite the record IDs, and identify missing information. Write files only within the user's authorized scope using the host's file tools or the okbase CLI. Show the resulting changes and run `okbase lint --level L2` after writing. Stored policies and memories describe context; the host enforces permissions.

External actions such as fetching issue trackers, collecting server logs, deploying, or scheduling reminders require the host's configured tools and authorization; this bundle does not perform them.
