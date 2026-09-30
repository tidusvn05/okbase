#!/usr/bin/env python3
"""1) Sinh nội dung tài liệu từ spec bằng Claude CLI (cache ở bodies.json)
2) Dựng bundle OKF: bundle-E (tổ chức thư mục + index.md) và bundle-V (thêm _views/ theo metadata)
3) Sheet: data/*.csv + sources/sheets/*.md (bảng markdown, như bản import)
"""
import csv, json, re, shutil, subprocess, concurrent.futures as cf
from pathlib import Path

import os
ROOT = Path(__file__).resolve().parent
SUF = os.environ.get("SUF", "")  # "" hoặc "-x20"
SPECS = json.loads((ROOT / f"specs{SUF}.json").read_text())
BODIES_F = ROOT / "bodies.json"
LANG_NAME = {"vi": "Vietnamese", "en": "English", "ja": "Japanese"}
SCHEMA = json.dumps({"type": "object", "properties": {"docs": {"type": "array", "items": {"type": "object", "properties": {
    "id": {"type": "string"}, "title": {"type": "string"}, "description": {"type": "string"}, "body": {"type": "string"}},
    "required": ["id", "title", "description", "body"]}}}, "required": ["docs"]})
PROMPT = """You write internal business documents for "Hikari Home", a home-appliance company operating in Vietnam (VN) and Japan (JP).
For EACH spec below write one document IN THE SPEC'S LANGUAGE (lang):
- title: natural document title (in that language). For policies include the region and the version (e.g. "v2"). For contracts include the customer name and contract type. For meeting notes include the date. For product specs include the model code.
- description: one sentence summary (in that language).
- body: Markdown, 120-220 words (Japanese: 300-500 characters), realistic tone, with 2-4 "##" sections.
  * Every value in `facts` MUST appear exactly as a number in the body (money with currency, e.g. "500,000 VND" or "5,000 JPY").
  * Policies: state the effective date; if status=deprecated say it was superseded; if status=draft say it is a draft proposal pending approval.
  * Contracts: parties, contract type, effective_from and effective_to dates, contract_value with currency, payment terms, renewal notice.
  * Meeting notes: date, attending department, discussion, decisions (include the facts).
  * Do NOT list tags, department or status as metadata lines; those live in frontmatter.
Return JSON {docs:[{id,title,description,body}]} with the same ids.

Specs:
"""


def gen_bodies():
    bodies = json.loads(BODIES_F.read_text()) if BODIES_F.exists() else {}

    def missing_facts(s, b):
        txt = b["body"].replace(",", "").replace(".", "").replace(" ", "")
        return [k for k, v in s["facts"].items() if str(v) not in txt]

    for attempt in range(3):
        todo = [s for s in SPECS if not s.get("filler") and (s["id"] not in bodies or missing_facts(s, bodies[s["id"]]))]
        if not todo:
            break
        print(f"attempt {attempt}: generating {len(todo)} docs")
        batches = [todo[i:i + 8] for i in range(0, len(todo), 8)]

        def run(batch):
            slim = [{k: v for k, v in s.items() if k not in ("topic_names",)} for s in batch]
            p = subprocess.run(["claude", "-p", "--output-format", "json", "--no-session-persistence", "--setting-sources", "local",
                                "--tools", "", "--model", "sonnet", "--json-schema", SCHEMA],
                               input=PROMPT + json.dumps(slim, ensure_ascii=False, indent=1), capture_output=True, text=True,
                               cwd=ROOT / "work", timeout=900)
            env = json.loads(p.stdout)
            return env["structured_output"]["docs"], env.get("total_cost_usd", 0)

        (ROOT / "work").mkdir(exist_ok=True)
        cost = 0
        with cf.ThreadPoolExecutor(5) as ex:
            for docs, c in ex.map(run, batches):
                cost += c
                for d in docs:
                    bodies[d["id"]] = d
        BODIES_F.write_text(json.dumps(bodies, ensure_ascii=False, indent=1))
        print(f"  cost ${cost:.2f}")
    bad = [s["id"] for s in SPECS if not s.get("filler") and (s["id"] not in bodies or missing_facts(s, bodies[s["id"]]))]
    print("docs with missing facts:", bad)
    return bodies


