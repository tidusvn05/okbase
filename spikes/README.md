# Spikes

| Spike | Câu hỏi | Kết luận chính | Chi tiết |
|---|---|---|---|
| `embed-bench` vòng 1–2 | Model embedding nào cho vi/en/ja? | **EmbeddingGemma-300M Q4** (188MB, ~0.5GB RAM, R@1 0.85 / R@3 0.96). Dự phòng: bge-m3 int8 (MIT). Họ e5 yếu khi hỏi khác ngôn ngữ; Qwen3 và nomic quá chậm trên CPU | `embed-bench/RESULTS.md`, `results/v1`, `results/v2` |
| `embed-bench` vòng 3 | Ngưỡng BM25? Số chunk mỗi file? | **Không trộn BM25 vào xếp hạng** (gain 0–1 điểm). Concept OKF ngắn ≈ 1.4–2.2 chunk/file; docs dài ≈ 10 chunk/file | `embed-bench/RESULTS.md`, `results/gate-*.log` |
| `embed-bench` vòng 4/4b | Catalog hay tool? Hay cách truyền thống? (100 tài liệu ngắn) | Độ chính xác như nhau (~97%). **Catalog phải nằm trong system prompt** (nếu không, chi phí ×2.5). Bundle nhỏ thì nhồi cả bundle (full-context) nhanh và rẻ nhất | `embed-bench/results/e2e/` |
| `okf-scale` vòng 1 | Bundle lớn (docs OpenClaw 60k → 4.4M token), 7 cách | Mọi cách đạt 90–100%. `full` chỉ dùng khi ≤ ~30k token. **D (top-6 + catalog + tool)** ổn định nhất ở L/XL. E (agent + grep thư mục) mạnh ngang D mà không cần index | `okf-scale/RESULTS.md`, `okf-scale/results/` |
| `okf-scale` vòng 2 | Nâng cấp `kb_grep`; nhắc agent kiểm tra lại | **G2 lexical (không embedding) = 100 / 100 / 93 / 90%**, ngang E/D, câu tiếng Việt từ 85% lên 95%. Quy tắc kiểm tra bằng prompt và cờ độ tin cậy theo điểm **không hiệu quả** | như trên |
| `biz-meta` | Tag/metadata: tổ chức thư mục là đủ hay cần tool? Sheet? (×1 = 151 tài liệu, ×20 = 3.020 tài liệu / sheet ~10k dòng) | ×1: agent tự grep frontmatter, đạt 100%; tool chỉ giảm chi phí. ×20: **`data_query` bắt buộc** (sheet: 10/10, rẻ hơn 9×, nhanh hơn 4×; không có tool thì agent bỏ cuộc). **`kb_query`** giữ list đầy đủ, count/facet rẻ hơn 30–45%. View files không mở rộng được | `biz-meta/RESULTS.md`, `biz-meta/results/` |

Các quyết định rút ra đã được ghi vào `../docs/PLAN.md` (okfkit, §1 bảng S1–S6 và §5). Spike được chuyển từ repo qobot sang đây ngày 2026-10-01; các bản ghi cũ trong `results/` có thể còn chứa đường dẫn `qobot/spikes/...`.

Những gì **không lưu** (dựng lại được): model và cache (`.fastembed_cache`, `.models`, `.embcache`), corpus (`.corpora`, `.okf-samples`), bundle OKF và chỉ mục (`okf-scale/bundles`), prompt của từng lượt chạy.

## Ghi chú về dữ liệu (mã nguồn mở)
- Bundle business (`biz-meta`) và bộ câu hỏi đa ngôn ngữ (`embed-bench/data`) là **dữ liệu tổng hợp**, nội dung do LLM viết hoặc sinh bằng template. Không phải dữ liệu thật của công ty nào.
- Câu hỏi trong `okf-scale/questions.json` được LLM sinh từ docs OpenClaw (MIT). Corpus gốc không được commit; `okf-scale/build_bundles.py` tải và dựng lại.
- Sample OKF của Google (Apache-2.0) được clone về `.okf-samples/` khi chạy, không commit.
- `results/*.jsonl.gz` chứa đường dẫn trên máy chạy spike và câu trả lời do LLM sinh; chỉ dùng để phân tích.
