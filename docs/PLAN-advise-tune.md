# Kế hoạch: `okfkit advise` và fine-tune embedding (`okfkit embed tune`)

Trạng thái: **đề xuất, chưa triển khai** (2026-10-02). Bổ sung cho `PLAN.md` (§1 S11, §13, §14).
Bằng chứng: `spikes/embed-tune/RESULTS.md`.

## 0. Mục tiêu

1. **`okfkit advise`:** nhìn vào một project hoặc bundle và đề xuất nên dùng okfkit thế nào, từ đơn
   giản nhất đến phức tạp, kèm lệnh cụ thể cho từng bước.
2. **Fine-tune embedding trọn gói:**
   - Agent (Claude Code, Codex, …) viết câu hỏi theo một **tiêu chuẩn có sẵn**.
   - okfkit kiểm tra dữ liệu, train, export, đánh giá, tải hoặc kích hoạt model, và quay lại bản cũ khi cần.
   - Người dùng chỉ cần nói với agent "tune embedding cho bundle này".

**Ràng buộc (AGENTS.md):**
- Core không chứa model: `advise` thuộc core (lexical, không model); training nằm trong module opt-in `embed-tune`.
- Mặc định chỉ đọc: mọi file sinh ra nằm trong state dir; `okfkit.toml` chỉ bị sửa khi có cờ ghi.
- Không telemetry: okfkit **không tự gọi LLM**, việc viết câu hỏi do agent của người dùng làm.

---

## 1. `okfkit advise`: đề xuất cách dùng theo project

### 1.1 Tín hiệu đo được (không cần model)

| Tín hiệu | Lấy từ | Dùng để |
|---|---|---|
| Số tài liệu, token ước tính, chunk/tài liệu | `stats` | Chọn mức full-context, lexical hay embed |
| Tỷ lệ ngôn ngữ (vi / ja / en / khác) | analyzer (đếm script, dấu tiếng Việt) | Bundle đa ngôn ngữ thì cần embed (S1: BM25 hỏi khác ngôn ngữ chỉ đạt 3.5%) |
| Mức chuẩn L0–L3, description thiếu, trùng lặp | `lint` | Đề xuất adopt / curate trước khi làm gì khác |
| CSV/TSV/XLSX, kích thước | quét file | Bật module `data` (S5) |
| `index.md`, `_meta/vocabulary.md`, type schema | quét file | Đánh giá mức tổ chức |
| Tuỳ chọn người dùng khai báo | cờ `--agent claude\|codex\|host`, `--users-lang vi,ja`, `--private`, `--gpu` | Cách triển khai, có gửi tài liệu lên LLM được không |

### 1.2 Các mức đề xuất (đi lần lượt; chỉ lên mức tiếp khi có lý do)

| Mức | Khi nào (theo spike) | okfkit làm gì | Lệnh chính |
|---|---|---|---|
| **0. Full context** | ≤ ~30k token (S4) | Nhét cả bundle hoặc catalog vào system prompt, không cần index | `okfkit catalog` |
| **1. Lexical (mặc định)** | Mọi bundle lớn hơn mức 0 | MCP stdio + skill; `kb_grep` / `kb_query` / `kb_get` | `okfkit agent install`, `okfkit mcp serve` |
| **1+. Tổ chức lại** | Mức chuẩn < L2, thiếu description, có nội dung trùng | Adopt, lint, vocab trước khi thêm máy móc (bài học trung tâm của PLAN §1) | `okfkit adopt --plan`, `okfkit lint --level L2` |
| **2. Data** | Có CSV/XLSX, nhất là sheet lớn | Bật `data_query` (S5: không có SQL thì agent bỏ cuộc) | tự bật; `okfkit data tables` |
| **3. Embed** | Người dùng hỏi khác ngôn ngữ với tài liệu, bundle lớn, hoặc host cần pre-retrieval | Chọn model theo license, ngôn ngữ, phần cứng (Gemma Q4 / bge-m3 / API) | `okfkit embed enable`, `okfkit embed index` |
| **4. Fine-tune** | Đủ điều kiện mức 3, **và** `embed eval` cho thấy yếu (nhất là khác ngôn ngữ), ≥ ~100 tài liệu, được phép gửi tài liệu cho LLM của agent | Quy trình §2 | `okfkit embed tune …` |

