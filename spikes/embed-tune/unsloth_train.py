#!/usr/bin/env python3
"""Spike S11 on a GPU with Unsloth (Colab T4 or any NVIDIA GPU with >= 3 GB VRAM).

Same data and loss as train.py (CPU, plain sentence-transformers + peft), following Unsloth's
EmbeddingGemma notebook (https://colab.research.google.com/github/unslothai/notebooks/blob/main/nb/EmbeddingGemma_(300M).ipynb).

Colab:
  1. Upload work/pairs.jsonl next to this script (or set PAIRS).
  2. !pip install unsloth
  3. !python unsloth_train.py all     # ml | oc | all
  4. Download models/unsloth-<set>/ and evaluate here:  .venv/bin/python evaluate.py models/unsloth-<set> unsloth-<set>
"""
import json, os, sys, time

from unsloth import FastSentenceTransformer, is_bf16_supported  # import unsloth before transformers
from datasets import Dataset
from sentence_transformers import SentenceTransformerTrainer, SentenceTransformerTrainingArguments, losses
from sentence_transformers.training_args import BatchSamplers

which = sys.argv[1] if len(sys.argv) > 1 else "all"
pairs = [json.loads(l) for l in open(os.environ.get("PAIRS", "work/pairs.jsonl"))]
pairs = [p for p in pairs if which == "all" or p["set"] == which]

model = FastSentenceTransformer.from_pretrained(model_name="unsloth/embeddinggemma-300m", max_seq_length=1024, full_finetuning=False)
model = FastSentenceTransformer.get_peft_model(
    model, r=32, lora_alpha=64, lora_dropout=0, bias="none",
    target_modules=["q_proj", "k_proj", "v_proj", "o_proj", "gate_proj", "up_proj", "down_proj"],
    use_gradient_checkpointing="unsloth", random_state=3407, task_type="FEATURE_EXTRACTION")

title = lambda p: p["title"] or "none"
ds = Dataset.from_dict({"query": [p["query"] for p in pairs], "passage": [f"title: {title(p)} | text: {p['text']}" for p in pairs]})
args = SentenceTransformerTrainingArguments(
    output_dir="output", num_train_epochs=2, per_device_train_batch_size=32, learning_rate=2e-4, warmup_ratio=0.1,
    logging_steps=5, report_to="none", bf16=is_bf16_supported(), lr_scheduler_type="linear",
    batch_sampler=BatchSamplers.NO_DUPLICATES,
    # The query prompt; documents already carry the "title: … | text: …" format.
    prompts={"query": model.prompts["query"]},
)
t = time.time()
SentenceTransformerTrainer(model=model, args=args, train_dataset=ds, loss=losses.MultipleNegativesRankingLoss(model)).train()
print(f"trained on {len(ds)} pairs in {time.time() - t:.0f}s")
model.save_pretrained_merged(f"models/unsloth-{which}", tokenizer=model.tokenizer, save_method="merged_16bit")
