# Kế hoạch: onboarding qua agent (agent-friendly setup)

Trạng thái: **O0–O5 xong** với Claude (2026-10-02) và Codex (2026-10-03, `spikes/codex`); kết quả ở §6. Lỗi sandbox Codex chặn ghi `.codex/` đã xử lý (`sandbox_blocked`, bước cho người dùng trong `onboard`). Bổ sung cho `PLAN.md` và `PLAN-advise-tune.md`.

## 0. Mục tiêu

Người dùng chỉ cần nói với agent (Claude Code, Codex, …): **"thiết lập okbase cho thư mục này"**.
Agent tự làm mọi bước: cài, phân tích, cấu hình, kiểm tra. Agent chỉ hỏi lại người dùng ở những
quyết định thuộc về họ: license, tải dữ liệu lớn, gửi tài liệu cho LLM, sửa hoặc xoá file.

Muốn vậy, CLI phải là **giao diện cho agent**, không chỉ cho người:
1. Mỗi lệnh cho agent biết kết quả, bước tiếp theo và chỗ nào cần hỏi người dùng.
2. Đầu ra, mã thoát và lỗi ổn định để agent xử lý được bằng máy.
3. Không bao giờ chờ nhập tương tác; mọi thao tác ghi, xoá hoặc tải đều cần cờ rõ ràng.

Kèm theo là 3 việc còn treo từ lần rà soát vòng đời MCP (§2).

## 1. Hiện trạng (rà soát 2026-10-02)

| | Hiện trạng | Vấn đề cho agent |
|---|---|---|
| Hỏi tương tác | Không có (chỉ `tune submit -` đọc stdin) | Tốt |
| `--json` | 31 chỗ, gồm mọi lệnh đọc, `tune`, `clean`, `agent status` | Lệnh ghi như `agent install/uninstall`, `embed enable/disable`, `adopt`, `dict install` chỉ in text |
| Lỗi | Text trên stderr (`error:` / `hint:`) | Không có mã lỗi để agent phân nhánh |
| Mã thoát | Lỗi nào cũng là 1; `lint` có lỗi cũng 1; `tune submit` bị từ chối cũng 1 | Không phân biệt "có phát hiện", "cần người dùng đồng ý" và "sai tham số" |
| Bước tiếp theo | Có ở `tune` (`next`) và `advise` | Các lệnh khác không có |
| Khởi đầu | README cho người | Agent chưa biết bắt đầu từ đâu, và cũng chưa biết cài okbase |

## 2. Việc treo từ rà soát vòng đời MCP (làm trước, P0)

1. **Index cũ trong phiên MCP.** Server stdio và HTTP chỉ đồng bộ index lúc khởi động; tài liệu sửa giữa phiên (kể cả do chính agent sửa theo `okbase-curate`) không được thấy.
   - **Sửa:** trước mỗi lần gọi tool, nếu lần đồng bộ trước đã quá 2 giây thì đồng bộ dần (chỉ kiểm tra thời gian sửa file, index lại file đổi). Server HTTP làm tương tự, có khoá để tránh đồng bộ chồng.
   - **Test:** sửa file giữa phiên thì `kb_grep` thấy ngay.
2. **Phạm vi Codex.** MCP của Codex hiện luôn là toàn cục (`~/.codex/config.toml`). Kiểm chứng Codex 0.159 có hỗ trợ `.codex/config.toml` theo project hay không.
   - Nếu có: `--codex` cài vào project, `--user` cài toàn cục, giống Claude Code.
   - Nếu không: giữ như hiện tại và ghi rõ trong tài liệu.
3. **Tài liệu vòng đời và phạm vi** trong `docs/usage.md`:
   - stdio: agent tự bật và tắt server, mỗi phiên một tiến trình, N phiên × RAM model.
   - HTTP: chạy thủ công hoặc qua systemd/Docker, Ctrl-C để dừng gọn.
   - Bảng phạm vi project / user cho từng agent.

## 3. Thiết kế

### 3.1 Điểm vào duy nhất: `okbase onboard`