**Trục triển khai** (độc lập với các mức trên):
- Một người dùng CLI agent: stdio + skill.
- Nhóm dùng chung: `mcp serve --http` + token.
- Ứng dụng host: thư viện `Bundle` / `router()`.

### 1.3 Đầu ra

Gồm phần text cho người đọc và `--json` (cùng schema với tool MCP `kb_advise`, nếu thêm tool này):

```
$ okfkit advise --users-lang vi,ja
Bundle: 287 docs, ~1.1M tokens, en 92% / vi 5% / ja 3%, level L1 (41 docs without description)

Recommended path
  1. [now]   Lexical + skills      okfkit agent install --agent claude
  2. [now]   Fix descriptions       okfkit lint --level L2   (41 docs; see skill okfkit-curate)
  3. [next]  Embeddings             users ask in vi/ja, docs are en → cross-language needs embeddings
                                    okfkit embed enable --model embeddinggemma-300m-q4
  4. [maybe] Fine-tune              ask your agent: "tune embeddings for this bundle"
Not needed: data (no tables), full-context (too large)
```

- **`advise` không ghi gì.** `--apply <bước>` chỉ chạy đúng lệnh in ra, sau khi người dùng đồng ý.
- Ngưỡng là hằng số có trích dẫn spike (như `FULL_MODE_MAX_TOKENS`), có test snapshot.
- Mở rộng `recommend_mode()` hiện có thay vì viết song song.

---

## 2. Fine-tune trọn gói: `okfkit embed tune`

### 2.1 Luồng

```
 advise / embed eval ──► tune init ──► [agent] tune next / tune submit (lặp) ──► tune check
                                                                                    │
     rollback ◄── tune activate ◄── tune eval ◄── tune export ◄── tune train ◄──────┘
```

| Bước | Lệnh | Ai làm | Ghi chú |
|---|---|---|---|
| 1 | `okfkit embed tune init [--langs vi,ja,en] [--budget N]` | okfkit | Chọn passage theo tiêu chuẩn §3, chia batch, tách sẵn tập held-out **theo tài liệu**. Ghi vào `<state>/tune/<run>/`. In ước tính số câu, token agent cần đọc, thời gian train |
| 2 | `okfkit embed tune next` | agent | In batch tiếp theo: passage + **prompt ngắn** (tiêu chuẩn §3) + định dạng JSONL cần trả về |
| 3 | `okfkit embed tune submit <batch> [file\|-]` | agent → okfkit | Kiểm tra ngay (§3.4); lỗi in theo từng dòng để agent sửa và nộp lại. Lặp 2–3 đến khi hết batch |
| 4 | `okfkit embed tune check` | okfkit | Báo cáo theo tiêu chuẩn: tổng số, phân bố ngôn ngữ và loại câu, trùng lặp. Chưa đạt thì không cho train |
| 5 | `okfkit embed tune train [--backend local\|colab]` | okfkit (Python) | §2.2 |
| 6 | `okfkit embed tune export` | okfkit (Python) | Gộp adapter → ONNX → Q4; ghi manifest §2.3 |
| 7 | `okfkit embed tune eval` | okfkit (Rust, đúng đường chạy thật) | Trên held-out: lexical vs model gốc vs model tuned (R@1/R@3/MRR, khác ngôn ngữ) + bộ hồi quy chung. Có **gate**, §2.4 |
| 8 | `okfkit embed tune activate --write` | okfkit | Ghi `[embed] model = "custom:<name>"`; embed lại bundle ở nền (cache khoá theo hash model) |
| — | `okfkit embed tune rollback --write`, `tune status`, `tune runs` | okfkit | Quay về model trước; xem tiến độ và lịch sử |

