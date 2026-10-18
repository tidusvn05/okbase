---
type: "Guide"
title: "Prepare an engineering leadership report"
description: "Summarize delivery, capacity and technical risks from current source records."
tags: ["workflow"]
lang: en
status: stable
updated: "2026-10-18"
example: false
---
# Prepare an engineering leadership report

Use this recipe when the user requests this work. Read the cited sources first and distinguish recorded facts from assumptions.

1. Choose the reporting period and evidence cutoff.
2. Read tasks, RFCs, ownership and risks; verify whether previous report claims are still current.
3. Write a report with explicit requests, source IDs and uncertainties; avoid converting planned capacity into completed output.

## Source records

- [reports/week-42](../reports/week-42.md)
- [risks/monitoring-evidence](../risks/monitoring-evidence.md)
- [team/capacity](../team/capacity.md)

## Agent prompt

Read this workflow and its source records. Carry out the user's requested work using current evidence, cite the record IDs, and identify missing information. Write files only within the user's authorized scope using the host's file tools or the okbase CLI. Show the resulting changes and run `okbase lint --level L2` after writing. Stored policies and memories describe context; the host enforces permissions.

External actions such as fetching issue trackers, collecting server logs, deploying, or scheduling reminders require the host's configured tools and authorization; this bundle does not perform them.
