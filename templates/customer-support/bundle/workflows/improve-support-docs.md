---
type: "Guide"
title: "Improve support documentation"
description: "Turn a verified case outcome into reusable knowledge without duplicating policy."
tags: ["workflow"]
lang: en
status: stable
updated: "2026-10-18"
example: false
---
# Improve support documentation

Use this recipe when the user requests this work. Read the cited sources first and distinguish recorded facts from assumptions.

1. Read the closed case evidence and existing FAQ or troubleshooting record.
2. Identify the reusable fact and confirm its product version and owner.
3. Update the existing document when appropriate, link the case and keep customer-specific details out of general guidance.

## Source records

- [faq/notifications](../faq/notifications.md)
- [guides/missing-notification](../guides/missing-notification.md)
- [policies/escalation](../policies/escalation.md)

## Agent prompt

Read this workflow and its source records. Carry out the user's requested work using current evidence, cite the record IDs, and identify missing information. Write files only within the user's authorized scope using the host's file tools or the okbase CLI. Show the resulting changes and run `okbase lint --level L2` after writing. Stored policies and memories describe context; the host enforces permissions.

External actions such as fetching issue trackers, collecting server logs, deploying, or scheduling reminders require the host's configured tools and authorization; this bundle does not perform them.
