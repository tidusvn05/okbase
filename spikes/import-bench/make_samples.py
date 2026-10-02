#!/usr/bin/env python3
"""Spike S10-lite: generate test documents with known content (vi/ja/en, headings, lists, tables,
two columns, a scanned page) so converters can be checked against ground truth.
Usage: ../embed-tune/.venv/bin/python make_samples.py  (needs python-docx python-pptx openpyxl fpdf2 pillow)"""
from pathlib import Path

from docx import Document
from fpdf import FPDF
from openpyxl import Workbook
from PIL import Image, ImageDraw, ImageFont
from pptx import Presentation
from pptx.util import Inches

HERE = Path(__file__).resolve().parent
OUT = HERE / "samples"
OUT.mkdir(exist_ok=True)
FONT = str(HERE / "fonts/NotoSans.ttf")
FONT_JP = str(HERE / "fonts/NotoSansJP.ttf")

VI = {
    "title": "Chính sách đổi trả hàng",
    "intro": "Khách hàng được đổi trả sản phẩm trong vòng 30 ngày kể từ ngày nhận hàng.",
    "h2": "Điều kiện áp dụng",
    "items": ["Còn nguyên tem và hoá đơn", "Sản phẩm chưa qua sử dụng", "Không áp dụng cho hàng giảm giá trên 50%"],
    "table": [["Loại hàng", "Thời hạn", "Phí"], ["Điện thoại", "15 ngày", "Miễn phí"], ["Tủ lạnh", "30 ngày", "200.000 đồng"]],
}
JA = {
    "title": "返品ポリシー",
    "intro": "お届けから8日以内であれば返品を受け付けます。",
    "h2": "対象条件",
    "items": ["未開封・未使用の商品", "レシートが必要です", "セール品は対象外"],
    "table": [["商品", "期限", "送料"], ["冷蔵庫", "8日", "当社負担"], ["スマートフォン", "14日", "お客様負担"]],
}


def docx(lang, d):
    doc = Document()
    doc.add_heading(d["title"], 1)
    doc.add_paragraph(d["intro"])
    doc.add_heading(d["h2"], 2)
    for it in d["items"]:
        doc.add_paragraph(it, style="List Bullet")
    t = doc.add_table(rows=len(d["table"]), cols=3)
    for r, row in enumerate(d["table"]):
        for c, v in enumerate(row):
            t.cell(r, c).text = v
    doc.save(OUT / f"policy-{lang}.docx")


def pptx():
    p = Presentation()
    s = p.slides.add_slide(p.slide_layouts[1])
    s.shapes.title.text = VI["title"]
    body = s.placeholders[1].text_frame
    body.text = VI["intro"]
    for it in VI["items"]:
        body.add_paragraph().text = it
    s2 = p.slides.add_slide(p.slide_layouts[5])
    s2.shapes.title.text = "Q3 results"
    rows = [["Region", "Revenue", "Growth"], ["VN", "1.2B", "12%"], ["JP", "3.4B", "4%"]]
    tbl = s2.shapes.add_table(3, 3, Inches(1), Inches(2), Inches(6), Inches(1.5)).table
    for r, row in enumerate(rows):
        for c, v in enumerate(row):
            tbl.cell(r, c).text = v
    p.save(OUT / "deck-vi.pptx")


def xlsx():
    wb = Workbook()
    ws = wb.active
    ws.title = "sales"
    for row in [["month", "region", "revenue"], ["2026-01", "VN", 120], ["2026-01", "JP", 340], ["2026-02", "VN", 130]]:
        ws.append(row)
    wb.save(OUT / "sales.xlsx")


def pdf(lang, d, font, two_columns=False):
    p = FPDF()
    p.add_page()
    p.add_font("N", "", font)
    p.add_font("N", "B", font)
    p.set_font("N", size=18)
    p.cell(0, 10, d["title"], new_x="LMARGIN", new_y="NEXT")
    p.set_font("N", size=11)
    p.multi_cell(0, 6, d["intro"])
    p.set_font("N", size=14)
    p.cell(0, 10, d["h2"], new_x="LMARGIN", new_y="NEXT")
    p.set_font("N", size=11)
    for it in d["items"]:
        p.cell(0, 6, "• " + it, new_x="LMARGIN", new_y="NEXT")
    p.ln(4)
    with p.table() as t:
        for row in d["table"]:
            r = t.row()
            for v in row:
                r.cell(v)
    if two_columns:
        p.ln(6)
        left = " ".join(["Cột trái nói về bảo hành điện thoại trong mười hai tháng."] * 4)
        right = " ".join(["Cột phải nói về giao hàng nội thành trong hai ngày."] * 4)
        y = p.get_y()
        p.set_xy(10, y)
        p.multi_cell(90, 6, left)
        p.set_xy(110, y)
        p.multi_cell(90, 6, right)
    name = f"policy-{lang}{'-2col' if two_columns else ''}.pdf"
    p.output(str(OUT / name))


def scanned():
    img = Image.new("RGB", (1240, 400), "white")
    dr = ImageDraw.Draw(img)
    f = ImageFont.truetype(FONT, 40)
    dr.text((40, 60), VI["title"], fill="black", font=f)
    dr.text((40, 160), VI["intro"][:48], fill="black", font=ImageFont.truetype(FONT, 28))
    path = OUT / "scan.png"
    img.save(path)
    p = FPDF()
    p.add_page()
    p.image(str(path), x=10, y=10, w=190)
    p.output(str(OUT / "scanned-vi.pdf"))
    path.unlink()


def html():
    (OUT / "confluence-vi.html").write_text(f"""<!DOCTYPE html><html><head><title>{VI['title']}</title>
<style>.x{{}}</style><script>var a=1;</script></head><body>
<div id="header"><nav><a href="/">Trang chủ</a> &gt; <a href="/cs">CSKH</a></nav></div>
<div id="main-content" class="wiki-content"><h1>{VI['title']}</h1><p>{VI['intro']}</p>
<h2>{VI['h2']}</h2><ul>{''.join(f'<li>{i}</li>' for i in VI['items'])}</ul>
<table><tbody>{''.join('<tr>' + ''.join(f'<td>{c}</td>' for c in r) + '</tr>' for r in VI['table'])}</tbody></table>
<div class="confluence-information-macro"><p>Liên hệ phòng CSKH để được hỗ trợ.</p></div></div>
<div id="footer">Powered by Confluence</div></body></html>""", encoding="utf-8")


docx("vi", VI)
docx("ja", JA)
pptx()
xlsx()
pdf("vi", VI, FONT)
pdf("ja", JA, FONT_JP)
pdf("vi", VI, FONT, two_columns=True)
scanned()
html()
print("\n".join(sorted(p.name for p in OUT.iterdir())))
