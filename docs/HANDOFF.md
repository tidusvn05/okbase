# okbase — Bàn giao triển khai

> Ngày: 2026-10-01 · Người bàn giao: phiên thiết kế và spike (Claude Code)
> Đối tượng: agent hoặc dev sẽ triển khai okbase. Đọc hết file này trước khi viết code.

## 1. Bạn đang nhận gì

| Tài liệu | Vai trò |
|---|---|
| `docs/PLAN.md` (v2.1) | **Nguồn sự thật** về thiết kế: chuẩn L0–L3, kiến trúc, module/profile, tool, skill, lộ trình, OSS (§17) |
| `docs/HANDOFF.md` (file này) | Cách bắt đầu, thứ tự việc, tiêu chí hoàn thành, những điều không được làm |
| `AGENTS.md` (gốc repo, tiếng Anh) | Quy ước làm việc hằng ngày cho mọi agent/contributor |
| `spikes/` | Bằng chứng thực nghiệm và **code mẫu dùng lại được**. `spikes/README.md` có bảng tổng hợp |

Trạng thái (2026-10-03): v0.1–v0.3 đã xong và qua nghiệm thu; các plan bổ sung (advise/tune, tình huống thực tế, onboarding, import) đã triển khai. Xem `docs/PLAN.md` §13 (Trạng thái). Repo có lịch sử commit nhưng **chưa push** (chưa có tổ chức GitHub). File này giữ lại làm tài liệu bàn giao ban đầu cho v0.1; các quy tắc ở §5 vẫn áp dụng.

Dự án là **mã nguồn mở** (MIT OR Apache-2.0). Mọi thứ công khai (README, rustdoc, CLI help, commit message, CONTRIBUTING) viết bằng **tiếng Anh**.

## 2. Tóm tắt thiết kế trong 10 dòng

1. okbase = CLI + MCP server + thư viện Rust, giúp agent làm việc với bundle markdown theo **OKF v0.2**.
2. **Mặc định lexical, không có model:** catalog + `grep` v2 + `query` (metadata) + `get`/`list`. Spike S4: độ chính xác ngang embedding.
3. **Embedding là module opt-in** (`embed-local`: EmbeddingGemma-300M Q4 hoặc bge-m3 int8; `embed-api`), đến ở v0.3.
4. **Dữ liệu bảng cần SQL** (module `data`, v0.2). Spike S5: không có SQL thì agent bỏ cuộc với sheet ~10k dòng.
5. **Chuẩn okbase L0–L3** + `okbase lint --level` + `okbase adopt` (markdown thường → OKF).
6. **Skills** (`okbase-answer`, `-curate`, `-adopt`, `-import`, `-author`), cài bằng `okbase agent install --claude|--codex`.
7. **Chỉ đọc mặc định.** Round-trip giữ nguyên key lạ, thứ tự và comment. Index có thể đặt ngoài bundle.
8. **Không biết người dùng:** mọi thao tác đọc nhận `Scope` do host truyền vào (qobot là host đầu tiên).
9. Mở rộng qua **capability registry**, profile (`minimal`/`standard`/`full`), `_meta/types`, `_meta/vocabulary.md`, plugin CLI `okbase-<x>`, trait Rust.
10. Mọi mặc định đều gắn với một spike. **Muốn đổi mặc định thì phải có số liệu** (eval).

## 3. Code mẫu từ spike, nên dùng lại

