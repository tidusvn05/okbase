# Plan: `okbase advise` and embedding fine-tuning (`okbase embed tune`)

Status: **phases 0–5 implemented** (2026-10-02; see §7 for deviations from the plan and acceptance results). Supplements [`../design.md`](../design.md) (§1 S11, §13, §14).
Evidence: `spikes/embed-tune/RESULTS.md`.

## 0. Goals

1. **`okbase advise`:** look at a project or bundle and recommend how to use okbase, from the simplest
   to the most complex, with a concrete command for each step.
2. **End-to-end embedding fine-tuning:**
   - The agent (Claude Code, Codex, …) writes questions following a **predefined standard**.
   - okbase validates the data, trains, exports, evaluates, downloads or activates the model, and rolls back to the previous one when needed.
   - The user only has to tell the agent "tune embeddings for this bundle".

**Constraints (AGENTS.md):**
- Core contains no model: `advise` belongs to core (lexical, no model); training lives in the opt-in `embed-tune` module.
- Read-only by default: every generated file lives in the state dir; `okbase.toml` is only modified with a write flag.
- No telemetry: okbase **never calls an LLM itself**; writing questions is done by the user's agent.

---

## 1. `okbase advise`: per-project usage recommendations

### 1.1 Measurable signals (no model needed)

| Signal | Source | Used for |
|---|---|---|
| Document count, estimated tokens, chunks/document | `stats` | Choosing between full-context, lexical and embed |
| Language mix (vi / ja / en / other) | analyzer (script counts, Vietnamese diacritics) | A multilingual bundle needs embeddings (S1: BM25 with cross-language queries reaches only 3.5%) |
| Conformance level L0–L3, missing descriptions, duplicates | `lint` | Recommend adopt / curate before anything else |
| CSV/TSV/XLSX, size | file scan | Enable the `data` module (S5) |
| `index.md`, `_meta/vocabulary.md`, type schema | file scan | Assess how well the bundle is organized |
| User-declared options | flags `--agent claude\|codex\|host`, `--users-lang vi,ja`, `--private`, `--gpu` | Deployment style; whether documents may be sent to an LLM |

### 1.2 Recommendation levels (taken in order; move up only when there is a reason)

| Level | When (per spikes) | What okbase does | Main commands |
|---|---|---|---|
| **0. Full context** | ≤ ~30k tokens (S4) | Put the whole bundle or the catalog into the system prompt; no index needed | `okbase catalog` |
| **1. Lexical (default)** | Every bundle larger than level 0 | MCP stdio + skill; `kb_grep` / `kb_query` / `kb_get` | `okbase agent install`, `okbase mcp serve` |
| **1+. Reorganize** | Conformance level < L2, missing descriptions, duplicated content | Adopt, lint, vocab before adding machinery (the central lesson of design §1) | `okbase adopt --plan`, `okbase lint --level L2` |
| **2. Data** | CSV/XLSX present, especially large sheets | Enable `data_query` (S5: without SQL the agent gives up) | enabled automatically; `okbase data tables` |
| **3. Embed** | Users ask in a different language than the documents, large bundle, or the host needs pre-retrieval | Choose a model by license, language, hardware (Gemma Q4 / bge-m3 / API) | `okbase embed enable`, `okbase embed index` |
| **4. Fine-tune** | Meets level 3, **and** `embed eval` shows weakness (especially cross-language), ≥ ~100 documents, documents may be sent to the agent's LLM | The workflow in §2 | `okbase embed tune …` |

**Deployment axis** (independent of the levels above):
- A single CLI-agent user: stdio + skill.
- A shared team: `mcp serve --http` + token.
- Host application: the `Bundle` / `router()` library.

### 1.3 Output

Human-readable text plus `--json` (no MCP tool; see §6):

```
$ okbase advise --users-lang vi,ja
Bundle: 287 docs, ~1.1M tokens, en 92% / vi 5% / ja 3%, level L1 (41 docs without description)

Recommended path
  1. [now]   Lexical + skills      okbase agent install --agent claude
  2. [now]   Fix descriptions       okbase lint --level L2   (41 docs; see skill okbase-curate)
  3. [next]  Embeddings             users ask in vi/ja, docs are en → cross-language needs embeddings
                                    okbase embed enable --model embeddinggemma-300m-q4
  4. [maybe] Fine-tune              ask your agent: "tune embeddings for this bundle"
Not needed: data (no tables), full-context (too large)
```

