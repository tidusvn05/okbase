# Kế hoạch: các tình huống thực tế (repo phần mềm, thư mục rỗng, OKF chưa chuẩn, bundle trong thư mục con…)

Trạng thái: **đã triển khai U1–U8** (U9 = import, v0.4) (2026-10-02; 4 câu hỏi ở §4 chốt theo đề xuất). Bằng chứng: chạy bản build hiện tại trên 4 thư
mục mẫu (§1). Liên quan: `PLAN-onboarding.md`, `docs/usage.md`.

## 0. Kết luận ngắn

okbase hiện **giả định thư mục được chỉ định là một bundle**. Giả định đó đúng với thư mục tri thức
"sạch", nhưng sai trong phần lớn tình huống thực tế:
- repo phần mềm có `docs/`;
- thư mục rỗng;
- OKF chỉ hỏng vài file;
- bundle nằm trong thư mục con của repo.

Khi đó `onboard`/`advise` đưa ra lời khuyên sai, và `adopt` có thể **sửa nhầm file của thư viện bên
thứ ba** hoặc **làm hỏng site docs**. Cần thêm một lớp **hiểu thư mục** đứng trước mọi lời khuyên, và
hai thay đổi an toàn trong `adopt` và bộ duyệt file.

## 1. Thử nghiệm (2026-10-02, bản build hiện tại)

| Thư mục mẫu | okbase hiện làm | Vấn đề |
|---|---|---|
| **A. Repo phần mềm** (`README`, `CHANGELOG`, `src/`, `node_modules/`, `target/`, `mkdocs.yml`, `docs/` 4 trang) chạy ở gốc repo | Coi **cả repo** là bundle (6 tài liệu, gồm `node_modules/leftpad/README.md`); đề xuất "tạo bản OKF của cả thư mục" | Không bỏ qua thư mục vendor/build; không nhận ra tri thức nằm ở `docs/` |
| A, `adopt .` (chỉ lập kế hoạch) | Sẽ **sửa** `node_modules/leftpad/README.md`, tạo `node_modules/index.md`, `target/index.md`; **đổi tên `docs/index.md` → `overview.md`** dù đã nhận ra MkDocs | Sửa file của thư viện ngoài; làm hỏng trang chủ site docs (MkDocs/Docusaurus dùng `index.md`) |
| A, `-b docs adopt` | Không nhận ra MkDocs (`mkdocs.yml` ở thư mục cha) → "Plain"; cũng đổi tên `index.md` | Chỉ tìm dấu hiệu site trong chính thư mục |
| **B. Thư mục rỗng** | "0 docs, level **L2**"; đề xuất cài agent và hỏi "May I improve descriptions…" | Vô nghĩa: không có gì để cài hay curate; thiếu lối vào "bắt đầu một kho tri thức" |
| **C. OKF có 2/11 file hỏng** (1 thiếu frontmatter, 1 YAML lỗi) | Level "below L0" → hỏi "**thư mục là markdown thường**, tạo bản OKF?" | Sai: 9/11 file đã đúng chuẩn; chỉ cần sửa 2 file tại chỗ |
| **D. Repo có `knowledge/` (OKF chuẩn) cho bot** chạy ở gốc | Coi gốc repo là bundle; `README.md` làm level tụt; đề xuất adopt cả repo | Không phát hiện bundle chuẩn nằm ở `knowledge/` |

## 2. Tình huống thực tế và giải pháp

### 2.1 Repo phần mềm có `docs/`

**Nhu cầu:** agent lập trình (Claude Code, Codex) trả lời và cập nhật theo docs của dự án; docs vẫn
phải render trên site (MkDocs, Docusaurus, Hugo, Mintlify) và nằm trong git cùng code.

**Giải pháp đề xuất:**
1. **Không lấy cả repo làm bundle.** Chạy ở gốc repo, `onboard` nhận ra đây là repo phần mềm (`.git`
   kèm `Cargo.toml`/`package.json`/`pyproject.toml`/`go.mod`…) và đề xuất các thư mục tri thức
   (`docs/`, `doc/`, `documentation/`, `website/docs/`, `wiki/`, thư mục OKF con), kèm số tài liệu và
   level. Có một ứng viên rõ ràng thì dùng luôn (`-b docs`); nhiều ứng viên thì hỏi người dùng.
   MCP vẫn cài ở **gốc repo** (project = repo), bundle là `docs/`.