| Cần làm | Xem | Ghi chú |
|---|---|---|
| Chunker H2/H3 (bỏ qua code fence, gộp < min, cắt > max) | `spikes/embed-bench/src/bundle.rs` → `chunk_doc` | Đang dùng tokenizer Gemma để đếm token; lõi không có model nên cần bộ đếm xấp xỉ (§6 T4) |
| **grep v2** (regex, bỏ dấu, frontmatter, path glob, context, files_only, gợi ý khi rỗng) | `bundle.rs` → `grep`, `fold`, `glob_re` | Đã đo: G lên G2 tăng 3–7 điểm, câu tiếng Việt 85% → 95% |
| `get` theo section, cắt bớt kèm danh sách heading | `bundle.rs` → `serve()` nhánh `kb_get` | |
| **kb_query** (filter, `active_on`, facets, sum, sort) | `spikes/biz-meta/mcp_meta.py` → `match`, `kb_query` | Ngữ nghĩa đã được kiểm chứng bằng 336 lượt |
| **data_query** (SQLite `query_only`, SELECT only, giới hạn số dòng) | `mcp_meta.py` → `data_query`, `data_tables` | Thêm timeout (progress handler) |
| MCP stdio tối giản (JSON-RPC) | `bundle.rs` → `serve()` | Bản production dùng **rmcp** |
| Analyzer: NFKC, bỏ dấu tiếng Việt, lindera | `spikes/embed-bench/src/main.rs` → `Analyzer`, `fold_latin` | lindera 6: `load_dictionary("embedded://ipadic")` |
| Embedding (v0.3) | `main.rs` → `load()`; `bundle.rs` → `index()` (cache theo hash, sắp xếp theo độ dài) | fastembed phải tắt default features, dùng rustls |
| Eval với agent CLI thật | `spikes/okf-scale/run.py`, `report.py`; `spikes/biz-meta/run.py`, `report.py` | Mẫu cho `okbase-eval` |
| Chuyển docs có frontmatter lạ (Mintlify `summary`) sang OKF | `spikes/okf-scale/build_bundles.py` | Mẫu cho `adopt` |

**Lưu ý:** code spike viết để đo nhanh, không theo chuẩn production (unwrap, không có test). Hãy chép **logic và ngữ nghĩa**, không chép nguyên xi.

## 4. Quy ước kỹ thuật đã chốt

| Mục | Giá trị |
|---|---|
| Rust | edition 2024, stable; **MSRV 1.89** (serde-saphyr ≥ 1.2; rmcp 3.x cần 1.88); workspace `resolver = "3"` |
| Lint | `clippy -D warnings`; `unsafe_code = "forbid"` ở mọi crate trừ khi có lý do ghi rõ |
| Lỗi | `thiserror` trong lib, `anyhow` chỉ trong CLI |
| Async | tokio; lõi đọc đồng bộ được (API sync + async wrapper), vì CLI và grep không cần async |
| YAML | **serde-saphyr** (không dùng serde_yaml, serde_yml). **Round-trip giữ comment không thể làm bằng serde** → lưu raw frontmatter text và sửa theo từng key ở mức text (xem T1) |
| SQLite | rusqlite 0.40 (`bundled`), FTS5 |
| Markdown | pulldown-cmark (heading, link); không render lại body |
| Tiếng Nhật | lindera 6 (`embed-ipadic`); tiếng Việt: unicode-normalization |
| CLI | clap 4 (derive); mọi lệnh đọc có `--json` |
| MCP | rmcp 3.5 (pin minor), stdio ở v0.1, streamable HTTP ở v0.3 |
| TLS | chỉ **rustls** (không OpenSSL) |
| Log | tracing; không có telemetry |
| Tên tool MCP | `kb_catalog`, `kb_list`, `kb_grep`, `kb_get`, `kb_query`, `kb_links`, `data_tables`, `data_query`, `kb_search` (embed), `kb_write` (write); prefix đổi được |

Phiên bản crate được ghi theo lần kiểm tra ngày 2026-09-30. Dùng bản mới nhất tương thích khi bắt đầu, và ghi vào `Cargo.lock`.

## 5. Không được làm

1. **Không đưa embedding, ONNX hay model vào lõi** hoặc vào build mặc định `okbase`.
2. **Không ghi vào bundle** khi người dùng chưa yêu cầu rõ (`--write`, `--fix-safe`, module `write`).
3. **Không phá round-trip:** parse → write không đổi gì thì file phải giống byte-by-byte.
4. **Không trộn BM25 với vector** trong ranking (spike S2).
5. **Không sinh view đầy đủ** (bảng mọi tài liệu theo tag); chỉ sinh từ vựng và facet (S5).
6. **Không tự quyết định phân quyền theo người dùng**; chỉ thực thi `Scope` được truyền vào.
7. **Không đổi mặc định** (mode, chunk size, output tool, nội dung skill) khi chưa có eval chứng minh.
8. Không commit cache, model, corpus hay bundle đã dựng (`.gitignore` đã có sẵn).
9. Không dùng tên hay logo gây hiểu nhầm là sản phẩm của Google.

