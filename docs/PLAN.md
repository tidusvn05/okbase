# okfkit — Kế hoạch & kiến trúc

> Phiên bản: 2.1 (2026-10-01) · Trạng thái: **Chốt để triển khai** · **Dự án mã nguồn mở** (MIT OR Apache-2.0) · Bàn giao: `docs/HANDOFF.md`
> Thay đổi so với 1.0: embedding chuyển thành **opt-in** (mặc định lexical); thêm **chuẩn okfkit** (mức L0–L3) và luồng **adopt** (markdown thường → OKF); thêm **Agent Skills**; thiết kế **mô-đun + profile + điểm mở rộng**.
> Bằng chứng thực nghiệm: `spikes/` (xem `spikes/README.md`)

## 0. Tóm tắt

**okfkit** giúp agent AI (Claude Code, Codex, OpenCode, qobot, MCP client bất kỳ) làm việc với **một thư mục tài liệu markdown** nhanh, rẻ và chính xác, theo định dạng **OKF (Open Knowledge Format)**.

Bốn cách dùng chính:

| Bạn có… | okfkit làm | Lệnh |
|---|---|---|
| **Bundle OKF có sẵn** | Dùng ngay, không cần cấu hình, không sửa file | `okfkit mcp serve` / `okfkit agent install` |
| **Thư mục markdown thường** (docs, wiki, Obsidian, docs site) | Chuyển sang chuẩn okfkit: thêm frontmatter, `index.md`, mô tả, tag; xem trước diff được | `okfkit adopt` |
| **PDF, Sheet, Drive…** | Import thành markdown theo mục và dataset SQL | `okfkit import` (module) |
| **Agent cần dùng hiệu quả** | Skill + hướng dẫn + MCP tool được thiết kế theo kết quả đo | `okfkit agent install` |

**Mặc định đơn giản:** không có model, không cần mạng, không cần cấu hình; chế độ **lexical** (catalog + `grep` v2 + `query` + `get`). Các chức năng nặng (embedding, import, sync nguồn, ghi, eval) là **module opt-in**.

---

## 1. Bằng chứng từ spike → quyết định

| # | Spike | Kết quả chính | Quyết định cho okfkit |
|---|---|---|---|
| S1 | 12 model embedding, 300 câu vi/en/ja (`embed-bench`) | EmbeddingGemma Q4 tốt nhất (R@1 0.85, R@3 0.96, 188MB, ~0.5GB RAM); BM25 hỏi khác ngôn ngữ chỉ 3.5% | Nếu bật embedding, dùng EmbeddingGemma Q4 (hoặc bge-m3 int8, MIT). **Tìm sẵn tài liệu (pre-retrieval) khi hỏi khác ngôn ngữ cần embedding** |
| S2 | Ngưỡng BM25 | Trộn BM25 và embedding không có lợi (0 đến +1 điểm) | Không trộn. Lexical dùng cho `grep` |
| S3 | Catalog hay tool, với Claude CLI thật | Độ chính xác như nhau; catalog phải nằm trong system prompt (nếu không, chi phí ×2.5) | `catalog()` là sản phẩm chính; hướng dẫn host đặt catalog trong system prompt |
| S4 | Docs OpenClaw 60k → 4.4M token, 9 cách | **Lexical (G2) 100/100/93/90% ≈ embedding (D) 97/100/100/93%**; agent + Read/Grep cũng ngang; embedding giúp ít lượt hơn (~2 so với ~4.5); nhắc "kiểm tra lại" bằng prompt không hiệu quả; lỗi còn lại do tài liệu trùng lặp | **Mặc định lexical**; `grep` phải mạnh ngang Grep của CLI; embedding là opt-in để tối ưu tốc độ; lint phát hiện trùng lặp |
| S5 | Metadata/tag + sheet, 151 → 3.020 tài liệu (`biz-meta`) | Nhỏ: agent grep frontmatter là đủ. Lớn: `query` giữ đầy đủ list, rẻ hơn 30–45%. **Sheet ~10k dòng: không có SQL thì agent bỏ cuộc; có SQL thì 10/10, rẻ hơn 9×** | `query` thuộc lõi; `data` là module (bật tự động khi có CSV/XLSX); không sinh view đầy đủ; catalog kèm từ vựng tag và facet |
| S6 | Chunk và tốc độ index | ~3 chunk/s/8 CPU; 4.4M token ≈ 64 phút | Embedding chạy nền, cache theo hash; lexical sẵn sàng ngay |
| S11 | Fine-tune EmbeddingGemma bằng LoRA (cách của Unsloth) trên câu hỏi tổng hợp (`spikes/embed-tune`) | Trong miền: S1 R@1 0.853 → 0.943, khác ngôn ngữ 0.815 → 0.95; không quên miền khác; S4 bão hoà (27 → 28/30). CPU ~25 phút train, ~$3/1k cặp; mỗi lần đổi model phải embed lại cả bundle | Không đưa training vào okfkit. Ứng viên: cho phép nạp model ONNX do người dùng tự cung cấp (opt-in), **sau khi** kiểm tra ONNX/Q4 còn giữ mức cải thiện |

**Bài học trung tâm: tổ chức bundle tốt là yếu tố quyết định.** Agent mạnh chỉ với Read/Grep đã làm tốt trên bundle có `index.md`, `description` rõ và metadata nhất quán. Vì vậy okfkit tập trung vào ba việc:
1. **giúp tổ chức tốt** (chuẩn, adopt, lint);
2. **giúp agent tận dụng tổ chức đó** (catalog, skill, tool);
3. chỉ thêm máy móc nặng khi cần: SQL cho dữ liệu bảng, embedding cho tốc độ và pre-retrieval.

**Chưa đo, cần spike trước khi chốt:**
- S7: chế độ lexical với **Codex** và model nhỏ.
- S8: **adopt có thật sự cải thiện độ chính xác** của agent trên markdown thường không (so trước và sau).
- S9: **skill có làm agent dùng tool đúng hơn** không (spike S5: agent chỉ dùng `kb_query` ở 32/48 câu).

