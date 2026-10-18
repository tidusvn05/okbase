# okbase knowledge templates

Review a bundle's Markdown before choosing it. The CLI embeds the same files for offline use.
[`catalog.yaml`](catalog.yaml) is the built-in registry: adding a directory and catalog entry needs
no CLI code changes. A new binary release includes the updated built-in catalog; local and Git
sources can be used immediately with a CLI supporting their format version.

| Template | Use it for |
| --- | --- |
| [software-project](software-project/README.md) | Architecture, APIs, decisions and delivery |
| [personal-assistant](personal-assistant/README.md) | Personal context, preferences and reminders |
| [project-assistant](project-assistant/README.md) | Project memory, tasks, reports and investigations |
| [infra-ops](infra-ops/README.md) | Servers, services, mappings, deployments and incidents |
| [product-management](product-management/README.md) | Discovery, priorities and delivery outcomes |
| [engineering-lead](engineering-lead/README.md) | Team capacity, engineering health and decisions |
| [executive](executive/README.md) | Strategy, business metrics and executive reporting |
| [team-handbook](team-handbook/README.md) | Onboarding and team operating knowledge |
| [customer-support](customer-support/README.md) | Support procedures, policies and escalation |
| [research](research/README.md) | Research questions, sources and findings |

```sh
okbase templates
okbase -b ./kb init --template software-project --print
okbase -b ./kb init --template software-project
```

Domain records are fictional examples. Read the selected bundle's getting-started guide before
replacing them. The user owns initialized files; updating okbase never updates a bundle in place.

To author a template, copy [`_starter/`](_starter/README.md) and follow the
[public v1 format](../docs/template-format.md). Validate with `init --template-path ... --print`.
See [CLI usage](../docs/templates.md) for local catalogs and Git repository sources.

This directory is also the `okbase-templates` asset crate. Its build script reads the catalog and
embeds declared template directories; no manually maintained Rust registry or generated source copy
is committed. Keeping the assets inside this package makes crates.io archives self-contained.