def filler_body(s):
    """Tài liệu nhiễu (SCALE>1): nội dung template, nhất quán với metadata."""
    name = s["topic_names"][{"vi": 0, "en": 1, "ja": 2}[s["lang"]]]
    facts = ", ".join(f"{k} = {v}" for k, v in s["facts"].items())
    if s["kind"] == "Contract":
        t = {"vi": f"{name.capitalize()} – {s['customer']}", "en": f"{name.title()} – {s['customer']}", "ja": f"{s['customer']} {name}"}[s["lang"]]
        body = (f"## Parties\nHikari Home and {s['customer']} ({s['region']}).\n\n## Term\nEffective {s['effective_from']} to {s['effective_to']}.\n\n"
                f"## Value\n{s['contract_value']:,} {s['currency']}. {facts}.\n")
    elif s["kind"] == "Meeting Note":
        t = f"{name} – {s['meeting_date']} ({s['department']})"
        body = f"## Attendees\n{s['department']} team.\n\n## Decisions\n{facts}.\n"
    else:
        t = f"{name} ({s['department']}) {s.get('model', '')} {s.get('region', '')}".strip()
        body = f"## Overview\n{name}.\n\n## Details\n{facts or 'See related documents.'}\n"
    return {"title": t, "description": f"{s['kind']}: {t}", "body": body}


def frontmatter(s, b, style):
    q = lambda x: json.dumps(x, ensure_ascii=False)
    lines = ["---", f"type: {s['kind']}", f"title: {q(b['title'])}", f"description: {q(b['description'])}"]
    if style == "inline":
        lines.append(f"tags: [{', '.join(s['tags'])}]")
    else:
        lines += ["tags:"] + [f"  - {t}" for t in s["tags"]]
    lines += [f"status: {s['status']}", f"lang: {s['lang']}", f"department: {s['department']}", f"updated: {s['updated']}"]
    for k in ("region", "customer", "version", "supersedes", "effective_from", "effective_to", "contract_value", "currency", "model", "meeting_date"):
        if k in s:
            lines.append(f"{k}: {q(s[k]) if isinstance(s[k], str) and ' ' in s[k] else s[k]}")
    lines += [f"generated: {{by: qobot-spike/0.1, at: {s['updated']}T09:00:00Z}}", "---", ""]
    return "\n".join(lines)


def write_indexes(root, docs):
    by_dir = {}
    for d in docs:
        by_dir.setdefault(str(Path(d["id"]).parent), []).append(d)
    alldirs = {"."}
    for k in by_dir:
        p = Path(k)
        while str(p) != ".":
            alldirs.add(str(p))
            p = p.parent
    for dd in sorted(alldirs):
        depth = 0 if dd == "." else dd.count("/") + 1
        subs = sorted(x for x in alldirs if x != "." and x.count("/") + 1 == depth + 1 and (dd == "." or x.startswith(dd + "/")))
        out = ['---\nokf_version: "0.2"\n---' if dd == "." else "", f"# Index: {'/' if dd == '.' else dd}", ""]
        if subs:
            out += ["## Subdirectories"] + [f"* [{s}](/{s}/index.md)" for s in subs] + [""]
        if by_dir.get(dd):
            out += ["## Documents"] + [f"* [{d['title']}](/{d['id']}.md) - {d['description']}" for d in sorted(by_dir[dd], key=lambda x: x["id"])]
        (root / dd / "index.md").write_text("\n".join(x for x in out if x is not None) + "\n")


