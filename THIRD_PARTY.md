# Third-party notices

okfkit is licensed under MIT OR Apache-2.0. This file records third-party material
that is bundled in the repository or compiled into release binaries, beyond ordinary
Rust crate dependencies (whose licenses are checked by `cargo deny check`, see `deny.toml`).

## Compiled into binaries

| Component | Used by | License | Notes |
|---|---|---|---|
| ONNX Runtime (via `ort`, feature `local` of `okfkit-embed`; off by default) | Local embedding models | MIT | Prebuilt binaries are downloaded by the `ort` build script (rustls) and linked into builds with embed-local (such as `okfkit-full`) only. |
| mecab-ipadic 2.7.0 dictionary (feature `ja-embedded` of `okfkit-analyze`, off by default) | Japanese tokenization | NAIST / ICOT Free Software notice (permissive; the notice and its NO WARRANTY section must accompany redistributions) | Only embedded in builds with `ja-embedded`; the release archives of such builds must include the notice (shipped in the `lindera-ipadic` crate as `NOTICE.txt`). |

## Downloaded at runtime

| Component | When | License | Notes |
|---|---|---|---|
| mecab-ipadic 2.7.0 source (`https://Lindera.dev/mecab-ipadic-2.7.0-20250920.tar.gz`, md5-verified) | First time okfkit tokenizes Japanese text (default build); disable with `OKFKIT_OFFLINE=1` | NAIST / ICOT Free Software notice | Built into `<user cache>/okfkit/dict/` on the user's machine; not redistributed by okfkit. |

`crates/okfkit-analyze/assets/ipadic-metadata.json` is copied from the `lindera-ipadic` crate (MIT).

## Repository data

| Path | Source | License |
|---|---|---|
| `fixtures/okf-official/` | Open Knowledge Format sample bundles (Google) | Apache-2.0, see the NOTICE file in that directory |
| `fixtures/openclaw-s/` | OpenClaw documentation (subset) | MIT, see the NOTICE file in that directory |

## Models

okfkit does not bundle any machine-learning model in the repository or in its binaries.
Opt-in embedding modules download models into a user cache and print the model license first.
