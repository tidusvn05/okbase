#!/usr/bin/env python3
"""Chấm (LLM judge mù cấu hình, theo key facts) + tổng hợp spike quy mô.

Usage: python3 report.py [--judge-model sonnet]
"""
import argparse, json, random, statistics, subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent
NAMES = {"F": "F full-context (không tool)", "D": "D qobot: top-6 + catalog + tool", "A": "A chỉ tool (search/get/grep/list)",
         "E": "E agent + thư mục (Read/Grep/Glob)", "H": "H RAG thuần (top-6, không tool)", "G": "G duyệt cây index.md (không embedding)", "I": "I = D không catalog", "D2": "D2 = D + quy tắc kiểm tra", "G2": "G2 lexical: index.md + kb_grep mạnh (không embedding)"}
SIZES = ["S", "M", "L", "XL"]
SCHEMA = json.dumps({"type": "object", "properties": {"grades": {"type": "array", "items": {"type": "object", "properties": {
    "key": {"type": "string"}, "correct": {"type": "boolean"}, "facts_hit": {"type": "integer"}, "reason": {"type": "string"}},
    "required": ["key", "correct", "facts_hit", "reason"]}}}, "required": ["grades"]})
PROMPT = """You grade answers of a documentation support bot (OpenClaw). For each item: the user QUESTION, the REFERENCE answer and KEY FACTS (English), and the bot ANSWER (maybe in Vietnamese/Japanese/English).
- facts_hit = how many KEY FACTS the answer states correctly (semantic match, any language).
- correct = true only if the answer contains ALL key facts (or clearly equivalent) and nothing that contradicts the reference. Saying "I don't know"/not found = false.
Return one grade per item using the same key. reason: max 12 words.

"""


def load():
    runs = []
    for f in (ROOT / "runs").glob("*/*/*.json"):
        if f.name.endswith(".mcp.json"):
            continue
        runs.append(json.loads(f.read_text()))
    return runs


def judge(runs, model):
    cache_f = ROOT / "results/judge.json"
    grades = json.loads(cache_f.read_text()) if cache_f.exists() else {}
    k = lambda r: f"{r['size']}-{r['cfg']}-{r['qid']}"
    todo = [r for r in runs if k(r) not in grades and not r["is_error"]]
    random.Random(11).shuffle(todo)
    for i in range(0, len(todo), 12):
        batch = todo[i:i + 12]
        alias = {k(r): f"x{random.Random(hash(k(r)) & 0xffff).randint(10**6, 10**7)}{n}" for n, r in enumerate(batch)}
        body = PROMPT + "".join(
            f"### item key={alias[k(r)]}\nQUESTION: {r['q']}\nREFERENCE: {r['_ref']}\n"
            f"KEY FACTS: {json.dumps(r['key_facts'], ensure_ascii=False)}\nANSWER: {r['answer']}\n\n" for r in batch)
        p = subprocess.run(["claude", "-p", "--output-format", "json", "--no-session-persistence", "--setting-sources", "local",
                            "--tools", "", "--model", model, "--json-schema", SCHEMA], input=body, capture_output=True, text=True,
                           cwd=ROOT / "work", timeout=900)
        env = json.loads(p.stdout)
        inv = {v: kk for kk, v in alias.items()}
        for g in (env.get("structured_output") or {}).get("grades", []):
            if g["key"] in inv:
                grades[inv[g["key"]]] = g
        cache_f.write_text(json.dumps(grades, ensure_ascii=False, indent=1))
        print(f"judged {min(i + 12, len(todo))}/{len(todo)}", flush=True)
    return grades


def pct(v, p):
    v = sorted(v)
    return v[min(len(v) - 1, int(round((len(v) - 1) * p)))] if v else 0


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--judge-model", default="sonnet")
    a = ap.parse_args()
    qs = {q["qid"]: q for q in json.loads((ROOT / "questions.json").read_text())}
    runs = load()
    for r in runs:  # đáp án tham chiếu nằm trong questions.json dưới key "answer"
        r["_ref"] = qs[r["qid"]]["answer"]
    grades = judge(runs, a.judge_model)
    k = lambda r: f"{r['size']}-{r['cfg']}-{r['qid']}"
    lines = ["| Bundle | Cách | Đúng | Facts TB | Nguồn gold | p50 / p90 | Lượt | Tool call | Token vào TB | Chi phí TB | Lỗi |",
             "|---|---|---|---|---|---|---|---|---|---|---|"]
    summary = {}
    for s in SIZES:
        for c in ["F", "D", "D2", "I", "A", "E", "H", "G", "G2"]:
            rs = [r for r in runs if r["size"] == s and r["cfg"] == c]
            if not rs:
                continue
            g = [grades.get(k(r)) for r in rs]
            ok = sum(1 for x in g if x and x["correct"]) / len(rs)
            facts = statistics.mean((x["facts_hit"] / max(1, len(qs[r["qid"]]["key_facts"]))) if x else 0 for x, r in zip(g, rs))
            tin = statistics.mean(sum((r["usage"] or {}).get(t, 0) for t in ("input_tokens", "cache_creation_input_tokens", "cache_read_input_tokens")) for r in rs)
            st = {"n": len(rs), "correct": ok, "facts": facts, "src": sum(1 for r in rs if r["gold"] in r["sources"]) / len(rs),
                  "p50": pct([r["wall_s"] for r in rs], .5), "p90": pct([r["wall_s"] for r in rs], .9),
                  "turns": statistics.mean(r["num_turns"] or 0 for r in rs), "tools": statistics.mean(len(r["tool_calls"]) for r in rs),
                  "tin": tin, "cost": statistics.mean(r["cost_usd"] or 0 for r in rs), "err": sum(1 for r in rs if r["is_error"])}
            summary[f"{s}/{c}"] = st
            lines.append(f"| {s} | {NAMES[c]} | **{ok:.0%}** | {facts:.0%} | {st['src']:.0%} | {st['p50']:.1f}s / {st['p90']:.1f}s | {st['turns']:.1f} | "
                         f"{st['tools']:.1f} | {tin:,.0f} | ${st['cost']:.3f} | {st['err']} |")
    # theo ngôn ngữ câu hỏi (gộp mọi bundle)
    ll = ["", "| Cách | vi | en | ja |", "|---|---|---|---|"]
    for c in ["F", "D", "D2", "I", "A", "E", "H", "G", "G2"]:
        cells = []
        for lang in ("vi", "en", "ja"):
            rs = [r for r in runs if r["cfg"] == c and r["lang"] == lang]
            cells.append(f"{sum(1 for r in rs if (grades.get(k(r)) or {}).get('correct')) / len(rs):.0%}" if rs else "—")
        ll.append(f"| {NAMES[c]} | " + " | ".join(cells) + " |")
    wrong = ["", "Sai (theo bundle/cách):"]
    for r in sorted(runs, key=lambda r: (SIZES.index(r["size"]), r["cfg"], r["qid"])):
        g = grades.get(k(r))
        if not g or not g["correct"]:
            wrong.append(f"- {r['size']}/{r['cfg']} q{r['qid']} [{r['lang']}] src={r['sources'][:2]} — {(g or {}).get('reason', 'ERROR/không chấm')}")
    rep = "\n".join(lines + ll + wrong)
    (ROOT / "results/report.md").write_text(rep)
    (ROOT / "results/summary.json").write_text(json.dumps(summary, indent=1))
    print("\n".join(lines + ll))
    print(f"\nTổng chi phí chạy: ${sum(r['cost_usd'] or 0 for r in runs):.2f} trên {len(runs)} lượt")


if __name__ == "__main__":
    main()
