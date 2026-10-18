---
type: "Guide"
title: "Onboard a developer"
description: "Build an accurate mental model before modifying the software."
tags: ["workflow"]
lang: en
status: stable
updated: "2026-10-18"
example: false
---
# Onboard a developer

Use this recipe when the user requests this work. Read the cited sources first and distinguish recorded facts from assumptions.

1. Read the overview, architecture and ownership records.
2. Trace one create request through the API, transaction and outbox.
3. Identify missing setup commands and ask the owner; do not invent runnable scripts.

## Source records

- [project/overview](../project/overview.md)
- [architecture/system](../architecture/system.md)
- [development/local-setup](../development/local-setup.md)

## Agent prompt

Read this workflow and its source records. Carry out the user's requested work using current evidence, cite the record IDs, and identify missing information. Write files only within the user's authorized scope using the host's file tools or the okbase CLI. Show the resulting changes and run `okbase lint --level L2` after writing. Stored policies and memories describe context; the host enforces permissions.

External actions such as fetching issue trackers, collecting server logs, deploying, or scheduling reminders require the host's configured tools and authorization; this bundle does not perform them.
