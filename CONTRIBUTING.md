# Contributing to okbase

Thanks for your interest! okbase is dual-licensed under MIT OR Apache-2.0. Unless you state otherwise, any contribution you submit is licensed the same way, with no additional terms.

## Before you start
- Read `AGENTS.md` (hard rules and conventions) and `docs/design.md` (design); releases follow `docs/releasing.md`.
- For larger changes (file format, public API, defaults, tool/skill wording), open an issue first. Changes to defaults need evaluation numbers. See `spikes/` for how earlier decisions were measured.

## Development
Requirements: stable Rust (MSRV 1.89) and [`cargo-deny`](https://github.com/EmbarkStudios/cargo-deny) (`cargo install cargo-deny --locked`).

The workspace lives in `crates/`. `spikes/` holds standalone experiments and is excluded from the workspace.

Run these before opening a pull request (CI runs the same checks, plus tests on Linux, macOS and Windows and an MSRV build):
```
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo deny check
```

## Pull requests
- Use conventional commit messages (`feat:`, `fix:`, `docs:`, `refactor:`, `test:`, `chore:`).
- Signing off commits (`git commit -s`, DCO) is encouraged.
- Add tests. For output meant for agents, add or update snapshot tests.
- Keep the core lexical and model-free. Heavy dependencies belong in opt-in features.

## Code of Conduct
This project follows the Contributor Covenant 2.1 (see `CODE_OF_CONDUCT.md`).
