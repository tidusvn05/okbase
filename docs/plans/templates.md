# Plan: practical starter knowledge templates

## Decisions

Provide ten offline templates: software-project, personal-assistant, project-assistant, infra-ops,
product-management, engineering-lead, executive, team-handbook, customer-support and research.
Templates are selected by the work to do, with audience metadata for developers, leaders, CEOs
and product/project managers. One main template creates one independently managed bundle; hosts
can connect several bundles through existing named instances. Users extend ordinary Markdown,
type schemas and vocabulary after initialization.

Each template has a coherent fictional scenario, five source-grounded trial questions and three
workflow guides. All domain records are English and marked example/draft; usage guides are stable
instructions. Work states use custom fields such as task_status and incident_status. OKF status
keeps its existing lifecycle semantics. Template audience metadata never grants read permissions.

## Implementation phases and acceptance

1. **Creation mechanism:** embed catalog-declared bundles; add `templates --json`, `init --template`
   and `init --print`. Preserve the empty initialization flow, allow title/description/language
   overrides and additive schemas. Preview and invalid requests create nothing; preflight rejects
   destination conflicts and symlinked generated paths. The shared writer uses create-new semantics
   for new files, preserving explicit index updates from `new`.
2. **Content:** ship the ten bundles, complete folder listings, schemas, vocabulary and provenance
   log. Every generated bundle passes L2 and broken-link validation. Each trial question has actual
   source evidence retrievable from the index. Workflows connect meetings/tasks/reports and
   servers/services/deployments/incidents, preserving unknowns rather than inventing outcomes.
3. **Discovery:** list templates and their intended audiences from the same registry in CLI and
   empty-folder onboarding. Keep initialization an explicit user choice. Update the author skill,
   README, usage documentation and agent entrypoints. Provide a guide for connecting several bundles
   and adapting bot hosts using existing MCP/CLI/Rust reads and authorized file-writing tools.
4. **Verification:** snapshot the catalog, preview paths, onboarding choices and author skill; test
   all ten generated bundles, 50 source-evidence cases, workflow queries, extension via `new`, scopes,
   compatibility and destination conflicts. Run workspace fmt/clippy/tests/deny and the lexical eval.
   Paid or quota-using agent evaluations require separate consent under the repository's spike rules.

## Interfaces and lifecycle

`templates --json` returns `{templates: [{id, title, description, roles, types}]}` without opening a
bundle or creating state. `init --template NAME` returns the existing created/next result plus
template. `init --print --json` returns template (null for the empty skeleton), files (path/content)
and an empty next list, with no created claim. Unknown template names use invalid_argument.

No migration is needed. Existing bundles are never reinitialized, combined or upgraded by this
feature. The initialization log records template and okbase version; the user owns subsequent edits.
No task engine, bot runtime, scheduler, deployment executor, account connector or MCP write tool is
added. The host retains all read scopes and execution permissions.

## Public template architecture (2026-10-19)

The public [v1 format](../template-format.md) replaces the internal body-in-YAML assets.
`templates/` is the only editable source: each template has a manifest, README and actual Markdown
bundle. Root `catalog.yaml` declares paths and display order. Metadata comes from manifests; document
types come from bundle schemas. The `okbase-templates` workspace crate lives at `templates/` and
its build script generates embedded assets from the catalog. This keeps crates.io archives
self-contained without hand-maintained mirrors or a Rust registry. Adding a built-in template needs
only content and a catalog entry, then a rebuild/release; it never needs CLI dispatch changes.

`okbase-adopt::templates` reads manifests, catalogs and UTF-8 bundle assets, selects IDs, applies
supported overrides, validates L2/schema conformance and plans files. Built-in, local and Git
sources share this pipeline. Local catalogs use `templates --source`; init uses `--template-path`
and optionally `--template ID`. Git sources use `--repo` for listing and `--template-repo` for init,
with `--source`/`--template-path` selecting a repository subdirectory and `--ref` selecting a revision.
The Git adapter requires Git on PATH, fetches into disposable storage and reads regular Git blobs;
it never checks out files or executes repository hooks, filters or submodules. HTTPS, SSH and local
Git directories are supported; remote fetches respect OKBASE_OFFLINE. Host authentication remains
outside template content. New external templates need no binary update while format v1 is supported.

Init preview and creation expose `source`; the log records the content version and exact Git
commit/path when applicable. Repository listing also returns `commit`. No source cache, installed
registry, custom executable renderer, automatic bundle upgrade or persistent source configuration is
introduced. Path containment, UTF-8, limits, evidence and conformance are checked before destination
writes. Source document frontmatter remains byte-identical; TOML edits target explicit language
settings. The authoring starter, JSON Schemas, documentation and local Git acceptance fixtures cover
the complete contribution path. Results remain in `spikes/templates/RESULTS.md`.
