# Spike: Embedding đa ngôn ngữ (vi / en / ja) — kết quả

Ngày chạy: 2026-09-30 · Máy: AMD EPYC 8 vCPU, 23GB RAM, không GPU · fastembed 7.1 (ONNX fp32), lindera 6.2 (IPADIC), SQLite FTS5

## Bộ dữ liệu

- `data/docs.json`: 30 tài liệu của một công ty giả định (10 vi, 10 en, 10 ja). Nội dung gồm chính sách, hướng dẫn, HR và lỗi thiết bị. Bộ dữ liệu cố ý có các cặp chủ đề gần nhau để gây nhiễu:
  - đổi trả ↔ hoàn tiền
  - vệ sinh điều hòa ↔ thay フィルター
  - hóa đơn VAT ↔ 領収書
  - tích điểm ↔ ポイント期限
  - nghỉ phép ↔ remote work
- `data/queries.json`: 90 câu hỏi. Mỗi tài liệu có 3 câu, bằng vi, en và ja, **diễn đạt khác** so với nội dung tài liệu.
  - 30 câu cùng ngôn ngữ với tài liệu (*same-lang*)
  - 60 câu khác ngôn ngữ (*cross-lang*)

Chạy lại: `cargo run --release -- <bm25|e5-small|e5-base|bge-m3>`. Kết quả chi tiết, gồm danh sách câu trượt, nằm trong `results/*.json`.

## Kết quả chất lượng (Recall@1 = top-1 đúng)

| Phương pháp | R@1 | R@3 | MRR | Same-lang R@1 | **Cross-lang R@1** |
|---|---|---|---|---|---|
| BM25 (FTS5 + lindera + bỏ dấu vi) | 0.27 | 0.36 | 0.32 | 0.73 | **0.03** |
| multilingual-e5-small | 0.48 | 0.78 | 0.64 | 0.93 | **0.25** |
| multilingual-e5-base | 0.68 | 0.87 | 0.79 | 1.00 | **0.52** |
| **bge-m3** | **0.98** | **1.00** | **0.99** | **1.00** | **0.97** |

Hybrid, thử trên cả 3 model:

| Fusion | e5-small R@1 | e5-base R@1 | bge-m3 R@1 |
|---|---|---|---|
| Dense thuần | 0.48 | 0.68 | 0.98 |
| RRF (BM25 w=1.0) | 0.36 | 0.37 | 0.38 |
| RRF (BM25 w=0.1) | 0.37 | 0.42 | 0.46 |
| Convex (cos + 0.3·bm25) | 0.41 | 0.50 | 0.87 |
| Convex (cos + 0.1·bm25) | 0.43 | 0.62 | 0.98 |

## Kết quả tài nguyên (đo khi model đã có sẵn trong cache)

| | BM25 | e5-small | e5-base | bge-m3 |
|---|---|---|---|---|
| Dung lượng model (fp32) | 0 (từ điển nhúng trong binary) | 465MB | 1.1GB | 2.2GB |
| Thời gian load | 8ms | 2.3s | 3.2s | 2.9s |
| RAM sau khi load | ~39MB | ~1.0GB | ~1.6GB | ~1.7GB |
| RAM đỉnh khi index (batch 32) | 39MB | 1.1GB | 2.0GB | 2.1GB |
| Tốc độ index (chunk ~150–250 token) | tức thì | 48 chunk/s | 16 chunk/s | **5 chunk/s** |
| Ước lượng index 10k chunk lần đầu | <1s | ~3.5 phút | ~10 phút | **~35 phút** |
| Truy vấn p50 / p95 | 0.4 / 0.7ms | 9 / 12ms | 23 / 27ms | 71 / 84ms |

Binary release khoảng 84MB (gồm onnxruntime và từ điển IPADIC).

## Nhận định

