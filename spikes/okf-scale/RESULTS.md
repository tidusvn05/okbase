# Spike: bundle OKF lớn (docs OpenClaw) — so sánh cách cho agent dùng knowledge

Dữ liệu đã lưu trong `results/`:
- `runs.jsonl.gz`: 780 bản ghi, gồm câu trả lời, nguồn, tool call, usage và chi phí
- `judge.json`: điểm chấm
- `report.md`: bảng kết quả và danh sách câu sai
- `summary.json`
- `*.retrieval.json`: top-6 chunk cho từng câu hỏi
- `gold.json`, `stats.json`, và các log

Bundle, chỉ mục và cache embedding **không lưu**; dựng lại bằng `build_bundles.py` và `embed-bench bundle-index`.

Ngày chạy: 2026-09-30. Claude Code 2.1.284, `--model sonnet`, 8 vCPU. Tổng chi phí: $18.29 cho 780 lượt, $3.75 để sinh câu hỏi, và khoảng $4 để chấm.

## Thiết lập

**Bundle.** Docs OpenClaw chuyển sang OKF (`build_bundles.py`): `summary` thành `description`, `type` suy ra từ thư mục, giữ nguyên key lạ, và **mỗi thư mục có một `index.md`**. Các bundle lồng nhau: S ⊂ M ⊂ L ⊂ XL.

| Bundle | File | Token | Chunk (H2/H3, 150–450 tok) | Catalog phẳng |
|---|---|---|---|---|
| S | 28 | ~60k | 201 | ~0.9k tok |
| M | 62 | ~150k | 510 | ~2k tok |
| L | 287 | ~1M | 3.171 | ~9k tok |
| XL | 1.270 | ~4.4M | 12.517 | ~44k tok (không đưa vào prompt; XL chỉ dùng `index.md` gốc) |

**Câu hỏi** (`gen_questions.py`): 30 câu (vi 10, en 10, ja 10) sinh từ 15 tài liệu "gold" có mặt trong mọi bundle. Mỗi câu hỏi vào một chi tiết cụ thể (giá trị mặc định, config key, flag CLI, giới hạn) và kèm 1–3 **key facts**.
- Docs viết bằng tiếng Anh, nên câu hỏi vi/ja là trường hợp hỏi khác ngôn ngữ.
- Câu tiếng Việt phần lớn không dấu.

**Chấm điểm:** LLM judge (sonnet), mù cấu hình. Một câu chỉ được tính **đúng** khi câu trả lời có đủ mọi key fact và không mâu thuẫn với tài liệu. Đây là tiêu chí khắt khe.

**Các cách so sánh** (`run.py`):

| | Cách | Knowledge trong prompt | Tool |
|---|---|---|---|
| F | Full-context | Toàn bộ bundle (system prompt, cache) | Không |
| D | qobot | Top-6 chunk (EmbeddingGemma Q4) + catalog trong system prompt | MCP: kb_search / kb_get(section) / kb_grep / kb_list |
| I | qobot không catalog | Top-6 chunk | MCP (như D) |
| A | Chỉ tool | — | MCP (như D) |
| E | Agent + thư mục | — | Read/Grep/Glob có sẵn của CLI (cwd = bundle) |
| H | RAG thuần | Top-6 chunk | Không |
| G | Duyệt cây index.md | `index.md` gốc | MCP kb_get / kb_grep / kb_list (**không embedding**) |

## Kết quả