## 6. Việc cho v0.1 (mục tiêu 2 tuần) — theo thứ tự

Mỗi task có tiêu chí hoàn thành. Làm tuần tự T0 → T9; T10 và T11 có thể làm song song khi T5 xong.

**T0 — Khởi tạo repo OSS**
- `git init`, workspace, crate rỗng: `okbase-core`, `okbase-standard`, `okbase-analyze`, `okbase-index`, `okbase-query`, `okbase-lint`, `okbase-mcp`, `okbase-skills`, `okbase` (facade), `okbase-cli`.
- `LICENSE-MIT`, `LICENSE-APACHE` (nguyên văn chính thức), `CODE_OF_CONDUCT.md` (Contributor Covenant 2.1, nguyên văn), cập nhật `CONTRIBUTING.md`, `SECURITY.md`, `THIRD_PARTY.md`.
- CI (GitHub Actions): fmt, clippy, test trên 3 OS, cargo-deny, MSRV.
- ✅ CI xanh trên workspace rỗng; `cargo deny check` pass.

**T1 — `okbase-core`: model và round-trip**
- `Concept { id, path, frontmatter: Frontmatter, body }`. `Frontmatter` giữ **raw text** và bản đã parse (serde-saphyr → `Value` có thứ tự).
- `set(key, value)` / `remove(key)` sửa text ở đúng khối của key top-level (giữ comment, thứ tự, định dạng của key khác). Key mới được thêm vào cuối.
- Validate OKF v0.2 (L0); trích link (markdown thường + wikilink); ID = path bỏ `.md`.
- ✅ Round-trip byte-identical trên 4 bundle OKF chính thức và fixture docs OpenClaw; test `set` một key chỉ đổi đúng các dòng của key đó; property test với frontmatter ngẫu nhiên.

**T2 — `okbase-standard`**
- Kiểm tra mức L0–L2 (§2 PLAN); ánh xạ frontmatter ngoại khi đọc (`summary`/`excerpt` → description, `categories` → tags, `sidebar_label` → title); đọc `_meta/vocabulary.md` (tag chuẩn + đồng nghĩa).
- ✅ Báo đúng mức cho fixture (bundle OKF chính thức ≈ L1; docs OpenClaw gốc: L0 fail nhưng đọc được qua ánh xạ).

**T3 — `okbase-analyze`**
- `fold()` (NFKC, bỏ dấu, `đ`→`d`, lowercase); tách từ tiếng Nhật cho FTS (lindera); stemming tiếng Anh; nhận diện ngôn ngữ (lingua, chỉ vi/en/ja, chỉ làm gợi ý).
- ✅ Unit test các ví dụ từ spike (`đổi trả` ~ `doi tra`, `ＡＢＣ` → `abc`, `食べた` → `食べる`).

**T4 — `okbase-index`**
- Schema SQLite v1 (§4.7 PLAN, trừ `chunk_vecs`): `docs`, `doc_fields`, `doc_tags`, `aliases`, `links`, `chunks`, `chunks_fts`, `meta(schema_version)`.
- Index tăng dần theo hash; `state_dir = auto` (`.okbase/` nếu bundle ghi được, còn không thì `~/.cache/okbase/bundles/<hash>`).
- **Bộ đếm token xấp xỉ** không cần model. Từ spike: en ≈ 1.24 token/từ, vi ≈ 1.28 token/âm tiết, ja ≈ 0.53 token/ký tự. Ghi rõ đây là ước lượng.
- ✅ Index bundle 1k tài liệu lần đầu ≤ 5s; sửa 1 file chỉ reindex file đó; xoá `.okbase/` rồi index lại cho kết quả giống hệt.

