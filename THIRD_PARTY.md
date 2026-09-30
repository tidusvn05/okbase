# Third-party notices

okfkit is licensed under MIT OR Apache-2.0. This file records third-party material
that is bundled in the repository or compiled into release binaries, beyond ordinary
Rust crate dependencies (whose licenses are checked by `cargo deny check`, see `deny.toml`).

## Compiled into binaries

| Component | Used by | License | Notes |
|---|---|---|---|
| _none yet_ | | | |

To do before the first release that embeds it: verify and record the license of the
lindera IPADIC dictionary (`embed-ipadic`), used for Japanese tokenization.

## Repository data

| Path | Source | License |
|---|---|---|
| `fixtures/okf-official/` (planned) | Open Knowledge Format samples (Google) | Apache-2.0, see the NOTICE file in that directory |
| `fixtures/openclaw-s/` (planned) | OpenClaw documentation | MIT, see the NOTICE file in that directory |

## Models

okfkit does not bundle any machine-learning model in the repository or in its binaries.
Opt-in embedding modules download models into a user cache and print the model license first.