1. **Embedding là bắt buộc với bot 3 ngôn ngữ.** BM25 chỉ tìm đúng 3% câu hỏi khác ngôn ngữ, và vẫn trượt 27% câu cùng ngôn ngữ khi người dùng diễn đạt khác tài liệu.
2. **bge-m3 vượt trội rõ ràng**, nhất là khi hỏi khác ngôn ngữ: 97%, so với 52% của e5-base và 25% của e5-small. e5-small không đủ tốt cho trường hợp vi↔ja.
3. **Chi phí của bge-m3 chấp nhận được trên server.** Truy vấn khoảng 70ms, RAM khoảng 2GB (dùng chung cho mọi bot). Điểm yếu duy nhất là index lần đầu chậm (5 chunk/s), nên phải chạy nền và index dần phần thay đổi.
4. **Hybrid kiểu "trộn đều" làm kết quả xấu đi.**
   - Với tiếng Việt, BM25 khớp các từ phổ biến như "không", "được", "có". Với tiếng Nhật, nó khớp các danh từ chung như 家電.
   - Khi chuẩn hoá điểm theo max của từng truy vấn, một kết quả khớp yếu cũng thành 1.0. Việc này đẩy tài liệu sai cùng ngôn ngữ lên trên tài liệu đúng khác ngôn ngữ.
   - RRF còn tệ hơn, vì trên tập nhỏ khoảng cách điểm giữa các hạng rất sát nhau.
   
   → **BM25 không nên là một nguồn xếp hạng ngang hàng với embedding.** Nên dùng nó cho:
   - (a) khớp chính xác các định danh như mã lỗi `E2`, `HTTP 429`, SKU hay tên riêng, thông qua bảng `aliases` hoặc token hiếm (IDF cao);
   - (b) chế độ dự phòng khi tắt embedding;
   - (c) lọc hoặc tìm kiếm trong CLI (`qobot kb search --lexical`).
5. **Bộ tách từ đã chạy đúng.** Lindera tách tiếng Nhật đúng; bỏ dấu tiếng Việt hoạt động. Để BM25 tốt hơn nữa cần bỏ stopword vi/ja và thêm stemming tiếng Anh; spike này chưa làm.
6. **Build:** phải tắt `native-tls` và dùng `rustls` (`default-features = false`) để không phụ thuộc OpenSSL.

## Giới hạn của spike

- Dữ liệu nhỏ (30 tài liệu, 90 câu hỏi) và do tôi tự soạn. Với vài nghìn tài liệu, điểm số của cả bốn phương pháp đều sẽ thấp hơn và khoảng cách giữa chúng có thể thay đổi. **Cần chạy lại trên dữ liệu thật.**
- Mỗi tài liệu là một chunk; chưa thử chia nhỏ theo heading.
- Chỉ chạy một lần, CPU 8 vCPU; chưa giới hạn số thread để mô phỏng máy nhỏ.
- Chưa thử bản int8 (lượng tử hoá) của bge-m3, thường nhỏ hơn khoảng 4 lần và nhanh hơn 2–3 lần, và chưa thử reranker.

## Đề xuất cho qobot

| Profile | Điều kiện | Cấu hình |
|---|---|---|
| **standard** (mặc định) | Server ≥ 4GB RAM trống | bge-m3 + BM25 dùng để khớp định danh (có ngưỡng) |
| **lite** | Máy yếu (<2GB) | e5-small hoặc tắt embedding; agent tự dịch truy vấn sang 3 ngôn ngữ rồi gọi `kb.search` nhiều lần |
| **api** | Không muốn chạy model local | `EmbeddingProvider` gọi API (Voyage/OpenAI/Gemini multilingual) |

Spike tiếp theo nên làm:
- (1) bge-m3 int8;
- (2) BM25 có ngưỡng dành cho định danh;
- (3) chunk theo heading;
- (4) chạy trên khoảng 500 tài liệu thật của bạn.

---

# Vòng 2 — Bộ dữ liệu khó (v2) và 12 model

Ngày chạy: 2026-09-30. Chạy lại bằng `./download.sh && ./run-v2.sh`. Log ở `results/run-v2.log`, chi tiết ở `results/v2/*.json`.

