---
name: Agent report
about: A bug or an improvement an agent found while using okbase in another project
title: "[agent] "
labels: []
---

<!--
Filed by an agent working in another project. From a non-interactive shell:
  gh api repos/tidusvn05/okbase/contents/.github/ISSUE_TEMPLATE/agent-report.md --jq .content | base64 -d > report.md
  # remove the frontmatter, fill in every section, then
  gh issue create -R tidusvn05/okbase --title "[agent] <summary>" --label bug --body-file report.md   # or --label enhancement
Before filing: search open and closed issues (`gh issue list -R tidusvn05/okbase --state all --search "<words>"`);
if one matches, comment there instead.
okbase is public: describe the project in general terms only. Never paste private document text,
internal paths or hostnames, names, credentials or customer data. Use a synthetic bundle to reproduce.
Keep the marker below; reuse the same id when the same problem is reported again.
-->
<!-- agent-report:<short-kebab-id> -->

## Kind

bug | improvement

## Summary

<!-- One sentence: what goes wrong, or what would work better. -->

## Context

- okbase: <!-- `okbase --version`; okbase or okbase-full -->
- Bundle: <!-- from `okbase status`: documents, tokens, level, profile (okf / docs-site / vault), main languages -->
- Platform: <!-- OS and CPU -->
- Agent: <!-- e.g. Claude Code + model, Codex + model; MCP or CLI -->

## What happened

<!-- The exact command(s), what was expected and what happened. Paste the output and exit code; `--json` output helps. -->

## How to reproduce

<!-- Minimal steps or a small synthetic bundle (a few files inline) that shows it. -->

## Proposal

<!-- For an improvement, or a fix you suggest: what should change. Optional for a bug. -->

## Evidence

<!-- Why it matters: counts, timings, how often it happened, what it cost (tokens, wrong answers). okbase changes defaults only with measurements. -->

## Workaround

<!-- What the agent did instead, if anything. -->

- [ ] I searched existing issues
- [ ] Nothing private is included
