#!/usr/bin/env python3
"""Spike S11: LoRA fine-tuning of EmbeddingGemma-300M on synthetic pairs (CPU; same method Unsloth
accelerates on GPU). Usage: .venv/bin/python train.py <ml|oc|all> [epochs]"""
import json, sys, time
from pathlib import Path

from datasets import Dataset
from peft import LoraConfig, TaskType
from sentence_transformers import SentenceTransformer, SentenceTransformerTrainer, SentenceTransformerTrainingArguments, losses
from sentence_transformers.training_args import BatchSamplers

HERE = Path(__file__).resolve().parent
BASE = "unsloth/embeddinggemma-300m"  # ungated mirror of google/embeddinggemma-300m (Gemma terms)
Q = "task: search result | query: "


def doc(title, text):
    return f"title: {title or 'none'} | text: {text}"


def main():
    which = sys.argv[1]
    epochs = int(sys.argv[2]) if len(sys.argv) > 2 else 2
    pairs = [json.loads(l) for l in (HERE / "work/pairs.jsonl").open()]
    pairs = [p for p in pairs if which == "all" or p["set"] == which]
    ds = Dataset.from_dict({"anchor": [Q + p["query"] for p in pairs], "positive": [doc(p["title"], p["text"]) for p in pairs]})
    model = SentenceTransformer(BASE, device="cpu")
    model.max_seq_length = 256
    model.add_adapter(LoraConfig(task_type=TaskType.FEATURE_EXTRACTION, r=16, lora_alpha=32, lora_dropout=0.05,
                                 target_modules=["q_proj", "k_proj", "v_proj", "o_proj"]))
    args = SentenceTransformerTrainingArguments(
        output_dir=str(HERE / "work" / f"ckpt-{which}"), num_train_epochs=epochs, per_device_train_batch_size=32,
        learning_rate=2e-4, warmup_ratio=0.1, batch_sampler=BatchSamplers.NO_DUPLICATES, logging_steps=5,
        save_strategy="no", report_to=[], seed=7, use_cpu=True)
    # Same objective as MultipleNegativesRankingLoss (32 in-batch negatives) with gradient caching, so
    # activations are held for 8 pairs at a time: long OpenClaw chunks were OOM-killed at batch 32.
    loss = losses.CachedMultipleNegativesRankingLoss(model, mini_batch_size=8)
    t = time.time()
    SentenceTransformerTrainer(model=model, args=args, train_dataset=ds, loss=loss).train()
    print(f"trained on {len(ds)} pairs x {epochs} epochs in {time.time() - t:.0f}s", flush=True)
    # Save only the adapter; merge.py merges it into the base weights in a fresh, smaller process.
    adapter = HERE / "models" / f"lora-{which}"
    model[0].auto_model.save_pretrained(str(adapter))
    print(f"saved adapter {adapter}", flush=True)


if __name__ == "__main__":
    main()