**Bộ dữ liệu v2** (`data/v2/`, mã nguồn sinh dữ liệu ở `data/v2/src/`):
- 100 tài liệu (33 vi, 33 en, 34 ja), chia thành **20 nhóm chủ đề gần giống nhau**. Mỗi nhóm có các tài liệu chỉ khác nhau một chi tiết (thiết bị, kênh bán, quốc gia, loại nghỉ phép…) và cố ý viết bằng các ngôn ngữ khác nhau. Ví dụ nhóm bảo hành gồm 6 tài liệu, nhóm mã lỗi E1–E6 gồm 5 thiết bị.
- 300 câu hỏi, mỗi tài liệu 3 câu (vi, en, ja). Khoảng 20% câu tiếng Việt không dấu, có từ lóng (bh, ship, pass, WFH), và có câu chỉ gồm từ khoá.

## Chất lượng (sắp theo R@1)

| Model | R@1 | R@3 | MRR | Same-lang R@1 | Cross-lang R@1 |
|---|---|---|---|---|---|
| gemma-q4 **+ reranker** bge-v2-m3 | 0.887 | 0.973 | 0.927 | 0.95 | 0.855 |
| gte-mb-int8 **+ reranker** | 0.883 | 0.967 | 0.923 | 0.94 | 0.855 |
| **EmbeddingGemma-300M** (fp32) | **0.863** | 0.957 | 0.913 | 0.94 | **0.825** |
| EmbeddingGemma Q (int8, dynamic) | 0.860 | 0.957 | 0.911 | 0.93 | 0.825 |
| **EmbeddingGemma Q4** | **0.847** | **0.960** | 0.905 | 0.93 | 0.805 |
| e5-small + reranker | 0.833 | 0.900 | 0.865 | 0.95 | 0.775 |
| gte-multilingual-base (fp32) | 0.807 | 0.920 | 0.865 | 0.87 | 0.775 |
| bge-m3 (fp32) | 0.780 | 0.940 | 0.861 | 0.89 | 0.725 |
| bge-m3 int8 | 0.780 | 0.933 | 0.856 | 0.89 | 0.725 |
| gte-multilingual-base int8 | 0.767 | 0.917 | 0.847 | 0.82 | 0.740 |
| Qwen3-Embedding-0.6B | 0.753 | 0.923 | 0.846 | 0.90 | 0.680 |
| nomic-embed-text-v2-moe | 0.703 | 0.933 | 0.823 | 0.88 | 0.615 |
| multilingual-e5-large | 0.483 | 0.730 | 0.635 | 0.90 | 0.275 |
| multilingual-e5-base | 0.460 | 0.697 | 0.601 | 0.88 | 0.250 |
| multilingual-e5-small | 0.390 | 0.563 | 0.506 | 0.91 | 0.130 |
| BM25 (FTS5 + lindera) | 0.333 | 0.377 | 0.363 | **0.93** | 0.035 |

## Tài nguyên (8 vCPU; cột "2 CPU" đo với `THREADS=2`)

| Model | Dung lượng | RAM sau load | RAM đỉnh | Query p50 (8 CPU / 2 CPU) | Index chunk/s (8 / 2 CPU) |
|---|---|---|---|---|---|
| **gemma-q4** | **188MB** | **~460MB** | **~600MB** | 38 / 49ms | 7.5 / 4.2 |
| gemma (fp32) | 1.2GB | ~770MB | ~980MB | 39ms | 10 |
| gemma-q (int8 dynamic) | 295MB | ~370MB | 1.7GB ¹ | 106ms | 7.7 |
| bge-m3 int8 | 558MB | ~1.1GB | ~1.7GB | 37 / 46ms | 6.2 / 3.2 |
| bge-m3 | 2.2GB | ~1.7GB | ~2.0GB | 77 / 93ms | 3.8 / 1.9 |
| gte-mb int8 | 341MB | ~900MB | ~1.25GB | 16 / 20ms | 13.6 / 8.8 |
| gte-mb | 1.2GB | ~1.8GB | ~3.2GB | 31ms | 8.7 |
| e5-small | 465MB | ~1.0GB | ~1.1GB | 9 / 12ms | 41 / 22 |
| Qwen3-0.6B (candle) | 1.1GB | ~2.4GB | ~3.5GB | **439ms** | **0.7** |
| nomic-v2-moe (candle) | 1.8GB | ~2.1GB | ~3.7GB | 190ms | 2.6 |
| + reranker bge-v2-m3 (top-20) | +2.2GB | | +~2GB | **+5–6 giây/câu** | — |

