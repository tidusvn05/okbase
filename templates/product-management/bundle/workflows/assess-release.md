---
type: "Guide"
title: "Assess product acceptance"
description: "Check release claims against requirements and the evidence cutoff."
tags: ["workflow"]
lang: en
status: stable
updated: "2026-10-18"
example: false
---
# Assess product acceptance

Use this recipe when the user requests this work. Read the cited sources first and distinguish recorded facts from assumptions.

1. Read every acceptance criterion and its available result.
2. List passed, failed and unverified criteria without treating missing data as success.
3. Write the review with source links, reporting period and follow-up owners; keep customer outcome claims separate from technical checks.

## Source records

- [requirements/notification-reliability](../requirements/notification-reliability.md)
- [reports/m1-review](../reports/m1-review.md)

## Agent prompt

Read this workflow and its source records. Carry out the user's requested work using current evidence, cite the record IDs, and identify missing information. Write files only within the user's authorized scope using the host's file tools or the okbase CLI. Show the resulting changes and run `okbase lint --level L2` after writing. Stored policies and memories describe context; the host enforces permissions.

External actions such as fetching issue trackers, collecting server logs, deploying, or scheduling reminders require the host's configured tools and authorization; this bundle does not perform them.
