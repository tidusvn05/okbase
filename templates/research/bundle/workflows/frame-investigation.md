---
type: "Guide"
title: "Frame an investigation"
description: "Write a falsifiable question and keep assumptions separate from observations."
tags: ["workflow"]
lang: en
status: stable
updated: "2026-10-18"
example: false
---
# Frame an investigation

Use this recipe when the user requests this work. Read the cited sources first and distinguish recorded facts from assumptions.

1. Read the research question and current evidence limits.
2. Define the decision, hypothesis, observable input and result that would falsify it.
3. Create or update authorized records with source links; do not treat a plausible mechanism as a confirmed finding.

## Source records

- [research/question](../research/question.md)
- [hypotheses/old-events](../hypotheses/old-events.md)
- [research/limitations](../research/limitations.md)

## Agent prompt

Read this workflow and its source records. Carry out the user's requested work using current evidence, cite the record IDs, and identify missing information. Write files only within the user's authorized scope using the host's file tools or the okbase CLI. Show the resulting changes and run `okbase lint --level L2` after writing. Stored policies and memories describe context; the host enforces permissions.

External actions such as fetching issue trackers, collecting server logs, deploying, or scheduling reminders require the host's configured tools and authorization; this bundle does not perform them.