¹ Model lượng tử hoá động không cho chia batch, nên fastembed phải embed tất cả văn bản trong một lần; RAM đỉnh cao vì vậy.

## Nhận định vòng 2

1. **Kết quả vòng 1 không còn đúng.** Trên bộ khó, bge-m3 tụt từ 0.98 xuống 0.78. **EmbeddingGemma dẫn đầu** (0.86) và tốt nhất khi hỏi khác ngôn ngữ (0.83).
2. **EmbeddingGemma Q4 có tỉ lệ chất lượng trên chi phí tốt nhất:**
   - chỉ mất 1.6 điểm R@1 so với fp32, R@3 vẫn đạt 0.96;
   - file 188MB, RAM khoảng 0.5GB (bằng 1/3–1/4 bge-m3);
   - truy vấn 38–49ms, kể cả khi giới hạn 2 CPU.
3. **Bản int8 của bge-m3 không mất chất lượng**, nhanh gấp 2 và nhẹ bằng 1/4 so với fp32. Đây là phương án dự phòng dùng license MIT.
4. **Qwen3 và nomic chạy qua candle trên CPU quá chậm** (0.7–2.6 chunk/s, truy vấn 190–440ms) mà chất lượng không hơn. Loại.
5. **Họ e5 yếu khi hỏi khác ngôn ngữ** (0.13–0.28). Loại.
6. **Reranker tăng thêm 3–4 điểm** (gemma-q4 từ 0.847 lên 0.887), nhưng tốn **5–6 giây mỗi câu** và thêm khoảng 2GB RAM trên CPU. Không phù hợp để chat realtime. Chỉ nên cân nhắc cho tác vụ nền (report, automation) hoặc khi có GPU.
7. **BM25 bổ sung cho embedding:** hỏi cùng ngôn ngữ BM25 đạt 0.93, cao hơn mọi embedding; hỏi khác ngôn ngữ thì gần 0. Nên thí nghiệm tiếp kiểu kết hợp có ngưỡng: chỉ tin BM25 khi điểm khớp cao và truy vấn cùng ngôn ngữ với tài liệu. Có thể đạt trên 0.9 mà không cần reranker.
8. **R@3 khoảng 0.96** với gemma. Bot nên đưa 3–5 tài liệu cho agent, để LLM tự chọn đúng tài liệu trong các tài liệu gần giống nhau.

## Đề xuất (thay cho đề xuất ở vòng 1)

| Profile | Model | Ghi chú |
|---|---|---|
| **standard** (mặc định) | **EmbeddingGemma-300M Q4** | ~0.5GB RAM, chạy được trên VPS 2 CPU. Cần kiểm tra điều khoản Gemma cho mục đích thương mại |
| standard-mit | bge-m3 int8 | Khi cần license MIT |
| quality | EmbeddingGemma fp32 hoặc Q4, cộng reranker cho tác vụ nền | Hoặc dùng GPU |
| lite | BM25 + agent tự dịch truy vấn | Máy rất yếu |
| api | Embedding qua API | Không tốn RAM |

---

# Vòng 3 — Ngưỡng BM25 và số chunk trên mỗi file

## A. Ngưỡng BM25 (`DATA=v2 MODEL=<m> embed-bench gate`, log ở `results/gate-*.log`)

**Cách làm:**
- `final = cos(q,d) + β·g(q,d)`, trong đó cổng `g` được thử theo 4 họ:
  - ngưỡng tuyệt đối `bm25 ≥ T`;
  - top-1 tự tin: `bm25 ≥ T` và top1/top2 ≥ R;
  - độ phủ token có trọng số IDF ≥ C (dạng cứng và dạng mềm);
  - mỗi họ trên đều có thêm biến thể "chỉ khi cùng ngôn ngữ" (nhận diện ngôn ngữ bằng lingua).
- Tham số β, T, R, C chọn bằng **2-fold cross-validation**, chia theo tài liệu đích; con số báo cáo là R@1 trên nửa dữ liệu không dùng để chọn tham số (held-out).