def write_views(root, docs):
    """_views/: các trang liệt kê theo metadata (sinh tự động), đọc được bằng Read/Grep thông thường."""
    v = root / "_views"
    v.mkdir()
    row = lambda d: f"| [{d['id']}](/{d['id']}.md) | {d['title']} | {d['kind']} | {d['status']} | {d['lang']} | {d['department']} | {d['updated']} | {', '.join(d['tags'])} |" + \
        (f" {d.get('customer', '')} | {d.get('effective_from', '')} → {d.get('effective_to', '')} | {d.get('contract_value', '')} {d.get('currency', '')} |" if d["kind"] == "Contract" else "")
    head = "| id | title | type | status | lang | department | updated | tags |\n|---|---|---|---|---|---|---|---|"
    chead = "| id | title | type | status | lang | department | updated | tags | customer | effective | value |\n|---|---|---|---|---|---|---|---|---|---|---|"

    def page(path, title, ds):
        p = v / path
        p.parent.mkdir(parents=True, exist_ok=True)
        contracts = all(d["kind"] == "Contract" for d in ds)
        p.write_text(f"# View: {title}\n\n{len(ds)} documents.\n\n{chead if contracts else head}\n" + "\n".join(row(d) for d in sorted(ds, key=lambda x: x["id"])) + "\n")

    groups = {"tags": lambda d: d["tags"], "types": lambda d: [d["kind"]], "status": lambda d: [d["status"]],
              "departments": lambda d: [d["department"]], "lang": lambda d: [d["lang"]]}
    idx = ["# Views (auto-generated from frontmatter)", "", "Lists of documents grouped by metadata. Each page is a table: id, title, type, status, lang, department, updated, tags.", ""]
    for g, f in groups.items():
        vals = sorted({x for d in docs for x in f(d)})
        idx.append(f"## By {g}")
        for val in vals:
            ds = [d for d in docs if val in f(d)]
            slug = val.lower().replace(" ", "-")
            page(f"{g}/{slug}.md", f"{g} = {val}", ds)
            idx.append(f"* [{val}](/_views/{g}/{slug}.md) - {len(ds)} documents")
        idx.append("")
    cons = [d for d in docs if d["kind"] == "Contract"]
    for cust in sorted({d["customer"] for d in cons}):
        page(f"customers/{cust.lower().replace(' ', '-')}.md", f"customer = {cust}", [d for d in cons if d["customer"] == cust])
    page("contracts-all.md", "all contracts (with effective dates and values)", cons)
    idx += ["## Contracts", "* [All contracts with dates/values](/_views/contracts-all.md)"] + \
           [f"* [customer: {c}](/_views/customers/{c.lower().replace(' ', '-')}.md)" for c in sorted({d['customer'] for d in cons})] + [""]
    page("recent.md", "updated in 2026 (newest first)", sorted([d for d in docs if d["updated"] >= "2026-01-01"], key=lambda x: x["updated"], reverse=True))
    idx.append("* [Recently updated (2026)](/_views/recent.md)")
    (v / "index.md").write_text("\n".join(idx) + "\n")


def write_sheets(root):
    desc = {"inventory": ("Inventory by SKU and warehouse (HN, HCM, Tokyo, Osaka)", "Tồn kho theo SKU và kho"),
            "sales_2026": ("Monthly sales 2026 by region (VN in VND, JP in JPY) and SKU", "Doanh số theo tháng 2026"),
            "price_list": ("Price list with VND/JPY prices and status (active/discontinued)", "Bảng giá")}
    (root / "data").mkdir()
    (root / "sources/sheets").mkdir(parents=True)
    for name, (d_en, _) in desc.items():
        shutil.copy(ROOT / f"data{SUF}/{name}.csv", root / f"data/{name}.csv")
        rows = list(csv.reader(open(ROOT / f"data{SUF}/{name}.csv")))
        md = [f"---\ntype: Dataset\ntitle: \"{name}\"\ndescription: \"{d_en} — imported from Google Sheets\"\ntags: [data, sheet]\nstatus: stable\nlang: en\n"
              f"department: Finance\nupdated: 2026-09-29\nresource: data/{name}.csv\n---\n", f"# {name}\n", f"{d_en}. {len(rows) - 1} rows. Source CSV: `data/{name}.csv`.\n",
              "| " + " | ".join(rows[0]) + " |", "|" + "---|" * len(rows[0])] + ["| " + " | ".join(r) + " |" for r in rows[1:]]
        (root / f"sources/sheets/{name}.md").write_text("\n".join(md) + "\n")


def main():
    bodies = gen_bodies()
    docs = []
    for n, s in enumerate(SPECS):
        b = filler_body(s) if s.get("filler") else bodies[s["id"]]
        docs.append({**s, "title": b["title"], "description": b["description"], "body": b["body"], "style": "inline" if n % 2 else "block"})
    for variant in ("E", "V"):
        root = ROOT / f"bundle-{variant}{SUF}"
        shutil.rmtree(root, ignore_errors=True)
        for d in docs:
            f = root / f"{d['id']}.md"
            f.parent.mkdir(parents=True, exist_ok=True)
            f.write_text(frontmatter(d, d, d["style"]) + d["body"].strip() + "\n")
        write_sheets(root)
        write_indexes(root, docs + [{"id": f"sources/sheets/{n}", "title": n, "description": "Dataset imported from Google Sheets (see data/*.csv)"} for n in ("inventory", "sales_2026", "price_list")])
        if variant == "V":
            write_views(root, docs)
            idx = (root / "index.md").read_text()
            (root / "index.md").write_text(idx + "\n## Views\n* [_views](/_views/index.md) - auto-generated lists of documents by tag, type, status, department, language, customer, and recent updates\n")
    meta = [{k: d[k] for k in d if k not in ("body", "style", "topic_names", "facts", "filler")} for d in docs]
    (ROOT / f"metadata{SUF}.json").write_text(json.dumps(meta, ensure_ascii=False, indent=1))
    print(f"bundles written: {len(docs)} docs")


if __name__ == "__main__":
    main()
