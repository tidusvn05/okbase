# Template format v1

A template is data: a `template.yaml` manifest, an optional human-facing `README.md`, and a
self-contained OKF bundle directory. The same contract applies to built-in, local and Git sources.
No executable hooks, dependency installation, interpolation, include directives or custom rendering
language are supported. Bundle files are ordinary UTF-8 text, copied with their relative paths.
Markdown bodies and frontmatter remain unchanged; init updates the root title/declared description,
explicit language overrides and provenance log, and adds requested type schemas that are absent.
An existing schema is never replaced by `--types`.

```yaml
format_version: 1
id: my-template
version: 1.0.0
title: My knowledge base
description: Verified guides for my team.
roles: [developer]
bundle: bundle
```

Required fields are `format_version`, `id`, `version`, `title`, `description` and `bundle`.
`roles` defaults to `[]`. `format_version` is the integer 1; unsupported versions and unknown fields
are rejected. `id` is a non-empty string of lowercase ASCII letters, digits and hyphens. `version`
is a non-empty author-assigned content version (SemVer is suggested). Title and description are
non-empty strings; the root index must have a `# Title` heading. Put the declared description on
its own line if `--description` should replace it. No other Markdown text is substituted.

`bundle` is a relative directory path within the template. The bundle must contain `index.md`,
`okbase.toml`, `_meta/vocabulary.md` and the schemas its documents require. It must reach L2 before
init accepts it. Unknown fields and comments in document frontmatter are preserved. `okbase.toml`
must be valid TOML with a `[bundle]` table and an absent or `okf` profile; its comments and unrelated settings survive explicit language overrides.
Use `okbase lint --level L3` on the source bundle to inspect additional findings such as links.

Optional grounded acceptance questions live in the manifest:

```yaml
questions:
  - question: How should a new topic be added?
    source: guides/getting-started
    evidence: Declare a canonical tag before using it in documents.
```

Each entry requires `question`, `source` (document ID without `.md`) and `evidence`. The referenced
file must contain the literal evidence. Questions are checked when loading; they do not constitute
an evaluation of a live agent.

A collection has a `catalog.yaml` at its root:

```yaml
format_version: 1
templates:
  - path: software-project
  - path: infra-ops
```

Each path identifies a template directory beneath the catalog. Display order follows the catalog.
Paths and template IDs must be unique. Names and descriptions come from each template manifest;
the catalog does not duplicate them. A repository may contain a single template at its root with
no catalog, or a collection at the root or in a subdirectory. A collection with multiple templates
requires `--template ID` for init. An invalid entry fails the entire catalog with an error.

## Portable paths and source handling

Paths must use `/`, be relative, and contain no empty, `.` or `..` components, backslashes, colons,
control characters or dot-prefixed components. Symlinks, submodules and special files are unsupported. Bundle
assets must be UTF-8; binary attachments and templated filenames are outside v1. A bundle is limited
to 4096 files and 32 MiB. These bounds protect initialization from oversized untrusted inputs;
they are not search or quality defaults. Git export applies the same limits to the selected source
directory. Select a narrow source directory when the repository also contains unrelated content.

Built-in assets come from the versioned root catalog and are embedded during compilation. Local
sources are read directly. Git HTTPS/SSH sources require Git on PATH and an explicitly supplied repository;
no network discovery occurs by default. The resolver fetches a branch, tag or commit into temporary
storage and exports regular Git blobs without checkout, filters, hooks or submodules. Git metadata
(dot-prefixed paths) is omitted during export. Files are not made executable. Temporary objects
and exports are removed when the operation ends. No persistent registry/cache is installed.
`--print` may fetch a selected repository but creates no destination files or bundle index state.
`OKBASE_OFFLINE=1` or `true` rejects network Git sources; local Git directories remain usable.

HTTPS URLs must not embed credentials, query strings or fragments. Authentication must be supplied
by the host environment, never by template files. Repository and global Git configuration are not imported. Host-supplied `GIT_ASKPASS` or SSH
keys/agent can authenticate private repositories. SSH defaults to batch mode and supports an optional port in `ssh://` URLs. Both `ssh://user@host/path` and `git@host:owner/repo.git` are supported.
Pin `--ref` to a commit for reproducibility; mutable branch/tag names resolve to a commit that is
returned by listing and recorded in init output and `log.md`, along with the repository and selected
subdirectory. Stored template instructions never grant host permissions. Review third-party content
before using it as agent context.

## Author checklist

1. Copy [`templates/_starter`](../templates/_starter/README.md), change the manifest and write real
   Markdown. Keep fictional domain examples explicitly marked `example: true`, `status: draft`.
2. Check the source bundle with `okbase -b ./my-template/bundle lint --level L3`.
3. Preview with `okbase -b ./trial init --template-path ./my-template --print --json`.
4. Initialize into a fresh directory, run lint and try source-grounded questions.
5. Publish the directory in any compatible Git repository. For a built-in contribution, add its
   path to `templates/catalog.yaml`; build and acceptance tests discover it without CLI edits.

The JSON Schemas alongside the catalog describe the v1 manifest and catalog structure. The CLI
also enforces filesystem containment, matching evidence, unique IDs and bundle conformance.