- **`advise` writes nothing** and only prints commands (no `--apply`; see §6).
- Thresholds are constants that cite their spike (like `FULL_MODE_MAX_TOKENS`) and have snapshot tests.
- Extend the existing `recommend_mode()` instead of writing a parallel one.

---

## 2. End-to-end fine-tuning: `okbase embed tune`

### 2.1 Flow

```
 advise / embed eval ──► tune init ──► [agent] tune next / tune submit (loop) ──► tune check
                                                                                     │
     rollback ◄── tune activate ◄── tune eval ◄── tune export ◄── tune train ◄───────┘
```

| Step | Command | Who | Notes |
|---|---|---|---|
| 1 | `okbase embed tune init [--langs vi,ja,en] [--budget N]` | okbase | Select passages per the standard in §3, split into batches, set aside a held-out set **by document**. Writes to `<state>/tune/<run>/`. Prints estimates of question count, tokens the agent must read, and training time |
| 2 | `okbase embed tune next` | agent | Prints the next batch: passages + **short prompt** (standard §3) + the JSONL format to return |
| 3 | `okbase embed tune submit <batch> [file\|-]` | agent → okbase | Validates immediately (§3.4); errors are printed per line so the agent can fix and resubmit. Repeat 2–3 until no batches remain |
| 4 | `okbase embed tune check` | okbase | Report against the standard: totals, language and question-kind distribution, duplicates. Training is refused until it passes |
| 5 | `okbase embed tune train [--backend local\|colab]` | okbase (Python) | §2.2 |
| 6 | `okbase embed tune export` | okbase (Python) | Merge adapter → ONNX → Q4; write the manifest in §2.3 |
| 7 | `okbase embed tune eval` | okbase (Rust, the same path used in production) | On held-out: lexical vs base model vs tuned model (R@1/R@3/MRR, cross-language) + shared regression set. Has a **gate**, §2.4 |
| 8 | `okbase embed tune activate --write` | okbase | Writes `[embed] model = "custom:<name>"`; re-embeds the bundle in the background (cache keyed by model hash) |
| — | `okbase embed tune rollback --write`, `tune status`, `tune runs` | okbase | Return to the previous model; view progress and history |

Steps 2–3 work with **any agent that has a shell**:
- The agent does not need to read long instructions: the CLI hands out small tasks and validates results immediately.
- Agents with subagents (Claude Code) can run several batches in parallel. `next --claim` locks a batch to avoid duplication.

### 2.2 Training backends

| Backend | When | How |
|---|---|---|
| **local** (default) | Python or `uv` available | okbase creates a venv in `<user cache>/okbase/tune-env` with `uv` from a **lockfile embedded in the binary** (torch, sentence-transformers, peft, optimum, onnxruntime). With CUDA it uses Unsloth (QLoRA, fast); without, CPU: LoRA + `CachedMultipleNegativesRankingLoss(mini_batch_size=8)`, ~25 minutes / 500 pairs, ~6.5 GB RAM (S11) |
| **colab** | No powerful machine | Exports `tune-<run>.zip` (data + a pre-generated Unsloth notebook); the user runs it on a Colab T4, then `okbase embed tune import <file>` |
| (later) **native** | When candle or burn is stable enough for LoRA on Gemma3 | Removes the Python dependency. Not part of this plan |

- The training and export scripts and the notebook are **embedded in the binary** and versioned with okbase.
- Python is invoked only when the user runs `tune train` or `tune export`. The first time, okbase asks before downloading about 1–2 GB (or use the `--yes` flag).

### 2.3 Custom models, model downloads, registry

- `okbase embed models` lists built-in and custom models, with license, size and eval scores.
- Commands to add and remove models:
  - `okbase embed models add <dir|hf-repo>`: load any ONNX model that has a manifest (including third-party models, not only tuned ones).
  - `okbase embed models pull <id>`: download ahead of time for offline use.
  - `okbase embed models remove <id>`.
- Self-tuned models are stored in `<user cache>/okbase/models/custom/<name>/` as `model.onnx` (Q4), the tokenizer and `okbase-model.toml`. The manifest records:
  - the base model and the inherited license (Gemma: reuses the existing license-acceptance step);
  - pooling, query/document prompts, dimensions, `max_length`;
  - the model's sha256, the training-data hash, the question-standard version;
  - eval scores at creation time.
- **The vector cache is keyed by the model's sha256** (currently keyed by name); this is a hard prerequisite before allowing custom models.
- okbase **does not distribute tuned models**. Teams that want to share one copy the model directory themselves, subject to the Gemma terms.

