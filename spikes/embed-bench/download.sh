#!/usr/bin/env bash
# Tải model ONNX tuỳ chỉnh (không có sẵn trong fastembed) vào .models/<name>/
set -euo pipefail
dl() { # name repo onnx_path
  local d=".models/$1"; mkdir -p "$d"
  for f in tokenizer.json config.json special_tokens_map.json tokenizer_config.json; do
    [ -s "$d/$f" ] || curl -sfL "https://huggingface.co/$2/resolve/main/$f" -o "$d/$f" || echo '{}' > "$d/$f"
  done
  [ -s "$d/model.onnx" ] || curl -fL --progress-bar "https://huggingface.co/$2/resolve/main/$3" -o "$d/model.onnx"
}
dl bge-m3-int8 Xenova/bge-m3 onnx/model_int8.onnx
dl gte-mb onnx-community/gte-multilingual-base onnx/model.onnx
dl gte-mb-int8 onnx-community/gte-multilingual-base onnx/model_int8.onnx
du -sh .models/*
