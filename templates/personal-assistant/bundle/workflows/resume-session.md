---
type: "Guide"
title: "Resume a session"
description: "Recover the next useful action from the handoff and cited context."
tags: ["workflow"]
lang: en
status: stable
updated: "2026-10-18"
example: false
---
# Resume a session

Use this recipe when the user requests this work. Read the cited sources first and distinguish recorded facts from assumptions.

1. Read the handoff and its linked project memory.
2. State the current goal, completed work, evidence gap and next action.
3. Continue only actions covered by the current authorization; obtain missing evidence before closing the item.

## Source records

- [handoffs/next-session](../handoffs/next-session.md)
- [journal/2026-10-14](../journal/2026-10-14.md)
- [projects/atlas](../projects/atlas.md)

## Agent prompt

Read this workflow and its source records. Carry out the user's requested work using current evidence, cite the record IDs, and identify missing information. Write files only within the user's authorized scope using the host's file tools or the okbase CLI. Show the resulting changes and run `okbase lint --level L2` after writing. Stored policies and memories describe context; the host enforces permissions.

External actions such as fetching issue trackers, collecting server logs, deploying, or scheduling reminders require the host's configured tools and authorization; this bundle does not perform them.
