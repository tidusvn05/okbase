---
type: "Guide"
title: "Turn a meeting into tracked work"
description: "Create linked decisions and owner-assigned tasks without losing the source meeting."
tags: ["workflow"]
lang: en
status: stable
updated: "2026-10-18"
example: false
---
# Turn a meeting into tracked work

Use this recipe when the user requests this work. Read the cited sources first and distinguish recorded facts from assumptions.

1. Read meeting outcomes and existing tasks; check for duplicate work.
2. Propose separate decisions and tasks with owner, acceptance criteria, due date and task_status.
3. With write authorization, update the records and link them back to the meeting; leave unknown owners or dates explicitly unresolved.

## Source records

- [meetings/m1-planning](../meetings/m1-planning.md)
- [tasks/at-42](../tasks/at-42.md)
- [tasks/at-43](../tasks/at-43.md)
- [decisions/m1-scope](../decisions/m1-scope.md)

## Agent prompt

Read this workflow and its source records. Carry out the user's requested work using current evidence, cite the record IDs, and identify missing information. Write files only within the user's authorized scope using the host's file tools or the okbase CLI. Show the resulting changes and run `okbase lint --level L2` after writing. Stored policies and memories describe context; the host enforces permissions.

External actions such as fetching issue trackers, collecting server logs, deploying, or scheduling reminders require the host's configured tools and authorization; this bundle does not perform them.
