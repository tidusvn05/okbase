---
type: "Guide"
title: "Answer a customer question"
description: "Ground the answer in the current product reference and policy."
tags: ["workflow"]
lang: en
status: stable
updated: "2026-10-18"
example: false
---
# Answer a customer question

Use this recipe when the user requests this work. Read the cited sources first and distinguish recorded facts from assumptions.

1. Read the product record and relevant FAQ.
2. Distinguish supported behavior from a claim about a specific customer event.
3. Draft a concise response with source references and evidence gaps; do not promise roadmap dates or send externally without authorization.

## Source records

- [products/atlas](../products/atlas.md)
- [faq/notifications](../faq/notifications.md)
- [policies/escalation](../policies/escalation.md)

## Agent prompt

Read this workflow and its source records. Carry out the user's requested work using current evidence, cite the record IDs, and identify missing information. Write files only within the user's authorized scope using the host's file tools or the okbase CLI. Show the resulting changes and run `okbase lint --level L2` after writing. Stored policies and memories describe context; the host enforces permissions.

External actions such as fetching issue trackers, collecting server logs, deploying, or scheduling reminders require the host's configured tools and authorization; this bundle does not perform them.
