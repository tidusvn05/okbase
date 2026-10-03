#!/usr/bin/env python3
"""S15: a smaller MCP tool list, and the answer skill's rules as MCP server instructions.

On the biz-meta x20 bundle (3,020 docs, sheets of ~10k rows, 48 questions), Claude Code with
Read/Grep/Glob and every okbase tool:
  K0  okbase before the change (bedbc52) + the spike's tool hints in the system prompt  (baseline)
  K1  okbase after the change              + the same hints                             (trim: cost)
  I1  okbase after the change, no hints: only okbase's server instructions             (S9 again)

Usage:
  python3 run.py run [--configs K0,K1,I1] [--limit N] [--jobs 6]
  python3 run.py report
Binaries: $BASE_BIN (K0) and ../../target/release/okbase (K1, I1). Costs Claude Code quota (~$6).
"""
import argparse, concurrent.futures as cf, json, os, subprocess, sys, time
from pathlib import Path

HERE = Path(__file__).resolve().parent
SPIKES = HERE.parent
NEW_BIN = SPIKES.parent / "target/release/okbase"
BASE_BIN = Path(os.environ.get("BASE_BIN", HERE / "work/okbase-base"))
sys.path.insert(0, str(SPIKES / "biz-meta"))
import run as biz  # noqa: E402  BASE, HINT, SCHEMA, QS
import report as biz_report  # noqa: E402  judge PROMPT, SCHEMA
sys.path.pop(0)

BIZ = SPIKES / "biz-meta/bundle-E-x20"
WORK = HERE / "work"
QS = json.loads((SPIKES / "biz-meta/questions-x20.json").read_text())
CFG = {"K0": (BASE_BIN, True), "K1": (NEW_BIN, True), "I1": (NEW_BIN, False)}


def stream(cmd, user, cwd):
    t = time.time()
    p = subprocess.run(cmd, input=user, capture_output=True, text=True, cwd=cwd, timeout=900)
    wall = time.time() - t
    env, calls = {}, []
    for line in p.stdout.splitlines():
        try:
            ev = json.loads(line)
        except json.JSONDecodeError:
            continue
        if ev.get("type") == "assistant":
            for c in ev["message"].get("content", []):
                if c.get("type") == "tool_use" and c.get("name") != "StructuredOutput":
                    calls.append({"tool": c["name"].removeprefix("mcp__kb__"), "args": c.get("input")})
        elif ev.get("type") == "result":
            env = ev
    return env, calls, wall, p.stderr


def run_one(cfg, q, model):
    d = HERE / "runs" / cfg
    d.mkdir(parents=True, exist_ok=True)
    out = d / f"{q['qid']}.json"
    if out.exists():
        return json.loads(out.read_text())
    binary, hints = CFG[cfg]
    state = WORK / f"state-{cfg}"
    sysf = d / f"{q['qid']}.system.txt"
    sysf.write_text(biz.BASE + (biz.HINT["QD"] if hints else ""))
    mcp = d / f"{q['qid']}.mcp.json"
    mcp.write_text(json.dumps({"mcpServers": {"kb": {"command": str(binary), "args": [
        "--state-dir", str(state), "--bundle", str(BIZ), "mcp", "serve", "--stdio"]}}}))
    cmd = ["claude", "-p", "--output-format", "stream-json", "--verbose", "--no-session-persistence",
           "--setting-sources", "local", "--strict-mcp-config", "--mcp-config", str(mcp),
           "--model", model, "--json-schema", biz.SCHEMA, "--append-system-prompt-file", str(sysf),
           "--tools", "Read,Grep,Glob", "--allowedTools", "Read,Grep,Glob,mcp__kb"]
    env, calls, wall, err = stream(cmd, f"User question:\n{q['q']}", BIZ)
    so = env.get("structured_output") or {}
    norm = lambda x: str(x).strip().removeprefix("./").removesuffix(".md").lstrip("/")
    u = env.get("usage") or {}
    rec = {"cfg": cfg, **q, "answer": so.get("answer", ""), "items": [norm(x) for x in so.get("items", [])],
           "sources": [norm(x) for x in so.get("sources", [])], "wall_s": round(wall, 2),
           "num_turns": env.get("num_turns"), "cost_usd": env.get("total_cost_usd"),
           "input_tokens": sum(u.get(k, 0) for k in ("input_tokens", "cache_creation_input_tokens", "cache_read_input_tokens")),
           "is_error": env.get("is_error", True) if env else True, "tool_calls": calls,
           "stderr": "" if env else err[-500:]}
    out.write_text(json.dumps(rec, ensure_ascii=False, indent=1))
    return rec


def prepare(cfgs):
    for cfg in cfgs:
        binary, _ = CFG[cfg]
        state = WORK / f"state-{cfg}"
        state.mkdir(parents=True, exist_ok=True)
        for args in (["index"], ["data", "tables"]):
            subprocess.run([str(binary), "--state-dir", str(state), "-b", str(BIZ), *args], check=True, capture_output=True)


def run(cfgs, limit, jobs, model):
    prepare(cfgs)
    todo = [(c, q, model) for q in QS[:limit] for c in cfgs]
    cost, done = 0.0, 0
    with cf.ThreadPoolExecutor(jobs) as ex:
        for f in cf.as_completed([ex.submit(run_one, *j) for j in todo]):
            r = f.result()
            done += 1
            cost += r["cost_usd"] or 0
            print(f"[{done}/{len(todo)}] {r['cfg']} q{r['qid']} {r['wall_s']}s turns={r['num_turns']} "
                  f"tools={len(r['tool_calls'])} (Σ ${cost:.2f})", flush=True)


