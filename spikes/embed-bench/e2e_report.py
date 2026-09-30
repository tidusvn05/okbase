#!/usr/bin/env python3
"""Chấm điểm (LLM judge, mù cấu hình) + tổng hợp kết quả thí nghiệm end-to-end.

Usage: python3 e2e_report.py [--judge-model sonnet] [--no-judge]
"""
import argparse, json, random, statistics, subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent
E2E = ROOT / "e2e"
DOCS = {d["id"]: d for d in json.loads((ROOT / "data/v2/docs.json").read_text())}

JUDGE_SCHEMA = json.dumps({
    "type": "object",
    "properties": {"grades": {"type": "array", "items": {
        "type": "object",
        "properties": {"key": {"type": "string"}, "correct": {"type": "boolean"}, "reason": {"type": "string"}},
        "required": ["key", "correct", "reason"]}}},
    "required": ["grades"],
})

JUDGE_PROMPT = """You grade answers of a support chatbot. For each item you get the user question, the REFERENCE document that contains the correct answer, and the bot ANSWER (may be in another language than the reference).
Mark correct=true only if the answer gives the key facts that the question asks for, consistent with the reference (right appliance / country / channel / leave type / numbers). Mark false if it uses facts from a different (sibling) policy, contradicts the reference, misses the key fact, or says the information is unavailable. Extra harmless detail is fine.
Return one grade per item with the same key. Reason: max 12 words.

"""


def load_runs():
    runs = []
    for f in sorted((E2E / "runs").glob("*/*.json")):
        if f.name.endswith(".mcp.json"):
            continue
        runs.append(json.loads(f.read_text()))
    return runs


def judge(runs, model):
    cache = E2E / "judge.json"
    grades = json.loads(cache.read_text()) if cache.exists() else {}
    todo = [r for r in runs if f"{r['cfg']}-{r['qid']}" not in grades]
    random.Random(7).shuffle(todo)  # trộn để judge không đoán được cấu hình
    for i in range(0, len(todo), 15):
        batch = todo[i:i + 15]
        body = JUDGE_PROMPT
        for n, r in enumerate(batch):
            d = DOCS[r["gold"]]
            body += (f"### item key={r['cfg']}-{r['qid']}\nQUESTION: {r['q']}\n"
                     f"REFERENCE ({d['lang']}): {d['title']}. {d['text']}\nANSWER: {r['answer']}\n\n")
        # key chứa cfg nhưng judge không được giải thích ý nghĩa; thay bằng mã ngẫu nhiên để mù hoàn toàn
        alias = {f"{r['cfg']}-{r['qid']}": f"k{random.Random(r['qid'] * 7 + ord(r['cfg'])).randint(10**5, 10**6)}" for r in batch}
        for k, v in alias.items():
            body = body.replace(f"key={k}\n", f"key={v}\n")
        p = subprocess.run(["claude", "-p", "--output-format", "json", "--no-session-persistence",
                            "--setting-sources", "local", "--tools", "", "--model", model,
                            "--json-schema", JUDGE_SCHEMA], input=body, capture_output=True, text=True,
                           cwd=E2E / "work", timeout=900)
        env = json.loads(p.stdout)
        inv = {v: k for k, v in alias.items()}
        for g in (env.get("structured_output") or {}).get("grades", []):
            if g["key"] in inv:
                grades[inv[g["key"]]] = {"correct": g["correct"], "reason": g["reason"]}
        cache.write_text(json.dumps(grades, ensure_ascii=False, indent=1))
        print(f"judged {min(i + 15, len(todo))}/{len(todo)}", flush=True)
    return grades


