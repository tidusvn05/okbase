#!/usr/bin/env python3
"""Merge a saved LoRA adapter into the base model in a fresh process (low memory).
Usage: .venv/bin/python merge.py <ml|oc|all>"""
import resource, sys
from pathlib import Path

from peft import PeftModel
from sentence_transformers import SentenceTransformer

HERE = Path(__file__).resolve().parent
BASE = "unsloth/embeddinggemma-300m"


def main():
    which = sys.argv[1]
    model = SentenceTransformer(BASE, device="cpu")
    peft = PeftModel.from_pretrained(model[0].auto_model, str(HERE / "models" / f"lora-{which}"))
    model[0].auto_model = peft.merge_and_unload()
    out = HERE / "models" / f"tuned-{which}"
    model.save(str(out))
    peak = resource.getrusage(resource.RUSAGE_SELF).ru_maxrss / 1024
    print(f"saved {out} (peak RSS {peak:.0f} MB)")


if __name__ == "__main__":
    main()
