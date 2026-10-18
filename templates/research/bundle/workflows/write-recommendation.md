---
type: "Guide"
title: "Write an evidence-based recommendation"
description: "Connect a decision proposal to observations, alternatives and remaining uncertainty."
tags: ["workflow"]
lang: en
status: stable
updated: "2026-10-18"
example: false
---
# Write an evidence-based recommendation

Use this recipe when the user requests this work. Read the cited sources first and distinguish recorded facts from assumptions.

1. Read the question, hypothesis and actual experiment evidence.
2. Describe what the evidence supports, alternatives and what remains unknown.
3. Write the authorized recommendation with source links and acceptance conditions; set decision_status to accepted only after its reviewer approves.

## Source records

- [decisions/monitoring-definition](../decisions/monitoring-definition.md)
- [evidence/demo-result](../evidence/demo-result.md)
- [research/limitations](../research/limitations.md)

## Agent prompt

Read this workflow and its source records. Carry out the user's requested work using current evidence, cite the record IDs, and identify missing information. Write files only within the user's authorized scope using the host's file tools or the okbase CLI. Show the resulting changes and run `okbase lint --level L2` after writing. Stored policies and memories describe context; the host enforces permissions.

External actions such as fetching issue trackers, collecting server logs, deploying, or scheduling reminders require the host's configured tools and authorization; this bundle does not perform them.