Các bước 2–3 dùng được với **mọi agent có shell**:
- Agent không cần đọc tài liệu dài: CLI đưa từng việc nhỏ và kiểm tra kết quả ngay.
- Agent có subagent (Claude Code) có thể chạy nhiều batch song song. `next --claim` khoá batch để tránh trùng.

### 2.2 Backend train

| Backend | Khi nào | Cách làm |
|---|---|---|
| **local** (mặc định) | Có Python hoặc `uv` | okfkit tạo venv trong `<user cache>/okfkit/tune-env` bằng `uv` theo **lockfile nhúng trong binary** (torch, sentence-transformers, peft, optimum, onnxruntime). Có CUDA thì dùng Unsloth (QLoRA, nhanh); không có thì CPU: LoRA + `CachedMultipleNegativesRankingLoss(mini_batch_size=8)`, ~25 phút / 500 cặp, ~6.5 GB RAM (S11) |
| **colab** | Không có máy mạnh | Xuất `tune-<run>.zip` (dữ liệu + notebook Unsloth sinh sẵn); người dùng chạy trên Colab T4 rồi `okfkit embed tune import <file>` |
| (sau) **native** | Khi candle hoặc burn đủ ổn cho LoRA trên Gemma3 | Bỏ phụ thuộc Python. Chưa làm trong kế hoạch này |

- Script train, export và notebook được **nhúng trong binary** và có phiên bản theo okfkit.
- Python chỉ được gọi khi người dùng chạy `tune train` hoặc `tune export`. Lần đầu okfkit hỏi trước khi tải khoảng 1–2 GB (hoặc dùng cờ `--yes`).

### 2.3 Model tuỳ chỉnh, tải model, registry

- `okfkit embed models` liệt kê model có sẵn và model tuỳ chỉnh, kèm license, kích thước, điểm eval.
- Các lệnh thêm và gỡ model:
  - `okfkit embed models add <dir|hf-repo>`: nạp ONNX bất kỳ có manifest (cả model ngoài, không chỉ model tune).
  - `okfkit embed models pull <id>`: tải trước để chạy offline.
  - `okfkit embed models remove <id>`.
- Model tự tune lưu tại `<user cache>/okfkit/models/custom/<name>/` gồm `model.onnx` (Q4), tokenizer và `okfkit-model.toml`. Manifest ghi:
  - model gốc và license kế thừa (Gemma: dùng chung bước chấp nhận license hiện có);
  - pooling, prompt query/document, số chiều, `max_length`;
  - sha256 của model, hash dữ liệu train, phiên bản tiêu chuẩn câu hỏi;
  - điểm eval lúc tạo.
- **Cache vector khoá theo sha256 model** (hiện khoá theo tên); đây là điều kiện bắt buộc trước khi cho phép model tuỳ chỉnh.
- okfkit **không phân phối model đã tune**. Nhóm muốn dùng chung thì tự chép thư mục model, chịu điều khoản Gemma.

### 2.4 Gate chất lượng (`tune eval`)

`activate` từ chối, trừ khi có `--force`, nếu không đạt **cả hai** điều kiện:
1. Trên held-out, R@1 của model tuned ≥ model gốc + 2 điểm, và R@1 khác ngôn ngữ không giảm.
2. Trên bộ hồi quy chung, không giảm quá 1 điểm. Bộ này nhỏ, nhúng sẵn: một phần fixture multilingual (S1).

Nếu có `_meta/eval/questions.jsonl` (câu hỏi do người viết; khuyến nghị ≥ 50 câu), gate dùng bộ này thay cho held-out tổng hợp. Lý do là tradeoff 2 của S11: câu hỏi train và eval cùng do một LLM sinh ra.

`okfkit embed eval [--quick]` cũng dùng được **không cần tune**: so lexical với Gemma Q4 với bge-m3 trên chính bundle của người dùng, làm căn cứ cho mức 3 và 4 của `advise`.

---

## 3. Tiêu chuẩn câu hỏi (okfkit question standard v1)

Mục đích: người dùng không phải tự quyết định gì. `tune init` áp dụng mặc định này; mọi giá trị chỉnh được trong `[embed.tune]`.

### 3.1 Chọn passage