**Độ chính xác của BM25 top-1 theo ngưỡng điểm** (−bm25 của FTS5, trên 100 tài liệu):

| T | ≥ 4 | ≥ 13 | ≥ 16.5 | ≥ 21 | ≥ 24 | ≥ 27 |
|---|---|---|---|---|---|---|
| Precision | 33% | 48% | 55% | 64% | 74% | 81% |
| Tỉ lệ query vượt ngưỡng | 100% | 60% | 40% | 20% | 10% | 5% |

**R@1 sau khi áp cổng:**

| Cổng | gemma-q4 | bge-m3-int8 | gte-mb-int8 |
|---|---|---|---|
| Dense, không dùng BM25 | **84.7%** | **77.3%** | **77.7%** |
| Tốt nhất trong mọi cổng (held-out) | 85.0% | 77.7% | 79.0% |
| Trần lý thuyết (dense đúng HOẶC BM25 đúng) | 86.7% | 79.7% | 82.7% |
| Query có định danh (21 câu: E2, 429…): dense / BM25 | 90.5% / 33.3% | 81.0% / 33.3% | 85.7% / 33.3% |

**Nhận định:**
1. **BM25 hầu như không bổ sung được gì cho dense.** Trần lý thuyết chỉ cao hơn dense 2–5 điểm. Cổng tốt nhất cộng được +0.3 đến +1.3 điểm, tương đương 1–4 trên 300 câu, nằm trong mức nhiễu; nhiều cổng còn làm kết quả thấp hơn dense khi đo held-out.
2. Lý do: 93% same-lang của BM25 ở vòng 2 là những câu **dense cũng đã tìm đúng** (gemma-q4 same-lang = 93%). Hai phương pháp đúng ở cùng các câu, không bù trừ cho nhau.
3. **Định danh cũng không cứu được BM25.** Các mã như E2 xuất hiện ở nhiều tài liệu gần giống nhau (mỗi thiết bị một bảng mã lỗi). Thứ phân biệt được các tài liệu này là tên thiết bị, mà tên thiết bị lại thường nằm ở ngôn ngữ khác với câu hỏi. Dense đạt 90%, BM25 chỉ 33%.
4. **Ngưỡng tuyệt đối không mang sang corpus khác được.** Điểm bm25 phụ thuộc vào số tài liệu (IDF) và độ dài tài liệu, nên không có một "con số ngưỡng" dùng chung.
5. **lingua** nhận diện đúng 100% câu tiếng Nhật và tiếng Anh, nhưng chỉ **90% câu tiếng Việt** (sai ở câu không dấu hoặc quá ngắn). Vì vậy cổng "cùng ngôn ngữ" đã không đáng tin sẵn.

**Kết luận: bỏ BM25 khỏi xếp hạng mặc định.** Chỉ giữ FTS5 cho:
- (a) công cụ tra cứu chính xác (`kb.grep`, `kb search --lexical`) dành cho agent và người;
- (b) khớp chính xác `aliases` hoặc title;
- (c) dự phòng khi embedding chưa sẵn sàng.

Tuỳ chọn `boost_lexical` vẫn để trong cấu hình (mặc định tắt). Chỉ bật khi `retrieval_log` trên dữ liệu thật (ví dụ corpus có SKU hoặc mã riêng cho từng tài liệu) chứng minh là có lợi.

## B. Số chunk trên mỗi file (`chunkstat`, tokenizer EmbeddingGemma, quy tắc §5.5: H2/H3, 200–600 token)

| Corpus | Số file | Token/file (TB / p50 / p90) | Chunk/file (TB / p90) | **Số file ≈ 1.000 chunk** |
|---|---|---|---|---|
| OKF mẫu (Google): acme_retail, ga4, stackoverflow, crypto_bitcoin | 53 | ~480–690 / 410–600 / 580–1.190 | 1.4–2.2 / 2–3 | **~450–720** |
| Docs OpenClaw (markdown tài liệu kỹ thuật thật) | 1.314 | 4.235 / 2.056 / 6.970 | 10.5 / 19 | **~96** |
| — concepts / channels / gateway / tools | 78–158 | 2.700–3.200 / 1.900–2.600 | 7.9–9.2 / 14–20 | ~110–125 |
| Bộ test v2 (tài liệu ngắn) | 100 | 113 | 1.0 | 1.000 |

