---
type: "Guide"
title: "Route a blocked request"
description: "Find the owner and provide enough evidence to unblock a teammate."
tags: ["workflow"]
lang: en
status: stable
updated: "2026-10-18"
example: false
---
# Route a blocked request

Use this recipe when the user requests this work. Read the cited sources first and distinguish recorded facts from assumptions.

1. Read the ownership directory and request format.
2. Identify the exact missing input and the affected goal.
3. Prepare the request with source links and redacted evidence; sending and permission changes require the appropriate external tooling.

## Source records

- [procedures/request-help](../procedures/request-help.md)
- [team/ownership](../team/ownership.md)
- [policies/access](../policies/access.md)

## Agent prompt

Read this workflow and its source records. Carry out the user's requested work using current evidence, cite the record IDs, and identify missing information. Write files only within the user's authorized scope using the host's file tools or the okbase CLI. Show the resulting changes and run `okbase lint --level L2` after writing. Stored policies and memories describe context; the host enforces permissions.

External actions such as fetching issue trackers, collecting server logs, deploying, or scheduling reminders require the host's configured tools and authorization; this bundle does not perform them.