- Tài liệu ngắn (≤ 512 token ước tính) dùng **nguyên tài liệu**. Tài liệu dài dùng **chunk của index** (đúng đơn vị mà search trả về).
- Bundle lớn: lấy mẫu ≤ 400 passage, **phân tầng** theo thư mục, type và ngôn ngữ. Bỏ chunk quá ngắn (< 40 token) và các trang mục lục hay index.
- Tách **15% tài liệu** (tối thiểu 20 tài liệu) làm held-out, tách theo tài liệu chứ không theo câu hỏi, để tránh lộ đề.

### 3.2 Số lượng

| | Mặc định | Ghi chú |
|---|---|---|
| Câu / tài liệu ngắn | 4 | S11 dùng 5 |
| Câu / chunk | 2 | S11 dùng 2 |
| Tổng cặp train | **600–1.200** | S11: 500–580 cặp đã đủ cho mức tăng lớn |
| Tối thiểu để được train | 300 | Dưới mức này chưa có bằng chứng |
| Chi phí tham khảo | ~$3 / 1.000 cặp với model cỡ Sonnet | `tune init` in ước tính trước |

### 3.3 Loại câu (cho mỗi passage)

Với 4 câu, mỗi loại một câu. Với 2 câu, luân phiên sao cho mỗi passage có 1 câu khác ngôn ngữ.

| `kind` | Mô tả | Vì sao |
|---|---|---|
| `natural` | Câu hỏi tự nhiên bằng ngôn ngữ của tài liệu | Cơ bản |
| `keyword` | Truy vấn kiểu agent: 3–8 từ khoá, có thể trộn thuật ngữ tiếng Anh | Người hỏi thật là **agent** (tradeoff 1 của S11) |
| `cross` | Hỏi bằng ngôn ngữ khác, lấy từ `--langs` | Mức tăng lớn nhất nằm ở đây (0.815 → 0.95) |
| `vague` | Mơ hồ, diễn đạt lại, có thể sai chính tả | Độ bền |

### 3.4 Luật kiểm tra (`tune submit` từ chối dòng vi phạm)

- JSONL, mỗi dòng: `{"passage": "<id>[#<chunk>]", "kind": "natural|keyword|cross|vague", "lang": "vi", "q": "…"}`.
- Đủ số câu và đủ loại cho mỗi passage; `lang` thuộc `--langs` và khớp ngôn ngữ phát hiện được trong `q`.
- Độ dài 3–30 từ (với ja: 5–60 ký tự).
- Không chép tiêu đề, không trùng ≥ 5 từ liên tiếp với passage, không chứa id hay tên file.
- Không trùng hoặc gần trùng câu khác (chuẩn hoá rồi so), và không trùng câu trong bộ eval do người viết.
- Câu phải trả lời được **chỉ từ passage đó**. Luật này chỉ nằm trong prompt; okfkit kiểm tra gián tiếp: cảnh báo nếu BM25 xếp passage đúng ngoài top-50 (nhiều khả năng câu quá chung chung).

### 3.5 Prompt ngắn (in bởi `tune next`, có phiên bản và test snapshot)

```
Write search queries that people or AI agents would use to find each passage below.
For each passage write: 1 natural question in the passage's language; 1 keyword query (3–8 words);
1 question in another language from {langs}; 1 vague or paraphrased question.
Do not copy the title or 5+ consecutive words. Each must be answerable from that passage alone.
Output JSONL only: {"passage": "...", "kind": "...", "lang": "...", "q": "..."} — then run:
okfkit embed tune submit {batch} -
```

---

## 4. Hướng dẫn cho agent: CLI, prompt ngắn hay skill?

**Đề xuất: CLI là nguồn chính, skill chỉ là lớp mỏng.**

