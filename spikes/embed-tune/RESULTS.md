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

## Phase 0: ONNX Q4 through okfkit (2026-10-02)
`export_onnx.py` patches the onnx-community reference export instead of re-exporting Gemma3:
it adds each LoRA weight delta (tuned − base, 96 attention matrices) to the reference fp32 weights
and re-quantizes only those matrices with the same scheme (MatMulNBits, 4 bits, block 32,
symmetric; re-quantizing the base weights reproduces the reference bytes at 99.98%, scales
exactly). Everything else (quantized embedding table, dense head) stays byte-identical. Fidelity:
tuned Q4 vs tuned PyTorch cosine 0.975, same as reference Q4 vs base PyTorch (0.976).

Measured with okfkit's `retrieval_eval` (the real search path), loaded as `custom:tuned-ml`:

| Model (Q4, okfkit) | S1 R@1 | cross-lang | same-lang | vi / en / ja | S4 top-6 | S4 first |
|---|---|---|---|---|---|---|
| EmbeddingGemma Q4 (v0.3) | 0.857 | 0.815 | 0.94 | 0.74 / 0.91 / 0.92 | 29/30 | 23 |
| tuned-ml Q4 | **0.917** | **0.920** | 0.91 | 0.83 / 0.96 / 0.96 | **30/30** | 24 |

Q4 keeps two thirds of the fp32 gain (+6.0 of +9.0 points; my threshold of 0.92 was missed by
one question) and S4 does not regress. Same-language R@1 drops 3 points: the gate in `okfkit embed
tune eval` reports it, so a user sees the trade. Verdict: go.

## Not verified yet
- A larger held-out eval per bundle (questions written by people, not the same generator).

## Pipeline and cost per bundle (measured here unless marked)

| Step | What | Time / cost |
|---|---|---|
| 1. Generate pairs | LLM writes vi/ja/en questions per doc or chunk | $3.41 for 1,080 pairs (~25 sonnet calls); wall time not recorded |
| 2. Train LoRA | 2 epochs, ~500 pairs | CPU: 23–27 min, 6–6.5 GB RAM. T4 + Unsloth: a few minutes (not measured) |
| 3. Merge | `merge.py` | ~10 s, ~2 GB RAM |
| 4. ONNX export + Q4 | what okfkit loads | not done yet (estimate: minutes) |
| 5. Re-embed the whole bundle | every vector changes with the model | S4 L (3,230 chunks): ~20 min with okfkit Q4, ~40 min fp32 PyTorch |
| 6. Eval | held-out questions, base vs tuned | ~40 min per model here (dominated by S4 embedding) |

End to end on CPU: ~1–1.5 h and $3–5 for a bundle of a few hundred docs.

## When documents change
- **Edits and new docs normally need no retraining.** The tuned model is still a general embedding
  model; okfkit's cache re-embeds only changed chunks. Evidence: tuned-oc never saw the S1 docs and
  still beat the base model on them (0.853 → 0.890).
- **Retrain when** a new domain, vocabulary or language becomes a large share of the bundle, a large
  part of the bundle is new (rule of thumb 20–30%, unmeasured), or a periodic eval drops.
- **A retrain** = generate pairs only for new/changed docs (keep the old pairs) → train again from the
  *base* model on all pairs (never stack on a tuned model) → export + eval → **re-embed the whole
  bundle**. The full re-embed, not the training, is the main recurring cost.

## Trade-offs
1. Real queries come from AI agents (short, keyword-like), not the human-style questions used for
   training and eval; real gains may be smaller.
2. Train and eval questions come from the same LLM and may share its style; a ~50-question set written
   by people is needed for an honest number.
3. Step 1 sends document text to an LLM API; private bundles need a local LLM (weaker questions).
4. Gains concentrate where the base model is weak (cross-language). On English long docs (S4) the
   base model is already 27–29/30, and the lexical path (`kb_grep`, 90–100% in okf-scale) gains nothing.
