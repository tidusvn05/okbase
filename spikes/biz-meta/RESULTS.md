# Spike: metadata/tag — cần tool lọc metadata và truy vấn sheet không?

Ngày chạy: 2026-09-30. Claude Code 2.1.284 (`--model sonnet`). Tổng chi phí: $11.84 cho 336 lượt, $4.29 để sinh nội dung, và khoảng $1.5 để chấm.

Dữ liệu đã lưu trong `results/`: `runs.jsonl.gz` (336 lượt), `judge.json`, `report.md` (bảng và danh sách câu sai), cùng log.

Chạy lại:
```
python3 specs.py && python3 build.py
SCALE=20 python3 specs.py && SUF=-x20 python3 build.py
python3 run.py
python3 run.py --suffix=-x20 --configs E,V,QD
python3 report.py
```

## Thiết lập

**Bundle business "Hikari Home"** (`specs.py`, `build.py`):
- 151 tài liệu vi/en/ja, frontmatter OKF đầy đủ: `type, title, description, tags` (một nửa dạng inline `[a, b]`, một nửa dạng block list), `status, lang, department, updated`, cùng `region, customer, version, supersedes, effective_from/to, contract_value, currency, model, meeting_date`.
- **Chính sách có nhiều phiên bản:** v1 deprecated, v2 stable, v3 draft.
- 3 sheet dạng CSV (`data/`), và bản bảng markdown trong `sources/sheets/` (như khi import).
- Nội dung do LLM viết từ spec; mọi con số trong spec đều được kiểm tra có mặt trong văn bản.

**×20:** thêm 2.869 tài liệu cùng schema (hợp đồng với 200 khách hàng, biên bản họp, SOP của 40 chi nhánh, chính sách TH/SG/KR…), tổng 3.020 tài liệu. Sheet lớn hơn: inventory 4.800 dòng, sales 9.806 dòng, price 1.200 dòng.

**48 câu hỏi** (vi 18, en 17, ja 13). **Đáp án tính bằng code** từ spec, không phụ thuộc LLM:

| Nhóm | Số câu | Ví dụ |
|---|---|---|
| list | 8 | "hợp đồng với Sakura Trading còn hiệu lực", "SOP kho đang draft", "tài liệu tag security cập nhật 2026" |
| version | 8 | "theo chính sách đổi trả **hiện hành** ở Nhật, trả trong bao nhiêu ngày" (có bản cũ và bản nháp với giá trị khác) |
| count | 8 | "bao nhiêu hợp đồng hết hạn trong 2026", "tổng giá trị hợp đồng ACME còn hiệu lực" |
| facet | 6 | "phòng ban nào có tài liệu tag privacy", "khách hàng nào có nhiều hợp đồng hiệu lực nhất" |
| sheet | 10 | "tổng tồn kho máy lọc khí ở kho HCM", "tháng nào JP có doanh thu cao nhất" |
| content | 8 | Câu hỏi nội dung bình thường (nhóm đối chứng) |

**Các cách so sánh** (`run.py`). Mọi cách đều có Read/Grep/Glob trên thư mục bundle:
- **E:** chỉ tổ chức thư mục + `index.md`.
- **V:** E + `_views/` sinh tự động (trang bảng theo tag, type, status, department, lang, customer, `contracts-all`, `recent`).
- **Q:** E + MCP `kb_query` (lọc frontmatter kết hợp nhiều điều kiện, `active_on`, khoảng ngày, facets, sum).
- **QD:** Q + MCP `data_tables` / `data_query` (SQL chỉ đọc trên sheet đã nạp vào SQLite).

**Chấm điểm:**
- Nhóm list: so tập id tự động (đúng khi trùng khớp hoàn toàn), kèm precision/recall.
- Các nhóm khác: LLM judge so với đáp án tính sẵn, mù cấu hình.

## Kết quả

| Nhóm | E | V | Q | QD | **E ×20** | **V ×20** | **QD ×20** |
|---|---|---|---|---|---|---|---|
| list (8) | 8 | 8 | 8 | 8 | 7 (recall 89%) | 8 | 8 |
| version (8) | 8 | 8 | 8 | 8 | 8 | 8 | 8 |
| count (8) | 8 | 8 | 8 | 8 | 8 | 8 | 8 |
| facet (6) | 6 | 6 | 6 | 6 | 4 | 5 | 5 |
| sheet (10) | 10 | 9 | 10 | 10 | 9 | 8 | **10** |
| content (8) | 8 | 8 | 8 | 8 | 8 | 8 | 7 |
| **Tổng** | **100%** | 98% | **100%** | **100%** | 92% | 94% | **96%** |
| Chi phí TB | $0.024 | $0.026 | $0.022 | **$0.017** | $0.056 | $0.070 | **$0.032** |
| p50 / p90 | 7.9 / 10.8s | 7.4 / 10.6s | 7.5 / 11.7s | **6.7 / 8.6s** | 10.3 / 26.4s | 9.6 / 36.6s | **7.3 / 13.9s** |
| Token vào TB | 18k | 18k | 20k | 20k | 33k | **44k** | 25k |

