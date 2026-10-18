---
type: "Guide"
title: "Record an executive decision"
description: "Preserve the alternatives, rationale, owner and revisit condition for a leadership choice."
tags: ["workflow"]
lang: en
status: stable
updated: "2026-10-18"
example: false
---
# Record an executive decision

Use this recipe when the user requests this work. Read the cited sources first and distinguish recorded facts from assumptions.

1. Locate the objective, initiative and existing decision.
2. Prepare alternatives and evidence limits, including the consequences of deferring the choice.
3. Record the approved outcome and affected source links within write authorization; keep operational permissions with the execution host.

## Source records

- [objectives/q4](../objectives/q4.md)
- [decisions/m1-scope](../decisions/m1-scope.md)
- [initiatives/m1](../initiatives/m1.md)

## Agent prompt

Read this workflow and its source records. Carry out the user's requested work using current evidence, cite the record IDs, and identify missing information. Write files only within the user's authorized scope using the host's file tools or the okbase CLI. Show the resulting changes and run `okbase lint --level L2` after writing. Stored policies and memories describe context; the host enforces permissions.

External actions such as fetching issue trackers, collecting server logs, deploying, or scheduling reminders require the host's configured tools and authorization; this bundle does not perform them.