---

## 2. Chuẩn okfkit (okfkit profile của OKF)

okfkit không tạo định dạng mới. **Chuẩn okfkit = OKF v0.2 + các quy ước giúp agent làm việc tốt**, chia theo mức. Mọi mức đều là OKF hợp lệ.

| Mức | Tên | Yêu cầu | Lợi ích (theo spike) |
|---|---|---|---|
| **L0** | OKF | Mỗi `.md` (trừ index/log) có frontmatter với `type` | Công cụ OKF nào cũng đọc được |
| **L1** | Navigable | L0 + mọi tài liệu có `title` và **`description`** (1 câu); **`index.md` ở mỗi thư mục**; ID ổn định | Catalog và `index.md` có nghĩa, nên agent tìm đúng file bằng Read/Grep hoặc `kb_list` (S3/S4) |
| **L2** | Structured | L1 + `tags` theo từ vựng (`_meta/vocabulary.md`), `lang`, `status` (stable/deprecated/draft) với `supersedes`, `updated`; field tuỳ biến theo schema của type | `query` chính xác, chọn đúng phiên bản hiện hành, lọc được khi bundle lớn (S5) |
| **L3** | Curated | L2 + không có nội dung gần trùng (hoặc đã liên kết về trang canonical); `sources`/`verified` cho tài liệu quan trọng; không có link gãy; không quá `stale_after` | Giảm loại lỗi còn lại ở quy mô lớn: thông tin rải ở nhiều tài liệu (S4) |

`okfkit lint --level L2` báo bundle đang ở mức nào và còn thiếu gì. `okfkit adopt` đưa markdown thường lên L1 tự động, và lên L2 với sự hỗ trợ của agent.

### 2.1 Cấu trúc thư mục khuyến nghị (không bắt buộc)

```
my-bundle/
├── okfkit.toml         # tuỳ chọn — không có thì dùng mặc định
├── index.md, log.md    # OKF (okfkit sinh/cập nhật)
├── knowledge/…         # tri thức đã biên soạn
├── sources/…           # bản import (PDF theo mục, web…), có provenance
├── data/               # dataset: *.csv|*.xlsx + <name>.md (type: Dataset, schema tự sinh)
├── _meta/
│   ├── vocabulary.md   # từ vựng tag + đồng nghĩa vi/en/ja + facet
│   └── types/<Type>.md # (tuỳ chọn) schema field cho từng type: kiểu, bắt buộc, giá trị hợp lệ
└── .okfkit/            # (gitignore) index, cache — hoặc đặt ngoài bundle (§3.1)
```

### 2.2 Frontmatter

| Nhóm | Field |
|---|---|
| OKF bắt buộc | `type` |
| okfkit L1 | `title`, `description` |
| okfkit L2 | `tags`, `lang`, `status`, `supersedes`, `updated`, `aliases`, `audience`, `effective_from`, `effective_to` |
| OKF v0.2 (L3) | `sources[]`, `generated{by,at}`, `verified[]`, `stale_after`, `resource` |
| Tuỳ biến | Field bất kỳ; kiểu khai báo trong `_meta/types/<Type>.md` hoặc `okfkit.toml [fields]` |

Nguyên tắc ghi: **round-trip giữ nguyên key lạ, thứ tự key, comment và body.** okfkit chỉ sửa đúng field mà nó được yêu cầu sửa.

---

## 3. Luồng sử dụng

### 3.1 Bundle OKF có sẵn (không cấu hình, không sửa file)

```
cd existing-bundle
okfkit status                  # mức L0–L3, số tài liệu/token, mode khuyến nghị, gợi ý
okfkit agent install --claude  # đăng ký MCP + cài skill cho Claude Code (hoặc --codex)
```

- **Chỉ đọc mặc định:** okfkit không ghi vào bundle khi chưa có lệnh ghi rõ ràng (`adopt --write`, `index --write-index-md`, `lint --fix`).
- **Index có thể đặt ngoài bundle:** `state_dir = "auto"` sẽ dùng `.okfkit/` nếu bundle ghi được, còn không thì dùng `~/.cache/okfkit/bundles/<hash>/`. Nhờ vậy dùng được với bundle chỉ đọc (repo người khác, thư mục mount).
- Chấp nhận bundle "lệch chuẩn": thiếu `description` thì dùng câu đầu của nội dung, thiếu `index.md` thì sinh catalog ảo trong bộ nhớ. Không từ chối bundle nào (theo quy tắc conformance của OKF).
- Nhận dạng thêm frontmatter phổ biến ngoài OKF: `summary` (Mintlify/OpenClaw), `excerpt`, `tags`/`categories` (Jekyll/Hugo), `sidebar_label` (Docusaurus). Các field này được ánh xạ khi đọc, không sửa file.

### 3.2 Markdown thường → chuẩn okfkit (`okfkit adopt`)

```
okfkit adopt ./docs --plan            # báo cáo: sẽ thêm gì, ở đâu (không ghi)
okfkit adopt ./docs --out ./docs-okf  # ghi sang thư mục mới (an toàn)
okfkit adopt ./docs --write           # sửa tại chỗ (yêu cầu git sạch hoặc --force), có log.md
okfkit adopt ./docs --level L2 --with-agent claude   # nhờ agent viết description/tag còn thiếu
```

