---
type: "Guide"
title: "Write a source-backed PRD"
description: "Turn a user problem into testable acceptance criteria and explicit exclusions."
tags: ["workflow"]
lang: en
status: stable
updated: "2026-10-18"
example: false
---
# Write a source-backed PRD

Use this recipe when the user requests this work. Read the cited sources first and distinguish recorded facts from assumptions.

1. Read the vision and research, including sample limitations.
2. Draft the problem, outcome, acceptance criteria and exclusions; cite the supporting evidence.
3. Review scope against the roadmap and record an authorized decision when priorities change.

## Source records

- [product/vision](../product/vision.md)
- [research/handoff-interviews](../research/handoff-interviews.md)
- [requirements/notification-reliability](../requirements/notification-reliability.md)

## Agent prompt

Read this workflow and its source records. Carry out the user's requested work using current evidence, cite the record IDs, and identify missing information. Write files only within the user's authorized scope using the host's file tools or the okbase CLI. Show the resulting changes and run `okbase lint --level L2` after writing. Stored policies and memories describe context; the host enforces permissions.

External actions such as fetching issue trackers, collecting server logs, deploying, or scheduling reminders require the host's configured tools and authorization; this bundle does not perform them.