### 2.4 Quality gate (`tune eval`)

`activate` refuses, unless `--force` is given, if the model does not meet **both** conditions:
1. On held-out, the tuned model's R@1 ≥ the base model's + 2 points, and cross-language R@1 does not drop.
2. On the shared regression set, no drop of more than 1 point. This set is small and embedded: part of the multilingual fixture (S1).

If `_meta/eval/questions.jsonl` exists (human-written questions; ≥ 50 recommended), the gate uses it instead of the synthetic held-out set. The reason is tradeoff 2 of S11: training and eval questions are generated by the same LLM.

`okbase embed eval [--quick]` also works **without tuning**: it compares lexical vs Gemma Q4 vs bge-m3 on the user's own bundle, as the basis for levels 3 and 4 of `advise`.

---

## 3. Question standard (okbase question standard v1)

Purpose: the user does not have to decide anything. `tune init` applies these defaults; every value can be changed in `[embed.tune]`.

### 3.1 Passage selection

- Short documents (≤ 512 estimated tokens) are used **whole**. Long documents use **index chunks** (the same unit search returns).
- Large bundles: sample ≤ 400 passages, **stratified** by directory, type and language. Drop very short chunks (< 40 tokens) and table-of-contents or index pages.
- Set aside **15% of documents** (minimum 20 documents) as held-out, split by document rather than by question, to avoid leakage.

### 3.2 Quantities

| | Default | Notes |
|---|---|---|
| Questions / short document | 4 | S11 used 5 |
| Questions / chunk | 2 | S11 used 2 |
| Total training pairs | **600–1,200** | S11: 500–580 pairs were enough for a large gain |
| Minimum to allow training | 300 | Below this there is no evidence |
| Reference cost | ~$3 / 1,000 pairs with a Sonnet-class model | `tune init` prints an estimate up front |

### 3.3 Question kinds (per passage)

With 4 questions, one of each kind. With 2 questions, rotate so that each passage has 1 cross-language question.

| `kind` | Description | Why |
|---|---|---|
| `natural` | Natural question in the document's language | Baseline |
| `keyword` | Agent-style query: 3–8 keywords, may mix in English terms | The real askers are **agents** (tradeoff 1 of S11) |
| `cross` | Asked in another language, taken from `--langs` | This is where the largest gain is (0.815 → 0.95) |
| `vague` | Vague, paraphrased, possibly misspelled | Robustness |

### 3.4 Validation rules (`tune submit` rejects violating lines)

- JSONL, one line each: `{"passage": "<id>[#<chunk>]", "kind": "natural|keyword|cross|vague", "lang": "vi", "q": "…"}`.
- Enough questions and every kind for each passage; `lang` is in `--langs` and matches the language detected in `q`.
- Length 3–30 words (for ja: 5–60 characters).
- Do not copy the title, no overlap of ≥ 5 consecutive words with the passage, no ids or file names.
- No duplicates or near-duplicates of other questions (compared after normalization), and none of the questions in the human-written eval set.
- The question must be answerable **from that passage alone**. This rule exists only in the prompt (the BM25 warning originally planned was not implemented).

### 3.5 Short prompt (printed by `tune next`, versioned and snapshot-tested)

```
Write search queries that people or AI agents would use to find each passage below.
For each passage write: 1 natural question in the passage's language; 1 keyword query (3–8 words);
1 question in another language from {langs}; 1 vague or paraphrased question.
Do not copy the title or 5+ consecutive words. Each must be answerable from that passage alone.
Output JSONL only: {"passage": "...", "kind": "...", "lang": "...", "q": "..."} — then run:
okbase embed tune submit {batch} -
```

---

## 4. Guidance for agents: CLI, short prompt or skill?

**Recommendation: the CLI is the primary source; the skill is only a thin layer.**

| Approach | Pros | Cons |
|---|---|---|
| Long instructions in a skill | Loaded automatically when needed | Version drift from the binary; only agents that support skills can use it; agents easily skip steps |
| **CLI-guided + short per-step prompts** (`tune next` / `submit` / `status` always print the "next step") | Same version as the binary; works with any agent that has a shell; immediate validation that does not depend on the agent doing it right | The agent must know the first command |
| **Skill `okbase-tune` (~30 lines)** | Triggered by natural phrasing ("tune embeddings", "Vietnamese search is poor") | — |

