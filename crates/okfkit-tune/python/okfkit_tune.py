#!/usr/bin/env python3
"""okfkit embed tune: LoRA fine-tuning of EmbeddingGemma-300M and export to okfkit's ONNX Q4 format.

Embedded in the okfkit binary and run by `okfkit embed tune train|export` inside a private
virtual environment. Method and defaults come from spike S11 (spikes/embed-tune/RESULTS.md).

  train  <train.jsonl> <out dir> [--epochs N] [--seed N]
         LoRA (r=16, alpha=32, q/k/v/o) with in-batch negatives; writes <out dir>/adapter.
         Uses Unsloth on a CUDA GPU when installed, else sentence-transformers + peft
         (on CPU with gradient caching so memory stays ~6.5 GB).
  export <adapter dir> <reference dir> <out dir> --name NAME [--provenance JSON]
         Merges the adapter, writes the changed matrices into the reference ONNX Q4 graph
         (MatMulNBits, 4 bits, block 32, symmetric) and an okfkit-model.json manifest.

Inputs are formatted exactly as okfkit embeds them: queries as
"task: search result | query: <q>", documents as "title: <title> | text: <text>".
"""
import argparse, json, re, shutil, sys, time
from pathlib import Path

BASE = "unsloth/embeddinggemma-300m"  # ungated mirror of google/embeddinggemma-300m (Gemma Terms of Use)
QUERY = "task: search result | query: "
TARGETS = ["q_proj", "k_proj", "v_proj", "o_proj"]
BLOCK = 32


def log(msg):
    print(msg, flush=True)


def train(args):
    import torch
    from datasets import Dataset

    rows = [json.loads(l) for l in Path(args.data).open(encoding="utf-8") if l.strip()]
    if not rows:
        sys.exit(f"{args.data}: no training pairs")
    ds = Dataset.from_dict({
        "anchor": [QUERY + r["query"] for r in rows],
        "positive": [f"title: {r['title']} | text: {r['text']}" for r in rows],
    })
    gpu = torch.cuda.is_available()
    log(f"pairs: {len(rows)}  device: {'cuda' if gpu else 'cpu'}  epochs: {args.epochs}")
    model, backend = None, "peft"
    if gpu:
        try:
            from unsloth import FastSentenceTransformer  # noqa: import before transformers

            model = FastSentenceTransformer.from_pretrained(BASE, max_seq_length=256, full_finetuning=False)
            model = FastSentenceTransformer.get_peft_model(
                model, r=16, lora_alpha=32, lora_dropout=0.0, target_modules=TARGETS, bias="none",
                use_gradient_checkpointing=False, random_state=args.seed, task_type="FEATURE_EXTRACTION")
            backend = "unsloth"
        except ImportError:
            model = None
    from peft import LoraConfig, TaskType
    from sentence_transformers import (SentenceTransformer, SentenceTransformerTrainer,
                                       SentenceTransformerTrainingArguments, losses)
    from sentence_transformers.training_args import BatchSamplers

    if model is None:
        model = SentenceTransformer(BASE, device="cuda" if gpu else "cpu")
        model.max_seq_length = 256
        model.add_adapter(LoraConfig(task_type=TaskType.FEATURE_EXTRACTION, r=16, lora_alpha=32,
                                     lora_dropout=0.05, target_modules=TARGETS))
    # Same objective as MultipleNegativesRankingLoss (32 in-batch negatives); on CPU, gradient
    # caching keeps activations for 8 pairs at a time (long chunks were OOM-killed at batch 32).
    loss = (losses.MultipleNegativesRankingLoss(model) if gpu
            else losses.CachedMultipleNegativesRankingLoss(model, mini_batch_size=8))
    out = Path(args.out)
    targs = SentenceTransformerTrainingArguments(
        output_dir=str(out / "checkpoints"), num_train_epochs=args.epochs, per_device_train_batch_size=32,
        learning_rate=2e-4, warmup_ratio=0.1, batch_sampler=BatchSamplers.NO_DUPLICATES, logging_steps=5,
        save_strategy="no", report_to=[], seed=args.seed, use_cpu=not gpu)
    t = time.time()
    SentenceTransformerTrainer(model=model, args=targs, train_dataset=ds, loss=loss).train()
    adapter = out / "adapter"
    model[0].auto_model.save_pretrained(str(adapter))
    (out / "train-info.json").write_text(json.dumps({
        "base": BASE, "backend": backend, "device": "cuda" if gpu else "cpu", "pairs": len(rows),
        "epochs": args.epochs, "seconds": round(time.time() - t), "lora": {"r": 16, "alpha": 32, "targets": TARGETS},
    }, indent=2) + "\n")
    log(f"trained in {time.time() - t:.0f}s; adapter: {adapter}")


