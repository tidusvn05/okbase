---
type: "FAQ"
title: "Notification questions"
description: "Answer common delivery questions while preserving uncertainty about a particular customer event."
tags: ["faq","notifications","example"]
lang: en
status: draft
updated: "2026-10-18"
example: true
"owner": "atlas-team"
"project_id": "atlas"
---
# Notification questions

> Fictional example: replace this record with your own information before relying on it.

## Do retries create duplicates?
Retries are designed to reuse the event identity so that one assignment event produces one notification. A specific customer report still needs evidence.
## Are digest emails available?
Digest emails are not available in the current release.
## How do I report a missing notification?
Provide the issue ID, approximate event time with timezone and the affected delivery channel. Never request a password or raw authentication token.
See [the troubleshooting guide](../guides/missing-notification.md).
