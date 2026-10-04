# Releasing okbase

How okbase is versioned, what a release contains, and the steps to cut one. Releases are built by
`.github/workflows/release.yml` when a version tag is pushed; users install them with the `curl`
command in the README.

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
- the public Rust API of the `okbase` crate.

**Not public:**
- the on-disk index and caches (okbase rebuilds them when their schema changes);
- internal crates' APIs, until they are published on crates.io;
- the HTTP API under `/api/` that the `okbase view` web app uses (its bodies are the `--json`
  structs, but routes and parameters follow the embedded app).

Changing a default (search mode, ranking, chunk size, tool descriptions, skill text) needs eval
data (AGENTS.md) and is called out in the release notes even when it is not breaking.

The version lives once, in `[workspace.package]` of the root `Cargo.toml`. Between releases it
carries a `-dev` suffix (`0.1.0-dev`).

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

Tags with a suffix (`v0.2.0-rc.1`) become pre-releases. `install.sh` installs the latest
non-pre-release unless `--version` is given.

## Before a release

- [ ] CI is green on `main`: fmt, clippy, tests on Linux, macOS and Windows, the full build, MSRV,
      `cargo deny`.
- [ ] `cargo run --release -p okbase-eval -- lexical` shows no regression.
- [ ] Changes to tools, skills or defaults since the last release have their agent eval in
      `spikes/` (and `spikes/README.md` is updated).
- [ ] `docs/usage.md`, `README.md` and `llms.txt` match the release: commands, sizes, privacy table.
- [ ] Breaking changes are marked in commits and explained in `docs/usage.md` when users must act.

## Cutting a release

```sh
git switch main && git pull --ff-only
V=0.1.0                                    # the new version, without "v"

# 1. Set the version (workspace package and the internal crates' version requirements).
sed -i "s/^version = \".*\"/version = \"$V\"/" Cargo.toml          # [workspace.package]
sed -i "s/\(okbase[a-z-]* = { path = \"crates\/[a-z-]*\", version = \"\)[^\"]*/\1$V/" Cargo.toml
cargo check --workspace                     # updates Cargo.lock

# 2. The changelog, reviewed by hand.
git cliff --tag "v$V" -o CHANGELOG.md

# 3. Commit only the release files, tag, push.
git add Cargo.toml Cargo.lock CHANGELOG.md
git status --short                          # nothing else staged
git commit -m "chore(release): v$V"
git tag -a "v$V" -m "okbase v$V"
git push origin main "v$V"
```

Check that `sed` changed only the intended lines (`git diff Cargo.toml`) before committing. Never
use `git commit -a` here: it would also commit unrelated local changes.

The workflow then:
1. checks that the tag equals the version and that `CHANGELOG.md` has its section;
2. runs the tests;
3. builds the nine archives (five okbase, four okbase-full);
4. publishes the release with the notes and `SHA256SUMS`.

`workflow_dispatch` with an existing tag rebuilds it as a draft release (for a failed run).

After the workflow:
- [ ] Install with the README command on Linux or macOS, and with `install.ps1` on Windows.
- [ ] `okbase --version` shows the new version.
- [ ] `okbase doctor` is ok on a fixture bundle.
- [ ] The Linux binaries need no newer glibc than the table above says (2.35, okbase-full 2.39):
      `objdump -T okbase | grep -o 'GLIBC_[0-9.]*' | sort -uV | tail -n 1` on each Linux archive.
- [ ] Set the version back to the next `-dev` (`0.1.1-dev`) in a `chore(release)` commit.

## Fixes to a release

- **Fix forward.** Release `vX.Y.(Z+1)` from `main`. For an older line, fix on a `release/vX.Y`
  branch made from the tag, then cherry-pick.
- **A broken release:**
  1. Mark it in its notes ("Broken: use vX.Y.Z+1").
  2. Publish the fix.
  3. Never replace assets of a published tag: the checksums users verified must stay valid.
- **Security fixes:** follow SECURITY.md (private advisory first, then a patch release that names
  the advisory).

## Not automated yet

- **Publishing the crates on crates.io.** It needs a crates.io token in the repository secrets and
  publishing in dependency order (`okbase-core` first, `okbase-cli` last). The crates also need:
  - their own license files;
  - a crates.io README without repository-relative links;
  - tests that skip when `fixtures/` is absent.
- **Signed artifacts.** Releases use checksums only; Sigstore or minisign signatures are possible
  later.