def onnx_name(hf):
    m = re.fullmatch(r"(?:model\.)?layers\.(\d+)\.(self_attn|mlp)\.(\w+_proj)\.weight", hf)
    if not m:
        return None
    return f"model.layers.{m[1]}.{'attn' if m[2] == 'self_attn' else 'mlp'}.{m[3]}.MatMul.weight"


def q4(w):
    import numpy as np
    from onnxruntime.capi._pybind_state import quantize_matmul_4bits

    k, n = w.shape
    kb = (k + BLOCK - 1) // BLOCK
    packed = np.zeros((n, kb, BLOCK // 2), dtype=np.uint8)
    scales = np.zeros(n * kb, dtype=np.float32)
    zp = np.zeros(n * ((kb + 1) // 2), dtype=np.uint8)
    quantize_matmul_4bits(packed, w, scales, zp, BLOCK, n, k, True)
    return packed, scales


def export(args):
    import numpy as np
    import onnx
    from onnx import numpy_helper
    from peft import PeftModel
    from sentence_transformers import SentenceTransformer

    ref, out = Path(args.reference), Path(args.out)
    st = SentenceTransformer(BASE, device="cpu")
    base = {k: v.detach().clone() for k, v in st[0].auto_model.state_dict().items()}
    merged = PeftModel.from_pretrained(st[0].auto_model, args.adapter).merge_and_unload()
    model = onnx.load(str(ref / "onnx" / "model_q4.onnx"))
    init = {i.name: i for i in model.graph.initializer}
    changed = 0
    for name, w in merged.state_dict().items():
        if name not in base or bool((w == base[name]).all()):
            continue
        target = onnx_name(name)
        if target is None or target + "_Q4" not in init:
            sys.exit(f"changed weight {name} has no quantized counterpart in the reference graph")
        packed, scales = q4(np.ascontiguousarray(w.detach().float().numpy().T))
        shape = numpy_helper.to_array(init[target + "_Q4"]).shape
        init[target + "_Q4"].CopyFrom(numpy_helper.from_array(packed.reshape(shape), target + "_Q4"))
        init[target + "_scales"].CopyFrom(numpy_helper.from_array(scales, target + "_scales"))
        changed += 1
    if changed == 0:
        sys.exit("the adapter changes no weights")
    out.mkdir(parents=True, exist_ok=True)
    onnx.save(model, str(out / "model_q4.onnx"), save_as_external_data=True, all_tensors_to_one_file=True,
              location="model_q4.onnx_data", size_threshold=1024)
    for f in ("tokenizer.json", "config.json", "special_tokens_map.json", "tokenizer_config.json"):
        shutil.copy(ref / f, out / f)
    manifest = {
        "format": 1, "name": args.name, "description": args.description or f"EmbeddingGemma 300M Q4 tuned ({args.name})",
        "base": "embeddinggemma-300m-q4", "license": "Gemma Terms of Use",
        "license_url": "https://ai.google.dev/gemma/terms", "onnx": "model_q4.onnx",
        "external_data": ["model_q4.onnx_data"], "pooling": "output", "prompting": "gemma",
        "dim": 768, "max_length": 2048,
        "provenance": {**json.loads(args.provenance or "{}"), "patched_matrices": changed},
    }
    (out / "okfkit-model.json").write_text(json.dumps(manifest, indent=2) + "\n")
    log(f"exported {changed} matrices to {out}")


def main():
    p = argparse.ArgumentParser(prog="okfkit_tune")
    sub = p.add_subparsers(dest="cmd", required=True)
    t = sub.add_parser("train")
    t.add_argument("data")
    t.add_argument("out")
    t.add_argument("--epochs", type=int, default=2)
    t.add_argument("--seed", type=int, default=7)
    e = sub.add_parser("export")
    e.add_argument("adapter")
    e.add_argument("reference")
    e.add_argument("out")
    e.add_argument("--name", required=True)
    e.add_argument("--description", default="")
    e.add_argument("--provenance", default="")
    sub.add_parser("check")  # imports only: verifies the environment
    args = p.parse_args()
    if args.cmd == "train":
        train(args)
    elif args.cmd == "export":
        export(args)
    else:
        import numpy, onnx, onnxruntime, peft, sentence_transformers, torch  # noqa: F401

        log(f"ok: torch {torch.__version__}, cuda {torch.cuda.is_available()}")


if __name__ == "__main__":
    main()