Contents of the `okbase-tune` skill:
- When to use it; always run `okbase advise` first.
- Ask the user before sending documents to an LLM (private) and before downloading the training environment.
- Loop `tune next` → write JSONL → `tune submit` until `status` reports done; subagents may be used for parallel batches.
- Do not modify the bundle; do not `--force` the gate.

Other components:
- **No skill:** `okbase embed tune guide` prints the full instructions (used as an `AGENTS.md` section for Codex), like the fallback mechanism in design §6.
- **`advise`** also prints the line "if using an agent: say *tune embedding for this bundle*" when it recommends level 4.

---

## 5. Roadmap

| Phase | Content | Done when |
|---|---|---|
| **0. Technical gate** (~1 day) | Export S11's tuned-ml to ONNX + Q4; run okbase's `retrieval_eval` | Q4 still keeps ≥ 2/3 of the gain (S1 R@1 ≥ 0.92). **If not: stop the tune work** (or support fp32 only), still do phases 1–2 |
| **1. `advise`** (core) | Signals §1.1, levels §1.2, `--json`, snapshot tests, extend `recommend_mode` | Recommends the expected levels on fixtures, OpenClaw L and biz-meta |
| **2. Custom models** | Manifest, `models add/pull/remove`, cache keyed by sha256, `custom:<name>` | Loads a third-party ONNX model; switching models re-embeds correctly; round-trip test |
| **3. `embed eval` + question data** | `eval --quick`; `tune init/next/submit/check/status`; standard §3; skill `okbase-tune` + `guide` | Claude Code **and** Codex complete steps 1–4 on `fixtures/multilingual` using only the CLI and skill (**spike S12**: rejected-line rate, turns, cost) |
| **4. Train + export** | Local backend (uv, CPU/CUDA + Unsloth), colab zip + `import`, ONNX Q4 export | Reproduce S11 on CPU end to end with a sequence of okbase commands |
| **5. Gate + activate** | `tune eval` (Rust), gate §2.4, `activate/rollback`, docs | From a new bundle: the agent completes the whole workflow; the gate blocks a bad model (tested with a deliberately broken trained model) |

Estimate: phase 0 about 1 day; phases 1–2 about 1 week; phases 3–5 about 2 weeks.

---

## 6. Decisions made (best practice, 2026-10-02)

1. **Train/export through Python** in an optional module (`embed-tune`, included in okbase-full): a
   separate environment in the user cache, pinned versions, using `uv` if available, otherwise `venv` + pip; created only with
   `--yes`. Colab is the fallback. Native (Rust) training is deferred.
2. **Self-tuned models live in the user cache**; teams share them with `okbase embed models add <shared directory>`.
   Not stored in the bundle.
3. **`advise` only prints commands** (`--json` for agents), no `--apply`.
4. **No `kb_advise` MCP tool**: this is a setup-time task; every extra tool costs tokens on every turn.
5. **Default languages** for `tune init`: the languages making up ≥ 5% of the bundle, plus `en`.

## 7. Implementation and acceptance

| Phase | Commit | Notes |
|---|---|---|
| 0 | `562cc27` | Q4 export by writing the changed matrices into the reference Q4 graph (no re-export of Gemma3). S1 R@1 0.857 → 0.917, cross-language 0.815 → 0.92, S4 29 → 30/30. Passed |
| 1 | `d4a52e3` | `okbase advise` (+ `content_langs`) |
| 2 | `8a18314` | `custom:<name>`, manifest `okbase-model.json`, vectors keyed by `custom:<name>@<hash>` |
| 3 | `9261958` | crate `okbase-tune`, `tune init/next/submit/status/check/runs/guide`, `embed eval`, skill `okbase-tune` |
| 4–5 | `6ce5ed4` | `tune setup/train/import/export/eval/activate/rollback`, gate, embedded shared regression set |

Deviations from the plan:
- Export uses the merged weights directly (no 1.2 GB fp32 ONNX needed); about 1% of Q4 bytes in the changed
  matrices differ compared with adding the delta, within quantization error.
- Passages are always index chunks (a short document = 1 chunk), the same unit search returns.
- Added `tune setup --yes` to build the environment in parallel while the agent writes questions.
- After acceptance testing with real agents: Vietnamese is counted in syllables (keyword 2–12, copying ≥ 8 syllables), keyword
  queries may mention the title, Vietnamese `vague` questions may omit diacritics; the prompt states the Japanese limits explicitly.
