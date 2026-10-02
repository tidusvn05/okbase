# Kế hoạch: import tài liệu (PDF, Office, HTML) — v0.4

Trạng thái: **I1–I4 xong** (2026-10-02); I5 (chạy thử với agent) ở §7. Bằng chứng: `spikes/import-bench/RESULTS.md`
(S10-lite). Thay mục "v0.4 — Import + source" của `PLAN.md` §13 về phần chuyển đổi (connector
Google Drive/Sheets vẫn để sau).

## 0. Quyết định (2026-10-02)

1. **Bộ chuyển đổi:** anydoc (Office, PDF qua pdf-inspector) và htmd (HTML), bọc trong wrapper
   `okfkit-convert`. Không dùng model, không gọi dịch vụ, **không bao giờ dùng OCR qua dịch vụ ngoài**.
2. **Hai chế độ:**
   - **đọc trực tiếp** (mặc định, không ghi file): file nguồn được index thẳng, kết quả chuyển đổi
     lưu trong index (cache theo hash);
   - **chuyển hẳn** (`import --write`): ghi markdown có nguồn gốc để biên tập và chắt lọc.
3. **Có trong bản mặc định** (binary tăng khoảng 10 MB).
4. Kèm theo: module data đọc thêm `.xls/.xlsm/.xlsb/.ods`; `.txt` (và `.rst`/`.adoc` dạng văn bản
   thuần) được index.

## 1. Wrapper `okfkit-convert`

- `convert(path, bytes) -> Converted { markdown, title, format, pages, flags }`.
- **PDF:** dùng thẳng `pdf_inspector::extract_pages_markdown_mem`, lấy từng trang:
  - trang chữ giữ nguyên, có dấu trang `<!-- page N -->` để trích dẫn;
  - **trang scan không làm hỏng cả tài liệu**: để một khối "page N needs OCR" và ghi vào `flags`;
  - trang nhiều cột ghi vào `flags.columns` để người hoặc agent xem lại;
  - tiêu đề lấy từ metadata `/Title`, hoặc heading đầu tiên.
- **Office/EPUB/RTF:** anydoc. Bảng tính (`xlsx`, `xls`…) **không** đi qua đây: module data phục vụ
  chúng bằng SQL.
- **HTML:** bỏ `script/style/nav/header/footer/aside`, ưu tiên `<main>`/`<article>`/`#main-content`;
  htmd chuyển đổi; bảng không có hàng tiêu đề được dựng lại thành bảng markdown.
- **Văn bản thuần** (`.txt`, `.rst`, `.adoc`): giữ nguyên, tiêu đề lấy từ dòng đầu.
- **Giới hạn:** bỏ qua file > 64 MB (báo lý do); file có mật khẩu báo `encrypted`; lỗi không làm
  dừng cả lượt đồng bộ.

## 2. Đọc trực tiếp (index)

- Bộ duyệt file (đã tôn trọng `.gitignore`/`.okfkitignore`) lấy thêm các đuôi nguồn. Id của tài
  liệu là đường dẫn giữ đuôi (`manuals/printer.pdf`), nên không trùng với `printer.md`.
- Frontmatter tổng hợp:
  - `type: Source`, `title`, `description` (câu đầu);
  - `source: {path, format, pages}`;
  - `generated` (các trường do okfkit điền).
- Mọi tool (`grep`, `get`, `search`, `catalog`) chạy trên các tài liệu này như với markdown.
- Bảng `sources` trong index ghi trạng thái từng file: `ok | partial (cần OCR) | encrypted | error`,
  danh sách trang cần OCR và trang nhiều cột.
- **Nếu đã chuyển hẳn** (có file markdown khai `source.path` trỏ tới file gốc) thì bỏ bản đọc trực
  tiếp, tránh trùng.

## 3. Chuyển hẳn: `okfkit import`

