"""Scanned-PDF samples for the page-image benchmark (S14), one per encoding a scanner or a PDF
library produces. Ground truth: the source image is kept next to each PDF (`<name>.src.png`).
Usage: ../embed-tune/.venv/bin/python make_samples.py  (needs pillow fpdf2 img2pdf)"""
import io
from pathlib import Path

import img2pdf
from fpdf import FPDF
from PIL import Image, ImageDraw, ImageFont

HERE = Path(__file__).resolve().parent
OUT = HERE / "samples"
OUT.mkdir(exist_ok=True)
FONT = str(HERE / "../import-bench/fonts/NotoSans.ttf")

TEXT = [
    "Chính sách đổi trả hàng",
    "Khách hàng được đổi trả sản phẩm trong vòng 30 ngày",
    "kể từ ngày nhận hàng. Còn nguyên tem và hoá đơn.",
    "Điện thoại: 15 ngày — Tủ lạnh: 30 ngày (phí 200.000 đồng)",
]


def page(w, h, mode="RGB"):
    """A scan-like page: title, paragraphs, a table grid, light noise."""
    img = Image.new("RGB", (w, h), "white")
    dr = ImageDraw.Draw(img)
    s = w / 2480
    dr.text((int(200 * s), int(250 * s)), TEXT[0], fill="black", font=ImageFont.truetype(FONT, int(90 * s)))
    f = ImageFont.truetype(FONT, int(50 * s))
    y = 450
    for _ in range(12):
        for line in TEXT[1:]:
            dr.text((int(200 * s), int(y * s)), line, fill=(20, 20, 20), font=f)
            y += 75
        y += 40
    for i in range(4):  # table grid
        dr.line([(200 * s, (y + i * 90) * s), (2280 * s, (y + i * 90) * s)], fill="black", width=max(1, int(3 * s)))
    return img.convert(mode)


def fpdf_pdf(name, images):
    p = FPDF(unit="pt", format="A4")
    for img in images:
        p.add_page()
        p.image(img, x=0, y=0, w=595, h=842)
    p.output(str(OUT / name))


def i2p(name, blobs):
    (OUT / name).write_bytes(img2pdf.convert(blobs, layout_fun=img2pdf.get_layout_fun((img2pdf.mm_to_pt(210), img2pdf.mm_to_pt(297)))))


def blob(img, fmt, **kw):
    b = io.BytesIO()
    img.save(b, fmt, **kw)
    return b.getvalue()


A4 = (2480, 3508)  # 300 dpi
rgb = page(*A4)
gray = page(*A4, "L")
bw = page(*A4, "1")
small = page(1240, 1754)  # 150 dpi
small_gray = page(1240, 1754, "L")
for name, img in [("rgb", rgb), ("gray", gray), ("bw", bw), ("rgb150", small), ("gray150", small_gray)]:
    img.save(OUT / f"{name}.src.png")

# JPEG, the usual scanner output (DCTDecode, passed through as is).
i2p("jpeg-gray-300dpi.pdf", [blob(gray, "JPEG", quality=85)])
i2p("jpeg-rgb-300dpi.pdf", [blob(rgb, "JPEG", quality=85)])
# Flate with PNG predictors: what fpdf2 / many libraries write for PNG input.
fpdf_pdf("flate-rgb-150dpi.pdf", [small])
fpdf_pdf("flate-gray-300dpi.pdf", [gray])
# 1-bit black and white: img2pdf keeps a 1-bit PNG as Flate; CCITT G4 from TIFF.
i2p("flate-1bit-300dpi.pdf", [blob(bw, "PNG")])
i2p("ccitt-g4-300dpi.pdf", [blob(bw, "TIFF", compression="group4")])
# A 50-page scan (throughput, and the page lookup on a long file).
i2p("jpeg-gray-50pages.pdf", [blob(small_gray, "JPEG", quality=80)] * 50)
print(sorted(p.name for p in OUT.iterdir()))