| Bundle | Cách | Đúng | Facts | p50 / p90 | Lượt | Tool call | Token vào | Chi phí |
|---|---|---|---|---|---|---|---|---|
| S 60k | F full | **100%** | 100% | 4.6 / 6.6s | 2.0 | 0 | 89k | $0.034 |
| | D qobot | 97% | 99% | 6.7 / 8.8s | 2.2 | 0.1 | 10k | $0.020 |
| | I | 100% | 100% | 6.8 / 8.8s | 2.3 | 0.3 | 9k | $0.020 |
| | A tools | 100% | 100% | 10.2 / 11.3s | 5.5 | 3.5 | 17k | $0.021 |
| | E folder | 100% | 100% | 7.1 / 10.1s | 4.1 | 2.1 | 18k | $0.019 |
| | H RAG | 97% | 97% | **4.3 / 5.0s** | 2.0 | 0 | 6k | $0.018 |
| | G tree | 97% | 98% | 7.0 / 9.2s | 4.9 | 2.9 | 16k | $0.019 |
| M 150k | F full | 100% | 100% | 4.6 / 11.7s | 2.0 | 0 | **220k** | **$0.104** |
| | D qobot | 100% | 98% | 6.8 / 9.5s | 2.1 | 0.1 | 11k | $0.018 |
| | I | 100% | 100% | 6.8 / 9.3s | 2.1 | 0.1 | 8k | $0.016 |
| | A tools | 97% | 98% | 10.2 / 13.1s | 5.5 | 3.5 | 18k | $0.022 |
| | E folder | 100% | 100% | 7.4 / 9.8s | 4.2 | 2.2 | 18k | $0.018 |
| | H RAG | 97% | 95% | 4.2 / 5.4s | 2.0 | 0 | 6k | $0.016 |
| | G tree | 93% | 97% | 6.9 / 8.7s | 5.0 | 3.0 | 16k | $0.019 |
| L 1M | D qobot | **100%** | 99% | 7.1 / 16.2s | 2.2 | 0.2 | 24k | $0.023 |
| | I | 93% | 97% | 7.1 / 14.9s | 2.2 | 0.2 | 8k | $0.018 |
| | A tools | 90% | 96% | 10.3 / 19.4s | 5.1 | 3.1 | 17k | $0.019 |
| | E folder | 97% | 99% | 6.9 / 8.5s | 4.4 | 2.4 | 19k | $0.022 |
| | H RAG | 97% | 97% | 4.7 / 7.8s | 2.0 | 0 | 6k | $0.017 |
| | G tree | 90% | 96% | 7.2 / 9.4s | 4.8 | 2.8 | 18k | $0.024 |
| XL 4.4M | D qobot | 93% | 97% | 6.9 / 9.2s | 2.0 | **0.0** | 8k | $0.018 |
| | I | 93% | 98% | 6.9 / 8.8s | 2.0 | 0.0 | 7k | $0.016 |
| | A tools | 90% | 94% | 10.2 / 12.8s | 5.1 | 3.1 | 17k | $0.021 |
| | E folder | 93% | 98% | 7.0 / 9.9s | 4.7 | 2.7 | 22k | $0.026 |
| | H RAG | 90% | 94% | 4.7 / 5.4s | 2.0 | 0 | 6k | $0.016 |
| | G tree | 90% | 95% | 7.1 / 9.0s | 4.6 | 2.6 | 21k | $0.028 |

**Theo ngôn ngữ câu hỏi (gộp mọi bundle):**

| Cách | vi | en | ja |
|---|---|---|---|
| F | 100% | 100% | 100% |
| D | 98% | 95% | 100% |
| I | 98% | 92% | 100% |
| A | 88% | 95% | 100% |
| E | 100% | 92% | 100% |
| H | 98% | 88% | 100% |
| G | 85% | 92% | 100% |

**Retrieval thuần** (EmbeddingGemma Q4, tài liệu gold trong top-6 chunk):
- S 30/30, M 30/30, L 30/30, XL 26/30;
- tài liệu gold đứng đầu: 30 / 28 / 23 / 22.

**Index:**
- Tốc độ index: XL 12.5k chunk mất **64 phút** (3.2 chunk/s, chunk trung bình khoảng 300 token, có CPU bị chia cho các job khác).
- Sắp xếp chunk theo độ dài trước khi chia batch: **nhanh hơn 1.45 lần**.
- L và M dùng lại cache embedding của XL (hash nội dung), nên index lại mất 0–2 giây.

## Nhận định

