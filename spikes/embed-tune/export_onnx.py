#!/usr/bin/env python3
"""Export a tuned EmbeddingGemma to okbase's ONNX Q4 format by patching the reference model.

LoRA changes only some linear weights, so instead of re-exporting the graph we take the
onnx-community reference export (the one okbase already uses), add each weight delta
(tuned - base) to its fp32 values and re-quantize just those matrices with the same scheme
(MatMulNBits, 4 bits, block 32, symmetric). Everything else stays byte-identical.

Usage: .venv/bin/python export_onnx.py <tuned dir> <reference onnx dir> <out dir> [name]
  reference dir: model.onnx(+_data) and model_q4.onnx(+_data) from onnx-community/embeddinggemma-300m-ONNX
"""
import glob, json, re, shutil, sys
from pathlib import Path

import numpy as np
import onnx
from huggingface_hub import snapshot_download
from onnx import numpy_helper
from onnxruntime.capi._pybind_state import quantize_matmul_4bits
from safetensors.numpy import load_file

BASE = "unsloth/embeddinggemma-300m"
BLOCK = 32


def onnx_name(hf):
    m = re.fullmatch(r"layers\.(\d+)\.(self_attn|mlp)\.(\w+_proj)\.weight", hf)
    if not m:
        return None
    return f"model.layers.{m[1]}.{'attn' if m[2] == 'self_attn' else 'mlp'}.{m[3]}.MatMul.weight"


def q4(w):
    k, n = w.shape
    kb = (k + BLOCK - 1) // BLOCK
    packed = np.zeros((n, kb, BLOCK // 2), dtype=np.uint8)
    scales = np.zeros(n * kb, dtype=np.float32)
    zp = np.zeros(n * ((kb + 1) // 2), dtype=np.uint8)
    quantize_matmul_4bits(packed, w, scales, zp, BLOCK, n, k, True)
    return packed, scales


def main():
    tuned, ref, out = Path(sys.argv[1]), Path(sys.argv[2]), Path(sys.argv[3])
    base_w = load_file(glob.glob(snapshot_download(BASE) + "/model.safetensors")[0])
    tuned_w = load_file(str(tuned / "model.safetensors"))
    fp32 = {i.name: i for i in onnx.load(str(ref / "model.onnx")).graph.initializer}
    model = onnx.load(str(ref / "model_q4.onnx"))
    qinit = {i.name: i for i in model.graph.initializer}
    changed = 0
    for name, w in tuned_w.items():
        if name not in base_w or np.array_equal(w, base_w[name]):
            continue
        target = onnx_name(name)
        if target is None or target not in fp32:
            sys.exit(f"changed weight {name} has no counterpart in the reference graph; cannot patch")
        patched = (numpy_helper.to_array(fp32[target]) + (w - base_w[name]).T).astype(np.float32)
        packed, scales = q4(np.ascontiguousarray(patched))
        for suffix, arr in (("_Q4", packed.reshape(numpy_helper.to_array(qinit[target + "_Q4"]).shape)),
                            ("_scales", scales)):
            qinit[target + suffix].CopyFrom(numpy_helper.from_array(arr, target + suffix))
        changed += 1
    out.mkdir(parents=True, exist_ok=True)
    onnx.save(model, str(out / "model_q4.onnx"), save_as_external_data=True, all_tensors_to_one_file=True,
              location="model_q4.onnx_data", size_threshold=1024)
    for f in ("tokenizer.json", "config.json", "special_tokens_map.json", "tokenizer_config.json"):
        shutil.copy(ref / f, out / f)
    name = sys.argv[4] if len(sys.argv) > 4 else out.name.lower()
    manifest = {
        "format": 1, "name": name, "description": f"EmbeddingGemma 300M Q4 fine-tuned ({tuned.name})",
        "base": "embeddinggemma-300m-q4", "license": "Gemma Terms of Use",
        "license_url": "https://ai.google.dev/gemma/terms", "onnx": "model_q4.onnx",
        "external_data": ["model_q4.onnx_data"], "pooling": "output", "prompting": "gemma",
        "dim": 768, "max_length": 2048, "provenance": {"tuned_from": str(tuned), "patched_matrices": changed},
    }
    (out / "okbase-model.json").write_text(json.dumps(manifest, indent=2) + "\n")
    print(f"patched {changed} matrices -> {out / 'model_q4.onnx'}")


if __name__ == "__main__":
    main()