| Bước | Tự động (không LLM) | Có agent hỗ trợ (tuỳ chọn) |
|---|---|---|
| 1. Nhận dạng | Loại site (Obsidian, Docusaurus, Hugo, Mintlify, MkDocs, thư mục thường); đọc frontmatter có sẵn và ánh xạ | — |
| 2. `type` | Suy ra từ thư mục/tên file/heading theo luật (`faq/` → FAQ, `adr-*` → Decision…); mặc định `Document` | Gợi ý type chính xác hơn |
| 3. `title` | H1 → frontmatter có sẵn → tên file | — |
| 4. `description` | Câu đầu có nghĩa (bỏ admonition, badge, code); đánh dấu `generated: {by: okfkit-heuristic}` | **Viết mô tả 1 câu** (quan trọng nhất cho catalog) |
| 5. Link | Chuyển wikilink `[[x]]` và link tương đối sang dạng OKF; báo link gãy | — |
| 6. Cấu trúc | Sinh `index.md` mọi thư mục; `log.md`; gợi ý tách file quá dài (> N token) theo H2 | Đề xuất cấu trúc thư mục |
| 7. L2 | `lang` (nhận diện), `updated` (git log/mtime), tag từ thư mục | Tag theo từ vựng, `status`/`supersedes` khi phát hiện nhiều phiên bản |
| 8. Báo cáo | Diff, lint trước/sau, mức L đạt được | — |

- Bước 4 và 7 có agent tham gia được thực hiện qua **skill `okfkit-curate`** (§6) hoặc lệnh `adopt --with-agent`. Lệnh này gọi CLI agent qua module `agent-bridge`, là tuỳ chọn.
- Mọi giá trị do máy sinh đều ghi `generated{by,at}` (OKF v0.2), để người sau biết cần xác minh (`verified`).

### 3.3 Import tài liệu nguồn (module `import`)

- PDF, DOCX, HTML → `sources/<name>/<nn>-<section>.md`: cắt theo heading hoặc N trang, kèm `resource`, `pages`, `generated`; bảng chuyển thành markdown table; OCR là tuỳ chọn.
- XLSX, CSV, Google Sheets → dataset (module `data`) + `data/<name>.md`.
- Nguồn đồng bộ tăng dần (module `source-*`): thư mục, Google Drive/Sheets, Notion.
- **Import không sửa `knowledge/`.** Việc tóm lược `sources/` thành concept là việc của agent, thông qua skill `okfkit-curate` (`okfkit distill --plan` liệt kê những gì cần làm).

### 3.4 Agent dùng bundle (hằng ngày)

```
okfkit agent install --claude | --codex | --opencode | --print
```
- Đăng ký MCP server (`okfkit mcp serve --stdio`, hoặc HTTP khi chạy lâu dài).
- Cài **skills** (§6) vào thư mục skill của CLI; với CLI chưa hỗ trợ skill thì sinh đoạn `AGENTS.md` tương đương.
- In ra đoạn catalog và hướng dẫn nên đặt vào system prompt, nếu người dùng tự dựng agent.

---

## 4. Kiến trúc

### 4.1 Nguyên tắc
1. **Lõi nhỏ, không phụ thuộc nặng:** không có ONNX, không cần mạng, build nhanh. Mọi phần nặng là module.
2. **File là nguồn sự thật.** Index dựng lại được, và có thể đặt ngoài bundle.
3. **Không biết người dùng:** mọi thao tác đọc nhận `Scope` do host truyền vào.
4. **Output cho agent:** gọn, có ID để trích nguồn, có gợi ý khi không có kết quả, báo rõ khi bị cắt bớt.
5. **Mở rộng qua trait, capability và profile**, không sửa lõi.
6. **Đo được:** eval có sẵn, và mọi mặc định đều gắn với một spike.

### 4.2 Sơ đồ

```
   ┌────────────── Giao diện ──────────────────────────────────────────────────┐
   │ CLI `okfkit` (+ plugin `okfkit-<x>`)   MCP (stdio/HTTP)   Rust API   Skills│
   └──────────┬─────────────────────────────┬──────────────────┬───────────────┘
              ▼                             ▼                  ▼
   ┌──────────────────────── Facade `okfkit::Bundle` ───────────────────────────┐
   │ Capability registry: tool/command nào có mặt tuỳ module đã bật             │
   └──────────┬──────────────────────────────────────────────────────────────────┘
   ┌──────────▼──────────── LÕI (luôn có) ─────────────────────────────────────┐
   │ core: parse/write round-trip, validate, links · standard: mức L0–L3, schema│
   │ index: metadata, tags, links, aliases, chunks, FTS (analyzer vi/en/ja)     │
   │ read: grep v2 · get · list · query(filter/facet/sum) · catalog · stats     │
   │ maint: lint (rule engine) · adopt (heuristic) · index.md/log.md/vocabulary │
   └───────┬───────────────┬──────────────┬───────────────┬────────────────────┘
   ┌───────▼─────┐ ┌───────▼──────┐ ┌─────▼───────┐ ┌─────▼──────────────────┐
   │ data        │ │ embed-local  │ │ import-*    │ │ write · watch · eval   │
   │ CSV/XLSX →  │ │ embed-api    │ │ pdf/docx/   │ │ source-gdrive/notion…  │
   │ SQLite, SQL │ │ search,      │ │ html/xlsx   │ │ agent-bridge (gọi CLI) │
   │ (duckdb)    │ │ retrieve     │ │             │ │ ann (usearch)          │
   └─────────────┘ └──────────────┘ └─────────────┘ └────────────────────────┘
        (MODULE: Cargo feature khi build + bật/tắt bằng cấu hình khi chạy)
```

### 4.3 Crates