Mật độ token (tokenizer EmbeddingGemma):
- en ≈ 1.24 token/từ;
- vi ≈ 1.28 token/âm tiết;
- ja ≈ 0.53 token/ký tự.

Một chunk 600 token tương đương khoảng 480 từ tiếng Anh, 470 âm tiết tiếng Việt, hoặc 1.100 ký tự tiếng Nhật.

---

# Vòng 4 — End-to-end với agent CLI thật: đưa catalog vào prompt hay dùng tool?

Ngày chạy: 2026-09-30. Claude Code 2.1.284, `--model sonnet`. Tắt toàn bộ tool có sẵn (`--tools ""`); agent chỉ dùng được MCP `kb` (Rust, `embed-bench mcp`) với các tool `kb_search` (EmbeddingGemma Q4), `kb_grep`, `kb_get`, `kb_list`. Bundle OKF được dựng từ data/v2 (100 tài liệu + `index.md`).

Chạy lại: `DATA=v2 embed-bench e2e-prep && python3 e2e_run.py && python3 e2e_report.py`. Dữ liệu đã lưu trong `results/e2e/` (`runs.jsonl.gz` 180 lượt A–F, `judge.json`, `report.md`, `summary.json`, log).

**30 câu hỏi**, mỗi ngôn ngữ 10 câu:
- 20 câu "khó": dense top-1 sai;
- 10 câu "dễ": dense top-1 đúng.

**Chấm điểm:** LLM judge (sonnet), mù cấu hình (mã item được xáo trộn), so với tài liệu đáp án.

| Cấu hình | Trả lời đúng | Nguồn đúng | Thời gian p50 / p90 | Lượt agent | Tool call TB | Không gọi tool | Chi phí TB |
|---|---|---|---|---|---|---|---|
| A. Chỉ tool | 97% | 97% | 9.5s / 12.3s | 4.8 | 2.7 | 0% | $0.017 |
| B. Catalog (trong user message) + tool | 97% | 97% | 8.4s / 11.0s | 3.5 | 1.4 | 0% | $0.043 |
| C. Top-5 + catalog (user message) + tool | 97% | 97% | 6.8s / 9.8s | 2.3 | 0.2 | 83% | $0.043 |
| **D. Top-5 + catalog trong system prompt + tool** | **97%** | 97% | **6.2s / 8.4s** | **2.2** | **0.2** | **87%** | **$0.017** |

Tổng chi phí thí nghiệm: $3.60.

**Nhận định:**
1. **Độ chính xác như nhau ở cả 4 cách** (97%, câu sai duy nhất giống nhau ở cả 4). Câu sai là q56, và đây là lỗi của bộ dữ liệu, không phải của agent: câu hỏi bằng tiếng Nhật về mua số lượng lớn, agent trả lời theo chính sách tại Nhật ("bảo hành giống khách lẻ"), trong khi đáp án mẫu là chính sách Việt Nam. Như vậy thực chất cả 4 cách đều đạt 100%.
   - Đáng chú ý: agent **tự sửa được các ca dense xếp sai**. Với 20 câu khó, dense top-1 sai nhưng câu trả lời vẫn đúng 95–100%, nhờ agent đọc được nhiều tài liệu trong top-5 hoặc tự tìm thêm.
2. **Khác biệt nằm ở tốc độ và chi phí:**
   - A (chỉ tool) cần trung bình 2.7 lần gọi tool và 4.8 lượt agent, nên chậm nhất.
   - C/D có sẵn tài liệu trong prompt, nên 83–87% câu trả lời được ngay mà không gọi tool.
3. **Vị trí đặt catalog quyết định chi phí.**
   - Ở B/C, catalog (~9k token) nằm chung khối với câu hỏi. Câu hỏi thay đổi mỗi lượt nên catalog bị ghi lại vào cache ở **mỗi lượt**, làm chi phí tăng khoảng 2.5 lần.
   - Ở D, catalog nằm trong system prompt (phần ổn định) nên được đọc từ cache; chi phí **bằng A** trong khi nhanh nhất.