def pct(v, p):
    v = sorted(v)
    return v[min(len(v) - 1, int(round((len(v) - 1) * p)))] if v else 0


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--judge-model", default="sonnet")
    ap.add_argument("--no-judge", action="store_true")
    a = ap.parse_args()
    runs = load_runs()
    grades = {} if a.no_judge else judge(runs, a.judge_model)

    def stats(rs):
        n = len(rs)
        if not n:
            return None
        g = [grades.get(f"{r['cfg']}-{r['qid']}", {}).get("correct") for r in rs]
        tin = [sum((r["usage"] or {}).get(k, 0) for k in ("input_tokens", "cache_creation_input_tokens", "cache_read_input_tokens")) for r in rs]
        return {
            "n": n,
            "answer_ok": sum(1 for x in g if x) / n if any(x is not None for x in g) else None,
            "src_top1": sum(1 for r in rs if r["sources"][:1] == [r["gold"]]) / n,
            "src_any": sum(1 for r in rs if r["gold"] in r["sources"]) / n,
            "wall_p50": pct([r["wall_s"] for r in rs], .5), "wall_p90": pct([r["wall_s"] for r in rs], .9),
            "turns": statistics.mean(r["num_turns"] or 0 for r in rs),
            "tools": statistics.mean(len(r["tool_calls"]) for r in rs),
            "no_tool": sum(1 for r in rs if not r["tool_calls"]) / n,
            "in_tok": statistics.mean(tin),
            "out_tok": statistics.mean((r["usage"] or {}).get("output_tokens", 0) for r in rs),
            "cost": statistics.mean(r["cost_usd"] or 0 for r in rs),
            "errors": sum(1 for r in rs if r.get("is_error")),
        }

    names = {"A": "A chỉ tool", "B": "B catalog + tool", "C": "C top-5 + catalog + tool", "D": "D = C, catalog trong system prompt", "E": "E truyền thống: agent + thư mục bundle (Read/Grep/Glob)", "F": "F truyền thống: toàn bộ bundle trong prompt, không tool"}
    lines = ["| Cấu hình | Trả lời đúng | Nguồn đúng (top-1) | Nguồn có gold | Thời gian p50 / p90 | Lượt agent | Tool call TB | Không gọi tool | Token vào TB | Token ra TB | Chi phí TB |",
             "|---|---|---|---|---|---|---|---|---|---|---|"]
    subsets = {"all": lambda r: True, "hard (dense top-1 sai)": lambda r: not r["dense_top1_ok"], "easy (dense top-1 đúng)": lambda r: r["dense_top1_ok"]}
    out = {}
    for sname, f in subsets.items():
        out[sname] = {}
        if sname != "all":
            lines.append(f"| *{sname}* | | | | | | | | | | |")
        for c in "ABCDEF":
            s = stats([r for r in runs if r["cfg"] == c and f(r)])
            if not s:
                continue
            out[sname][c] = s
            ao = f"{s['answer_ok']:.0%}" if s["answer_ok"] is not None else "—"
            lines.append(f"| {names[c]} ({s['n']}) | **{ao}** | {s['src_top1']:.0%} | {s['src_any']:.0%} | {s['wall_p50']:.1f}s / {s['wall_p90']:.1f}s | "
                         f"{s['turns']:.1f} | {s['tools']:.1f} | {s['no_tool']:.0%} | {s['in_tok']:,.0f} | {s['out_tok']:,.0f} | ${s['cost']:.3f} |")
    # thống kê loại tool
    tl = ["", "| Cấu hình | kb_search | kb_grep | kb_get | kb_list | Read | Grep | Glob |", "|---|---|---|---|---|---|---|---|"]
    for c in "ABCDEF":
        rs = [r for r in runs if r["cfg"] == c]
        cnt = {t: sum(1 for r in rs for x in r["tool_calls"] if x["tool"] == t) for t in ("kb_search", "kb_grep", "kb_get", "kb_list", "Read", "Grep", "Glob")}
        tl.append(f"| {c} | " + " | ".join(str(cnt[t]) for t in cnt) + " |")
    # các câu sai
    wrong = ["", "Câu trả lời sai:"]
    for r in runs:
        g = grades.get(f"{r['cfg']}-{r['qid']}")
        if g and not g["correct"]:
            wrong.append(f"- {r['cfg']} q{r['qid']} [{r['lang']}→{r['gold']}] {r['q']} → src={r['sources'][:2]} — {g['reason']}")
    report = "\n".join(lines + tl + wrong)
    print(report)
    (E2E / "report.md").write_text(report)
    (E2E / "summary.json").write_text(json.dumps(out, indent=1))
    print(f"\nTổng chi phí: ${sum(r['cost_usd'] or 0 for r in runs):.2f}")


if __name__ == "__main__":
    main()