| Crate | Lõi / module | Nội dung |
|---|---|---|
| `okfkit-core` | lõi | Concept, Frontmatter (giữ thứ tự, key lạ, comment), parse/write, validate OKF, link, ID |
| `okfkit-standard` | lõi | Mức L0–L3, schema type (`_meta/types`), từ vựng tag, ánh xạ frontmatter ngoại (summary→description…) |
| `okfkit-analyze` | lõi | Analyzer trait: NFKC, bỏ dấu vi, lindera (ja, feature mặc định bật), stem en, nhận diện ngôn ngữ |
| `okfkit-index` | lõi | Schema SQLite, indexer tăng dần, chunker, catalog, facet, sinh index.md/log.md |
| `okfkit-query` | lõi | `grep` v2, `get`, `list`, `query`, `catalog`, `stats`, `recommend_mode` |
| `okfkit-lint` | lõi | Rule engine + rule chuẩn (L0–L3); `LintRule` trait |
| `okfkit-adopt` | lõi | Nhận dạng site, ánh xạ, heuristic title/description/type, tách file, kế hoạch/diff |
| `okfkit-data` | module `data` | Dataset → SQLite (DuckDB là feature con), `data.query` có giới hạn an toàn, schema doc |
| `okfkit-embed` | module `embed-local` / `embed-api` | Embedder trait, fastembed (EmbeddingGemma Q4 / bge-m3 int8), client HTTP; cache theo hash; batch theo độ dài |
| `okfkit-search` | module (cần embed) | Dense search, MMR, `retrieve`, lint trùng lặp theo nghĩa, ANN (feature `ann`) |
| `okfkit-import` | module `import-*` | Converter trait + pdf/docx/html/xlsx/csv |
| `okfkit-source` | module `source-*` | Source trait + fs/gdrive/gsheets/notion |
| `okfkit-mcp` | lõi (stdio) / module `http` | Tool registry theo capability, rmcp; `router()` để host nhúng |
| `okfkit-skills` | lõi | Skill và hướng dẫn nhúng trong binary; `agent install` cho claude/codex/opencode |
| `okfkit-bridge` | module `agent-bridge` | Gọi CLI agent (qua agent-core) cho adopt/curate/eval |
| `okfkit-eval` | module `eval` | Chạy bộ câu hỏi (retrieval hoặc agent), chấm điểm, báo chi phí và độ trễ (từ spike) |
| `okfkit` | facade | `Bundle`, `Scope`, `Capabilities`, re-export |
| `okfkit-cli` | binary | `okfkit`, tự tìm plugin `okfkit-<x>` trong PATH |

Các bản build phát hành:
- **`okfkit`**: lõi + data + mcp-http + import (PDF, Office, HTML; CSV/XLSX qua data). Không có ONNX; khoảng 34MB (2026-10-03). Từ điển tiếng Nhật tải về khi dùng lần đầu.
- **`okfkit-full`**: thêm embed-local, import-pdf/docx/html, source-*, eval.

### 4.4 Cấu hình: mặc định đơn giản, mở rộng dần

Không có `okfkit.toml` thì dùng mặc định. File đầy đủ nhất chỉ bật những gì cần:

```toml
# okfkit.toml — mọi mục đều tuỳ chọn
profile = "standard"            # minimal | standard | full  (tập mặc định của module)

[bundle]
languages = ["vi", "en", "ja"]
state_dir = "auto"              # auto | ".okfkit" | "~/.cache/okfkit/..."

[modules]                       # ghi đè profile
data = "auto"                   # auto: bật khi có *.csv/*.xlsx trong data/
embed = "off"                   # off | local | api
import = ["pdf", "xlsx"]
sources = []                    # ["gdrive", "notion"]
write = false                   # tool ghi cho agent
watch = false

[embed]                         # chỉ đọc khi modules.embed != off
model = "embeddinggemma-300m-q4"    # | bge-m3-int8 | api:<provider>/<model>

[standard]
target_level = "L2"             # lint so với mức này
vocabulary = "_meta/vocabulary.md"

[tools]                         # MCP/skill: đổi tên, ẩn, giới hạn
prefix = "kb"
disable = []
limits = { grep_lines = 40, get_tokens = 4000, data_rows = 100 }
```

| Profile | Module bật | Dùng khi |
|---|---|---|
| `minimal` | Lõi | CI, bundle nhỏ, máy yếu |
| `standard` (mặc định) | Lõi + data (auto) + import csv/xlsx | Hầu hết trường hợp dùng với Claude Code/Codex |
| `full` | Mọi module, gồm embed-local | Bundle lớn đa ngôn ngữ cần pre-retrieval nhanh; host kiểu qobot |

### 4.5 Điểm mở rộng

| Muốn thêm | Cách | Cần build lại? |
|---|---|---|
| Loại tài liệu mới với field riêng | `_meta/types/<Type>.md` (schema: field, kiểu, bắt buộc, enum) → lint, query, adopt hiểu | Không |
| Tag, đồng nghĩa, facet | `_meta/vocabulary.md` | Không |
| Luật đoán type khi adopt | `okfkit.toml [adopt.rules]` (glob → type/tags) | Không |
| Rule lint riêng | Rule khai báo (TOML: field X bắt buộc khi type Y, regex cấm…) | Không |
| Lệnh CLI mới | Plugin ngoài `okfkit-<name>` trong PATH (kiểu git), nhận `--bundle` và giao tiếp qua JSON | Không |
| Tool cho agent | MCP server ngoài (agent tự cấu hình), hoặc tool dạng "command" khai báo trong `okfkit.toml [tools.custom]` (chạy lệnh, JSON vào/ra) | Không |
| Skill riêng của dự án | `_meta/skills/<name>/SKILL.md`, được `agent install` cài cùng | Không |
| Converter, Source, Embedder, Analyzer, LintRule, ToolProvider | Cài trait trong Rust, đăng ký qua registry (feature) | Có |
| Hook in-process an toàn | WASM (extism): `on_index_doc`, `on_lint`, `on_tool_output` (giai đoạn sau) | Không |

**Capability registry:** mỗi module khai báo capability (`read.grep`, `data.sql`, `embed.search`, `import.pdf`…). CLI, MCP và skill chỉ hiện những gì đang có. Ví dụ: không có embed thì `kb_search` không xuất hiện, và skill hướng dẫn dùng grep/list.

### 4.6 API thư viện (cho host như qobot)

