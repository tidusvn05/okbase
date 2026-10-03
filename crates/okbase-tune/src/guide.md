# Fine-tuning okbase embeddings: guide for agents

Goal: make semantic search (`kb_search`) better on this bundle, mostly for questions asked in
another language than the documents. You write questions; okbase checks them, trains, measures
and only switches models when the tuned one is measurably better. The bundle is never modified.

## Before you start (ask the user)
1. `okbase advise` lists fine-tuning as a step. If it does not, stop: fine-tuning will not help.
2. Writing questions sends passages of the bundle to your model provider. Ask the user whether
   that is allowed for this bundle.
3. Training downloads a Python environment (~1–3 GB) and takes ~25 min on a CPU for ~500
   pairs (a few minutes on an NVIDIA GPU). Ask before `train`. Once the user agrees,
   `okbase embed tune setup --yes` can prepare it while you write questions.

## 1. Write the questions
```
okbase embed tune init --langs vi,ja,en   # languages people ask in; default: the bundle's + en
okbase embed tune next                    # prints a batch (passages + instructions) and claims it
okbase embed tune submit <n> answers.jsonl  # or pipe the JSONL into: okbase embed tune submit <n> -
```
Repeat `next` → write JSONL → `submit` until `next` says nothing is left. A rejected batch lists
every problem by line; fix them and submit the whole batch again. Batches are independent:
parallel subagents can each run `next` and work on their own batch (claims last 30 minutes).

Writing good questions (the okbase question standard v1):
- One object per line: `{"passage": "<key>", "kind": "natural|keyword|cross|vague", "lang": "vi", "q": "..."}`.
- Passage `[4]`: one of each kind. Passage `[2]`: one `cross` plus one other kind.
- `natural`: a real question in the passage's language. `keyword`: what an agent would type
  into a search tool, 2–8 words. `cross`: in another listed language. `vague`: paraphrased,
  incomplete or with typos.
- Answerable from that passage alone; do not copy the title or 5+ consecutive words; no file
  names or ids. Vary wording between passages; do not reuse templates.

## 2. Check, train, export
```
okbase embed tune check    # writes train.jsonl / heldout.jsonl; needs ≥ 300 training pairs
okbase embed tune train    # LoRA on EmbeddingGemma; --backend colab exports a notebook instead
okbase embed tune export   # ONNX Q4, installed as custom:<bundle>-<run>
```

## 3. Measure, then switch
```
okbase embed tune eval              # held-out questions: base model vs tuned model
okbase embed tune activate --write  # only if the gate passes; re-embeds the bundle
okbase embed tune rollback --write  # back to the previous model
```
Report the eval numbers to the user. Never pass `--force` to bypass the gate without the user's
explicit consent.

## When to tune again
Not after ordinary edits: the tuned model embeds new and changed documents like any model.
Start a new run when a new domain, vocabulary or language becomes a large part of the bundle,
or when a later `okbase embed tune eval` shows a drop. A new run always trains from the base
model and re-embeds the whole bundle once.
