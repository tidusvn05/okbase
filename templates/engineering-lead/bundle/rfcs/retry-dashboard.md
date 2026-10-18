---
type: "RFC"
title: "Retry-age dashboard RFC"
description: "Compare two monitoring definitions before accepting a dashboard implementation."
tags: ["rfc","monitoring","example"]
lang: en
status: draft
updated: "2026-10-18"
example: true
"owner": "atlas-team"
"project_id": "atlas"
---
# Retry-age dashboard RFC

> Fictional example: replace this record with your own information before relying on it.

## Question and options
Option A measures time since the last retry attempt. Option B measures age of the oldest still-pending event.
The proposed dashboard uses oldest pending event age because successful recent retries can hide older stuck work.
## Validation needed
This is a proposal, not an accepted implementation. Obtain the real query and test it with an old pending event plus a recent successful event.
See the [review standard](../standards/change-review.md), [debt item](../debt/dashboard-query.md), and [risk](../risks/monitoring-evidence.md).
