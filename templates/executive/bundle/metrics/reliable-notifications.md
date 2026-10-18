---
type: "Metric"
title: "Reliable notification rate definition"
description: "Define the denominator, measurement window and limits of the notification KPI."
tags: ["metrics","definition","example"]
lang: en
status: draft
updated: "2026-10-18"
example: true
"measurement_window": "UTC calendar week"
"owner": "atlas-team"
"project_id": "atlas"
"unit": "ratio"
---
# Reliable notification rate definition

> Fictional example: replace this record with your own information before relying on it.

## Definition
Reliable notification rate is distinct accepted assignment events delivered exactly once divided by distinct accepted assignment events in the same reporting window.
Use a UTC calendar week and count each event once. Events still within the agreed delivery window must be reported separately rather than silently classified as success.
## Source and uncertainty
The intended source is the outbox delivery export joined to accepted assignment events. No export is included in this demo, so the metric value is unknown.
The api-team owns semantic correctness; the platform-team owns the export. See [the reporting risk](../risks/metric-evidence.md).
