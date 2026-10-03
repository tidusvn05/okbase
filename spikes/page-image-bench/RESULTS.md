# S14 — handing a scanned PDF page to an agent as an image (2026-10-02)

Question: `okbase import ocr-next` exports the image embedded in a scanned page (commit
`0ea2323`). How does that compare with what an agent or a tool would otherwise do, rendering the
page, in time, fidelity, agent cost and weight? Which scans does it miss?

## Setup
- Machine: AMD EPYC, 8 vCPU, 23 GB RAM; okbase release build; median of 20 runs (okbase) or
  10 runs (Python), document opened inside each run.
- Samples (`make_samples.py`, not committed): one A4 page of Vietnamese text and a table grid,
  produced the ways scanners and PDF libraries store scans. The source image is kept, so fidelity
  is measured against ground truth.

| Sample | Encoding |
|---|---|
| `jpeg-gray-300dpi`, `jpeg-rgb-300dpi` | DCTDecode (JPEG), the usual scanner output (img2pdf) |
| `jpeg-gray-50pages` | the same, 150 dpi, last page of 50 (page lookup in a long file) |
| `flate-gray-300dpi`, `flate-rgb-150dpi` | FlateDecode with PNG predictors (fpdf2) |
| `flate-1bit-300dpi` | FlateDecode, 1-bit black and white |
| `ccitt-g4-300dpi` | CCITTFax G4, 1-bit (office scanners in black-and-white mode) |

- Alternatives:
  - **pdfium render** at 150 or 300 dpi (pypdfium2 5, PDFium 153, the Chrome engine): what a tool
    with a PDF renderer does;
  - **mupdf render** at 150 dpi (PyMuPDF 1.28, AGPL);
  - **mupdf extract**: the same idea as okbase (take the embedded image), reference only (AGPL).
- Measures:
  - time to a file in memory;
  - output size;
  - fidelity as mean absolute error (MAE, 0–255) against the source image after resizing;
  - Claude image tokens. Claude scales an image to ≤ 1568 px on the long edge and ≈ 1.15 MP,
    then charges about w·h/750 tokens.

## Results (`results/s14.json`)

| Sample | okbase | mupdf extract | pdfium 150 dpi | pdfium 300 dpi | mupdf 150 dpi |
|---|---|---|---|---|---|
| jpeg-gray-300dpi | **1.7 ms**, MAE 0.41 (bytes as embedded) | 1.4 ms, 0.41 | 240 ms, 3.26 | 804 ms, 1.26 | 188 ms, 3.49 |
| jpeg-rgb-300dpi | **0.8 ms**, 0.41 | 1.3 ms, 0.41 | 284 ms, 3.26 | 866 ms, 1.26 | 307 ms, 3.49 |
| jpeg-gray-50pages (page 50) | **2.9 ms**, 0.86 | 1.3 ms, 0.86 | 254 ms, 2.43 | 947 ms, 3.03 | 205 ms, 2.47 |
| flate-gray-300dpi | **82 ms**, **0.0** | 182 ms, 0.0 | 184 ms, 4.56 | 544 ms, 2.48 | 156 ms, 5.93 |
| flate-rgb-150dpi | **51 ms**, **0.0** | 123 ms, 0.0 | 197 ms, 4.14 | 635 ms, 3.42 | 176 ms, 5.43 |
| flate-1bit-300dpi | **88 ms**, **0.0** | 148 ms, 0.0 | 209 ms, 4.99 | 576 ms, 1.82 | 134 ms, 5.35 |
| ccitt-g4-300dpi | **not exported** at the time (see the follow-up below: now 86 ms, MAE 0.0) | 172 ms, 0.0 | 218 ms, 4.99 | 599 ms, 1.82 | 144 ms, 5.35 |

Output size:
- okbase equals the embedded image: 0.15–0.83 MB at 300 dpi.
- Renders at 150 dpi: 0.27–0.88 MB.
- Renders at 300 dpi: 0.6–2.4 MB.

Claude image tokens: **≈ 1,533 for every output**. Every page is larger than the model's limit
and is scaled down to the same size, so rendering at a higher resolution buys nothing for the
agent.

## Findings
1. **Exact and fast where it applies.**
   - JPEG scans: okbase hands over the embedded bytes in about 1–3 ms, versus about 0.2–0.95 s to
     render.
   - Flate scans: okbase is lossless (MAE 0) and 2–4× faster than rendering. Renders lose detail
     to resampling (MAE 3.5–6 at 150 dpi).
2. **Same agent cost.** Image tokens are capped by the model, so extraction costs the agent
   nothing extra. Resolution above about 1568 px is wasted either way.
3. **No new weight.** No crate was added to `Cargo.lock`: lopdf, flate2 and crc32fast already
   came with pdf-inspector. A renderer would add PDFium (a prebuilt C++ library of several MB per
   platform, shipped next to the binary) or MuPDF (AGPL, not acceptable for an MIT/Apache crate).
4. **Gap: CCITT G4.** Black-and-white office scanners commonly write CCITT G4, and okbase does
   not export it yet: the agent is told to open the PDF page and needs a renderer, as in I5.
   - A pure-Rust G4 decoder (for example the `fax` crate, MIT) would close the gap without a
     renderer.
   - JBIG2 is rarer, and its decoders are C (jbig2dec, AGPL), so it stays a fallback.
5. **Cost of PNG output.** The Flate path spends most of its time compressing the PNG
   (`Compression::default()`). A faster level is possible if 50–90 ms per page ever matters; it
   does not for one page per agent turn.

## Not covered
- Pages that are not one image per page: pages split into strips or tiles, or a scan with a text
  layer on top. The code takes the largest image, so a page made of tiles would lose part of the
  page.
- Real scanner files (only synthetic ones here). Rerun with the user's PDFs when available (S10).
- Transcription quality by a real agent on each output: it uses the user's quota, so it was not
  run. In I5 a Claude agent transcribed the Vietnamese sample, and later searches found the text.

## Reproduce
```
../embed-tune/.venv/bin/pip install pillow fpdf2 img2pdf pypdfium2 pymupdf numpy
../embed-tune/.venv/bin/python make_samples.py      # needs ../import-bench/fonts/NotoSans.ttf
cargo run --release > results/okbase.jsonl            # RUNS=20 by default
../embed-tune/.venv/bin/python compare.py            # writes results/s14.json and prints the table
```

## Follow-up: CCITT G4 (2026-10-03)

okbase now decodes CCITT Group 4 with the `fax` crate (MIT, pure Rust, about 25 KB of source).
On `ccitt-g4-300dpi`, okbase takes **86 ms with MAE 0.0**, against 156 ms for mupdf extract and
202 ms (MAE 4.99) for a PDFium render at 150 dpi. The same run also found that `DecodeParms` can
be an array, one entry per filter, as img2pdf writes it. The predictor lookup did not read that
form before; now it does, and `/Decode [1 0]` (inverted 1-bit images) is honoured. Group 3 fax
and JBIG2 still fall back to opening the PDF page. `results/s14.json` now holds this second run (other
timings within the run-to-run noise of the table above).