def grade(runs):
    gf = HERE / "results/judge.json"
    grades = json.loads(gf.read_text()) if gf.exists() else {}
    k = lambda r: f"{r['cfg']}-{r['qid']}"
    for r in runs:
        if r["cat"] == "list":
            exp, got = set(r["expected"]), set(r["items"])
            grades[k(r)] = {"correct": exp == got, "reason": f"missing={sorted(exp - got)} extra={sorted(got - exp)}"}
    todo = [r for r in runs if r["cat"] != "list" and k(r) not in grades and not r["is_error"]]
    for i in range(0, len(todo), 12):
        batch = todo[i:i + 12]
        alias = {k(r): f"g{n}{(i + n) * 7919 % 9000 + 1000}" for n, r in enumerate(batch)}
        body = biz_report.PROMPT + "".join(
            f"### item key={alias[k(r)]}\nQUESTION: {r['q']}\nEXPECTED: {json.dumps(r['expected'], ensure_ascii=False)}\n"
            f"ANSWER: {r['answer']}\nANSWER ITEMS: {r['items']}\n\n" for r in batch)
        (WORK / "judge").mkdir(parents=True, exist_ok=True)
        p = subprocess.run(["claude", "-p", "--output-format", "json", "--no-session-persistence", "--setting-sources", "local",
                            "--tools", "", "--model", "sonnet", "--json-schema", biz_report.SCHEMA], input=body,
                           capture_output=True, text=True, cwd=WORK / "judge", timeout=900)
        inv = {v: kk for kk, v in alias.items()}
        for g in (json.loads(p.stdout).get("structured_output") or {}).get("grades", []):
            if g["key"] in inv:
                grades[inv[g["key"]]] = g
        print(f"judged {min(i + 12, len(todo))}/{len(todo)}", flush=True)
    gf.parent.mkdir(exist_ok=True)
    gf.write_text(json.dumps(grades, ensure_ascii=False, indent=1))
    return grades


def report():
    runs = [json.loads(f.read_text()) for f in (HERE / "runs").glob("*/*.json") if not f.name.endswith(".mcp.json")]
    grades = grade(runs)
    ok = lambda r: bool((grades.get(f"{r['cfg']}-{r['qid']}") or {}).get("correct"))
    cfgs = [c for c in CFG if any(r["cfg"] == c for r in runs)]
    cats = sorted({r["cat"] for r in runs})
    mean = lambda xs: sum(xs) / len(xs) if xs else 0
    lines = ["| | " + " | ".join(cfgs) + " |", "|---|" + "---|" * len(cfgs)]
    for cat in cats:
        lines.append(f"| {cat} | " + " | ".join(f"{sum(ok(r) for r in runs if r['cfg'] == c and r['cat'] == cat)}/"
                                                f"{sum(1 for r in runs if r['cfg'] == c and r['cat'] == cat)}" for c in cfgs) + " |")
    by = {c: [r for r in runs if r["cfg"] == c] for c in cfgs}
    lines.append("| **all** | " + " | ".join(f"**{sum(map(ok, by[c]))}/{len(by[c])}**" for c in cfgs) + " |")
    rows = [("mean cost", lambda rs: f"${mean([r['cost_usd'] or 0 for r in rs]):.4f}"),
            ("mean input tokens", lambda rs: f"{mean([r.get('input_tokens', 0) for r in rs]):,.0f}"),
            ("mean turns", lambda rs: f"{mean([r['num_turns'] or 0 for r in rs]):.1f}"),
            ("mean wall time", lambda rs: f"{mean([r['wall_s'] for r in rs]):.1f} s"),
            ("kb_query used (runs)", lambda rs: str(sum(any(c['tool'] == 'kb_query' for c in r['tool_calls']) for r in rs))),
            ("data_query on sheet questions", lambda rs: str(sum(any(c['tool'] == 'data_query' for c in r['tool_calls']) for r in rs if r['cat'] == 'sheet'))),
            ("errors", lambda rs: str(sum(r['is_error'] for r in rs)))]
    for name, f in rows:
        lines.append(f"| {name} | " + " | ".join(f(by[c]) for c in cfgs) + " |")
    wrong = [f"- {r['cfg']} q{r['qid']} ({r['cat']}): {(grades.get(r['cfg'] + '-' + str(r['qid'])) or {}).get('reason', '')[:160]}"
             for c in cfgs for r in sorted(by[c], key=lambda r: r["qid"]) if not ok(r)]
    text = "\n".join(lines) + "\n\nMissed:\n" + "\n".join(wrong) + "\n"
    (HERE / "results/table.md").write_text(text)
    print(text)


if __name__ == "__main__":
    ap = argparse.ArgumentParser()
    ap.add_argument("cmd", choices=["run", "report"])
    ap.add_argument("--configs", default="K0,K1,I1")
    ap.add_argument("--limit", type=int, default=None)
    ap.add_argument("--jobs", type=int, default=6)
    ap.add_argument("--model", default="sonnet")
    a = ap.parse_args()
    run(a.configs.split(","), a.limit, a.jobs, a.model) if a.cmd == "run" else report()
