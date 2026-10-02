# Spikes

Thí nghiệm và benchmark đứng sau mọi mặc định của okfkit. Không đổi mặc định khi chưa có số liệu
mới. Mỗi spike là một thư mục, có `RESULTS.md` là nguồn sự thật. File này là mục lục và bảng số
liệu tra nhanh.

## 1. Mục lục

ID theo `docs/PLAN.md` §1. Ngày là ngày chạy.

| ID | Ngày | Thư mục | Câu hỏi | Kết luận chính | Quyết định ghi ở |
|---|---|---|---|---|---|
| S1 | 2026-09-30 | `embed-bench` vòng 1–2 | Model embedding nào cho vi/en/ja? | **EmbeddingGemma-300M Q4** (188 MB, ~0.5 GB RAM, R@1 0.85 / R@3 0.96). Dự phòng: bge-m3 int8 (MIT). Họ e5 yếu khi hỏi khác ngôn ngữ; Qwen3 và nomic quá chậm trên CPU | PLAN §1, §5 |
| S2 | 2026-09-30 | `embed-bench` vòng 3 | Ngưỡng BM25? Số chunk mỗi file? | **Không trộn BM25 vào xếp hạng** (lợi 0–1 điểm). Concept OKF ngắn ≈ 1.4–2.2 chunk/file; docs dài ≈ 10 chunk/file | PLAN §1; AGENTS.md (luật 4) |
| S3 | 2026-09-30 | `embed-bench` vòng 4/4b | Catalog hay tool? (100 tài liệu ngắn) | Chính xác như nhau (~97%). **Catalog phải nằm trong system prompt** (nếu không, chi phí ×2.5). Bundle nhỏ thì nạp cả bundle là nhanh và rẻ nhất | PLAN §1 |
| S4 | 2026-09-30 | `okf-scale` vòng 1–2 | Bundle lớn (docs OpenClaw 60k → 4.4M token): cách nào? | **G2 lexical (không embedding) 100/100/93/90%**, ngang embedding (D) và agent + grep (E). Câu tiếng Việt 85 → 95% nhờ `kb_grep` mạnh hơn. Nhắc agent "kiểm tra lại" bằng prompt **không hiệu quả** | PLAN §1, §5 |
| S5 | 2026-09-30 | `biz-meta` | Metadata/tag và sheet: cần tool không? (151 → 3.020 tài liệu) | ×1: agent grep frontmatter là đủ. ×20: **`data_query` bắt buộc** (sheet 10/10, rẻ hơn 9×); `kb_query` rẻ hơn 30–45%. View file không mở rộng được | PLAN §1, §5 |
| S6 | 2026-09-30 | `embed-bench` | Tốc độ chunk và index | ~3 chunk/s trên 8 CPU; 4.4M token ≈ 64 phút. Embedding chạy nền, lexical sẵn sàng ngay | PLAN §1, §11 |
| v0.1 | 2026-10-01 | `acceptance-v0.1` | okfkit thật có tái hiện G2 của S4 không? | **28/30 (93%)**, đạt mục tiêu. Skill không tăng độ chính xác ở bundle tra cứu, chi phí +60% | HANDOFF T11 |
| S8, S9 (v0.2) | 2026-10-01 | `acceptance-v0.2` | S5 ×20 với okfkit; adopt có giảm độ chính xác không; skill có giúp không | K 44/48, KS 45/48; sheet 9–10/10. Adopt không làm giảm. **S9 chưa kết luận được**: agent không gọi skill lần nào | PLAN §13 |
| v0.3 | 2026-10-01 | `acceptance-v0.3` | Retrieval qua đường thật của okfkit | S1 R@1 **0.857** (Gemma Q4), 0.750 (bge-m3); S4 top-6 29/30 và 30/30. Đã sửa ước lượng token (S4 26 → 29/30) | PLAN §13 |
| S10-lite | 2026-10-02 | `import-bench` | Import PDF/Office/HTML nhẹ và chính xác? | **anydoc + htmd**: chính xác với Office và PDF chữ (vi/ja); binary thêm ~10 MB; 0.3–10 ms mỗi tài liệu nhỏ. Còn yếu: hai cột giữa trang, bảng HTML không có `<th>`, trang scan | `docs/PLAN-import.md` |
| I5 | 2026-10-02 | `import-bench` | Agent thật trên thư mục PDF/DOCX/HTML hỗn hợp | Trả lời 3/3, trích dẫn file và trang. Hỏi consent trước khi OCR; tự thêm cờ consent 0 lần | `docs/PLAN-import.md` |
| S11 | 2026-10-01 → 02 | `embed-tune` | Fine-tune EmbeddingGemma (LoRA, cách của Unsloth) có lợi không? | Bản fp32: R@1 0.853 → 0.943. **Bản Q4 trong okfkit: 0.857 → 0.917**, S4 30/30. Quy trình agent + CLI đạt (held-out 0.875 → 0.925) | `docs/PLAN-advise-tune.md` |
| S13 | 2026-10-02 | `onboarding` | Agent tự cài okfkit chỉ từ một câu? | Claude 3/3 hoàn tất (35–51 s, ~50k token); tự thêm cờ consent 0 lần. **Codex chưa chạy** | `docs/PLAN-onboarding.md` |
| S13b | 2026-10-02 | `onboarding` | Thư mục thực tế: repo phần mềm, thư mục rỗng, bundle hỏng một phần | 3/3 hiểu đúng thư mục và hỏi đúng chỗ. Agent tìm ra 5 lỗi, đã sửa | `docs/PLAN-usecases.md` |
| S14 | 2026-10-02 | `page-image-bench` | Đưa trang scan cho agent: xuất ảnh nhúng hay render trang? | **Xuất ảnh nhúng**: JPEG 1–3 ms (render 0.2–0.95 s); Flate không mất chi tiết và nhanh hơn render 2–4×. Token ảnh như nhau (~1.5k). CCITT G4: 86 ms, chính xác (crate `fax`) | `docs/PLAN-import.md` §7 |
| S15 | 2026-10-03 | `mcp-tools` | Rút gọn danh sách tool có giảm chi phí không? Quy tắc của skill gửi qua instructions của MCP server có thay được gợi ý trong prompt không? | **Instructions thay được gợi ý**: không gợi ý vẫn 46/48 (bằng có gợi ý), dùng `kb_query`/`data_query` như nhau. Đây là lời giải cho S9. Rút gọn tool: −3% token, chi phí không đổi, độ chính xác không đổi. $6.29 | `PLAN.md` §13–14 |

