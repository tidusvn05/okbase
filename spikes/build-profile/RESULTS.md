# Development build debug information

## Question and decision

Issue [#39](https://github.com/tidusvn05/okbase/issues/39) reports a large `target/debug` after
workspace tests. Can we reduce it while retaining useful workspace stack traces?

Use `debug = "line-tables-only"` in `[profile.dev]` and `debug = false` in
`[profile.dev.package."*"]`. Tests inherit the dev profile. Workspace crates keep source
file/line information; dependencies omit debug information. Full variable inspection requires
the two Cargo overrides documented in `CONTRIBUTING.md`. Release settings are unchanged.

## Setup

Measured on 2026-10-18 at commit `9dbc907455946980c227216a53d339a4e64edfcf`, with its locked
dependencies, on Linux x86_64 (8 logical CPUs), Cargo 1.99.0 and rustc 1.99.0. Both sequential
builds used 4 jobs, incremental compilation, a warm registry/source cache, and separate empty
target directories. The first used Cargo's unmodified dev defaults; the second overrode only
the two debug settings above. Both ran `cargo test --workspace --no-run --locked`.

`benchmark.py` records monotonic elapsed time, allocated disk usage (`du -sk target/debug`),
apparent file sizes, and the executable count. Raw measurements and commands are in
[`results.json`](results.json). Build logs are not committed.

## Results

| Setting | Allocated debug directory | Elapsed | Executables |
|---|---:|---:|---:|
| Default full debug information | 10.45 GiB | 135.231 s | 39 |
| Workspace line tables; dependency debug off | 3.36 GiB | 98.888 s | 39 |

Allocated disk use fell **67.8%**, saving 7.09 GiB in this build. The largest executable fell
from 434,141,776 to 134,839,568 bytes. An `addr2line` check on the candidate's
`frontmatter::tests::splits_and_parses` test symbol resolved to
`crates/okbase-core/src/frontmatter.rs:467`, confirming workspace source locations survive.

This is one pair of builds on a shared host, without randomized order or an isolated OS cache.
The observed 26.9% time reduction is indicative, not a general performance guarantee. Full-feature
builds, incremental edits, debugger variable inspection, and other platforms were not benchmarked.
Normal repository checks and cross-platform CI must still pass before merging.

## Reproduce

Run this script against a clean checkout of the recorded baseline (the guard rejects a baseline
that already contains dev profile overrides). It uses installed Cargo and refuses existing target
directories; allow roughly 15 GiB free. It does not delete generated targets.

```sh
git worktree add --detach /tmp/okbase-profile-baseline 9dbc907455946980c227216a53d339a4e64edfcf
python3 spikes/build-profile/benchmark.py --workspace /tmp/okbase-profile-baseline \
  --output /tmp/okbase-profile-measurement --jobs 4
```

After recording the original measurements, the baseline target was removed to recover space;
the results and logs were retained. Neither published archives nor release profile settings
are affected by this development-only default.
