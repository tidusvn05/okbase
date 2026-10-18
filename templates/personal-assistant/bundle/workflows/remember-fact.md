---
type: "Guide"
title: "Remember a sourced fact"
description: "Capture durable knowledge with the source and date the user actually provided."
tags: ["workflow"]
lang: en
status: stable
updated: "2026-10-18"
example: false
---
# Remember a sourced fact

Use this recipe when the user requests this work. Read the cited sources first and distinguish recorded facts from assumptions.

1. Identify whether the input is a preference, project fact or temporary observation.
2. Check existing memory for contradictions and verify permission to retain the information.
3. Create or minimally update the right record, recording the source and observation date; link the journal instead of copying it.

## Source records

- [policies/memory](../policies/memory.md)
- [profile/preferences](../profile/preferences.md)
- [projects/atlas](../projects/atlas.md)

## Agent prompt

Read this workflow and its source records. Carry out the user's requested work using current evidence, cite the record IDs, and identify missing information. Write files only within the user's authorized scope using the host's file tools or the okbase CLI. Show the resulting changes and run `okbase lint --level L2` after writing. Stored policies and memories describe context; the host enforces permissions.

External actions such as fetching issue trackers, collecting server logs, deploying, or scheduling reminders require the host's configured tools and authorization; this bundle does not perform them.
