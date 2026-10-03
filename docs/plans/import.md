# Plan: document import (PDF, Office, HTML) — v0.4

Status: **I1–I5 done** (2026-10-02), plus S14 (exporting scanned page images); results in §7. Evidence: `spikes/import-bench/RESULTS.md`
(S10-lite), `spikes/page-image-bench/RESULTS.md` (S14). Replaces the conversion part of the "v0.4 — Import + source" item in [`../design.md`](../design.md) §13 (connectors
for Google Drive/Sheets remain deferred).

## 0. Decisions (2026-10-02)

1. **Converters:** anydoc (Office, PDF via pdf-inspector) and htmd (HTML), wrapped in
   `okbase-convert`. No models, no service calls, **never OCR through an external service**.
2. **Two modes:**
   - **direct read** (default, writes no files): source files are indexed directly, conversion results
     are stored in the index (cached by hash);
   - **full conversion** (`import --write`): writes markdown with provenance for editing and distillation.
3. **Included in the default build** (the binary grows by about 10 MB).
4. Also: the data module additionally reads `.xls/.xlsm/.xlsb/.ods`; `.txt` (and `.rst`/`.adoc` as plain
   text) are indexed.

## 1. The `okbase-convert` wrapper

- `convert(path, bytes) -> Converted { markdown, title, format, pages, flags }`.
- **PDF:** uses `pdf_inspector::extract_pages_markdown_mem` directly, page by page:
  - text pages are kept as is, with `<!-- page N -->` page markers for citation;
  - **a scanned page does not break the whole document**: it leaves a "page N needs OCR" block and is recorded in `flags`;
  - multi-column pages are recorded in `flags.columns` for a person or agent to review;
  - the title comes from the `/Title` metadata, or the first heading.
- **Office/EPUB/RTF:** anydoc. Spreadsheets (`xlsx`, `xls`…) do **not** go through here: the data module serves
  them with SQL.
- **HTML:** drop `script/style/nav/header/footer/aside`, prefer `<main>`/`<article>`/`#main-content`;
  htmd converts; tables without a header row are rebuilt as markdown tables.
- **Plain text** (`.txt`, `.rst`, `.adoc`): kept as is, title taken from the first line.
- **Limits:** skip files > 64 MB (reporting why); password-protected files report `encrypted`; errors do not
  stop the whole sync pass.

## 2. Direct read (index)

- The file walker (which already respects `.gitignore`/`.okbaseignore`) also picks up source extensions. A document's id
  is its path including the extension (`manuals/printer.pdf`), so it does not collide with `printer.md`.
- Synthesized frontmatter:
  - `type: Source`, `title`, `description` (first sentence);
  - `source: {path, format, pages}`;
  - `generated` (the fields filled in by okbase).
- All tools (`grep`, `get`, `search`, `catalog`) work on these documents as on markdown.
- A `sources` table in the index records each file's state: `ok | partial (needs OCR) | encrypted | error`,
  plus the lists of pages needing OCR and multi-column pages.
- **If already fully converted** (a markdown file declares `source.path` pointing to the original file), the direct-read
  copy is dropped, to avoid duplicates.

## 3. Full conversion: `okbase import`

| Command | What it does |
|---|---|
| `import --plan` (default, read-only) | Lists files, formats, page counts, pages needing OCR, failed files |
| `import --write [--out sources]` | Writes `sources/<path>.md` with provenance frontmatter (`source.path`, `source.hash`, `converter`, `imported`), `generated`, plus any OCR text already available; **automatically splits** long documents (> 6k tokens) by level 1–2 headings into several parts |
| Rerunning `import --write` | Only rewrites files whose source changed **and** whose markdown has not been hand-edited (hash compared at import time); hand-edited copies are reported as conflicts and need `--force` |
| `import status` | `stale` (source changed), `orphan` (source gone), `needs-ocr`, `columns`, `edited` |

## 4. OCR by the agent (no OCR engine in okbase)

- `import ocr-next`: prints the file and page that need OCR (with the reason). A multimodal agent reads that page
  (Claude Code can read PDFs/images) and transcribes it.
- `import ocr-submit <file> --page N -`: stores the text in the state dir (keyed by the file's hash, not written to the
  bundle). Direct read and `import --write` merge this text into the right page.
- Images (`png/jpg`) use the same flow: each image is treated as one page needing OCR.

## 5. Agents and existing parts

- Skill **`okbase-import`**: when to import, OCR by the agent, reviewing multi-column pages, distilling
  sources into conforming documents with `sources[]`.
- `scan`/`onboard`:
  - a folder with only PDF/DOCX → "readable right away";
  - pages needing OCR → ask the user whether the agent may read the page images (sends content to the model
    provider; part of the consent catalog).
- `doctor`: reports documents needing OCR or stale imports. `advise`: counts source documents toward size
  and languages.

## 6. Roadmap

| Phase | Content | Done when |
|---|---|---|
| I1 | `okbase-convert` (PDF per page, Office, HTML, text); data also reads xls/ods | The S10-lite sample set gives the same results as the spike; a PDF with 1 scanned page still keeps the text pages |
| I2 | Direct read in the index; `sources` table | `grep`/`get`/`catalog` work on PDF/DOCX/PPTX/HTML; editing a source file updates results |
| I3 | `import --plan/--write/status`, splitting long documents, protecting hand-edited copies from overwrite | Safe to rerun; duplicates are removed |
| I4 | OCR by the agent; skill `okbase-import`; scan/onboard/doctor integration | An agent completes a folder with scanned pages |
| I5 | Trial with a real agent (mixed PDF/DOCX/HTML folder) | The agent answers questions from source documents correctly, citing file and page |

## 7. Implementation

| Phase | Commit | Notes |
|---|---|---|
| I1 | `90f8603`, `d0eba82` | `okbase-convert`. PDF uses `pdf-inspector` directly page by page (a scanned page does not lose the whole document). Data also reads xlsm/xlsb/xls/ods. `ttf-parser` (unmaintained, a dependency of pdf-inspector) is recorded in `deny.toml` |
| I2 | `3bf8b0d` | Direct read in the index; `sources` table; merging OCR text; yielding to the imported copy within the same sync pass |
| I3 | `f9e9979` | `import` (plan), `--write` (splits > 6k tokens, keeps hand edits, reports orphans), `status`, `ocr-next`/`ocr-submit` |
| I4 | `7354629` | `scan` counts source documents; `onboard` asks about OCR (consent); `doctor` reports unreadable files and pages without text; skill `okbase-import`; `new --source`; docs |
| After I5 | `0ea2323` | `import ocr-next` exports images of scanned pages (JPEG kept as is; Flate images, including PNG predictor and 1-bit black-and-white, as PNG); for fax/JBIG2 it instructs opening the PDF page directly. Fixed an index-lock deadlock in `ocr_next` |
| S14 | `992b919` | Benchmark `spikes/page-image-bench`: extracting embedded images is faster than rendering (JPEG 1–3 ms vs 0.2–0.95 s; Flate 2–4×), with no loss of detail and the same image tokens. CCITT G4 still missing (proposed crate `fax`) |
| CCITT G4 | `565455d` | Exports CCITT Group 4 black-and-white scan images (crate `fax`, MIT, pure Rust): 86 ms for A4 at 300 dpi, zero pixel errors. Reads array-form `DecodeParms` (img2pdf) and `/Decode [1 0]`. Group 3 and JBIG2 still instruct opening the PDF page directly |