Agent chỉ cần nhớ **một lệnh**. `okbase onboard` đọc trạng thái thật của máy và bundle rồi in
**kế hoạch thiết lập dành cho agent**, cập nhật lại sau mỗi lần chạy:

```
$ okbase onboard            # trong thư mục bundle (hoặc -b DIR)
okbase onboarding: /home/u/kb (287 docs, ~1.1M tokens, en 92% / vi 8%, level L1)

Done
  ✓ okbase 0.3.0 (okbase-full: embeddings, fine-tuning)
  ✓ index up to date

To do (run in order; stop at every ASK and wait for the user's answer)
  1. [run]  okbase agent install --claude                       writes .mcp.json, .claude/skills
  2. [ask]  "41 documents have no description. May I write descriptions and an index.md
            into /home/u/kb? (files change; review with git diff)"
            yes → follow the okbase-curate skill    no → skip
  3. [ask]  "People ask in Vietnamese but the documents are English. Semantic search needs the
            EmbeddingGemma model (188 MB, Gemma Terms of Use: https://ai.google.dev/gemma/terms).
            Do you accept the license and the download? (MIT alternative: bge-m3, 570 MB, weaker)"
            yes → okbase embed enable --accept-license && okbase embed index
            alt → okbase embed enable --model bge-m3-int8 && okbase embed index
  4. [run]  okbase doctor                                       verify
  5. [tell] "Restart the agent session (or /mcp) to load the okbase tools."

Rules for agents: never pass --accept-license, --yes, --write, --force or --replace unless the user
agreed to that step; never edit documents without asking.
```

- **Có `--json`:** mỗi bước có `id`, `kind` (`run` / `ask` / `tell`), `command`, `writes` (đường dẫn), `question` (câu hỏi gợi ý cho người dùng), `options` (đáp án → lệnh tương ứng), `done_when` (lệnh kiểm tra xong), `why` (lý do, trích spike).
- **Dựa trên phần sẵn có:** `advise` cho biết nên làm gì, `agent status` / `embed status` / `lint` cho biết đã làm gì, và danh mục consent (§3.3) cho biết bước nào phải hỏi.
- **Chạy lặp lại được:** chạy lại sau mỗi bước, các bước đã xong chuyển sang mục "Done". Agent cứ lặp "onboard → làm bước đầu tiên chưa xong" cho đến khi danh sách trống.
- **Tuỳ chọn:** `--for claude|codex|team|host`, `--user-langs`, `--private` (giống `advise`), và `--goal` (`answer` | `curate` | `serve-team`) để rút gọn kế hoạch.

### 3.2 Hợp đồng máy (áp dụng cho mọi lệnh)

| Hạng mục | Quy ước |
|---|---|
| `--json` | Mọi lệnh, kể cả lệnh ghi. Lệnh ghi trả `{"changes": [{action, path, why}], "next": [...]}` |
| `next` | Mọi kết quả JSON có `next`: danh sách lệnh gợi ý tiếp (có thể rỗng) |
| Lỗi | Với `--json`, lỗi in ra stdout dạng `{"error": {"code", "message", "hint", "next"}}`. `code` cố định, có tài liệu (ví dụ `bundle_not_found`, `name_conflict`, `license_required`, `consent_required`, `not_built`, `invalid_argument`) |
| Mã thoát | 0 thành công · 1 lỗi · 2 sai cú pháp (clap) · **3 cần người dùng đồng ý** (thiếu `--yes` / `--accept-license` / `--write`) · **4 có phát hiện** (`lint` có lỗi, `tune submit` bị từ chối, gate không đạt) |
| Không tương tác | Cam kết: không lệnh nào chờ nhập từ TTY. Có test quét mọi lệnh với stdin đóng |
| Xem trước | Mọi lệnh ghi đều có `--print` (hoặc mặc định chạy thử như `clean`) |
| Ổn định | Tên trường JSON và mã lỗi được snapshot-test; thay đổi nghĩa là thay đổi phiên bản |

