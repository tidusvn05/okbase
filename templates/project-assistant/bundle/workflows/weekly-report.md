---
type: "Guide"
title: "Prepare a project status report"
description: "Generate a source-backed report from current task states and decisions."
tags: ["workflow"]
lang: en
status: stable
updated: "2026-10-18"
example: false
---
# Prepare a project status report

Use this recipe when the user requests this work. Read the cited sources first and distinguish recorded facts from assumptions.

1. Query tasks with `okbase query --type Task --facet task_status` and inspect blockers.
2. Read the milestone and accepted decisions, then choose the reporting period and evidence cutoff.
3. Write an authorized report citing task and investigation IDs; do not infer completion from a meeting promise or a missing external update.

## Source records

- [project/context](../project/context.md)
- [reports/week-42](../reports/week-42.md)
- [tasks/at-43](../tasks/at-43.md)

## Agent prompt

Read this workflow and its source records. Carry out the user's requested work using current evidence, cite the record IDs, and identify missing information. Write files only within the user's authorized scope using the host's file tools or the okbase CLI. Show the resulting changes and run `okbase lint --level L2` after writing. Stored policies and memories describe context; the host enforces permissions.

External actions such as fetching issue trackers, collecting server logs, deploying, or scheduling reminders require the host's configured tools and authorization; this bundle does not perform them.
