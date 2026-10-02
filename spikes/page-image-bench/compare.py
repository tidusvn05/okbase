"""S14: the same pages through the alternatives an agent or a tool would otherwise use, then
fidelity and agent image cost for every output (okfkit's included).
Usage: ../embed-tune/.venv/bin/python compare.py  (after `cargo run --release > results/okfkit.jsonl`)"""
import io
import json
import statistics
import time
from pathlib import Path

import numpy as np
import pymupdf
import pypdfium2 as pdfium
from PIL import Image

HERE = Path(__file__).resolve().parent
S, OUT = HERE / "samples", HERE / "out"
RUNS = 10


def timed(fn):
    ts, out = [], None
    for _ in range(RUNS):
        t = time.perf_counter()
        out = fn()
        ts.append((time.perf_counter() - t) * 1000)
    return statistics.median(ts), out


def pdfium_render(path, page, dpi):
    def f():
        doc = pdfium.PdfDocument(str(path))
        pil = doc[page - 1].render(scale=dpi / 72).to_pil()
        b = io.BytesIO()
        pil.save(b, "PNG")
        return "png", b.getvalue()
    return f


def mupdf_render(path, page, dpi):
    def f():
        doc = pymupdf.open(str(path))
        return "png", doc[page - 1].get_pixmap(dpi=dpi).tobytes("png")
    return f


def mupdf_extract(path, page):
    def f():
        doc = pymupdf.open(str(path))
        imgs = doc[page - 1].get_images()
        if not imgs:
            return "none", b""
        x = doc.extract_image(imgs[0][0])
        return x["ext"], x["image"]
    return f


def source_of(name):
    kind = "bw" if "1bit" in name or "ccitt" in name else "gray" if "gray" in name else "rgb"
    dpi = "150" if "150dpi" in name or "50pages" in name else ""
    return S / f"{kind}{dpi}.src.png"


def fidelity(img_bytes, name):
    """Mean absolute error (0–255) against the source image, after resizing to its size."""
    if not img_bytes:
        return None
    src = Image.open(source_of(name)).convert("L")
    out = Image.open(io.BytesIO(img_bytes)).convert("L").resize(src.size)
    return round(float(np.abs(np.asarray(src, float) - np.asarray(out, float)).mean()), 2)


def claude_tokens(img_bytes):
    """Claude vision: long edge capped at 1568 px and ~1.15 MP, then ≈ w·h / 750 tokens."""
    if not img_bytes:
        return None
    w, h = Image.open(io.BytesIO(img_bytes)).size
    k = min(1.0, 1568 / max(w, h), (1_150_000 / (w * h)) ** 0.5)
    return round(w * k * h * k / 750)


def size(img_bytes):
    return "x".join(map(str, Image.open(io.BytesIO(img_bytes)).size)) if img_bytes else None


rows = [json.loads(line) for line in (HERE / "results/okfkit.jsonl").read_text().splitlines()]
for r in rows:
    data = next(OUT.glob(f"{r['sample']}.okfkit.*"), None)
    r["img"] = data.read_bytes() if data else b""
for pdf in sorted(S.glob("*.pdf")):
    name = pdf.stem
    page = 50 if "50pages" in name else 1
    for tool, fn in [
        ("pdfium render 150 dpi", pdfium_render(pdf, page, 150)),
        ("pdfium render 300 dpi", pdfium_render(pdf, page, 300)),
        ("mupdf render 150 dpi", mupdf_render(pdf, page, 150)),
        ("mupdf extract", mupdf_extract(pdf, page)),
    ]:
        ms, (ext, img) = timed(fn)
        rows.append({"sample": name, "tool": tool, "page": page, "ms": round(ms, 2), "ext": ext, "bytes": len(img), "img": img})
        safe = tool.replace(" ", "-")
        if img:
            (OUT / f"{name}.{safe}.{ext}").write_bytes(img)
for r in rows:
    img = r.pop("img")
    r["pixels"] = size(img)
    r["mae"] = fidelity(img, r["sample"])
    r["claude_tokens"] = claude_tokens(img)
(HERE / "results").mkdir(exist_ok=True)
(HERE / "results/s14.json").write_text(json.dumps(rows, indent=1, ensure_ascii=False))
for r in sorted(rows, key=lambda r: (r["sample"], r["tool"])):
    print(f"{r['sample']:20} {r['tool']:22} {r['ms']:8.2f} ms {r['ext']:4} {r['bytes']:9} B {str(r['pixels']):10} mae={r['mae']} tok={r['claude_tokens']}")