**Chi phí và p50 / p90 theo nhóm ở ×20:**

| Nhóm | E | V | QD |
|---|---|---|---|
| list | $0.063 · 15 / 26s | $0.093 · 16 / 31s | $0.063 · 14 / 23s |
| count | $0.054 · 11 / 20s | $0.067 · 10 / 22s | **$0.037 · 8 / 10s** |
| facet | $0.056 · 12 / 24s | $0.083 · 9 / 22s | **$0.046 · 10 / 17s** |
| sheet | $0.105 · 22 / **60s** | $0.122 · 37 / **60s** | **$0.012 · 6 / 7s** |
| version / content | ≈ $0.022 · 7s | ≈ $0.022 · 8s | ≈ $0.020 · 7s |

**Agent có tự dùng tool không?**
- `data_query`: dùng ở 10/10 câu sheet (cả ×1 và ×20).
- `kb_query`: ×1 dùng ở 63/96 lượt (Q và QD); ×20 dùng ở 32/48 lượt.
- `_views/`: ×1 dùng ở 17/48 câu; ×20 dùng ở 15/48 câu.

**Các câu sai cần lưu ý:**
- q29 (×20) sai ở **cả 3 cách**. Lỗi nằm ở bộ dữ liệu: các chính sách "nhiễu" TH/SG/KR có status draft nên lọt vào đáp án đúng, dù không phải "phiên bản mới chờ duyệt".
- q41 (QD×20): 40 SOP chi nhánh trùng tên với SOP gốc, tức là câu hỏi trở nên mơ hồ.
- q15 bị judge chấm sai ở lần đầu và đã được chấm lại.

## Nhận định

1. **Ở quy mô nhỏ (151 tài liệu, sheet ≤ 500 dòng), giả thuyết "grep không xử lý tốt tag" không đúng.** E đạt 100% ở mọi nhóm: agent tự grep frontmatter (`^(status|effective_to|customer):`), tự lấy giao, và tự cộng vài chục số. Tool chỉ giúp **rẻ hơn**: `kb_query` rẻ hơn khoảng 20% ở list/count/facet; `data_query` rẻ hơn 64% ở câu sheet.
2. **Ở ×20 (3.020 tài liệu, sheet khoảng 10k dòng), giả thuyết đúng một phần:**
   - **Sheet là điểm gãy rõ nhất.** E và V mất 22–37s (p90 60s), tốn $0.10–0.12 mỗi câu, và có câu **bỏ cuộc**: agent tự nói không có công cụ tính để cộng 4.900 dòng. QD đạt 10/10, $0.012, 6s, tức **rẻ hơn khoảng 9 lần và nhanh hơn khoảng 4 lần**.
   - **List bị thiếu khi tập kết quả lớn.** E bỏ sót 41/45 SOP ở một câu (recall 89%). V và QD đạt 100%.
   - **Count/facet:** độ chính xác tương đương, nhưng QD rẻ hơn 30–45% và p90 nhanh hơn khoảng 2 lần.
   - Câu version và content thì không có khác biệt giữa các cách.
3. **View files (`_views/`) không mở rộng được.** Ở ×20, mỗi trang view là một bảng hàng trăm dòng, nên V **tốn token nhất** (44k vào mỗi lượt, $0.070) và chậm nhất ở p90. Agent cũng chỉ mở view ở khoảng 1/3 số câu. Views đầy đủ không nên làm mặc định; chỉ nên giữ một trang nhỏ liệt kê từ vựng tag kèm số lượng.
4. **`kb_query` có giá trị, nhưng agent không luôn dùng** (32/48 ở ×20): đôi khi agent vẫn grep. Có thể cải thiện bằng mô tả tool rõ hơn, hoặc bằng catalog chứa danh sách tag và facet để agent biết khi nào nên lọc.
5. **Tổ chức vẫn là nền tảng.** q41 cho thấy khi có nhiều biến thể cùng tên (SOP chi nhánh), câu hỏi trở nên mơ hồ. Metadata tốt (`branch`, `scope`) cộng với lọc theo metadata là cách phân biệt chúng.

## Kết luận cho qobot
- **`data_query` (sheet → SQLite, SQL chỉ đọc): bắt buộc.** Đây là lợi ích lớn nhất, và không có cách thay thế bằng tổ chức thư mục.
- **`kb_query` (lọc frontmatter, facets, sum, `active_on`): nên có**, và cùng bộ tham số `filter` cho `kb_search`/`kb_grep`. Lợi ích tăng theo kích thước bundle: đầy đủ kết quả với list lớn, rẻ hơn và nhanh hơn với count/facet.
- **Views:** chỉ sinh trang từ vựng tag và facet (nhỏ), không sinh bảng đầy đủ.
- **Giới hạn:** dữ liệu tổng hợp; nội dung của tài liệu ×20 sinh bằng template; mỗi cấu hình chạy một lần; 48 câu.
