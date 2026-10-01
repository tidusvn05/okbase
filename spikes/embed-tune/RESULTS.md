# S11 — fine-tuning EmbeddingGemma-300M (LoRA, Unsloth method)

Question: does a small LoRA fine-tune on synthetic (question, passage) pairs from a bundle beat the
stock model, and does it hurt other domains? Run on 2026-10-01, CPU only (8 vCPU, 23 GB RAM), with
plain sentence-transformers + peft: the same recipe Unsloth's EmbeddingGemma notebook runs on a GPU
(`unsloth_train.py` is the GPU version, not run here: no NVIDIA GPU).

## Setup
- **Data** (`gen_queries.py`, Claude sonnet, $3.41): 1,080 pairs in `results/pairs.jsonl.gz`.
  - `ml`: 100 docs of `fixtures/multilingual` × 5 questions (2 vi, 2 ja, 1 en; ≥ 2 cross-language) = 500.
  - `oc`: 300 random chunks of the okf-scale L bundle (OpenClaw docs) × 2 questions (1 en + 1 vi/ja) = 580.
  - The generator never saw the eval questions; near-copies of them were dropped (0 found).
- **Training** (`train.py`): base `unsloth/embeddinggemma-300m` (ungated mirror, Gemma terms), LoRA r=16
  α=32 on q/k/v/o, in-batch negatives (MNRL, batch 32, `NO_DUPLICATES`), lr 2e-4, 2 epochs,
  inputs cut at 256 tokens, model prompts (`task: search result | query:` / `title: … | text:`).
  `ml` used plain MNRL (peak ≈ 6 GB); `oc` (long chunks) was OOM-killed at batch 32 and ran with
  `CachedMultipleNegativesRankingLoss(mini_batch_size=8)` (same objective, ≈ 6.5 GB). The adapter is
  7.6 MB; `merge.py` merges it into a plain 1.2 GB model (exportable to ONNX).
- **Eval** (`evaluate.py`): same protocol as okfkit's `examples/retrieval_eval.rs`: S1 = 300 vi/en/ja
  questions over 100 docs (doc-level R@k); S4 = 30 questions over okf-scale L (287 docs, 3,230 chunks),
  gold in the top-6 chunks with ≤ 2 per doc. fp32 PyTorch for all three rows, so they compare like for like.

## Results

| Model | Trained on | S1 R@1 | S1 R@3 | S1 MRR | S1 cross-lang R@1 | S4 top-6 | S4 first |
|---|---|---|---|---|---|---|---|
| base fp32 | — | 0.853 | 0.960 | 0.908 | 0.815 | 27/30 | 22 |
| tuned-ml | S1 docs (in-domain for S1) | **0.943** | **0.980** | **0.963** | **0.950** | 28/30 | 23 |
| tuned-oc | S4 bundle (in-domain for S4) | 0.890 | 0.963 | 0.929 | 0.870 | 28/30 | 23 |

Training time on CPU: 1,376 s (ml) and 1,615 s (oc) for 2 epochs; on a T4 with Unsloth this is ~1–2 min.
For reference, okfkit's own ONNX Q4 run (v0.3) scored S1 0.857 and S4 29/30 (`../acceptance-v0.3/RESULTS.md`).

## Findings
1. **In-domain gains are large.** Tuning on synthetic questions about the same docs lifts S1 R@1 by
   +9 points and cross-language R@1 by +13.5 (0.815 → 0.95), the model's weakest spot. The eval
   questions were not in the training data, but the documents were. This is the "tune on your own
   bundle" scenario, not proof of general improvement.
2. **No forgetting; some transfer.** Neither tuned model got worse on the other domain. Tuning on
   English-heavy OpenClaw chunks with vi/ja questions still lifted S1 cross-language R@1 by +5.5
   (0.815 → 0.87): much of the gain is learning vi/ja → passage matching, not the docs themselves.
3. **S4 is saturated.** 27 → 28/30 is a one-question difference on 30 questions, so it's within
   noise. A long-docs bundle needs a harder eval to show a gain.
4. **Cost is small but not zero.** About $3 of LLM calls per ~1k pairs, plus ~25 min CPU or ~2 min
   GPU per bundle. Training requires Python + torch (+ Unsloth for a GPU), which okfkit's Rust core
   cannot and must not carry (hard rule 1).

## Not verified yet
- ONNX export + Q4 quantization of a tuned model (what okfkit actually loads): does the gain survive?
- A larger held-out eval per bundle (questions written by people, not the same generator).

## Recommendation
Do not add training to okfkit. If anything, add an opt-in way to load a **user-supplied ONNX
embedding model** (path + pooling + prompts in `okfkit.toml`), and document a recipe (this folder:
`gen_queries.py` → `unsloth_train.py` on Colab → ONNX export) for teams whose bundle is mostly
non-English or cross-language. A tuned model is a Gemma derivative: users may build one under the
Gemma terms, but okfkit must not ship tuned weights.

## Reproduce
```
python3 -m venv .venv && .venv/bin/pip install torch --index-url https://download.pytorch.org/whl/cpu
.venv/bin/pip install sentence-transformers peft datasets
python3 gen_queries.py            # needs the S4 index from ../acceptance-v0.3 and Claude Code quota
.venv/bin/python evaluate.py unsloth/embeddinggemma-300m base-fp32
.venv/bin/python train.py ml 2 && .venv/bin/python merge.py ml && .venv/bin/python evaluate.py models/tuned-ml tuned-ml
```
`models/`, `work/` and `.venv/` are git-ignored.