4. Khi có sẵn catalog, agent dùng `kb_get` để đọc tài liệu theo id (B: 39 lần get, chỉ 3 lần search). Khi không có catalog, agent phải search trước (A: 42 lần search, 16 lần grep). **Không cấu hình nào gọi `kb_list`.**
5. **Mức tối thiểu khoảng 6 giây** gồm thời gian khởi động CLI, khởi động MCP server (mỗi lượt nạp lại model embedding, khoảng 2.5s) và 2 lượt model. Trong qobot, MCP server sẽ chạy **lâu dài trong gateway** (streamable HTTP, model đã nạp sẵn), nên bỏ được khoảng 2–3 giây này.

**Giới hạn:**
- Bundle nhỏ: 100 tài liệu ngắn, catalog khoảng 9k token.
- Chỉ 30 câu, mỗi cấu hình chạy một lần.
- Chỉ thử một model (sonnet).
- Với tài liệu dài hoặc bundle lớn, A sẽ cần nhiều lượt gọi tool hơn, nên khoảng cách với D có thể còn lớn hơn.

## Vòng 4b — So sánh với 2 cách "truyền thống" (không dùng qobot)

- **E. Agent + thư mục bundle:** `cwd` là thư mục bundle; agent chỉ dùng tool đọc file có sẵn (Read/Grep/Glob). Không có tool của qobot, không tìm sẵn tài liệu, không đưa catalog vào prompt.
- **F. Toàn bộ bundle trong system prompt, không tool:** 100 tài liệu, khoảng 27k token. Bundle được cache: trung bình mỗi lượt đọc 25k token từ cache và chỉ ghi mới 1.6k.

| Cấu hình | Trả lời đúng | Thời gian p50 / p90 | Lượt agent | Tool call TB | Chi phí TB |
|---|---|---|---|---|---|
| A. Chỉ tool qobot | 97% | 9.5s / 12.3s | 4.8 | 2.7 | $0.017 |
| D. Top-5 + catalog (system) + tool qobot | 97% | 6.2s / 8.4s | 2.2 | 0.2 | $0.017 |
| **E. Agent + thư mục (Read/Grep/Glob)** | 97% | 6.6s / 8.3s | 4.3 | 2.3 (Read 36, Grep 32) | $0.032 |
| **F. Toàn bộ bundle trong prompt** | 97% | **4.1s / 5.0s** | 2.0 | 0 | **$0.014** |

Tổng chi phí vòng 4 + 4b: $4.99.

**Nhận định:**
1. **Độ chính xác vẫn như nhau** (câu sai duy nhất vẫn là q56, lỗi của dữ liệu). Ở quy mô 100 tài liệu ngắn, mọi cách đều trả lời đúng.
2. **F nhanh nhất và rẻ nhất** khi bundle nhỏ: không có vòng gọi tool nào, và bundle được cache.
3. **E vẫn tìm được tài liệu khác ngôn ngữ dù chỉ có grep.** Agent tự viết pattern đa ngôn ngữ, ví dụ `air conditioner|điều hòa|エアコン`. LLM tự bù được nhược điểm của tìm kiếm theo từ khoá. Nhưng E tốn nhiều lượt hơn, đắt gấp khoảng 2 lần D/F, và cần bật tool đọc file.
4. **Chi phí và độ trễ của F tăng tuyến tính theo kích thước bundle** (các ước lượng dưới đây chưa đo):
   - 100–200 concept OKF ngắn tương đương khoảng 50–140k token; 100–200 tài liệu dài tương đương khoảng 400–800k token, vượt khả năng hoặc quá đắt;
   - mỗi lần cache hết hạn (TTL 1 giờ), lượt đầu tiên phải ghi lại toàn bộ bundle vào cache;
   - chưa đo độ chính xác khi context dài và có nhiều tài liệu gần giống nhau.
   
   → F chỉ phù hợp khi bundle nhỏ.
