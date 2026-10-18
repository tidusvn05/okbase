---
type: "Reference"
title: "Atlas project overview"
description: "Understand the Atlas issue-tracking product, its boundaries and component owners."
tags: ["project","ownership","example"]
lang: en
status: draft
updated: "2026-10-18"
example: true
"owner": "atlas-team"
"project_id": "atlas"
---
# Atlas project overview

> Fictional example: replace this record with your own information before relying on it.

## Purpose and boundaries
Atlas is a fictional issue tracker for small engineering teams. The first release supports projects, issues and comments; billing is outside this release.
The Atlas API owns writes and a worker sends notifications. Web clients call the API over HTTPS; they never access the database directly.
## Where to start
Read the [architecture](../architecture/system.md), [storage decision](../decisions/postgresql.md), and [API contract](../api/issues.md). The api-team owns API behavior; the platform-team owns deployment.
Before changing behavior, locate its owner and acceptance criteria, then check the [release checklist](../delivery/release.md).
