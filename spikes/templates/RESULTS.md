# Starter templates: offline acceptance

Date: 2026-10-19. Implementation plan: [templates](../../docs/plans/templates.md).
User guide: [starter knowledge templates](../../docs/templates.md).

## Question and setup

Can the shipped templates create useful, conformant knowledge bundles without a model, network,
new runtime service or custom template engine? Do the existing CLI, Rust and MCP reads preserve
their behavior on the new document types and workflow fields?

The ten embedded manifests contain 73 fictional domain records. The renderer adds ten
getting-started guides, 30 workflow guides, type schemas, vocabulary, listings, configuration and
an initialization log. Domain examples are draft/example records; instructions are stable guides.
Trials use five questions per template, each with a particular source ID and literal evidence.

Acceptance runs through the real CLI and the existing index/read APIs in temporary directories.
Tests close stdin and set OKBASE_OFFLINE=1. No agent CLI, model, provider API or paid quota was used.

## Initial acceptance on development baseline 32381e1

| Check | Result |
|---|---|
| Template catalog | 10 entries; no bundle or index needed |
| Generated bundle conformance | 10/10 reached L2; zero errors and warnings under L3 lint |
| Source evidence retrieval | 50/50 indexed get/grep cases contained the specified supporting passage |
| Workflows | 30 guides discoverable through normal metadata queries |
| Links | All ten generated graphs had zero broken links |
| Extension | `new --type Task` used the template schema and updated the listing; filling its required fields restored error-free L2 |
| Existing contracts | CLI and MCP task-filter results matched; custom task states and service-to-server filters worked |
| Host scopes | A denied personal-profile path remained inaccessible through the read API |
| Preview and preservation | Preview created nothing; unknown templates, unsafe types, occupied destinations, existing bundles and symlinked destination directories were rejected |
| Compatibility | Empty initialization retained Guide/Policy/FAQ; title, description, language and additive type overrides worked |
| Snapshots | Catalog, preview paths, onboarding choices and rendered author skill reviewed |

The seven template acceptance tests passed. The full workspace run passed 202 tests, with four
existing ignored tests. `cargo fmt --all -- --check` and
`cargo clippy --workspace --all-targets --all-features -- -D warnings` passed.

The existing lexical eval also passed without baseline changes:

| Suite | Result | Recorded baseline |
|---|---|---|
| business | 44/48 | 44/48 |
| multilingual | 27/30 | 27/30 |

## Initial environment findings and remaining limitations

- The first workspace run encountered the existing inside/outside-Codex test's inherited
  CODEX_THREAD_ID. Running with that environment variable removed lets the test set its own
  inside-Codex case explicitly.
- An existing `.okbase` cache inside the GA4 fixture was copied by the index test's recursive
  fixture copier, invalidating its clean-input assumption. The cache was temporarily moved out
  for the successful workspace run and restored afterward; no tracked fixture or existing test
  was changed to bypass the assertion.
- `cargo deny check` did **not** pass: advisories failed; bans, licenses and sources passed.
  The audit reported RUSTSEC-2026-0346, -0347, -0348 and -0349 for rustls 0.23.45, and
  RUSTSEC-2026-0367, -0368 and -0369 for zip 8.6.0. Both package versions were confirmed in
  baseline 32381e1's lockfile before this feature. The feature adds only a direct dependency on the already
  present serde-saphyr package; it does not update or suppress these dependencies or advisories.
- The 50 source checks establish that the recorded evidence survives rendering, indexing and
  retrieval. They do **not** measure natural-language answer accuracy, autonomous template
  selection or whether a real agent follows the updated author/onboarding instructions.
  A quota-using agent evaluation remains unrun pending the consent required by the spike rules.
- Linux was tested locally. Windows/macOS behavior is covered by the existing CI matrix, not
  claimed as a local result. The Unix symlink test is platform-gated.
- Templates document host workflows; no bot runtime, scheduler, task synchronization or
  infrastructure execution was exercised or added.

## Publication revalidation on main 2047e0a

The feature was isolated from unrelated local changes and applied to current main (okbase 0.2.5)
in a clean worktree before publication. This base includes dependency and test-isolation maintenance
that was absent from the initial development checkout.

- All seven template acceptance tests passed again, including ten L2 bundles and 50 source checks.
- `cargo test --workspace` passed 214 tests with four existing ignored tests, without fixture-cache
  or Codex environment workarounds in the clean worktree.
- Formatting and workspace clippy with all targets/features passed.
- Release lexical eval passed with no regressions: business 44/48 and multilingual 27/30,
  matching their recorded baselines.
- `cargo deny check` passed advisories, bans, licenses and sources with the existing main policies;
  this feature adds no advisory exceptions or dependency version changes.
- Onboarding snapshot paths now use a relative bundle path so Unix and Windows serialize the same
  commands without platform-specific path escaping or shell-quoting differences.
- Quota-using real-agent behavior remains unmeasured; no such evaluation was run while publishing.

## Reproduce

```sh
cargo test -p okbase-cli --test templates
cargo test -p okbase-skills --test install
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo deny check
cargo run --release -p okbase-eval -- lexical
```

For a local workspace run, start with clean fixtures (preserve any existing ignored fixture caches
outside the copied input and restore them afterward). The Codex environment workaround described
above applies to the old development baseline; current main isolates that test explicitly. The
initial dependency-audit failures are retained as historical evidence; the publication base passes
the audit, as recorded above.

## Public template format and reusable sources (2026-10-19)

Moved the ten starters to root `templates/` as reviewable OKF Markdown bundles. The declarative
catalog drives build-time embedding through the self-contained `okbase-templates` asset crate;
there is no template ID list in CLI/Rust dispatch. Local directories, collections and Git sources
use the same v1 manifest and validated init planner. An authoring starter and JSON Schemas document
the public contract. Existing initialized bundles are not changed.

Validation on the feature branch:

- Workspace: **220 passed, 4 ignored**; fmt and all-feature clippy passed; cargo deny passed.
- Existing built-in acceptance: **10/10** L2 bundles without L3 errors/warnings, **50/50** indexed
  source-evidence checks and all **30** workflow guides retained.
- Six custom-source acceptance tests cover local catalogs, ID selection, manifest versions/unknown
  fields, missing evidence/schemas, invalid TOML, an oversized sparse asset, path containment,
  symlinks, offline transport behavior, Git root templates/collections, historical commit pinning,
  exact commit/subdirectory provenance and no checkout hook execution. Markdown/frontmatter and
  CRLF root-index bytes survive copying; TOML comments/custom settings survive language overrides.
- `cargo package -p okbase-templates --allow-dirty` packaged and independently verified the asset
  crate (**341 files**, about **239 KiB** uncompressed), including its Markdown assets. Workspace
  publication order discovers the new crate automatically; **23** release-helper tests passed.

Git acceptance uses temporary local repositories and needs no service credentials. Templates are
UTF-8 data; external repo scripts, filters, hooks and submodules are not executed. HTTPS/SSH transport
requires the host's Git and authentication. No paid/live-agent evaluation was performed; source
retrieval checks do not measure how a bot follows third-party instructions.

```sh
cargo test -p okbase-cli --test templates --test custom_templates
cargo package -p okbase-templates --allow-dirty
python3 -m unittest discover -s scripts -p 'test_*.py'
```

Release-mode lexical replay after the public-format change: **business 44/48**, **multilingual
27/30**, identical to the recorded baselines (no regression).