### 3.3 Danh mục consent: những gì agent phải hỏi

Nhúng trong binary (`okbase onboard --json` trả kèm; skill và guide trích từ đây):

| Hành động | Cờ cần người dùng đồng ý | Câu hỏi gợi ý phải nêu |
|---|---|---|
| Chấp nhận license model (Gemma) | `--accept-license` | Tên license, URL, dung lượng, lựa chọn MIT |
| Tải model, từ điển, môi trường Python | `--yes` (tune setup/train), `embed index` | Dung lượng, nơi lưu |
| Gửi đoạn tài liệu cho nhà cung cấp LLM (tune) | (bước agent tự làm) | Tài liệu có được phép gửi không |
| Sửa tài liệu (`adopt --write`, `lint --fix-safe`, `vocab --write`, curate) | `--write` / sửa file | File nào đổi; khuyên dùng git |
| Ghi cấu hình agent | `agent install` (không cần cờ) | Báo trước cho người dùng; hỏi nếu là `--user` |
| Ghi đè server khác cùng tên | `--replace` | Đang trỏ tới đâu, sẽ trỏ tới đâu |
| Xoá dữ liệu | `clean --yes`, `agent uninstall --all` | Dung lượng, cái gì sẽ mất |
| Bỏ qua gate chất lượng | `tune activate --force` | Số liệu gate |

Thiếu cờ thì lệnh thoát **mã 3**, kèm `error.code = consent_required` và trường `question` chứa sẵn câu cần hỏi. Agent chỉ việc chuyển câu đó cho người dùng.

### 3.4 Kiểm tra: `okbase doctor`

Gộp `agent status`, `embed status`, `dict status`, độ mới của index và build features thành **một báo cáo đạt/không đạt**, có `--json`, mỗi lỗi kèm lệnh sửa. Thêm **thử MCP thật**: tự chạy `okbase mcp serve --stdio` cho từng bundle đã cài, gửi `initialize` → `tools/list` → `kb_catalog`. Nhờ vậy agent chứng minh được thiết lập chạy được trước khi bảo người dùng khởi động lại.

### 3.5 Agent tìm ra okbase như thế nào (bootstrap)

1. **Chưa cài okbase:** README có mục **"For agents"** ngắn, kèm file `llms.txt` ở gốc repo, cho biết cách cài (binary phát hành hoặc `cargo install`), rồi chạy `okbase onboard` và làm theo. Người dùng chỉ cần dán link repo cho agent.
2. **Đã cài okbase, chưa cài skill:** `okbase --help` có dòng đầu "Agents: start with `okbase onboard`". Lỗi "chưa thiết lập" cũng gợi ý `onboard`.
3. **Đã thiết lập:** các skill `okbase-*` lo việc hằng ngày. Skill mới **`okbase-setup`**: khi người dùng nhờ thêm bundle, bật tính năng, chuyển máy hay gỡ, thì chạy `okbase onboard` (hoặc `--goal`), tuân thủ danh mục consent và xác nhận bằng `doctor`.
4. **Gỡ bỏ cũng qua agent:** `okbase onboard --goal remove` in kế hoạch gỡ: `agent uninstall --all`, rồi `clean --all --yes` (hỏi người dùng), rồi lệnh gỡ binary.

### 3.6 Nội dung hướng dẫn cho agent: một nguồn, nhiều nơi hiển thị

Viết một lần, trong binary:
- **Quy tắc consent** (§3.3) và **bảng lệnh** (sinh từ định nghĩa clap): `okbase help --agent` in một trang gọn (< 2k token) gồm hợp đồng máy, danh mục consent, các lệnh chính và ví dụ.
- **Hiển thị lại ở:** đầu ra `onboard`, skill `okbase-setup`, khối AGENTS.md cho Codex, `llms.txt`.
- **Snapshot test** để các bản không lệch nhau.

## 4. Lộ trình