| Lệnh | Việc |
|---|---|
| `import --plan` (mặc định, chỉ đọc) | Liệt kê file, định dạng, số trang, trang cần OCR, file lỗi |
| `import --write [--out sources]` | Ghi `sources/<đường dẫn>.md` với frontmatter nguồn gốc (`source.path`, `source.hash`, `converter`, `imported`), `generated`, kèm văn bản OCR đã có; **tự tách** tài liệu dài (> 6k token) theo heading cấp 1–2 thành nhiều phần |
| Chạy lại `import --write` | Chỉ ghi lại file có nguồn đã đổi **và** bản markdown chưa bị sửa tay (so hash lúc import); bản đã sửa tay thì báo xung đột, cần `--force` |
| `import status` | `stale` (nguồn đổi), `orphan` (nguồn mất), `needs-ocr`, `columns`, `edited` |

## 4. OCR nhờ agent (không có engine OCR trong okfkit)

- `import ocr-next`: in file và trang cần OCR (kèm lý do). Agent đa phương thức đọc trang đó
  (Claude Code đọc được PDF/ảnh) rồi chép lại.
- `import ocr-submit <file> --page N -`: lưu văn bản vào state dir (theo hash của file, không ghi vào
  bundle). Bản đọc trực tiếp và `import --write` ghép văn bản này vào đúng trang.
- Ảnh (`png/jpg`) dùng cùng luồng: mỗi ảnh coi như một trang cần OCR.

## 5. Agent và các phần sẵn có

- Skill **`okfkit-import`**: khi nào import, OCR nhờ agent, xem lại trang nhiều cột, chắt lọc
  nguồn thành tài liệu chuẩn có `sources[]`.
- `scan`/`onboard`:
  - thư mục chỉ có PDF/DOCX → "đọc được ngay";
  - có trang cần OCR → hỏi người dùng có cho agent đọc ảnh trang không (gửi nội dung cho nhà
    cung cấp model, thuộc danh mục consent).
- `doctor`: báo tài liệu cần OCR hoặc bản import đã cũ. `advise`: tính cả tài liệu nguồn vào kích
  thước và ngôn ngữ.

## 6. Lộ trình

| Phase | Nội dung | Tiêu chí xong |
|---|---|---|
| I1 | `okfkit-convert` (PDF từng trang, Office, HTML, văn bản); data đọc thêm xls/ods | Bộ mẫu S10-lite cho kết quả như spike; PDF có 1 trang scan vẫn giữ các trang chữ |
| I2 | Đọc trực tiếp trong index; bảng `sources` | `grep`/`get`/`catalog` chạy trên PDF/DOCX/PPTX/HTML; sửa file nguồn thì kết quả cập nhật |
| I3 | `import --plan/--write/status`, tách tài liệu dài, chống ghi đè bản đã sửa | Chạy lại an toàn; bản trùng bị loại |
| I4 | OCR nhờ agent; skill `okfkit-import`; tích hợp scan/onboard/doctor | Agent hoàn tất một thư mục có trang scan |
| I5 | Chạy thử với agent thật (thư mục PDF/DOCX/HTML hỗn hợp) | Agent trả lời đúng câu hỏi từ tài liệu nguồn, trích dẫn file và trang |

## 7. Triển khai

| Phase | Commit | Ghi chú |
|---|---|---|
| I1 | `90f8603`, `d0eba82` | `okfkit-convert`. PDF dùng thẳng `pdf-inspector` từng trang (trang scan không làm mất cả tài liệu). Data đọc thêm xlsm/xlsb/xls/ods. `ttf-parser` (không còn bảo trì, phụ thuộc của pdf-inspector) được ghi nhận trong `deny.toml` |
| I2 | `3bf8b0d` | Đọc trực tiếp trong index; bảng `sources`; ghép văn bản OCR; nhường chỗ cho bản đã import ngay trong cùng lượt đồng bộ |
| I3 | `f9e9979` | `import` (kế hoạch), `--write` (tách > 6k token, giữ bản sửa tay, báo mồ côi), `status`, `ocr-next`/`ocr-submit` |
| I4 | `7354629` | `scan` đếm tài liệu nguồn; `onboard` hỏi về OCR (consent); `doctor` báo file không đọc được và trang chưa có chữ; skill `okfkit-import`; `new --source`; tài liệu |
| Sau I5 | (commit này) | `import ocr-next` xuất ảnh của trang scan (JPEG giữ nguyên; ảnh Flate, kể cả PNG predictor và đen trắng 1-bit, thành PNG); fax/JBIG2 thì hướng dẫn mở thẳng trang PDF. Sửa deadlock khoá index trong `ocr_next` |
