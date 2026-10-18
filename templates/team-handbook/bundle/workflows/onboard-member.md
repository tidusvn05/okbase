---
type: "Guide"
title: "Onboard a team member"
description: "Build context and route setup requests to the right owner."
tags: ["workflow"]
lang: en
status: stable
updated: "2026-10-18"
example: false
---
# Onboard a team member

Use this recipe when the user requests this work. Read the cited sources first and distinguish recorded facts from assumptions.

1. Read the charter, ownership map and first-week guide.
2. Identify the required access and missing setup inputs.
3. Prepare owner-routed requests with the member; verify actual setup evidence before marking onboarding complete.

## Source records

- [onboarding/first-week](../onboarding/first-week.md)
- [team/ownership](../team/ownership.md)
- [policies/access](../policies/access.md)

## Agent prompt

Read this workflow and its source records. Carry out the user's requested work using current evidence, cite the record IDs, and identify missing information. Write files only within the user's authorized scope using the host's file tools or the okbase CLI. Show the resulting changes and run `okbase lint --level L2` after writing. Stored policies and memories describe context; the host enforces permissions.

External actions such as fetching issue trackers, collecting server logs, deploying, or scheduling reminders require the host's configured tools and authorization; this bundle does not perform them.
