# S10-lite — light document import: anydoc (office, PDF) + htmd (HTML) (2026-10-02)

Question: is there a light, accurate way to bring PDF/DOCX/PPTX/XLSX/HTML into a bundle, without
models, LibreOffice or a service? Measured on this machine (8 vCPU), release build.

Candidates: [anydoc](https://github.com/firecrawl/anydoc) 0.2.4 (Firecrawl, MIT, pure Rust, uses
pdf-inspector; doc/docx/ppt/pptx/xls/xlsx/odt/ods/odp/rtf/epub/csv/pdf; no HTML; no OCR, scanned
pages return `NeedsOcr`, optional hosted OCR sends files to Firecrawl) and
[htmd](https://crates.io/crates/htmd) 0.5 (HTML → markdown, turndown-like).

Samples (`make_samples.py`, ground truth known): DOCX vi/ja (headings, list, table), PPTX vi (+ a
table slide), XLSX, text PDF vi/ja (Noto fonts), PDF with a side-by-side two-column block, a scanned
(image-only) PDF, a Confluence-like HTML page, and a real paper (arXiv 1706.03762, two-column,
formulas, tables). Inputs are not committed (generated or downloaded).

## Weight

| | Value |
|---|---|
| Crates added (anydoc + htmd) | 145 in the dependency tree |
| Release binary (stripped) with both | 11.9 MB (htmd alone 1.5 MB → anydoc ≈ 10 MB, mostly pdf-inspector and its embedded font/CMap data) |
| Clean build | 81 s |
| Conversion time | 0.3–10 ms per small document; 172 ms for the 15-page paper |

## Accuracy

| Input | Result |
|---|---|
| DOCX vi / ja | exact: headings, bullets, table, every diacritic and kana/kanji |
| PPTX | exact: slide titles as headings, bullets, the table slide as a table |
| XLSX | a markdown table (okfkit's SQL data module is the better path for sheets) |
| Text PDF vi / ja | exact, including headings (from font size), bullets and the table |
| PDF, two side-by-side columns in the middle of a page | **lines of the two columns interleaved** (reading order lost) |
| Real two-column paper | body text in the right order; headings partly noisy (author names, licence line as headings); the main table right except a few split cells |
| Scanned PDF | clean `NeedsOcr { pages: [1] }` error, nothing invented |
| HTML (htmd) | headings, lists, text exact; **a table without `<th>` became one cell per line**; page chrome must be removed first (nav/header/footer) |

## Verdict

anydoc is light (no models, no runtime services) and accurate for office files and text PDFs in
vi/en/ja; it is the converter to use. Gaps to handle in okfkit, not in the converter:
1. **Scans**: report the pages; let a multimodal agent transcribe them (no OCR engine in okfkit),
   or run an OCR tool the user already has. Never use hosted OCR by default (it sends the file out).
2. **Layout risk in PDFs**: flag pages whose text looks interleaved or very short so an agent or a
   person reviews them; mark imported documents `generated` until reviewed.
3. **HTML**: htmd plus a main-content step (drop nav/header/footer/scripts) and a table fix for
   header-less tables; or convert HTML tables through anydoc-like logic.
4. **Size**: ≈ 10 MB more; keep import optional (a feature, or the `okfkit-import` plugin binary).

## I5 — agents on a folder of source documents (2026-10-02)

After I1–I4 (`docs/PLAN-import.md`): two Claude subagents on copies of a folder with a Word
policy, a Japanese PDF, a PowerPoint deck, a saved Confluence page, a scanned PDF and a 15-page
paper; only the user's sentence and the binary.

| | A: user allowed reading scanned pages | B: no such consent |
|---|---|---|
| Setup | `onboard` → `agent install` → `doctor` (MCP ok) | same |
| Scanned page | `import ocr-next` → read the page → `import ocr-submit` (stored in state); found afterwards | asked: "May I read that page and transcribe it? Its content goes to my model provider" |
| Answers | 3/3 with file (and page) citations, using only okfkit commands; noted the scan's text is cut off | correct, and noticed the Vietnamese and Japanese policies disagree, asking which applies |
| Consent flags on their own | 0 | 0 |

Follow-up: the agent had no PDF renderer and extracted the scan's image itself. `ocr-next` could
hand the page image directly when a page is a single embedded image (the usual scan), so any
multimodal agent can read it.
