#!/usr/bin/env python3
"""Chấm + tổng hợp spike metadata.
- list: so tập `items` (id) với đáp án — chính xác tuyệt đối (đúng khi trùng khớp hoàn toàn), kèm precision/recall.
- còn lại (version/count/facet/sheet/content): LLM judge so với đáp án tính sẵn, mù cấu hình.
"""
import json, random, statistics, subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent
CFG = {"E": "E agent + thư mục (Read/Grep/Glob)", "V": "V = E + _views/ (trang liệt kê theo metadata)",
       "Q": "Q = E + kb_query (lọc metadata)", "QD": "QD = Q + data_query (SQL trên sheet)",
       "E-x20": "E ×20 (3.020 tài liệu, sheet ~10k dòng)", "V-x20": "V ×20", "QD-x20": "QD ×20"}
CATS = ["list", "version", "count", "facet", "sheet", "content"]
CAT_NAME = {"list": "Liệt kê theo bộ lọc", "version": "Chi tiết theo bản hiện hành", "count": "Đếm / tổng theo metadata",
            "facet": "Khám phá (phòng ban/khách hàng nào…)", "sheet": "Số liệu trên sheet", "content": "Nội dung thường (đối chứng)"}
SCHEMA = json.dumps({"type": "object", "properties": {"grades": {"type": "array", "items": {"type": "object", "properties": {
    "key": {"type": "string"}, "correct": {"type": "boolean"}, "reason": {"type": "string"}}, "required": ["key", "correct", "reason"]}}}, "required": ["grades"]})
PROMPT = """Grade answers of a business assistant against the EXPECTED answer (computed from ground-truth data). Answers may be in Vietnamese/English/Japanese.
- Numbers must match exactly (ignore formatting like separators/currency symbols; units must be consistent).
- For "value" answers with several fields (e.g. month + revenue, sku + units, customers + count) ALL fields must match.
- For facet/set answers (e.g. list of departments/categories/customers/document ids) the set must match exactly (names may be translated or abbreviated, e.g. CS = Customer Support = カスタマーサポート).
- For version questions the answer must give the value of the CURRENT document (not old/draft versions).
Return one grade per item with the same key. reason: max 12 words.

"""


def main():
    runs = [json.loads(f.read_text()) for f in (ROOT / "runs").glob("*/*.json") if not f.name.endswith(".mcp.json")]
    for r in runs:
        if str(r.get("scale", "")).startswith("-"):
            r["cfg"] = r["cfg"] + r["scale"]
    k = lambda r: f"{r['cfg']}-{r['qid']}"
    gf = ROOT / "results/judge.json"
    grades = json.loads(gf.read_text()) if gf.exists() else {}
    # list: chấm tự động
    for r in runs:
        if r["cat"] == "list":
            exp, got = set(r["expected"]), set(r["items"])
            tp = len(exp & got)
            grades[k(r)] = {"correct": exp == got, "precision": tp / len(got) if got else 0.0, "recall": tp / len(exp) if exp else 1.0,
                            "reason": f"missing={sorted(exp - got)} extra={sorted(got - exp)}"}
    todo = [r for r in runs if r["cat"] != "list" and k(r) not in grades and not r["is_error"]]
    random.Random(5).shuffle(todo)
    for i in range(0, len(todo), 12):
        batch = todo[i:i + 12]
        alias = {k(r): f"g{n}{random.Random(i + n).randint(1000, 9999)}" for n, r in enumerate(batch)}
        body = PROMPT + "".join(f"### item key={alias[k(r)]}\nQUESTION: {r['q']}\nEXPECTED: {json.dumps(r['expected'], ensure_ascii=False)}\n"
                                f"ANSWER: {r['answer']}\nANSWER ITEMS: {r['items']}\n\n" for r in batch)
        p = subprocess.run(["claude", "-p", "--output-format", "json", "--no-session-persistence", "--setting-sources", "local", "--tools", "",
                            "--model", "sonnet", "--json-schema", SCHEMA], input=body, capture_output=True, text=True, cwd=ROOT / "work", timeout=900)
        inv = {v: kk for kk, v in alias.items()}
        for g in (json.loads(p.stdout).get("structured_output") or {}).get("grades", []):
            if g["key"] in inv:
                grades[inv[g["key"]]] = g
        gf.write_text(json.dumps(grades, ensure_ascii=False, indent=1))
        print(f"judged {min(i + 12, len(todo))}/{len(todo)}", flush=True)
    gf.write_text(json.dumps(grades, ensure_ascii=False, indent=1))

    ok = lambda r: bool((grades.get(k(r)) or {}).get("correct"))
    L = ["| Nhóm câu hỏi (số câu) | " + " | ".join(CFG[c].split(' ')[0] for c in CFG) + " |", "|---|" + "---|" * len(CFG)]
    for cat in CATS:
        cells = []
        for c in CFG:
            rs = [r for r in runs if r["cfg"] == c and r["cat"] == cat]
            cells.append(f"{sum(map(ok, rs))}/{len(rs)}" if rs else "—")
        n = len([r for r in runs if r["cfg"] == "E" and r["cat"] == cat])
        L.append(f"| {CAT_NAME[cat]} ({n}) | " + " | ".join(cells) + " |")
    tot = []
    for c in CFG:
        rs = [r for r in runs if r["cfg"] == c]
        tot.append(f"**{sum(map(ok, rs)) / len(rs):.0%}**" if rs else "—")
    L.append("| **Tổng** | " + " | ".join(tot) + " |")
    L += ["", "| Cách | Đúng | p50 / p90 | Lượt | Tool call | Token vào | Chi phí TB | List: precision / recall |", "|---|---|---|---|---|---|---|---|"]
    for c, name in CFG.items():
        rs = [r for r in runs if r["cfg"] == c]
        if not rs:
            continue
        w = sorted(r["wall_s"] for r in rs)
        tin = statistics.mean(sum((r["usage"] or {}).get(t, 0) for t in ("input_tokens", "cache_creation_input_tokens", "cache_read_input_tokens")) for r in rs)
        lr = [grades.get(k(r), {}) for r in rs if r["cat"] == "list"]
        L.append(f"| {name} | {sum(map(ok, rs)) / len(rs):.0%} | {w[len(w) // 2]:.1f}s / {w[int(len(w) * .9)]:.1f}s | {statistics.mean(r['num_turns'] or 0 for r in rs):.1f} | "
                 f"{statistics.mean(len(r['tool_calls']) for r in rs):.1f} | {tin:,.0f} | ${statistics.mean(r['cost_usd'] or 0 for r in rs):.3f} | "
                 f"{statistics.mean(g.get('precision', 0) for g in lr):.0%} / {statistics.mean(g.get('recall', 0) for g in lr):.0%} |")
    W = ["", "Câu sai:"]
    for r in sorted(runs, key=lambda r: (r["qid"], r["cfg"])):
        if not ok(r):
            W.append(f"- q{r['qid']} {r['cat']} [{r['lang']}] {r['cfg']}: {(grades.get(k(r)) or {}).get('reason', 'ERROR')}")
    rep = "\n".join(L + W)
    (ROOT / "results/report.md").write_text(rep)
    print("\n".join(L))
    print(f"\nTổng chi phí: ${sum(r['cost_usd'] or 0 for r in runs):.2f} / {len(runs)} lượt")


if __name__ == "__main__":
    main()
