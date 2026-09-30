# Third-party notices

okfkit is licensed under MIT OR Apache-2.0. This file records third-party material
that is bundled in the repository or compiled into release binaries, beyond ordinary
Rust crate dependencies (whose licenses are checked by `cargo deny check`, see `deny.toml`).

## Compiled into binaries

| Component | Used by | License | Notes |
|---|---|---|---|
| mecab-ipadic 2.7.0 dictionary (via `lindera-ipadic`, feature `ja` of `okfkit-analyze`) | Japanese tokenization | NAIST / ICOT Free Software notice (permissive; the notice and its NO WARRANTY section must accompany redistributions) | Downloaded at build time by the `lindera-ipadic` build script (checksum verified) and embedded in the binary. The full notice ships in the `lindera-ipadic` crate as `NOTICE.txt`; release archives must include it. |

## Repository data

| Path | Source | License |
|---|---|---|
| `fixtures/okf-official/` | Open Knowledge Format sample bundles (Google) | Apache-2.0, see the NOTICE file in that directory |
| `fixtures/openclaw-s/` | OpenClaw documentation (subset) | MIT, see the NOTICE file in that directory |

## Models

okfkit does not bundle any machine-learning model in the repository or in its binaries.
Opt-in embedding modules download models into a user cache and print the model license first.
