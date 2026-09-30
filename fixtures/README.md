# Test fixtures

Small bundles used by okfkit tests. Each directory carries its own license and NOTICE.

| Directory | Content | License |
|---|---|---|
| `okf-official/` | The four sample bundles from the OKF repository | Apache-2.0 |
| `openclaw-s/` | 31 pages of OpenClaw gateway docs (Mintlify frontmatter, not OKF) | MIT |
| `business/` | Synthetic vi/en/ja company bundle with typed metadata and CSV sheets (biz-meta spike, x1) | MIT OR Apache-2.0 |

Do not edit fixture files by hand: tests assert byte-identical round-trips against them.