## 2. Số liệu tra nhanh

Máy chạy: AMD EPYC 8 vCPU, 23 GB RAM, không GPU; bản release. Chi tiết và điều kiện đo xem cột
"Nguồn".

| Hạng mục | Số liệu | Nguồn |
|---|---|---|
| Chất lượng retrieval (S1, 300 câu vi/en/ja) | Gemma Q4 R@1 0.857; tune Q4 0.917; bge-m3 int8 0.750 | `acceptance-v0.3`, `embed-tune` |
| Bundle lớn, lexical (S4, 30 câu) | 93% (28/30) với Claude Sonnet, ~$0.029/câu, 4.6 lượt | `acceptance-v0.1` |
| Embed cả bundle | ~3 chunk/s (8 CPU); 4.4M token ≈ 64 phút | `embed-bench` |
| Fine-tune (CPU) | 320 cặp × 2 epoch: 627 s; export 20 s; eval 70 s; venv 1.7 GB | `embed-tune` |
| Chuyển tài liệu | 0.3–10 ms/tài liệu nhỏ; 172 ms cho bài báo 15 trang | `import-bench` |
| Xuất ảnh trang scan | JPEG 1–3 ms; Flate 50–90 ms (A4 300 dpi); render PDFium 150 dpi ~0.2 s, 300 dpi ~0.6–0.95 s | `page-image-bench` |
| Kích thước binary | anydoc +~10 MB; htmd +1.5 MB; page image +0 crate | `import-bench`, `page-image-bench` |
| Chi phí mỗi câu (biz ×20, Claude Sonnet, Claude Code 2.1.284) | ~$0.043; ~34k token đầu vào, 4.1–4.2 lượt | `mcp-tools` |
| Agent tự cài đặt | 35–51 s, 51–56k token mỗi lần (Claude) | `onboarding` |
| Chi phí sinh câu hỏi để tune | ~$3 cho ~1k cặp (Claude Sonnet) | `embed-tune` |