| Phase | Nội dung | Tiêu chí xong |
|---|---|---|
| **O0** (P0) | §2: index tự đồng bộ trong phiên MCP; phạm vi Codex; tài liệu vòng đời | Sửa file giữa phiên thì tool thấy ngay; bảng phạm vi được kiểm chứng trên Codex 0.159 |
| **O1** | Hợp đồng máy §3.2: JSON cho lệnh ghi, `next`, mã lỗi, mã thoát 3/4, test không tương tác | Snapshot JSON cho mọi lệnh; test chạy mọi lệnh với stdin đóng không bị treo |
| **O2** | `okbase onboard` (text + JSON), danh mục consent §3.3, `help --agent` | Kế hoạch đúng trên fixtures (small/medium/large, có bảng, đa ngôn ngữ); chạy lại thì các bước đã xong biến mất |
| **O3** | `okbase doctor` kèm thử MCP thật | Phát hiện bundle đã di chuyển, binary mất, license chưa chấp nhận, MCP không khởi động được |
| **O4** | Bootstrap §3.5: mục "For agents" trong README, `llms.txt`, skill `okbase-setup`, `onboard --goal remove` | Agent chỉ có link repo vẫn tự cài và thiết lập được |
| **O5** | **Spike S13:** Claude Code và Codex trên HOME sạch, chỉ với câu "set up okbase for this folder" | ≥ 9/10 lần hoàn tất; 100% các bước consent được hỏi (0 lần tự thêm `--accept-license`/`--yes`/`--write`); đo số lượt, thời gian, chi phí |

Ước tính: O0 khoảng 1 ngày; O1–O3 khoảng 1 tuần; O4–O5 khoảng 3–4 ngày.

## 5. Quyết định (đã chốt 2026-10-02)

1. Điểm vào: **`okbase onboard`**.
2. Mã thoát 3 (cần đồng ý) và 4 (có phát hiện) được áp dụng trước v1.0; `lint` có lỗi chuyển từ 1 sang 4.
3. `agent install` cấp project: agent chỉ báo trước, không cần hỏi; `--user` phải hỏi.
4. Chưa phát hành binary (đang phát triển). Sau này mở mã nguồn và publish crates.io, nên bootstrap
   dùng `cargo install okbase-cli` (sau khi publish) và tạm thời `cargo install --git <repo>` / `--path`.

## 6. Triển khai

| Phase | Commit | Ghi chú |
|---|---|---|
| O0 | `c07acae` | Index tự đồng bộ trước mỗi lần gọi tool (quá 2 giây); search tự embed tối đa 64 chunk mới. Codex 0.159 đọc `<project>/.codex/config.toml` cho project đã được trust (đã kiểm chứng), nên `--codex` giờ cài theo project |
| O1–O2 | `903a1f5` | Mã lỗi, JSON lỗi, mã thoát 3/4, JSON cho lệnh ghi; `okbase onboard` (run/ask/tell, `--goal`); `help --agent`; danh mục consent |
| O3 | `ee9d65c` | `okbase doctor`, kèm lượt gọi MCP thật |
| O4 | `6167059` | README "For agents", `llms.txt`, skill `okbase-setup`, khối AGENTS.md trỏ tới `onboard`; test giữ các bản hướng dẫn thống nhất |
| O5 | `be73fa6` | S13 với Claude: 3/3 tự thiết lập, 0 lần tự thêm cờ đồng ý (`spikes/onboarding/RESULTS.md`). Sửa 3 lỗi do agent phát hiện |
| O5 (Codex) | `b98cebf` (sửa) | S13 với Codex (gpt-6.1-sol, gpt-6-luna; 6 thư mục): 12/12 tìm ra `onboard`, 0 lần tự thêm cờ đồng ý. Sandbox workspace-write của Codex chặn ghi `.codex/`: `agent install --codex` chỉ báo "Permission denied" mà không có gợi ý (`spikes/codex/RESULTS.md`) |

Khác với kế hoạch: trường `next` có ở lệnh thiết lập, lỗi và các lệnh `onboard/doctor/tune`. Lệnh
đọc giữ nguyên JSON của tool MCP (luật AGENTS.md: schema `--json` = đầu ra tool MCP).
