---
type: "Guide"
title: "Record a reproducible experiment"
description: "Preserve inputs, procedure, clock and actual output so the result can be checked."
tags: ["workflow"]
lang: en
status: stable
updated: "2026-10-18"
example: false
---
# Record a reproducible experiment

Use this recipe when the user requests this work. Read the cited sources first and distinguish recorded facts from assumptions.

1. Read the protocol and expected result.
2. Run only available, authorized tools against the approved data; record actual commands and outputs separately from expectations.
3. Update the evidence record with artifact references, observation date and limitations; keep failed or inconclusive results visible.

## Source records

- [experiments/retry-age](../experiments/retry-age.md)
- [evidence/demo-result](../evidence/demo-result.md)
- [research/limitations](../research/limitations.md)

## Agent prompt

Read this workflow and its source records. Carry out the user's requested work using current evidence, cite the record IDs, and identify missing information. Write files only within the user's authorized scope using the host's file tools or the okbase CLI. Show the resulting changes and run `okbase lint --level L2` after writing. Stored policies and memories describe context; the host enforces permissions.

External actions such as fetching issue trackers, collecting server logs, deploying, or scheduling reminders require the host's configured tools and authorization; this bundle does not perform them.