2. **Giữ nguyên bố cục gốc**, không tổ chức lại theo "chuẩn" thư mục. Chuẩn okbase (PLAN §2.1) chỉ là
   khuyến nghị; cấu trúc của site docs là đúng với dự án đó.
3. **Ba cách dùng, chọn theo việc ai làm chủ docs:**

   | Cách | Khi nào | Tác động |
   |---|---|---|
   | **a. Dùng nguyên trạng** (mặc định) | Docs đã có tiêu đề và cấu trúc rõ | Không sửa gì; grep/get/list/catalog chạy được trên markdown thường; catalog lấy tiêu đề và câu đầu |
   | **b. Bổ sung metadata tại chỗ, tương thích site** (đề xuất cho dự án muốn agent chính xác hơn) | Docs thiếu mô tả; nhóm review qua PR | `adopt --write` ở **chế độ site**: chỉ *thêm* `title`/`description`/`type` vào frontmatter (MkDocs, Docusaurus và Hugo bỏ qua key lạ, `description` còn dùng cho SEO); **không đổi tên `index.md`**, không tạo `index.md` liệt kê |
   | c. Bản sao OKF riêng (`adopt --out`) | Lưu trữ, hoặc docs không thuộc quyền sửa | Cảnh báo sẽ lệch dần với bản gốc |

4. **Profile `docs-site`** (`okbase.toml [bundle] profile = "docs-site"`, tự bật khi phát hiện site):
   `index.md` được coi là trang nội dung (trang chủ của thư mục); okbase tự sinh danh sách thư mục
   lúc đọc (`list`, catalog) mà không ghi file; lint không đòi `index.md` liệt kê. Level vẫn đo như
   cũ để biết mức metadata.
5. **Docs lớn** (vài trăm trang trở lên): `advise` đã đề xuất search và semantic search theo kích
   thước và ngôn ngữ; giờ áp dụng trên đúng thư mục docs thay vì cả repo.
6. **CI:** đề xuất bước `okbase -b docs lint --level L1 --format sarif` để PR docs không làm hỏng metadata.

### 2.2 Thư mục chưa có gì (bắt đầu kho tri thức mới)

**Giải pháp:** `onboard` nhận ra thư mục rỗng (hoặc không có `.md`) và chuyển sang luồng **khởi tạo**:
1. **Hỏi:** kho tri thức về gì, ai dùng (người hay bot), hỏi bằng ngôn ngữ nào; có sẵn tài liệu ở đâu
   không (PDF, DOCX, export từ wiki).
2. **`okbase init`** (mới) tạo khung:
   - `index.md` có `okf_version`;
   - `_meta/vocabulary.md` (tag theo lĩnh vực);
   - `_meta/types/` cho vài loại tài liệu phổ biến (Policy, Guide, FAQ, Reference) hoặc theo câu trả lời ở bước 1;
   - `okbase.toml`.
3. **Thêm nội dung:**
   - `okbase new --type Guide "Tiêu đề"` (mới; PLAN §6 đã dự kiến) tạo tài liệu có frontmatter đúng schema;
   - skill **`okbase-author`** để agent viết tài liệu mới chuẩn L2;
   - tài liệu sẵn có dạng PDF/DOCX thì dùng **import (v0.4)**. Trước khi có v0.4, `onboard` nói rõ và gợi ý chuyển đổi tạm bằng công cụ ngoài.
4. Có nội dung rồi mới tới cài agent và các bước khác.

### 2.3 Đã có OKF nhưng chưa chuẩn

**Giải pháp:** phân loại theo **tỉ lệ tài liệu đạt chuẩn** chứ không chỉ dùng level của cả bundle (level là "mắt xích yếu nhất"):

| Tình trạng | Đề xuất |
|---|---|
| ≥ 80% file đạt L0 | "N file cần sửa": liệt kê; `adopt --write --only <các file thiếu frontmatter>` để điền phần thiếu tại chỗ; YAML hỏng thì agent sửa tay theo vị trí lỗi lint báo; rồi curate lên L2 |
| 20–80% | Như trên, kèm hỏi người dùng có muốn chuẩn hoá cả thư mục tại chỗ (`adopt --write`, review bằng git diff) |
| < 20% | Coi là markdown thường (luồng adopt hiện tại) |