```rust
let bundle = Bundle::open(dir, OpenOptions::default()
    .profile(Profile::Standard)
    .embedder(shared_embedder.clone())      // tuỳ chọn; None = lexical
    .state_dir(StateDir::Auto))?;
bundle.sync(SyncMode::Incremental).await?;
let caps = bundle.capabilities();            // biết module nào đang có
let scope = Scope::all().deny("memory/people/**").filter(MetaFilter::not_audience("private"));

bundle.catalog(&CatalogOptions::default(), &scope)?;         // cho system prompt (+ vocabulary/facets)
bundle.recommend_mode(&scope);                               // Full | Retrieval | Lexical
bundle.retrieve(q, TokenBudget(3000), &scope).await?;       // chỉ khi có embed
bundle.grep(&req, &scope)?; bundle.query(&mq, &scope)?; bundle.get(&id, &sel, &scope)?;
bundle.data()?.query(sql, Limits::default())?;               // khi module data bật
bundle.lint(&LintConfig::level(Level::L2))?;
bundle.write(ConceptWrite { .. }).await?;                    // khi module write bật, validate + index/log
okfkit_mcp::router(bundle.clone(), scope_provider);          // mount vào axum của host
bundle.watch();                                              // Stream<ChangeEvent>
```

### 4.7 Chỉ mục

| Bảng | Luôn có | Nội dung |
|---|---|---|
| `docs`, `doc_fields`, `doc_tags`, `aliases`, `links` | ✅ | Metadata đã có kiểu, tag gốc và dạng chuẩn hoá, link và backlink |
| `chunks`, `chunks_fts` | ✅ | Chunk theo H2/H3 (150–450 token, tiền tố `title > heading`), FTS qua Analyzer |
| `chunk_vecs` | module embed | Vector f32 nạp vào RAM (brute-force; ANN khi > ~100k chunk) |
| `datasets.sqlite` | module data | Mỗi dataset một bảng + `_schema` |

- Index tăng dần theo hash. Lexical sẵn sàng ngay; embedding chạy nền, có tiến độ, cache toàn cục theo hash (`~/.cache/okfkit/emb`).
- Trong lúc embedding chưa xong, `recommend_mode()` trả về `Lexical`.

---

## 5. Công cụ cho agent (MCP / CLI `--json`)

| Tool | Module | Mô tả (dựa trên spike) |
|---|---|---|
| `kb_catalog` | lõi | Catalog (≤ N token) + từ vựng tag và facet; host nên đặt vào system prompt thay vì để agent gọi |
| `kb_list` | lõi | `index.md` của thư mục |
| `kb_grep` | lõi | Regex/alternation, không phân biệt hoa thường và dấu, tìm cả frontmatter, `path` glob, `context`, `files_only`, `filter`; gợi ý khi rỗng (S4: G tăng lên G2) |
| `kb_get` | lõi | Theo `section`/`lines`/`max_tokens`; báo cắt bớt kèm danh sách heading |
| `kb_query` | lõi | Lọc metadata, `active_on`, `facets`, `sum_field`, `sort`, `count_only` (S5) |
| `kb_links` | lõi | Link và backlink |
| `data_tables`, `data_query` | data | SQL chỉ đọc (S5: bắt buộc cho sheet) |
| `kb_search` | embed | Tìm ngữ nghĩa đa ngôn ngữ, có `filter` |
| `kb_write`, `kb_propose` | write | Ghi có validate, cập nhật index/log (tắt mặc định) |

Output thống nhất ở cả MCP và `--json`. Tên tool đổi được bằng `[tools].prefix`.

---

## 6. Agent Skills

Skills (chuẩn AgentSkills, `SKILL.md`) là **cách rẻ nhất để agent dùng okfkit đúng**. Skill chỉ được nạp khi cần, nên không tốn token thường trực như hướng dẫn dài trong system prompt. Skill được nhúng trong binary và cài bằng `okfkit agent install`.

| Skill | Khi nào agent dùng | Nội dung chính (theo spike) |
|---|---|---|
| **`okfkit-answer`** | Trả lời câu hỏi từ bundle | Đọc catalog hoặc `index.md` trước. **Câu liệt kê, đếm, lọc: dùng `kb_query`** (S5). **Số liệu: dùng `data_query`**, không tự cộng (S5). Mã hoặc chuỗi chính xác: `kb_grep` với alternation, thuật ngữ tiếng Anh và đồng nghĩa (S4). Câu hỏi theo nghĩa: `kb_search` nếu có, không thì `kb_grep` + `kb_list`. Tài liệu dài: `kb_get` theo `section`. Ưu tiên `status: stable`, xem `supersedes`. Câu hỏi nhiều ý: đủ từng ý. **Luôn trích id** |
| **`okfkit-curate`** | Duy trì và nâng cấp bundle | Chạy `okfkit lint --level L2`, sửa theo thứ tự tác động: description thiếu hoặc kém → index.md → tag theo từ vựng → status/supersedes → gộp nội dung trùng bằng trang canonical. Viết description 1 câu cụ thể. Ghi `generated`. Không xoá nội dung |
| **`okfkit-adopt`** | Chuyển thư mục markdown sang chuẩn | Chạy `okfkit adopt --plan`, rà soát, rồi `--out` hoặc `--write`; viết description và tag cho những file heuristic đánh dấu yếu; chạy lint lại |
| **`okfkit-import`** | Đưa PDF/Sheet vào | `okfkit import`, kiểm tra `sources/` và dataset, dùng `distill --plan` để tạo concept có `sources[]` |
| **`okfkit-author`** | Viết tài liệu mới | Dùng `okfkit new --type …` (theo schema), frontmatter đủ L2, link tới trang liên quan, cập nhật từ vựng nếu tag mới |

- Skill **tự thích nghi theo capability**: phần hướng dẫn cho `kb_search` và `data_query` chỉ xuất hiện khi module tương ứng bật (sinh lúc `agent install`).
- **Dự phòng khi không có MCP:** skill hướng dẫn gọi `okfkit … --json` qua shell, để dùng được cả ở CLI chưa cấu hình MCP.
- Dự án có thể thêm skill riêng trong `_meta/skills/`.
- **Cần đo (S9):** so sánh agent có skill và không có skill về tỉ lệ dùng đúng tool (mục tiêu `kb_query` ≥ 45/48 so với 32/48), độ chính xác, số lượt và chi phí.

---

## 7. CLI

