---
type: "Guide"
title: "Local development setup"
description: "Prepare the fictional Atlas development environment without changing shared infrastructure."
tags: ["development","onboarding","example"]
lang: en
status: draft
updated: "2026-10-18"
example: true
"owner": "atlas-team"
"project_id": "atlas"
---
# Local development setup

> Fictional example: replace this record with your own information before relying on it.

## Prerequisites
The example stack uses Node.js, PostgreSQL and Redis; choose versions from the real repository's lockfiles and runtime declarations when adapting this guide.
Copy the repository's example environment file to a local untracked file, start local dependencies, apply migrations, and run the API and worker in separate terminals. This starter bundle contains documentation, not the application or its scripts.
## Verify
Create a test issue and confirm a worker delivery in the local environment. Run the [test plan](testing.md) before opening a PR. Never point a local test at production.
Read [project ownership](../project/overview.md) when a required command or secret is missing.
