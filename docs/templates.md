# Start with a knowledge template

Choose a starter bundle when you want to try okbase before writing your first documents. Each
template contains a coherent fictional example, five trial questions with cited evidence, three
workflow guides, type schemas and a tag vocabulary. The ten built-in templates are embedded in both builds;
creating one requires no network, model or account. Browse their actual Markdown in
[`templates/`](../templates/README.md) before choosing. Local and Git templates use the same
[public v1 format](template-format.md).

```sh
okbase templates
okbase templates --json
okbase -b ./project-kb init --template software-project --print
okbase -b ./project-kb init --template software-project
okbase -b ./project-kb get guides/getting-started
okbase -b ./project-kb onboard
```

`init` is an explicit write command. `--print` previews every file without creating the destination
or writing files. Initialization refuses existing Markdown and occupied destination files; it never
upgrades or overwrites an existing bundle. The initialization log records the template and okbase
version. After creation, the Markdown is yours: upgrading okbase does not rewrite it.

## Choose by the work you do

| Template | Audience | Knowledge and workflows |
|---|---|---|
| `software-project` | Developers, tech leads | Project overview, architecture, storage ADR, API contract, data model, local setup, tests, release and rollback; onboard a developer, change a contract, prepare a release |
| `personal-assistant` | Individuals, developers, leaders | Preferences, goals, durable project memory, daily journal, session handoff and weekly review; remember a sourced fact, resume a session, correct memory |
| `project-assistant` | Developers, project managers, leaders | Project context, meeting, accepted scope, two tasks, investigation and weekly report; turn meetings into tasks, report progress, investigate blockers |
| `infra-ops` | DevOps, SRE, tech leads | Production/staging, two servers, three services, policy, deployment, deploy/rollback runbooks, incident and postmortem; map impact, review a deployment, investigate an incident |
| `product-management` | Product/project managers, founders | Vision, customer research, PRD, roadmap, scope decision and acceptance review; write a PRD, review priorities, assess release acceptance |
| `engineering-lead` | Tech leads, engineering managers | Ownership, review policy, RFC, debt task, capacity snapshot, risk and report; review an RFC, triage debt, prepare a leadership report |
| `executive` | CEOs, founders, executives | Strategy, objective, KPI definition, initiative, scope decision, risk and briefing; prepare a briefing, record a decision, review an objective |
| `team-handbook` | Leaders, team members, new joiners | Charter, ownership directory, first-week guide, communication/access policies and request procedure; onboard a member, capture agreements, route requests |
| `customer-support` | Support, customer success, product managers | Product behavior, FAQ, escalation policy, troubleshooting, triage procedure and case; answer a question, escalate a case, improve reusable guidance |
| `research` | Developers, product managers, leaders | Question, hypothesis, experiment protocol, evidence, limitations and proposed decision; frame an investigation, record an experiment, write a recommendation |

The fictional Atlas examples are self-contained in each bundle. A topic may occur in several
templates to demonstrate different work; separate bundles are not synchronized copies of one
canonical project. When adapting them, pick an authoritative owner for each real fact and link
to that source rather than maintaining contradictory copies.

## Try a real workflow

Read `guides/getting-started` in the generated bundle for its trial questions and workflow IDs.
Paste a request such as the following into your connected agent:

```text
Read this knowledge base's getting-started guide and workflows. Answer the five trial questions
with source IDs. Then explain which example records I should replace for my own project.
```

For a software project:

```text
Use the software-project bundle to explain the request path and storage decision. If we change
issue creation, identify the API, data-model, test and release documents that need review.
```

For a project assistant:

```text
Read workflows/weekly-report. List the blocked tasks, their owners and missing inputs, then
draft a weekly report using the task records as sources. Keep unverified findings explicit.
```

The task state is a custom field, independent of the document's OKF lifecycle:

```sh
okbase -b ./assistant-kb query --type Task --field task_status=blocked
okbase -b ./assistant-kb query --type Task --facet task_status
```

For infrastructure operations:

```text
Read workflows/investigate-incident. Trace atlas-api to its server and database dependency,
then compare the deployment, incident and postmortem. Separate observations from hypotheses
and list the additional logs or metrics needed to establish a cause.
```

```sh
okbase -b ./operations-kb query --type Service --field server_id=app-01 --field environment=production
okbase -b ./operations-kb get incidents/inc-07
okbase -b ./operations-kb graph --json
```