| Cách | Ưu | Nhược |
|---|---|---|
| Hướng dẫn dài trong skill | Tự nạp khi cần | Lệch phiên bản với binary; chỉ agent hỗ trợ skill mới dùng được; agent dễ bỏ sót bước |
| **CLI dẫn đường + prompt ngắn theo từng bước** (`tune next` / `submit` / `status` luôn in "bước tiếp theo") | Cùng phiên bản với binary; dùng được với mọi agent có shell; kiểm tra ngay, không phụ thuộc agent có làm đúng hay không | Agent phải biết lệnh đầu tiên |
| **Skill `okfkit-tune` (~30 dòng)** | Kích hoạt từ câu nói tự nhiên ("tune embedding", "tìm kiếm tiếng Việt kém") | — |

Nội dung skill `okfkit-tune`:
- Khi nào dùng; luôn chạy `okfkit advise` trước.
- Hỏi người dùng trước khi gửi tài liệu cho LLM (private) và trước khi tải môi trường train.
- Lặp `tune next` → viết JSONL → `tune submit` cho đến khi `status` báo xong; có thể dùng subagent cho các batch song song.
- Không sửa bundle; không `--force` gate.

Các thành phần khác:
- **Không có skill:** `okfkit embed tune guide` in toàn bộ hướng dẫn (dùng làm đoạn `AGENTS.md` cho Codex), giống cơ chế dự phòng của PLAN §6.
- **`advise`** cũng in dòng "nếu dùng agent: hãy nói *tune embedding for this bundle*" khi đề xuất mức 4.

---

## 5. Lộ trình

| Phase | Nội dung | Tiêu chí xong |
|---|---|---|
| **0. Gate kỹ thuật** (~1 ngày) | Export tuned-ml của S11 sang ONNX + Q4; chạy `retrieval_eval` của okfkit | Q4 vẫn giữ ≥ 2/3 mức tăng (S1 R@1 ≥ 0.92). **Nếu không đạt: dừng phần tune** (hoặc chỉ hỗ trợ fp32), vẫn làm phase 1–2 |
| **1. `advise`** (core) | Tín hiệu §1.1, các mức §1.2, `--json`, snapshot test, mở rộng `recommend_mode` | Đề xuất đúng các mức kỳ vọng trên fixtures, OpenClaw L và biz-meta |
| **2. Model tuỳ chỉnh** | Manifest, `models add/pull/remove`, cache khoá theo sha256, `custom:<name>` | Nạp được model ONNX ngoài; đổi model thì embed lại đúng; test round-trip |
| **3. `embed eval` + dữ liệu câu hỏi** | `eval --quick`; `tune init/next/submit/check/status`; tiêu chuẩn §3; skill `okfkit-tune` + `guide` | Claude Code **và** Codex hoàn thành bước 1–4 trên `fixtures/multilingual` chỉ nhờ CLI và skill (**spike S12**: tỉ lệ dòng bị từ chối, số lượt, chi phí) |
| **4. Train + export** | Backend local (uv, CPU/CUDA + Unsloth), colab zip + `import`, export ONNX Q4 | Tái hiện S11 trên CPU từ đầu đến cuối bằng một chuỗi lệnh okfkit |
| **5. Gate + activate** | `tune eval` (Rust), gate §2.4, `activate/rollback`, tài liệu | Từ một bundle mới: agent làm hết quy trình; gate chặn được model kém (test bằng model train hỏng cố ý) |

Ước tính: phase 0 khoảng 1 ngày; phase 1–2 khoảng 1 tuần; phase 3–5 khoảng 2 tuần.

---

## 6. Quyết định cần maintainer chốt

1. **Phụ thuộc Python (qua `uv`) cho train/export** trong module opt-in `embed-tune` có chấp nhận được không? Phương án khác: chỉ hỗ trợ colab, hoặc chờ native (candle).
2. Model tự tune lưu ở **user cache** (mặc định đề xuất) hay cho phép lưu trong bundle `.okfkit/models/` để chia sẻ nhóm (file ~200 MB, điều khoản Gemma)?
3. `advise --apply` có nên tồn tại không, hay `advise` chỉ in lệnh?
4. Thêm tool MCP `kb_advise` hay chỉ để ở CLI?
5. Ngôn ngữ mặc định cho `--langs` khi không khai báo: lấy theo phân bố ngôn ngữ của bundle cộng thêm `en`?