```
# Dùng ngay
okfkit status                         # mức L, số tài liệu/token, module, mode khuyến nghị, gợi ý tiếp theo
okfkit agent install --claude|--codex|--opencode [--skills-only|--mcp-only]
okfkit mcp serve [--stdio | --http :7331 --token …]

# Đọc
okfkit grep 'E2|429' [--path 'sop/**'] [-C 1] [--files-only]
okfkit get <id> [--section …] | okfkit list [dir]
okfkit query 'type=Policy tag:billing status!=deprecated active_on=2026-10-01' [--facets tags] [--sum contract_value]
okfkit catalog [--max-tokens 10000]
okfkit data tables | okfkit data sql "select …"              # module data
okfkit search "câu hỏi" | okfkit retrieve "…" --budget 3000  # module embed

# Tổ chức
okfkit lint [--level L2] [--fix-safe] [--format text|json|sarif]
okfkit adopt <dir> --plan | --out <dir> | --write [--level L2 --with-agent claude]
okfkit index [--full] [--write-index-md] [--watch]
okfkit vocab [--suggest] | okfkit new --type <Type> <id> | okfkit validate --okf

# Module
okfkit embed enable [--model …] | okfkit embed status
okfkit import <file|dir> | okfkit source add|sync …
okfkit eval run questions.json [--mode retrieval|agent --cli claude]
okfkit modules                        # module có trong build + đang bật
```

---

## 8. Đa ngôn ngữ

- Lõi: NFKC, bỏ dấu tiếng Việt (grep/FTS), lindera (tiếng Nhật), stemming tiếng Anh.
- Nhận diện ngôn ngữ (lingua) chỉ để gợi ý: vi không dấu chỉ đúng khoảng 90%.
- Từ vựng tag đa ngôn ngữ, gồm đồng nghĩa.
- Không có embedding thì câu hỏi khác ngôn ngữ dựa vào agent tự dịch từ khoá (skill hướng dẫn). S4 cho thấy cách này đủ tốt với Claude.
- Có embedding thì EmbeddingGemma xử lý xuyên ngôn ngữ trực tiếp, cho phép pre-retrieval.

---

## 9. Lint (rule theo mức)

| Mức | Rule |
|---|---|
| L0 | Frontmatter parse được; có `type` |
| L1 | `title`/`description` có mặt và đủ cụ thể (độ dài, không lặp lại title); `index.md` mọi thư mục; ID ổn định |
| L2 | Tag thuộc từ vựng (hoặc báo đồng nghĩa khác ngôn ngữ); field đúng kiểu theo schema của type; `status`/`supersedes` nhất quán; không có nhiều `stable` cùng chủ đề/vùng; `effective_*` hợp lệ; dataset có schema doc |
| L3 | Nội dung gần trùng (cần embed; không có thì dùng heuristic shingle); link gãy; mồ côi; quá `stale_after`; tài liệu quan trọng thiếu `sources`/`verified` |
| Chung | Tài liệu quá dài (gợi ý tách); PDF import có trang trống hoặc bảng lỗi |

Output text/JSON/SARIF; `--fix-safe` chỉ sửa những gì an toàn (sinh index.md, chuẩn hoá tag, thêm `lang`).

---

## 10. Bảo mật

- **Chỉ đọc mặc định**; mọi thao tác ghi cần lệnh hoặc module rõ ràng.
- Chuẩn hoá path, chặn path traversal.
- SQL: `query_only`, chỉ SELECT, timeout, giới hạn số dòng, không ATTACH.
- MCP HTTP: token, bind localhost mặc định; host cung cấp `ScopeProvider`.
- Output kèm `id`/`resource` để host đánh dấu nội dung *untrusted*.
- Secret của source lấy từ env/keyring, không nằm trong `okfkit.toml`.
- `agent-bridge` chỉ chạy khi người dùng gọi, và in rõ lệnh CLI sẽ chạy.

---

## 11. Hiệu năng mục tiêu (8 CPU, theo spike)

| Thao tác | Mục tiêu |
|---|---|
| `okfkit status` / index lexical 1k tài liệu lần đầu | ≤ 5s |
| `grep` trên 4.4M token | ≤ 500ms |
| `query` trên 3k tài liệu | ≤ 20ms |
| `data.query` aggregate 10k dòng | ≤ 50ms |
| `search` (module embed, ≤ 20k chunk) | p50 ≤ 60ms |
| Embed lần đầu | ≈ 3 chunk/s (EmbeddingGemma Q4), chạy nền |
| Binary `okfkit` (không ONNX) | ≤ 100MB (nâng từ 25MB ngày 2026-10-03; hiện ~34MB) |

---

## 12. Kiểm thử và đánh giá

- Golden round-trip: bundle OKF chính thức (acme_retail, ga4, stackoverflow, crypto_bitcoin) phải có diff = 0; test giữ comment và key lạ.
- **Fixture adopt:** docs OpenClaw (Mintlify), một vault Obsidian mẫu, một thư mục docs Docusaurus, và thư mục markdown "bẩn". Kiểm tra `adopt --plan` ổn định (snapshot) và lint sau adopt đạt L1.
- Snapshot output tool và skill sinh ra theo capability.
- `okfkit-eval lexical` (đã có, 2026-10-03): chạy lại tool call thật của agent trong spike S3/S5 trên `fixtures/business` và `fixtures/multilingual`, so với đáp án; vài giây, không cần model; chạy trong `cargo test`, báo lỗi khi một ca từng đạt bị trượt.
- `okfkit-eval` (còn lại, từ spike): chế độ retrieval (R@k, hiện là `examples/retrieval_eval.rs`) và agent (claude/codex, chấm theo key facts hoặc đáp án tính sẵn, chi phí, độ trễ). Fixture: v2 đa ngôn ngữ (S1), OpenClaw S/M/L (S4), business ×1/×20 (S5).
- CI: fmt, clippy, test, deny; eval lexical nhanh trên fixture nhỏ.

---

## 13. Lộ trình

Ước lượng cho 1 dev full-time.

**Trạng thái (2026-10-03):**