1. **Về chất lượng, mọi cách đều đạt 90–100% ở mọi quy mô.** Với n = 30, chênh 1 câu là 3.3 điểm, nên phần lớn khác biệt nằm trong mức nhiễu.
   - Các câu sai ở L/XL **giống nhau giữa các cách** (q22, q18, q9). Nguyên nhân là trong corpus lớn, cùng một thông tin xuất hiện ở nhiều tài liệu với mức đầy đủ khác nhau. Agent trả lời từ tài liệu "gần đúng" và thiếu một key fact.
   - Đây là giới hạn của dữ liệu, không riêng cách nào. Giải pháp nằm ở chất lượng knowledge (khử trùng lặp, liên kết giữa các tài liệu), không nằm ở retrieval.
2. **F (full-context) chỉ hợp với bundle nhỏ.** Ở S (60k token), F đúng 100% và nhanh (4.6s) nhưng **đắt hơn D khoảng 1.7 lần**. Ở M (150k), mỗi lượt tốn 220k token vào và $0.10, gấp khoảng 6 lần D. L và XL thì không thể dùng F.
3. **H (RAG thuần) nhanh nhất** (4.2–4.7s) và rẻ nhất, nhưng không có đường sửa sai. Khi retrieval trượt thì chỉ biết trả lời "không biết" hoặc trả lời thiếu (XL 90%, en 88%).
4. **D (qobot) ổn định nhất khi bundle lớn:** L 100%, XL 93%, khoảng 2 lượt agent, $0.018–0.023.
   - Trong đo đạc, D mất khoảng 7s. Khoảng 2.5s trong số đó là do MCP server nạp lại model mỗi lượt. Khi MCP chạy lâu dài trong gateway, D ước tính còn khoảng 4.5–5s, ngang với H.
   - **Điểm yếu:** khi đã có top-6 trong prompt, agent **hầu như không kiểm tra lại** (XL: 0 tool call), kể cả khi tài liệu gold không nằm trong top-6 (4/30 câu).
5. **E (agent + thư mục, Read/Grep/Glob) mạnh ngang D ở mọi quy mô** (100 / 100 / 97 / 93%), **không cần index**.
   - Agent tự viết regex đa ngôn ngữ và đọc `index.md`.
   - Nhược điểm: gấp đôi số lượt, và phải mở quyền đọc file cho agent.
6. **G (duyệt cây index.md + get/grep qua MCP, không embedding)** thấp hơn E khoảng 3–7 điểm, và yếu nhất với câu tiếng Việt không dấu (85%).
   - Khác biệt so với E nằm ở **công cụ lexical**: `kb_grep` hiện chỉ tìm chuỗi con đơn giản, còn Grep của CLI hỗ trợ regex, alternation, glob và context.
   - Nâng `kb_grep` lên ngang Grep thì G có thể đạt mức của E mà **không cần mở quyền đọc file và không cần embedding**.
7. **A (chỉ tool) luôn chậm nhất** (≈10s, 5 lượt) và không chính xác hơn các cách khác. Không có lý do chọn A.
8. **Catalog trong prompt:** ở L, D (có catalog, 100%) tốt hơn I (không catalog, 93%), nhưng tốn thêm khoảng 15k token (được cache, +$0.005/lượt). Ở S, M và XL không có khác biệt. Nên giữ catalog khi nó ≤ ~10k token.
9. **Tiếng Nhật đạt 100% ở mọi cách.** Tiếng Việt không dấu làm giảm các cách dựa vào từ khoá (A 88%, G 85%).

## Giới hạn
- 30 câu, mỗi cấu hình chạy một lần, chỉ dùng model sonnet.
- Câu hỏi do LLM sinh từ đúng tài liệu gold, nên có thể dễ hơn câu hỏi thật.
- Tiêu chí chấm khắt khe (phải đủ mọi key fact).
- Latency có lẫn thời gian MCP nạp model mỗi lượt (D, I, A); E, F, H không có phần này.

---

# Vòng 2 — Nâng cấp `kb_grep` và quy tắc kiểm tra