5. Q4 quantization may erase part of the gain (unverified; the main open risk).
6. A tuned model is a Gemma derivative under the Gemma terms (fine for internal use). bge-m3 (MIT)
   could be tuned the same way (not tried).
7. Operations: model versioning and rollback (1.2 GB fp32 / ~200 MB Q4). The vector cache must key on a
   hash of the model file rather than the model name once custom models are allowed. Training needs
   Python/torch (ideally a GPU) outside okfkit.

## Next steps (proposed, not started)
1. Export tuned-ml to ONNX, quantize to Q4, rerun okfkit's `retrieval_eval`: does the gain survive?
   Stop here if it does not.
2. If it does: opt-in custom ONNX model in `okfkit.toml` (path, pooling, prompts, max length), vector
   cache keyed by model hash.
3. Document (or script) steps 1–6; measure Unsloth on a real GPU.

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

## Acceptance: the whole workflow with real agents (2026-10-02)

`okfkit-full` release build on `fixtures/multilingual` (a copy; state and models in a scratch dir).
Three Claude subagents in parallel, told only to use the CLI and `okfkit embed tune guide`:

| Step | Result |
|---|---|
| `tune init --langs vi,ja,en` | 100 passages, 20 documents held out, 10 batches |
| Questions (3 agents, `next`/`submit`) | all 10 batches in ~2.5 min; 13 submissions (7 accepted first time); 55–64k tokens per agent. Rejections: Japanese keyword over 25 characters (not stated in the prompt), Vietnamese 5-syllable "copies" and 9-syllable keywords (syllables counted as words), title in a keyword, Vietnamese without diacritics |
| `tune check` | 320 training pairs, 80 held-out questions; 100 of each kind |
| `tune setup --yes` | venv + pip (no uv on this machine): 87 s, 1.7 GB |
| `tune train` | 320 pairs × 2 epochs on CPU: 627 s |
| `tune export` | 20 s, installed `custom:kb-<run>` |
| `tune eval` (70 s) | held-out R@1 0.875 → **0.925**; cross-language 0.810 → 0.810; same-language 0.898 → 0.966; general set 0.857 → 0.913*. Gate passed |
| `activate --write` / search / `rollback --write` | okfkit.toml switched, bundle re-embedded, search used `custom:kb-<run>@<hash>`, rollback restored the file exactly (removed it: there was none) |

\* Here the general set is this very bundle, so that row is not independent.

Changes made from this run: Vietnamese is counted in syllables (keyword 2–12, copying from 8),
keyword queries may name the title, vague Vietnamese may drop diacritics, and the prompt states
the Japanese limits. Held-out cross-language did not improve on 80 questions (35 cross); the
gain came from same-language questions, unlike S11, whose training set was larger and
cross-language heavy.

## Dependency update: peft 0.21.1 → 0.21.2 (2026-10-03)

The same run as the acceptance above (same 320 training pairs and 80 held-out questions), with
only peft changed in the training environment. 0.21.2 only removes an encoder-decoder generation
hook, which embedding training does not use.

| | peft 0.21.1 | peft 0.21.2 |
|---|---|---|
| Loss at epoch 0.5 / 1 / 1.5 / 2 | 0.1988 / 0.1668 / 0.0817 / 0.0470 | 0.1983 / 0.1663 / 0.0790 / 0.0465 |
| Held-out R@1 (base → tuned) | 0.875 → 0.925 | 0.875 → 0.925 |
| Cross-language / same-language R@1 (tuned) | 0.810 / 0.966 | 0.810 / 0.966 |
| R@3 / MRR (tuned) | 1.0 / 0.9625 | 1.0 / 0.9625 |
| General set R@1 (tuned) | 0.913 | 0.917 |
| Gate | passed | passed |
| Train time | 627 s | 3,599 s* |

\* The machine was shared with another project's browser test suite (load 12–14 on 8 vCPU); the
CPU time per step is the same order, so this is contention, not peft.

Verdict: no change in quality; `python/requirements.txt` pins peft 0.21.2.