| Mốc | Trạng thái | Bằng chứng |
|---|---|---|
| v0.1 | ✅ Xong. G2 tái hiện 28/30 (93%) | `spikes/acceptance-v0.1` |
| v0.2 | ✅ Xong. S5 ×20: 44–45/48, sheet 9–10/10; S8 đạt (adopt không làm giảm). S9: agent không gọi skill; **S15** cho thấy instructions của MCP server thay được (46/48 không cần gợi ý) | `spikes/acceptance-v0.2` |
| v0.3 | ✅ Xong. S1 R@1 0.857; S4 top-6: 29/30 (Gemma), 30/30 (bge-m3) | `spikes/acceptance-v0.3` |
| advise + fine-tune | ✅ Xong (phase 0–5). Agent làm hết quy trình; Q4 giữ được mức cải thiện | `PLAN-advise-tune.md`, `spikes/embed-tune` |
| Tình huống thực tế | ✅ Xong (U1–U8) | `PLAN-usecases.md`, S13b |
| Onboarding qua agent | ✅ Xong (O1–O5) với Claude. **Codex chưa chạy** | `PLAN-onboarding.md`, S13 |
| v0.4 import | ✅ Phần chuyển đổi xong (I1–I5, S14). Còn: connector Google Drive/Sheets, `distill`, S10 với tài liệu thật | `PLAN-import.md` |
| Eval | `okfkit-eval lexical` chạy trong `cargo test`. Chế độ agent vẫn là script trong `spikes/` | §12 |
| v0.5 → v1.0 | Chưa bắt đầu | |

### v0.1 — Dùng ngay với bundle có sẵn (2 tuần)
- core (round-trip), standard (L0–L2, ánh xạ frontmatter ngoại), index (lexical), query (grep v2, get, list, query, catalog, stats), lint L0–L2, MCP stdio, CLI.
- **Skills `okfkit-answer`** + `agent install --claude/--codex`.
- ✅ Tiêu chí:
  - round-trip các bundle OKF chính thức có diff = 0;
  - tái hiện G2 của S4 (≥ 93% ở L) với Claude qua skill + MCP;
  - `okfkit status` chạy trên bundle chỉ đọc (state_dir ngoài).

### v0.2 — Adopt + data (2 tuần)
- `adopt` (nhận dạng site, heuristic, `--plan/--out/--write`), sinh index.md/log.md, vocab, skill `okfkit-adopt`/`okfkit-curate`.
- Module `data` (CSV/XLSX → SQLite, `data.query`, schema doc).
- **Spike S8:** docs markdown thường → adopt → so độ chính xác của agent trước và sau.
- **Spike S9:** có skill và không có skill.
- ✅ Tiêu chí: adopt docs OpenClaw gốc đạt L1 không cần sửa tay; tái hiện S5 ×20 (sheet 10/10); S8 cho thấy adopt không làm giảm (kỳ vọng tăng) độ chính xác.

### v0.3 — Module embed (opt-in) (2 tuần)
- `embed-local` (EmbeddingGemma Q4, bge-m3 int8), `embed-api`, cache, index nền, `search`, `retrieve`, `recommend_mode`, MCP HTTP, `router()` cho host.
- Build `okfkit-full`.
- ✅ Tiêu chí: tái hiện S1 v2 (R@1 ≥ 0.84); retrieval của S4 ≥ 97% ở L; binary `okfkit` mặc định vẫn không có ONNX.

### Đề xuất mới: `okfkit advise` + fine-tune embedding
- Xem `PLAN-advise-tune.md` (chưa chốt thứ tự so với v0.4; phase 0 là gate kỹ thuật ONNX/Q4).

### Đề xuất mới: các tình huống thực tế
- Xem `PLAN-usecases.md`: repo phần mềm có `docs/`, thư mục rỗng, OKF chưa chuẩn, bundle trong thư mục con; bộ quét thư mục, luật bỏ qua, `adopt` an toàn với site docs, profile `docs-site`.

### Đề xuất mới: onboarding qua agent
- Xem `PLAN-onboarding.md`: `okfkit onboard`, hợp đồng máy (JSON, mã lỗi, mã thoát), danh mục consent, `doctor`, bootstrap cho agent; kèm sửa index trong phiên MCP (P0).

### v0.4 — Import + source (2 tuần)
- **Phần chuyển đổi:** xem `PLAN-import.md` (anydoc + htmd, đọc trực tiếp và chuyển hẳn, OCR nhờ agent).
- import-pdf/docx/html, source fs/gdrive/gsheets, skill `okfkit-import`, `distill --plan`.
- **Spike S10:** khoảng 20 PDF và 5 sheet thật.
- ✅ Tiêu chí: agent trả lời đúng câu tra cứu và tổng hợp trên dữ liệu import.

### v0.5 → v1.0 — Mở rộng và ổn định (2–3 tuần)
- Plugin CLI `okfkit-<x>`, tool dạng command, lint rule khai báo, schema type, skill của dự án, eval hoàn chỉnh, watch, ANN, DuckDB.
- **Spike S7:** lexical với Codex và model nhỏ, để chọn profile mặc định cho từng CLI.
- Ổn định API (semver), tài liệu, ví dụ tích hợp.

**Tổng khoảng 10–11 tuần tới v1.0.** v0.1 (tuần 2) đã dùng được hằng ngày với Claude Code/Codex trên bundle có sẵn.

---

## 14. Rủi ro và quyết định mở