## 3. Chưa làm (cần người dùng đồng ý hoặc dữ liệu)

| ID | Việc | Vì sao chưa |
|---|---|---|
| S7 | Lexical với Codex và model nhỏ | Tốn quota của người dùng |
| S10 | ~20 PDF và 5 sheet thật | Cần tài liệu thật của người dùng |
| S12 | Codex sinh câu hỏi để tune | Tốn quota |
| S13 (Codex) | Codex tự cài okfkit | Tốn quota |
| S14 tiếp | Trang ghép nhiều ảnh; file scan thật | Chưa bắt đầu (CCITT G4 đã xong) |

## 4. Quy ước cho spike mới

- **Thư mục:** `spikes/<tên>/`.
  - `RESULTS.md`: câu hỏi, thiết lập (máy, phiên bản, mẫu), kết quả, nhận định, phần chưa đo, cách
    chạy lại.
  - Script để dựng dữ liệu và chạy.
  - `results/`: kết quả nhỏ, máy đọc được (`.json`, `.jsonl.gz`).
- **Không commit:** dữ liệu dựng lại được, model, cache, file sinh ra. Ghi chúng trong `.gitignore`
  của thư mục spike hoặc của repo.
- **Ngôn ngữ:** spike mới viết `RESULTS.md` bằng tiếng Anh, vì repo sẽ mở mã nguồn. Các spike cũ
  bằng tiếng Việt giữ nguyên.
- **Mục lục:** thêm một dòng vào bảng §1. Nếu có số liệu đo về thời gian, kích thước hay chi phí,
  thêm vào §2.
- **Quyết định:** ghi kết luận vào plan tương ứng trong `docs/`, kèm ID spike.
- **Chi phí:** ghi số tiền và lượt chạy khi spike dùng agent thật. Không chạy agent tốn quota khi
  người dùng chưa đồng ý.

## 5. Những gì không lưu (dựng lại được)

- Model và cache: `.fastembed_cache`, `.models`, `.embcache`, `embed-tune/models`.
- Corpus: `.corpora`, `.okf-samples`.
- Bundle OKF và chỉ mục: `okf-scale/bundles`.
- Mẫu tài liệu: `import-bench/samples`, `page-image-bench/samples`.
- Prompt của từng lượt chạy, thư mục `work/` và `runs/`.

Spike được chuyển từ repo qobot sang đây ngày 2026-10-01; các bản ghi cũ trong `results/` có thể
còn chứa đường dẫn `qobot/spikes/...`.

## 6. Ghi chú về dữ liệu (mã nguồn mở)

- Bundle business (`biz-meta`), bộ câu hỏi đa ngôn ngữ (`embed-bench/data`) và mẫu tài liệu
  (`import-bench`, `page-image-bench`) là **dữ liệu tổng hợp**, nội dung do LLM viết hoặc sinh bằng
  template. Không phải dữ liệu thật của công ty nào.
- Câu hỏi trong `okf-scale/questions.json` được LLM sinh từ docs OpenClaw (MIT). Corpus gốc không
  được commit; `okf-scale/build_bundles.py` tải và dựng lại.
- Sample OKF của Google (Apache-2.0) được clone về `.okf-samples/` khi chạy, không commit. Bài báo
  arXiv 1706.03762 trong `import-bench` được tải khi chạy, không commit.
- `results/*.jsonl.gz` chứa đường dẫn trên máy chạy spike và câu trả lời do LLM sinh; chỉ dùng để
  phân tích.
