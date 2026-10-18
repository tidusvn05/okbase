# Releasing okbase

How okbase is versioned, what a release contains, and the steps to cut one. Releases are built by
`.github/workflows/release.yml` when a version tag is pushed; users install them with the `curl`
command in the README or with `cargo install --locked okbase-cli` from crates.io.
A release is complete only when its GitHub assets and all 17 crates.io packages are available.

## Versioning

okbase follows [Semantic Versioning](https://semver.org/). Before 1.0, a minor release (0.x.0) may
break the public surface and a patch release (0.x.y) never does. From 1.0, only a major release
breaks it.

**The public surface** (a change here is breaking unless it only adds):
- CLI commands, flags and their defaults;
- the `--json` output of every command, which equals the MCP tool output;
- exit codes (0 ok, 1 error, 3 the user must decide, 4 findings) and error codes;
- MCP tool names, arguments and outputs, and the server instructions;
- skill names and the files `agent install` writes;
- `okbase.toml`, `.okbaseignore`, frontmatter written by okbase, and `OKBASE_*` environment
  variables;
- the public Rust APIs and features of every published `okbase` / `okbase-*` crate.

**Not public:**
- the on-disk index and caches (okbase rebuilds them when their schema changes);
- the HTTP API under `/api/` that the `okbase view` web app uses (its bodies are the `--json`
  structs, but routes and parameters follow the embedded app).

Changing a default (search mode, ranking, chunk size, tool descriptions, skill text) needs eval
data (AGENTS.md) and is called out in the release notes even when it is not breaking.

The version lives once, in `[workspace.package]` of the root `Cargo.toml`; every member and
internal dependency uses that version. Bump it only while preparing a concrete release. After a
release, main keeps the released version until the next release is prepared. Never open an
automatic development-version PR or append a `-dev` suffix.

## Commits and the changelog

Every commit on `main` follows [Conventional Commits](https://www.conventionalcommits.org/).
- **Types:** `feat`, `fix`, `perf`, `refactor`, `docs`, `build`, `ci`, `test`, `chore`.
- **Breaking changes:** mark them with `!` (`feat!: …`) and a `BREAKING CHANGE:` footer.

`CHANGELOG.md` and the release notes are generated from these commits by
[git-cliff](https://git-cliff.org) (`cliff.toml`). Commits that are not conventional are left out,
so write the subject for a user.

## What a release contains

For the tag `vX.Y.Z`, the GitHub release has:

| File | Contents |
|---|---|
| `okbase-vX.Y.Z-<target>.tar.gz` (`.zip` on Windows) | the default build: lexical tools, data, import, MCP |
| `okbase-full-vX.Y.Z-<target>.tar.gz` (`.zip` on Windows) | plus local embeddings and fine-tuning |
| `SHA256SUMS` | the checksum of every archive |

Every archive holds:
- the binary (`okbase` or `okbase.exe`);
- `README.md`, `CHANGELOG.md`, `LICENSE-MIT`, `LICENSE-APACHE`;
- `THIRD_PARTY.md`, and `THIRD_PARTY_LICENSES.md` (the licenses of every compiled crate, from
  cargo-about with `about.toml`).

okbase-full archives also hold ONNX Runtime's license and third-party notices in `licenses/`.
Neither build embeds models or language dictionaries.

| Target | Runner | Notes |
|---|---|---|
| `x86_64-unknown-linux-gnu` | ubuntu-22.04; okbase-full on ubuntu-24.04 | glibc 2.35 or newer; okbase-full 2.39 or newer |
| `aarch64-unknown-linux-gnu` | ubuntu-22.04-arm; okbase-full on ubuntu-24.04-arm | glibc 2.35 or newer; okbase-full 2.39 or newer |
| `x86_64-apple-darwin` | macos-14 (cross-compiled) | Intel Macs; okbase only (ONNX Runtime has no prebuilt library) |
| `aarch64-apple-darwin` | macos-14 | Apple silicon |
| `x86_64-pc-windows-msvc` | windows-latest | installed with `install.ps1` |

Only tags `vX.Y.Z` and `vX.Y.Z-rc.N` are accepted. RC tags become GitHub pre-releases and never
publish crates or images. Stable tags publish all 17 crates after the GitHub release. There is no
Dockerfile, so this repository publishes no container image. A future Dockerfile requires a stable-only
image job after the GitHub release, publishing to `ghcr.io/tidusvn05/okbase`.

Every release also includes `okbase-vX.Y.Z-source.tar.gz`, containing the tagged source and its
Cargo.toml version, covered by SHA256SUMS. If the workspace becomes library-only, publish that
source archive instead of binary builds. Cargo metadata binary targets drive the build gate;
examples are never treated as release binaries.

 `install.sh` installs the latest
non-pre-release unless `--version` is given.

## Before a release

- [ ] CI is green on `main`: fmt, clippy, tests on Linux, macOS and Windows, the full build, MSRV,
      `cargo deny`.
- [ ] `cargo run --release -p okbase-eval -- lexical` shows no regression.
- [ ] Changes to tools, skills or defaults since the last release have their agent eval in
      `spikes/` (and `spikes/README.md` is updated).
- [ ] `docs/usage.md`, `README.md` and `llms.txt` match the release: commands, sizes, privacy table.
- [ ] Breaking changes are marked in commits and explained in `docs/usage.md` when users must act.
- [ ] Package CI passes: all publishable crates build from their packaged sources.
- [ ] Every crate trusts this repository, `release.yml` and environment `crates-io`; the workflow
      obtains its short-lived credential through OIDC. Configuration/bootstrap credentials stay local.

## Prepare, approve, merge and tag

Read the real repository state first, including remote main, existing releases, Cargo metadata,
Dockerfile presence and tool versions. Work in an isolated worktree if local changes exist. Reuse
installed Cargo (workspace packaging requires a Cargo with `publish --workspace`, tested with 1.99;
the library MSRV remains 1.89). Never include unrelated changes or PRs.

Prepare a branch `release/vX.Y.Z`:

```sh
scripts/release.py version X.Y.Z
git cliff --tag vX.Y.Z -o CHANGELOG.md
scripts/verify.sh
scripts/release.py publish --dry-run --allow-dirty
scripts/release.py pending
git add Cargo.toml Cargo.lock CHANGELOG.md # plus explicitly reviewed release tooling changes
git commit -m "chore(release): vX.Y.Z"
```

The version helper updates workspace/member versions, internal normal/build/dev dependencies and
Cargo.lock. Dry-run packages and builds the whole workspace using Cargo's local packaging registry,
so internal versions need not exist on crates.io yet. `--allow-dirty` is allowed only with dry-run;
real publishing requires a clean tree. Fix advisories and yanked dependencies before tagging.

Scan the complete diff for secrets before **every push**, then push the branch and open a PR titled
`chore(release): vX.Y.Z`. Separate setup PRs are optional; one release PR may contain both tooling
and release preparation. Wait for all current-base CI checks, review the final diff and changelog.

Ask the maintainer **once**, only after the changes and checks are reviewable, specifying the version,
PRs to merge, any new permanent crate names, tag, outputs and necessary trust/bootstrap configuration.
Before approval, do not merge, push tags, change publishing configuration or publish. Approval covers
the complete sequence below and routine fixes within its scope; do not ask again at each step.
A version change or expansion of scope still needs a new decision.

After approval:

1. Ensure the GitHub environment `crates-io` and each crate's trust match this repository,
   `release.yml` and `crates-io`. Add only missing settings, using the existing Cargo credential
   for configuration when authorized. Follow bootstrap below only for genuinely new crates.
2. Merge the approved PRs when current-base CI is green. If setup and release PRs are separate,
   update the release branch after setup merges and wait for CI again.
3. Tag the approved commit on main and scan before pushing:
   ```sh
   git fetch origin main
   git tag -a vX.Y.Z origin/main -m "okbase vX.Y.Z"
   git push origin vX.Y.Z
   ```
4. Watch Release through `check → build → release → crates`. Check validates the exact tag format,
   main ancestry, workspace/member versions, changelog, repository verification and publish dry-run.
   Build preserves the nine existing platform/variant archives; release adds source, SHA256SUMS
   and git-cliff notes. Only stable tags run the crates job.
5. Verify all outputs as described below. Keep main at the version just released.

### ZIP advisory assessment for v0.2.4

This release proposes three bounded exceptions for unreachable filesystem-extraction operations
in zip 8.6.0; it does **not** upgrade zip to a patched version. Read
[the assessment](security/zip-advisories.md) and include explicit acceptance of the exceptions in
the single release approval. CI verifies the audited readers, source hashes and dependency graph
and refuses an expired assessment (review before 2026-11-18). Upstream reader updates supporting
zip >=9 must replace these exceptions when available.

## Trusted Publishing and bootstrap

All automatic crates.io publication uses `rust-lang/crates-io-auth-action@v1` with
`permissions: id-token: write` in the GitHub environment `crates-io`. No crates.io API token belongs
in repository files or GitHub secrets. Every existing crate keeps its name, including the binary
package `okbase-cli` (which installs the `okbase` executable). `okbase-eval` stays unpublished.

Check existing owners and trust read-only before approval. After approval, configure existing crates
without republishing their versions:

```sh
scripts/release.py trustpub tidusvn05/okbase --only
```

This reads the credential saved by `cargo login`, creates only missing trust configurations, and
sets Trusted Publishing only. The credential must have `trusted-publishing` scope and ownership.
Never print the credential, put it in chat, or revoke an existing credential that might serve other work.
If scope is missing, the owner can configure each crate at crates.io → Settings → Trusted Publishing
for repository `tidusvn05/okbase`, workflow `release.yml`, environment `crates-io`, or log in locally
with an appropriately scoped credential and resume. Do not request release approval again.

### First publish or a new crate later

Trusted Publishing cannot create a crate. `pending` names new crates and automatic publishing
stops **before any upload** if one exists. Before requesting the single release approval:

1. Check the name on crates.io, including all equivalent hyphen/underscore spellings, and verify
   existing ownership. Preserve existing names. Any proposed new binary package name is permanent
   and must be part of that approval; library-only repositories need no binary-name decision.
2. List the exact new crate names and dependency order. Ask the owner to create a short-lived token
   with `publish-new`, `publish-update` and `trusted-publishing` scopes limited to those exact names.
   The owner enters it directly with `cargo login`, never through chat. Preserve any existing Cargo
   credential separately and restore it afterwards; do not overwrite or revoke another credential.
3. After release/bootstrap approval, publish the new crates manually from clean merged main with
   `cargo publish --locked --registry crates-io -p <crate>`, in `scripts/release.py order` order
   (including dev-dependencies), waiting for registry availability between dependents.
4. Run `scripts/release.py trustpub tidusvn05/okbase --crates <comma-separated-new-crates> --only`.
5. Log out only the temporary credential, restore any previous credential, and revoke the temporary
   token on crates.io. Never revoke unrelated credentials. Then push the approved tag.

Dependency cycles, including cycles through dev-dependencies, are fatal and reported before upload.
Do not work around cycles by publishing in an arbitrary order.

## Fixes to a release

- **Fix forward.** Release `vX.Y.(Z+1)` from `main`. For an older line, fix on a `release/vX.Y`
  branch made from the tag, then cherry-pick.
- **A broken release:**
  1. Mark it in its notes ("Broken: use vX.Y.Z+1").
  2. Publish the fix.
  3. Never replace assets of a published tag: the checksums users verified must stay valid.
- **Security fixes:** follow SECURITY.md (private advisory first, then a patch release that names
  the advisory).

## Publishing to crates.io

`scripts/release.py order` prints every publishable crate in dependency order, including dev and
build dependencies. `pending` reports missing versions and new crates. `publish` rejects prereleases,
dirty trees, new crates and yanked versions before uploading anything. Already-published unyanked
versions are skipped, so retrying after a partial publish is safe. It uses Cargo's configured
credential locally and the short-lived OIDC credential in CI.

HTTP 429 responses wait and retry. Registry HTTP requests honor `Retry-After` as seconds or an HTTP
date; Cargo publish errors use the registry's retry timestamp when present. If a job times out,
rerun it later; uploaded versions remain immutable. Never reuse a yanked version.

### Partial or failed releases

- Before the tag: fix the PR, rerun checks, merge only when green.
- Build failure: read the job logs and retry a transient failure. Never move a pushed tag. If the
  tagged source needs a code fix, prepare a new patch version and obtain the version decision.
- GitHub publication: dispatch `release.yml` for the existing tag. A complete public release is
  checksum-verified and skipped; existing assets are never overwritten. A draft or mismatched
  asset set stops for review. A published partial asset set requires a fix-forward version.
- Crates failure after GitHub publication: rerun the failed crates job. It skips published versions
  and uploads only missing ones. Check trust repository/workflow/environment when OIDC is refused.
- Treat the release as incomplete until registry, archives and installation checks finish.
- Yank only with explicit authorization: `cargo yank --version X.Y.Z <crate>`. Yanking does not
  remove an immutable version or make its version reusable. Publish the fix under a new version.

### Verify completion

Check GitHub release tag, stable/prerelease state and git-cliff notes. Download every archive plus
SHA256SUMS, run `sha256sum --check SHA256SUMS`, and inspect Cargo.toml in the source archive.
On a compatible host, run each applicable binary's `--version`, install with `install.sh` and test
`doctor` on a fixture with a temporary state directory. Verify Linux glibc requirements from the
platform table using `objdump -T`. Cross-platform execution needs matching hosts or CI; record limits.

For stable releases, check all 17 exact versions on crates.io (not yanked), and in a fresh project
add `okbase = "=X.Y.Z"` and run `cargo check`. Install with:

```sh
cargo install --locked --version X.Y.Z --root /tmp/okbase-registry-install okbase-cli
/tmp/okbase-registry-install/bin/okbase --version
```

For RC releases, verify no crates or images were published. If a Dockerfile and image job are added,
check the stable image tags and run the image's version command. Finish with release, crate and PR
links, verification results and only actions that still require the maintainer. Do not bump a
post-release development version.

The initial crates.io version 0.2.3 was published after its GitHub binary release. Its packaging
source is preserved by `crates-v0.2.3`; subsequent release tags cover both channels.

## Tooling provenance

The release helper and Trusted Publishing job are adapted from
[okbot's release helper](https://github.com/tidusvn05/okbot/blob/main/scripts/release.py),
[workflow](https://github.com/tidusvn05/okbot/blob/main/.github/workflows/release.yml) and
[release guide](https://github.com/tidusvn05/okbot/blob/main/docs/releasing.md). okbase keeps its own
platform variants, forbids RC crate/image publication, and leaves main at the released version.
