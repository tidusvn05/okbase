#!/usr/bin/env python3
"""Dựng các bundle OKF lồng nhau S ⊂ M ⊂ L ⊂ XL từ docs OpenClaw.

- Chuyển frontmatter sang OKF: type (theo thư mục), title, description (= summary), resource; giữ nguyên key lạ.
- Sinh index.md cho mọi thư mục (progressive disclosure) + index.md gốc.
- Chọn 15 tài liệu "gold" (dùng để sinh câu hỏi) có mặt trong mọi kích thước.
  S = gold + tài liệu cùng thư mục (gây nhiễu theo chủ đề) tới ~60k token; M ~150k; L ~1M; XL = toàn bộ.
"""
import json, random, re, shutil
from pathlib import Path

SRC = Path(__file__).resolve().parent.parent / "embed-bench/.corpora/openclaw/docs"
OUT = Path(__file__).resolve().parent / "bundles"
BUDGET = {"S": 60_000, "M": 150_000, "L": 1_000_000, "XL": 10**12}
TOK_PER_CHAR = 0.26  # đo bằng tokenizer EmbeddingGemma trên tiếng Anh
SKIP_DIRS = {"releases", "announcements", "snippets", "assets", "images"}
TYPE = {"concepts": "Concept", "gateway": "Reference", "cli": "CLI Reference", "tools": "Tool Guide",
        "channels": "Channel Guide", "plugins": "Plugin Guide", "providers": "Provider Guide",
        "install": "Install Guide", "help": "FAQ", "reference": "Reference", "automation": "Guide",
        "security": "Security Guide", "start": "Tutorial", "platforms": "Platform Guide", "nodes": "Guide",
        "web": "Guide", "specs": "Spec"}


def parse(p: Path):
    raw = p.read_text(errors="replace")
    fm, body = {}, raw
    m = re.match(r"^---\n(.*?)\n---\n?(.*)$", raw, re.S)
    if m:
        body = m.group(2)
        fmtext = m.group(1)
        for k in ("title", "summary", "description"):
            mm = re.search(rf'^{k}:\s*"?(.*?)"?\s*$', fmtext, re.M)
            if mm:
                fm[k] = mm.group(1).replace('\\"', '"')
        fm["_raw"] = fmtext
    return fm, body


def okf_doc(rel: Path, fm, body):
    top = rel.parts[0] if len(rel.parts) > 1 else "root"
    title = fm.get("title") or rel.stem.replace("-", " ").title()
    desc = fm.get("summary") or fm.get("description") or ""
    extra = "\n".join(l for l in fm.get("_raw", "").splitlines()
                      if not re.match(r"^(title|summary|description):", l))
    q = lambda s: json.dumps(s, ensure_ascii=False)
    head = (f"---\ntype: {TYPE.get(top, 'Doc Page')}\ntitle: {q(title)}\ndescription: {q(desc)}\n"
            f"resource: https://docs.openclaw.ai/{rel.with_suffix('').as_posix()}\n{extra}\n---\n").replace("\n\n---", "\n---")
    return head + body, title, desc


def main():
    docs = []
    for p in sorted(SRC.rglob("*.md")):
        rel = p.relative_to(SRC)
        if rel.parts[0] in SKIP_DIRS or p.name in ("index.md", "AGENTS.md", "docs_map.md"):
            continue
        fm, body = parse(p)
        text, title, desc = okf_doc(rel, fm, body)
        docs.append({"id": rel.with_suffix("").as_posix(), "rel": rel, "text": text, "title": title, "desc": desc,
                     "tok": int(len(text) * TOK_PER_CHAR), "dir": rel.parent.as_posix()})
    rng = random.Random(42)
    # gold: tài liệu dài vừa phải (800–2500 token), có description, rải khắp các thư mục
    cands = [d for d in docs if 800 <= d["tok"] <= 2500 and d["desc"] and d["dir"] != "."]
    rng.shuffle(cands)
    gold, used_top = [], {}
    for d in cands:
        top = d["rel"].parts[0]
        if used_top.get(top, 0) < 2:
            gold.append(d)
            used_top[top] = used_top.get(top, 0) + 1
        if len(gold) == 15:
            break
    gold_ids = {d["id"] for d in gold}
    # thứ tự thêm tài liệu: gold → cùng thư mục với gold → còn lại (ngẫu nhiên)
    gold_dirs = {d["dir"] for d in gold}
    same = [d for d in docs if d["dir"] in gold_dirs and d["id"] not in gold_ids]
    rest = [d for d in docs if d["dir"] not in gold_dirs]
    rng.shuffle(same)
    rng.shuffle(rest)
    order = gold + same + rest
    stats = {}
    for size, budget in BUDGET.items():
        chosen, tok = [], 0
        for d in order:
            if tok + d["tok"] > budget and chosen and size != "XL":
                if d["id"] in gold_ids:
                    raise SystemExit("gold vượt budget")
                continue
            chosen.append(d)
            tok += d["tok"]
        root = OUT / size
        shutil.rmtree(root, ignore_errors=True)
        for d in chosen:
            f = root / d["rel"]
            f.parent.mkdir(parents=True, exist_ok=True)
            f.write_text(d["text"])
        write_indexes(root, chosen)
        stats[size] = {"files": len(chosen), "tokens": tok}
        print(f"{size}: {len(chosen)} files, ~{tok:,} tokens")
    (OUT / "gold.json").write_text(json.dumps([{"id": d["id"], "title": d["title"], "tok": d["tok"]} for d in gold], indent=1))
    (OUT / "stats.json").write_text(json.dumps(stats, indent=1))


def write_indexes(root: Path, chosen):
    by_dir = {}
    for d in chosen:
        by_dir.setdefault(d["dir"], []).append(d)
    dirs = sorted({p.relative_to(root).as_posix() for p in root.rglob("*") if p.is_dir()} | {"."})
    for dd in dirs:
        lines = [f"# Index: {'/' if dd == '.' else dd}", ""]
        subs = sorted(x for x in dirs if x != dd and (x.count("/") == (0 if dd == "." else dd.count("/") + 1))
                      and (dd == "." or x.startswith(dd + "/")))
        if dd == ".":
            lines[0] = '---\nokf_version: "0.2"\n---\n# Index: /'
        if subs:
            lines += ["## Subdirectories"] + [f"* [{s}](/{s}/index.md) - {len([d for d in chosen if d['dir'] == s or d['dir'].startswith(s + '/')])} documents" for s in subs] + [""]
        if by_dir.get(dd):
            lines += ["## Documents"] + [f"* [{d['title']}](/{d['id']}.md) - {d['desc']}" for d in sorted(by_dir[dd], key=lambda x: x["id"])]
        (root / dd / "index.md").write_text("\n".join(lines) + "\n")
    # catalog phẳng toàn bundle (dùng cho cấu hình "catalog trong prompt")
    cat = ["# Knowledge catalog (all documents)", ""] + [f"- [{d['id']}] {d['title']} — {d['desc']}" for d in sorted(chosen, key=lambda x: x["id"])]
    (root.parent / f"{root.name}.catalog.md").write_text("\n".join(cat) + "\n")


if __name__ == "__main__":
    main()