| Rủi ro / câu hỏi | Hướng xử lý |
|---|---|
| Lexical chỉ mới đo với Claude Sonnet | Spike S7 (Codex, model nhỏ); profile có thể khác nhau theo CLI |
| Adopt heuristic viết description kém | Đánh dấu `generated`; skill `okfkit-curate` cho agent viết lại; S8 đo tác động |
| Skill không được agent dùng | Đã đo (S9, S15): agent không gọi skill, nhưng quy tắc gửi qua instructions của MCP server thì được làm theo. Skill giữ cho trường hợp không có MCP và cho việc biên tập |
| Tên "OKF" là spec của Google | Tên sản phẩm `okfkit`; ghi rõ "tool cộng đồng" |
| Spec OKF thay đổi | Lõi giữ key lạ; chuẩn okfkit là lớp riêng (`okfkit-standard`) |
| License Gemma | Chỉ liên quan khi bật embed-local; bge-m3 int8 (MIT) thay thế |
| Chất lượng PDF | Converter là trait; S10 |
| Model tự fine-tune: mất mức cải thiện khi lượng tử hoá Q4; câu hỏi tổng hợp khác câu hỏi thật của agent; dữ liệu nội bộ gửi lên LLM | S11: kiểm tra ONNX/Q4 trước khi làm tính năng; eval bằng câu hỏi do người viết; cache vector khoá theo hash model |
| rmcp thay đổi nhanh | Pin minor |

**Đã chốt:** license MIT OR Apache-2.0 (§17); publish crates.io từ v0.1 (bản 0.x); giữ tên mức L0–L3 và tên skill như trên (đổi được trước v1.0).

---

## 15. Quan hệ với dự án khác

- **qobot:** phụ thuộc crate `okfkit` (git tag); dùng `Bundle`, `catalog`, `recommend_mode`, `retrieve` (khi bật embed), `router` với `ScopeProvider`, `write`, `watch`. qobot chọn profile (thường là `full` cho bot chat đa ngôn ngữ cần nhanh). qobot không truy cập index nội bộ của okfkit.
- **agent-core:** chỉ dùng trong module `agent-bridge` (adopt/curate/eval gọi CLI agent).
- **Claude Code / Codex / OpenCode:** qua `okfkit agent install` (MCP + skills).

## 16. Việc cần làm ngay (tuần 1)

1. `git init`, workspace, CI, license.
2. `okfkit-core` round-trip (giữ comment, key lạ) cộng fixture bundle OKF chính thức và docs OpenClaw.
3. `okfkit-index` lexical + `okfkit-query` (dùng lại chunker và grep v2 từ `spikes/embed-bench/src/bundle.rs`, cùng `kb_query` từ `spikes/biz-meta/mcp_meta.py`).
4. CLI `status/grep/get/list/query/catalog` + MCP stdio + bản nháp skill `okfkit-answer`; thử với Claude Code trên fixture OpenClaw S.

---

## 17. Mã nguồn mở

| Hạng mục | Quyết định |
|---|---|
| License | **MIT OR Apache-2.0** (dual, theo thông lệ Rust); `LICENSE-MIT`, `LICENSE-APACHE` lấy nguyên văn từ nguồn chính thức |
| Tên | `okfkit` (crates.io và GitHub còn trống, kiểm tra ngày 2026-10-01). Ghi rõ trong README: *dự án cộng đồng độc lập, không liên kết với Google*; "OKF" là spec của Google Cloud |
| Ngôn ngữ tài liệu | README, rustdoc, CLI help, CONTRIBUTING: **tiếng Anh**. Tài liệu thiết kế nội bộ (`docs/PLAN.md`, `docs/HANDOFF.md`) hiện bằng tiếng Việt; dịch sang tiếng Anh trước khi công khai rộng (v0.2). Hoan nghênh tài liệu vi/ja |
| Governance | Maintainer chính quyết định (BDFL) đến v1.0; thay đổi lớn (format, API công khai, mặc định) cần issue/RFC ngắn trong `docs/rfcs/` kèm số liệu (spike/eval) |
| Đóng góp | PR + review; không CLA; `Signed-off-by` (DCO) khuyến khích; `CONTRIBUTING.md` hướng dẫn build, test, eval; Code of Conduct: Contributor Covenant 2.1 |
| Bảo mật | `SECURITY.md`: báo cáo riêng qua GitHub Security Advisories; không public issue cho lỗ hổng |
| Phiên bản | SemVer; 0.x có thể phá vỡ API nhưng phải ghi CHANGELOG; **định dạng trên đĩa** (frontmatter okfkit, `_meta/*`) ổn định sớm hơn API (từ v0.2) |
| Phát hành | Tag `vX.Y.Z` → CI build binary (Linux x86_64/aarch64, macOS arm64/x86_64, Windows x86_64) bằng cargo-dist, publish crates.io, CHANGELOG sinh từ commit (conventional commits), GitHub Release. Hai bản: `okfkit` và `okfkit-full` |
| CI | GitHub Actions: fmt, clippy `-D warnings`, test (Linux/macOS/Windows), cargo-deny (license allowlist + advisory), MSRV check, eval lexical nhanh trên fixture nhỏ |
| Phụ thuộc bên thứ ba | cargo-deny allowlist: MIT, Apache-2.0, BSD-2/3, ISC, Unicode-3.0, Zlib, MPL-2.0 (chỉ khi không sửa). Kiểm tra license của từ điển lindera IPADIC và ghi vào `THIRD_PARTY.md` |
| Model | **Không đóng gói model trong repo hay binary.** `okfkit embed enable` tải model về cache người dùng, in license của model và yêu cầu xác nhận với model có điều khoản riêng (Gemma); bge-m3 (MIT) là lựa chọn không cần xác nhận |
| Dữ liệu fixture | Sample OKF của Google (Apache-2.0) và docs OpenClaw (MIT): chỉ đưa subset nhỏ vào `fixtures/` kèm file NOTICE/attribution; corpus lớn tải bằng script, không commit |
| Spike | `spikes/` được commit (script, câu hỏi, `results/`, ~7MB); cache, model, corpus, bundle đã dựng nằm trong `.gitignore`. Bản ghi trong `results/` chứa đường dẫn máy cục bộ và là dữ liệu tổng hợp hoặc sinh bởi LLM; ghi chú điều này trong `spikes/README.md` |
| Telemetry | Không có |
| Hỗ trợ nền tảng | Linux, macOS, Windows (lõi); module embed-local: Linux/macOS x86_64 + arm64, Windows x86_64 (theo onnxruntime) |
