---
type: "Guide"
title: "Review product priorities"
description: "Expose the tradeoff and evidence behind a roadmap change."
tags: ["workflow"]
lang: en
status: stable
updated: "2026-10-18"
example: false
---
# Review product priorities

Use this recipe when the user requests this work. Read the cited sources first and distinguish recorded facts from assumptions.

1. Read the roadmap, current requirements and deferred decision.
2. Compare candidate work against the agreed outcome and distinguish demand evidence from anecdote.
3. Prepare a decision brief with alternatives and missing evidence; update planning records only within authorization.

## Source records

- [planning/roadmap](../planning/roadmap.md)
- [decisions/defer-digest](../decisions/defer-digest.md)
- [research/handoff-interviews](../research/handoff-interviews.md)

## Agent prompt

Read this workflow and its source records. Carry out the user's requested work using current evidence, cite the record IDs, and identify missing information. Write files only within the user's authorized scope using the host's file tools or the okbase CLI. Show the resulting changes and run `okbase lint --level L2` after writing. Stored policies and memories describe context; the host enforces permissions.

External actions such as fetching issue trackers, collecting server logs, deploying, or scheduling reminders require the host's configured tools and authorization; this bundle does not perform them.