Ngoài ra, `lint --fix-safe` hiện đã tạo `index.md` còn thiếu và sửa tag đồng nghĩa; cần nêu rõ trong kế hoạch.

### 2.4 Repo có thư mục con OKF làm tri thức cho bot

**Nhu cầu:** ứng dụng (bot hỗ trợ khách hàng) đọc `knowledge/`; lập trình viên dùng agent để trả lời
câu hỏi và cập nhật `knowledge/`; CI giữ chất lượng.

**Giải pháp:**
1. **Phát hiện:** khi quét gốc repo, thư mục con có `index.md` chứa `okf_version`, hoặc đa số file có
   `type`, được nhận là bundle → `-b knowledge`. Gốc repo không phải bundle.
2. **Agent của lập trình viên:** `okbase -b knowledge agent install` (project = gốc repo). Skill
   `okbase-curate` / `okbase-author` giúp cập nhật tri thức qua PR.
3. **Bot lúc chạy** (`advise --for host` đã có, cần công thức cụ thể theo ngôn ngữ của bot):
   - Rust: thư viện `okbase::Bundle` với `Scope` theo người dùng;
   - ngôn ngữ khác (Python, Node…): `okbase mcp serve --http` kèm token (MCP client có sẵn cho các ngôn ngữ phổ biến), hoặc gọi CLI `--json`;
   - catalog đặt trong system prompt của bot (S3).
4. **CI:** lint `knowledge/` trên mỗi PR (SARIF); tuỳ chọn chạy `embed tune eval` định kỳ nếu đã tune.

### 2.5 Các tình huống khác cần đáp ứng

| Tình huống | Hiện trạng | Đề xuất |
|---|---|---|
| **Monorepo** nhiều bộ docs (`services/*/docs`) | Phải tự chạy từng bộ | Bộ quét liệt kê mọi ứng viên; cài mỗi bộ với `--name` riêng (đã hỗ trợ), hoặc gộp thành một bundle bằng `--allow` |
| **Obsidian vault** cá nhân | `adopt` nhận ra Obsidian; tạo `index.md` ở mọi thư mục làm rối vault | Profile `vault`: không tạo `index.md`, giữ wikilink; khuyên dùng nguyên trạng cộng metadata tại chỗ; không fine-tune nếu riêng tư |
| **Export từ Confluence / Notion / Google Docs** (HTML, PDF, DOCX) | Không đọc được (`.md` only) | Import v0.4; trước đó, `onboard` báo số file không phải markdown và cách chuyển tạm |
| **Ổ chia sẻ chỉ đọc** (NAS) | Index tự chuyển vào cache (đã có) | Phục vụ nhóm bằng `mcp serve --http` |
| **Tài liệu nhạy cảm** | Có `--private` trong `advise` | Bộ quét hỏi về độ nhạy cảm; khi private: chỉ dùng model cục bộ, không fine-tune bằng LLM đám mây |
| **Thư mục có bảng tính** | Module data tự bật (đã có) | Giữ nguyên |
| **Rất lớn** (> 10k tài liệu) | `advise` đề xuất embed theo kích thước | Đo thêm (ANN/MRL ở v0.5+) |

## 3. Chức năng cần làm