**Thay đổi:**
- **`kb_grep` v2** (`embed-bench/src/bundle.rs`):
  - pattern là regex, hỗ trợ alternation (`a|b`);
  - không phân biệt hoa thường và dấu (NFKC, bỏ dấu tiếng Việt);
  - tìm cả trong frontmatter;
  - `path` (glob hoặc prefix), `context` (dòng xung quanh), `files_only` (xếp tài liệu theo số lần khớp), `limit`;
  - khi không khớp, gợi ý thử đồng nghĩa hoặc thuật ngữ tiếng Anh.
- **G2:** như G (index.md gốc + kb_list/kb_get/kb_grep, **không embedding**), dùng `kb_grep` v2 và hướng dẫn chiến lược: dịch từ khoá sang tiếng Anh, alternation các đồng nghĩa, `files_only` trước rồi `kb_get(section)`, đủ mọi ý của câu hỏi.
- **D2:** như D, thêm **quy tắc kiểm tra** trong system prompt: config key, giá trị mặc định, lệnh phải có nguyên văn trong văn bản đã đọc; nếu đoạn trích chưa đủ hoặc câu hỏi nhiều ý thì dùng tool.
- Cờ `low_confidence` theo điểm tương đồng **bị loại trước khi chạy**: phân bố điểm top-1 của các ca trượt (0.64–0.73 ở XL) nằm lẫn hoàn toàn với các ca đúng (0.54–0.77). Margin top1−top6 cũng không tách được hai nhóm.

**Kết quả** (240 lượt mới, $4.76; tổng cả spike 1.020 lượt, $23.05):

| Bundle | D | **D2** (+ kiểm tra) | E (Read/Grep/Glob) | G | **G2** (grep v2) |
|---|---|---|---|---|---|
| S 60k | 97% | 100% | 100% | 97% | **100%** |
| M 150k | 100% | 100% | 100% | 93% | **100%** |
| L 1M | 100% | 97% | 97% | 90% | **93%** |
| XL 4.4M | 93% | 87% | 93% | 90% | **90%** (facts 97%) |
| Tiếng Việt (gộp) | 98% | 98% | 100% | 85% | **95%** |
| Tool call TB (XL) | 0.0 | 0.2 | 2.7 | 2.6 | 2.4 |
| Chi phí TB | $0.018–0.023 | $0.019–0.023 | $0.018–0.026 | $0.019–0.028 | **$0.017–0.022** |

**Nhận định:**
1. **`kb_grep` v2 có tác dụng.** G2 vượt G ở S, M và L (+3 đến +7 điểm), và **câu tiếng Việt tăng từ 85% lên 95%** nhờ khớp không dấu. G2 gần bằng E (chênh ≤ 1 câu ở mọi quy mô) mà **không cần mở quyền đọc file và không cần embedding**. Chế độ `lexical` như vậy là khả thi.
2. **Quy tắc kiểm tra trong prompt không thay đổi hành vi.** D2 chỉ tăng tool call từ 0 lên 0.2 ở XL, và không cải thiện độ chính xác (XL 93% xuống 87%, trong mức nhiễu). Khi đã có đoạn trích "trông có vẻ đúng", agent tin vào đó; chỉ nhắc bằng prompt thì không đủ.
3. **Các lỗi còn lại chủ yếu là trả lời thiếu một key fact, và giống nhau giữa các cách.** q22 sai ở mọi cách từ L trở lên; q28 và q9 cũng lặp lại. Thông tin nằm rải ở nhiều tài liệu, nên đây là vấn đề chất lượng knowledge (trùng lặp, thiếu trang "canonical"), không phải vấn đề retrieval.
4. **Embedding đổi lại được gì so với G2?** Chất lượng tương đương, chi phí tương đương. Khác biệt là **số lượt agent**: D mất 2 lượt, G2 mất khoảng 4.5 lượt. Khi MCP chạy lâu dài (không nạp lại model mỗi lượt), ước tính D ≈ 4.5s còn G2 ≈ 6.5s. Cái giá của embedding là index XL mất khoảng 1 giờ CPU và khoảng 0.5GB RAM.
5. Với n = 30, chênh 1 câu là 3.3 điểm, nên mọi khác biệt ≤ 2 câu giữa D, D2, E, G2 **không có ý nghĩa thống kê**.
