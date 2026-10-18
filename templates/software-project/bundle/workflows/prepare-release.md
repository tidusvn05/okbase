---
type: "Guide"
title: "Prepare a release review"
description: "Collect compatibility evidence and a concrete rollback boundary."
tags: ["workflow"]
lang: en
status: stable
updated: "2026-10-18"
example: false
---
# Prepare a release review

Use this recipe when the user requests this work. Read the cited sources first and distinguish recorded facts from assumptions.

1. Read the release checklist and test evidence for the proposed commit.
2. List blockers, migration constraints and the previous artifact.
3. Write a release brief with evidence and outstanding approvals; deployment is performed by the authorized external tooling.

## Source records

- [delivery/release](../delivery/release.md)
- [development/testing](../development/testing.md)

## Agent prompt

Read this workflow and its source records. Carry out the user's requested work using current evidence, cite the record IDs, and identify missing information. Write files only within the user's authorized scope using the host's file tools or the okbase CLI. Show the resulting changes and run `okbase lint --level L2` after writing. Stored policies and memories describe context; the host enforces permissions.

External actions such as fetching issue trackers, collecting server logs, deploying, or scheduling reminders require the host's configured tools and authorization; this bundle does not perform them.