The personal-assistant template keeps preferences, lasting facts and temporary journals separate,
following the distinction described in [OpenClaw's memory documentation](https://docs.openclaw.ai/concepts/memory).
It is an OKF knowledge bundle for a bot host, not an OpenClaw runtime installation or native
workspace replacement. Its handoff records identify the next action and missing evidence;
saved preferences and memories do not confer permission to act.

## Replace examples and extend the bundle

All domain examples are English, with `example: true` and `status: draft`. The getting-started and
workflow guides are instructions, with `example: false` and `status: stable`. Fictional records may
have an accepted decision or a completed task while their document lifecycle remains draft.

```sh
okbase -b ./assistant-kb init --template project-assistant \
  --title "My project knowledge" --description "Delivery context for our team." \
  --langs vi,en --types Checklist
```

`--types` adds schemas to those supplied by the template. Without `--template`, `init --title`
continues to create the empty Guide/Policy/FAQ skeleton. `--langs` configures the languages people
write and ask in; it does not translate English examples or label them as another language.

1. Replace the domain examples with verified project records. Remove `example: true` after replacing
   the example; use `status: stable` only after the appropriate review. Delete unused examples and
   update their listings and links.
2. Create a record with `okbase new --type Task "Review rollout" --description "Review the deployment
   evidence before accepting M1." --tags task`. Fill the `to_fill` fields reported by the command
   and write the body. `new` creates the folder listing and keeps the record draft.
   Use `--dir <existing-folder>` when organizing by topic rather than the default type folder
   (for example, `--type API --dir api` in the software-project template).
3. Extend `_meta/types/<Type>.md` for new fields and types, and `_meta/vocabulary.md` for new tags
   and synonyms. Use `task_status` (todo/in_progress/blocked/done/cancelled) or `incident_status`
   (investigating/mitigating/resolved/closed) for work state. Keep `status` as draft/stable/deprecated.
4. Run `okbase lint --level L2` and the relevant trial questions again. Reports name their period,
   evidence cutoff and sources; unavailable measurements stay unknown.

## Several bundles and bot hosts

Use one main template per bundle. Connect several bundles to the same project with distinct names:

```sh
okbase -b ./project-kb init --template software-project
okbase -b ./assistant-kb init --template project-assistant
okbase -b ./operations-kb init --template infra-ops
okbase -b ./project-kb agent install --claude --name okbase-project
okbase -b ./assistant-kb agent install --claude --name okbase-assistant
okbase -b ./operations-kb agent install --claude --name okbase-ops
```

Choose the agent flag appropriate to your host. Inside Codex, its sandbox may require the user to
run registration in their terminal; `onboard` gives the exact next step. Each bundle has its own
tool prefix and configuration. A Markdown link resolves inside one bundle; it does not resolve
across separate servers.

A bot can read via MCP, CLI `--json` or the Rust library. Put its scoped catalog in the bot's context.
The host supplies the read `Scope` and authorized file-writing tools. Audience labels in the template
catalog and `owner` metadata do not enforce access permissions. Keep personal context scoped to its
intended audience and put credentials in the host's secret configuration.

Current MCP tools read knowledge. File updates use the CLI or the host's file tools within the user's
authorization. Logs, issue-tracker synchronization, deployment execution, reminder scheduling and
external message sending use separately configured host tools. The workflow guides explain what to
read, record and verify; creating a template does not start background work or connect accounts.

## Use a local or Git template

A source can be a single template directory or a collection with `catalog.yaml`:

```sh
okbase templates --source ./my-templates --json
okbase -b ./kb init --template-path ./my-template --print
okbase -b ./kb init --template-path ./my-templates --template my-template
```

For a repository, select its source directory and an optional branch, tag or commit:

```sh
okbase templates --repo https://github.com/acme/knowledge-templates --json
okbase -b ./kb init \
  --template-repo https://github.com/acme/knowledge-templates \
  --template-path templates/infra-ops --ref v1.0.0 --print
```

`--template-path` is relative to the repository when `--template-repo` is supplied. Omit it for a
root template/catalog. Use `--template ID` to select an entry from a collection; a single-entry
collection can be selected automatically. For repository listing, `--source` selects a subdirectory.
Git HTTPS, SSH (`ssh://...` or `git@host:owner/repo.git`) and local Git directories are supported.
Git must be on PATH; SSH uses the host's keys/agent and defaults to batch mode. Set authentication
in the host environment, never in template files or HTTPS URLs. Global Git config is not imported;
host-supplied `GIT_ASKPASS` or SSH authentication can be used for private repositories.

Repository listing returns the resolved `commit`. Init preview/creation returns `source`, and
`log.md` records the template content version and exact commit/path. Pin a commit for reproducible
initialization. Preview may fetch an explicitly chosen repository, but never creates the destination.
No template scripts, filters, checkout hooks or submodules are executed. The source bundle must
meet L2 and declare schemas for its document types before files are written.

## Create and contribute templates

Copy [`templates/_starter/`](../templates/_starter/README.md), edit its manifest and Markdown,
and follow the [v1 format specification](template-format.md). A single template or catalog can be
published in any compatible Git repository; users can initialize from it without a CLI release.

Built-in templates live in root `templates/`. Add a directory and an entry to `catalog.yaml`;
the asset crate embeds it automatically during build, with no CLI registry edit. Updating the
built-in offline catalog requires a new binary. Do not maintain a second copy inside a CLI crate.
For built-in contributions, include coherent English fictional records, five grounded trial questions,
three workflows, type schemas and links resolvable inside the source bundle. Acceptance tests discover
the catalog, initialize every bundle, lint it, retrieve evidence, query workflows and check its graph.
