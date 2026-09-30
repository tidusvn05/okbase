# v0.1 acceptance eval (HANDOFF T11)

Re-runs the okf-scale spike's **G2** configuration on bundle **L** (OpenClaw docs,
~1M tokens, 30 questions in vi/en/ja) with the MCP server replaced by
`okfkit mcp serve --stdio`, with (`OS`) and without (`O`) the `okfkit-answer`
skill. Same system prompt, questions, model and LLM judge as the spike.

Target: **≥ 93 % correct** (G2 in the spike).

```sh
cargo build --release -p okfkit-cli
python3 spikes/acceptance-v0.1/run.py            # both configs, ~$1.5 of Claude Code quota
python3 spikes/acceptance-v0.1/run.py --limit 3  # smoke test
```

Needs the okf-scale bundles (`spikes/okf-scale/bundles/L`, built by
`spikes/okf-scale/build_bundles.py`; git-ignored). Runs and the judge cache are
written to `runs/` and `results/`; `results/table.md` has the summary.
