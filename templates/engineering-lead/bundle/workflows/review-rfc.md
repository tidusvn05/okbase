---
type: "Guide"
title: "Review a technical RFC"
description: "Evaluate the problem, alternatives and validation plan before accepting a design."
tags: ["workflow"]
lang: en
status: stable
updated: "2026-10-18"
example: false
---
# Review a technical RFC

Use this recipe when the user requests this work. Read the cited sources first and distinguish recorded facts from assumptions.

1. Read the proposed RFC, relevant standards and ownership map.
2. Check rejected options, operational impact and the evidence needed to validate the claim.
3. Write the review outcome with unresolved questions; record an accepted decision only after the designated reviewer approves it.

## Source records

- [rfcs/retry-dashboard](../rfcs/retry-dashboard.md)
- [standards/change-review](../standards/change-review.md)
- [team/ownership](../team/ownership.md)

## Agent prompt

Read this workflow and its source records. Carry out the user's requested work using current evidence, cite the record IDs, and identify missing information. Write files only within the user's authorized scope using the host's file tools or the okbase CLI. Show the resulting changes and run `okbase lint --level L2` after writing. Stored policies and memories describe context; the host enforces permissions.

External actions such as fetching issue trackers, collecting server logs, deploying, or scheduling reminders require the host's configured tools and authorization; this bundle does not perform them.