| # | Chức năng | Mức | Vì sao |
|---|---|---|---|
| U1 | **Luật bỏ qua khi duyệt file:** tôn trọng `.gitignore` (thư viện `ignore`, như ripgrep), `.okbaseignore`, và danh sách mặc định (`node_modules`, `target`, `vendor`, `dist`, `build`, `site`, `_build`, `.venv`, `__pycache__`) | **P0** (an toàn) | Hiện có thể index và **sửa** file của thư viện ngoài |
| U2 | **`adopt` an toàn với site docs:** tìm dấu hiệu site ở thư mục cha tới gốc repo; với MkDocs/Docusaurus/Hugo/Mintlify/Obsidian thì không đổi tên `index.md`, không tạo `index.md` liệt kê; chỉ thêm frontmatter | **P0** | Hiện làm hỏng trang chủ site |
| U3 | **Bộ quét thư mục** (`okbase scan`, và là bước đầu của `onboard`): phân loại thư mục (rỗng / repo phần mềm / site docs / OKF / OKF một phần / markdown thường / file không phải markdown) và liệt kê ứng viên bundle kèm số tài liệu, level, tỉ lệ đạt chuẩn | **P1** | Nền cho mọi lời khuyên đúng |
| U4 | **`onboard` và `advise` theo phân loại:** chọn hoặc hỏi bundle; luồng khởi tạo cho thư mục rỗng; lời khuyên "sửa N file" cho OKF một phần; công thức cho bot (`--for host`) | **P1** | Thay các lời khuyên sai ở §1 |
| U5 | **Profile `docs-site` / `vault`** trong `okbase.toml`: `index.md` là nội dung, danh sách thư mục sinh lúc đọc, lint tương ứng | **P1** | Dùng tại chỗ mà không xung đột với site |
| U6 | **`adopt --only <glob>`** (sửa đúng các file thiếu) và báo tỉ lệ đạt chuẩn | P1 | OKF một phần |
| U7 | **`okbase init`**, **`okbase new --type`**, skill **`okbase-author`** | P2 | Bắt đầu kho mới; thêm tài liệu đúng chuẩn |
| U8 | **Công thức CI** (GitHub Actions: lint SARIF) và **công thức bot** (Python/Node qua HTTP MCP) trong `docs/usage.md` | P2 | Repo có bot |
| U9 | **Import PDF/DOCX/HTML** | v0.4 (đã có trong lộ trình) | Export wiki, tài liệu văn phòng |

**Kiểm chứng:** biến 4 thư mục mẫu ở §1 (cộng monorepo, Obsidian vault và thư mục có PDF) thành
**fixture kịch bản**. Test khẳng định `onboard` đưa ra đúng ứng viên và đúng câu hỏi, và
`adopt --write` không đụng tới file bị bỏ qua hay trang `index.md` của site. Sau đó chạy lại S13
với agent trên các thư mục này.

## 4. Câu hỏi cần chốt

1. Mặc định cho docs của repo phần mềm là **(a) dùng nguyên trạng**, kèm gợi ý **(b) bổ sung metadata
   tại chỗ** qua PR? (đề xuất: đúng vậy; không đề xuất bản sao `--out` cho docs đang được duy trì)
2. Profile `docs-site` cho phép `index.md` là trang nội dung, tức lệch khỏi OKF §3.1 (`index.md` dành
   cho danh sách). Chấp nhận như một profile của okbase, ghi rõ trong tài liệu? (đề xuất: có; bundle
   thuần OKF không bị ảnh hưởng)
3. Tôn trọng `.gitignore` theo mặc định? (đề xuất: có; `.okbaseignore` để thêm hoặc bớt)
4. Thứ tự làm: U1 + U2 (an toàn) ngay, rồi U3–U6, rồi U7–U8, còn U9 theo v0.4? (đề xuất: có)

## 5. Triển khai (2026-10-02)

| # | Commit | Ghi chú |
|---|---|---|
| U1 | `11bc53a` | `.gitignore` (cả thư mục cha), `.okbaseignore`, bỏ qua thư mục phụ thuộc. Danh sách mặc định giữ hẹp (`node_modules`, `__pycache__`…); `build/`, `vendor/`, `target/` để `.gitignore` quyết định, vì có thể là thư mục tri thức thật |
| U2, U5, U6 | `7ac51a3` | Profile `docs-site`/`vault` (nhận diện cả ở thư mục cha tới gốc repo; ghi đè bằng `okbase.toml`); `adopt` giữ `index.md`, không tạo danh sách hay log; `adopt --only` |
| U3, U4 | `1db3ab0` | `okbase scan`; `onboard` chọn hoặc hỏi bundle, có luồng thư mục rỗng, PDF, "sửa N file" và metadata cho site; lệnh của `onboard` giữ `-b` |
| U7 | `1d129c6` | `okbase init`, `okbase new`, skill `okbase-author` |
| U8 | `7626a83` | `docs/usage.md`: thư mục thực tế, profile, luật bỏ qua, CI, bot |

Đã kiểm chứng bằng test kịch bản (`onboard_understands_real_folders`): repo có MkDocs và `node_modules`, thư mục rỗng, OKF hỏng 1 file, repo bot có `knowledge/`.
