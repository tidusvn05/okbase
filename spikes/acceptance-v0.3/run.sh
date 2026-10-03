#!/usr/bin/env bash
# v0.3 retrieval evals (CPU only): S1 on fixtures/multilingual, S4 on okf-scale bundle L.
# Needs: OKBASE_MODELS_DIR with the models (EmbeddingGemma requires `okbase embed enable --accept-license`).
set -euo pipefail
cd "$(dirname "$0")/../.."
OUT=spikes/acceptance-v0.3/results
WORK=${WORK:-spikes/acceptance-v0.3/work}
mkdir -p "$OUT" "$WORK"
cargo build -q --release -p okbase --features embed-local --example retrieval_eval
BIN=target/release/examples/retrieval_eval
for model in ${MODELS:-embeddinggemma-300m-q4 bge-m3-int8}; do
  "$BIN" s1 fixtures/multilingual fixtures/multilingual/queries.json "$model" "$WORK/s1-$model" > "$OUT/s1-$model.json"
  "$BIN" s4 spikes/okf-scale/bundles/L spikes/okf-scale/questions.json "$model" "$WORK/s4-$model" > "$OUT/s4-$model.json"
done