**T5 — `okbase-query`**
- Port `grep` v2, `get`, `list` (index.md hoặc catalog ảo nếu thiếu), `query` (theo `mcp_meta.py`, dùng bảng đã có kiểu), `catalog` (≤ N token: phẳng + từ vựng + facet; lớn hơn: index.md gốc + facet), `stats`, `recommend_mode` (Full nếu ≤ ~30k token; không thì Lexical).
- Mọi hàm nhận `&Scope`.
- ✅ Snapshot output; hiệu năng theo §11 PLAN (grep 4.4M token ≤ 500ms, query 3k tài liệu ≤ 20ms); test Scope chặn đúng path và filter.

**T6 — `okbase-lint`**
- Rule engine + rule L0–L2 (§9 PLAN); output text/JSON/SARIF; `--fix-safe` (sinh index.md, chuẩn hoá tag).
- ✅ Bắt đúng các lỗi gài sẵn trong fixture `fixtures/lint/`.

**T7 — `okbase-mcp` (stdio)**
- rmcp; tool registry theo capability; tên và mô tả tool theo §5 PLAN. Mô tả là "prompt" cho agent, nên viết cẩn thận, lấy từ spike.
- ✅ Chạy được với `claude --mcp-config` và Codex; snapshot `tools/list`.

**T8 — `okbase-skills` + `agent install`**
- Skill `okbase-answer` (template theo capability, có dự phòng dùng CLI `--json`); `okbase agent install --claude` (đăng ký MCP + ghi skill vào thư mục skill của project hoặc user), `--codex` (cấu hình MCP + đoạn `AGENTS.md`), `--print`.
- ✅ Cài trên máy sạch, agent liệt kê được tool và nạp được skill.

**T9 — `okbase-cli`**
- `status`, `index`, `grep`, `get`, `list`, `query`, `catalog`, `lint`, `mcp serve --stdio`, `agent install`, `modules`; `--json` cho mọi lệnh đọc; thông báo lỗi có gợi ý.
- ✅ Kiểm thử CLI bằng `assert_cmd` + snapshot.

**T10 — Fixtures**
- `fixtures/okf-official/` (subset sample của Google, Apache-2.0, kèm NOTICE).
- `fixtures/openclaw-s/` (khoảng 30 file docs OpenClaw, MIT, kèm NOTICE).
- `fixtures/multilingual/` (bộ v2 từ `spikes/embed-bench/data/v2`).
- `fixtures/business/` (×1 từ `spikes/biz-meta`).
- `fixtures/lint/` (lỗi gài sẵn).

**T11 — Nghiệm thu v0.1 bằng eval** (tốn quota CLI, chạy tay)
- Dùng lại `spikes/okf-scale/run.py` với cấu hình G2, nhưng trỏ MCP sang `okbase mcp serve --stdio`, có và không có skill.
- ✅ Bundle L: ≥ 93% đúng (bằng G2 của spike). Ghi kết quả vào `spikes/acceptance-v0.1/`.

## 7. Sau v0.1

Theo §13 PLAN:
- v0.2: adopt + data; spike S8 (adopt) và S9 (skill).
- v0.3: embed opt-in, MCP HTTP, `router()`.
- v0.4: import/source; spike S10.
- v1.0: mở rộng, ổn định; spike S7 (Codex, model nhỏ).

Trước khi công khai rộng (v0.2), dịch `docs/PLAN.md` sang tiếng Anh (`docs/design.md`).

## 8. Người dùng hạ nguồn: qobot

qobot (`/home/beebiz/workspace/qobot`, xem `docs/PLAN.md` §5.1) chỉ dùng facade công khai: `Bundle::open`, `capabilities`, `catalog`, `recommend_mode`, `retrieve` (v0.3), `grep/get/query`, `data()`, `write`, `watch`, `okbase_mcp::router(bundle, ScopeProvider)`. Giữ ổn định các chữ ký này, và báo thay đổi qua CHANGELOG.

## 9. Câu hỏi còn mở (mặc định đã chọn, đổi được)

| Câu hỏi | Mặc định |
|---|---|
| Tổ chức/tài khoản GitHub để host repo | Chưa có. Hỏi maintainer trước khi push |
| Chủ sở hữu copyright trong LICENSE | "The okbase contributors" |
| Tên mức L0–L3, tên skill | Như PLAN; có thể đổi trước v1.0 |
| Publish crates.io từ v0.1 | Có (0.x) |
