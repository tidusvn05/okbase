#!/usr/bin/env python3
"""v0.2 evals (PLAN §13): okbase in the setups of the biz-meta and okf-scale spikes.

  biz (biz-meta ×20 bundle: 3,020 docs, sheets of ~10k rows, 48 questions)
    K   okbase MCP (kb_query, data_tables, data_query) + Read/Grep/Glob, spike QD prompt   -> S5 ×20 replication
    KS  as K, but no tool hints in the prompt; the okbase-answer skill instead              -> S9 (skill)
  s8 (okf-scale L questions, 30, OpenClaw docs)
    R   agent with Read/Grep/Glob on the raw markdown files of bundle L (no OKF frontmatter, no index.md)
    A   the same files after `okbase adopt` (L1)                                              -> S8 (adopt)

Usage:
  cargo build --release -p okbase-cli
  python3 run.py biz [--configs K,KS] [--limit N]
  python3 run.py s8  [--configs R,A] [--limit N]
  python3 run.py report
Costs Claude Code quota (~$6 for everything).
"""
import argparse, concurrent.futures as cf, json, shutil, subprocess, sys, time
from pathlib import Path

HERE = Path(__file__).resolve().parent
SPIKES = HERE.parent
OKBASE = SPIKES.parent / "target/release/okbase"
sys.path.insert(0, str(SPIKES / "biz-meta"))
import run as biz  # noqa: E402  BASE, HINT, SCHEMA
import report as biz_report  # noqa: E402  PROMPT, SCHEMA (judge)
sys.path.pop(0)
for m in ("run", "report"):
    sys.modules.pop(m)
sys.path.insert(0, str(SPIKES / "okf-scale"))
import run as scale  # noqa: E402  BASE, SCHEMA, QS, BUNDLES
import report as scale_report  # noqa: E402  judge
sys.path.pop(0)

BIZ = SPIKES / "biz-meta/bundle-E-x20"
WORK = HERE / "work"
RAW_L = WORK / "raw-L"
ADOPTED_L = WORK / "adopted-L"
OPENCLAW = SPIKES / "embed-bench/.corpora/openclaw/docs"


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


def mcp_file(path, bundle, state):
    path.write_text(json.dumps({"mcpServers": {"kb": {"command": str(OKBASE), "args": [
        "--state-dir", str(state), "--bundle", str(bundle), "mcp", "serve", "--stdio"]}}}))


# ---------------------------------------------------------------- biz (S5 ×20, S9)
def prepare_biz():
    state = WORK / "state-biz"
    state.mkdir(parents=True, exist_ok=True)
    subprocess.run([str(OKBASE), "--state-dir", str(state), "-b", str(BIZ), "index"], check=True, capture_output=True)
    subprocess.run([str(OKBASE), "--state-dir", str(state), "-b", str(BIZ), "data", "tables"], check=True, capture_output=True)
    # The skill goes into the (git-ignored) spike bundle, the agent's working directory.
    subprocess.run([str(OKBASE), "-b", str(BIZ), "agent", "install", "--claude", "--project", str(BIZ)], check=True, capture_output=True)
    (BIZ / ".mcp.json").unlink(missing_ok=True)
    return state


def run_biz(cfg, q, model, state):
    d = HERE / "runs" / cfg
    d.mkdir(parents=True, exist_ok=True)
    out = d / f"{q['qid']}.json"
    if out.exists():
        return json.loads(out.read_text())
    sysf = d / f"{q['qid']}.system.txt"
    sysf.write_text(biz.BASE + (biz.HINT["QD"] if cfg == "K" else ""))
    mcp = d / f"{q['qid']}.mcp.json"
    mcp_file(mcp, BIZ, state)
    tools = ["Read", "Grep", "Glob"] + (["Skill"] if cfg == "KS" else [])
    allowed = tools + ["mcp__kb__kb_query", "mcp__kb__data_tables", "mcp__kb__data_query"]
    cmd = ["claude", "-p", "--output-format", "stream-json", "--verbose", "--no-session-persistence",
           "--setting-sources", "project,local" if cfg == "KS" else "local", "--strict-mcp-config", "--mcp-config", str(mcp),
           "--model", model, "--json-schema", biz.SCHEMA, "--append-system-prompt-file", str(sysf),
           "--tools", ",".join(tools), "--allowedTools", ",".join(allowed)]
    env, calls, wall, err = stream(cmd, f"User question:\n{q['q']}", BIZ)
    so = env.get("structured_output") or {}
    norm = lambda x: str(x).strip().removeprefix("./").removesuffix(".md").lstrip("/")
    rec = {"suite": "biz", "cfg": cfg, **q, "answer": so.get("answer", ""), "items": [norm(x) for x in so.get("items", [])],
           "sources": [norm(x) for x in so.get("sources", [])], "wall_s": round(wall, 2), "num_turns": env.get("num_turns"),
           "cost_usd": env.get("total_cost_usd"), "is_error": env.get("is_error", True) if env else True,
           "tool_calls": calls, "stderr": "" if env else err[-500:]}
    out.write_text(json.dumps(rec, ensure_ascii=False, indent=1))
    return rec


# ---------------------------------------------------------------- s8 (adopt)
S8_SYSTEM = scale.BASE + ("- The documentation is in the current directory as Markdown files. Use Glob/Grep/Read to find and read "
                          "the relevant documents before answering.\n")


def prepare_s8():
    if not RAW_L.exists():
        # The raw originals of exactly the files in okf-scale bundle L.
        for f in sorted((scale.BUNDLES / "L").rglob("*.md")):
            rel = f.relative_to(scale.BUNDLES / "L")
            if rel.name == "index.md":
                continue
            dst = RAW_L / rel
            dst.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy(OPENCLAW / rel, dst)
    if not ADOPTED_L.exists():
        subprocess.run([str(OKBASE), "adopt", str(RAW_L), "--out", str(ADOPTED_L)], check=True)
        lint = subprocess.run([str(OKBASE), "--state-dir", str(WORK / "state-s8"), "-b", str(ADOPTED_L), "lint", "--level", "L1"],
                              capture_output=True, text=True)
        print(lint.stdout.strip().splitlines()[-1])


def run_s8(cfg, q, model):
    d = HERE / "runs" / cfg
    d.mkdir(parents=True, exist_ok=True)
    out = d / f"{q['qid']}.json"
    if out.exists():
        return json.loads(out.read_text())
    sysf = d / f"{q['qid']}.system.txt"
    sysf.write_text(S8_SYSTEM)
    cwd = RAW_L if cfg == "R" else ADOPTED_L
    cmd = ["claude", "-p", "--output-format", "stream-json", "--verbose", "--no-session-persistence", "--setting-sources", "local",
           "--strict-mcp-config", "--model", model, "--json-schema", scale.SCHEMA, "--append-system-prompt-file", str(sysf),
           "--tools", "Read,Grep,Glob", "--allowedTools", "Read,Grep,Glob"]
    env, calls, wall, err = stream(cmd, f"User question:\n{q['q']}", cwd)
    so = env.get("structured_output") or {}
    rec = {"suite": "s8", "size": "L", "cfg": cfg, **q, "answer": so.get("answer", ""),
           "sources": [Path(str(x)).as_posix().removesuffix(".md").lstrip("./") for x in so.get("sources", [])],
           "wall_s": round(wall, 2), "num_turns": env.get("num_turns"), "cost_usd": env.get("total_cost_usd"),
           "is_error": env.get("is_error", True) if env else True, "tool_calls": calls, "stderr": "" if env else err[-500:]}
    out.write_text(json.dumps(rec, ensure_ascii=False, indent=1))
    return rec


def run_all(jobs_list, fn, jobs):
    done = 0
    cost = 0.0
    with cf.ThreadPoolExecutor(jobs) as ex:
        for f in cf.as_completed([ex.submit(fn, *j) for j in jobs_list]):
            r = f.result()
            done += 1
            cost += r["cost_usd"] or 0
            print(f"[{done}/{len(jobs_list)}] {r['cfg']} q{r['qid']} {r['wall_s']}s turns={r['num_turns']} tools={len(r['tool_calls'])} (Σ ${cost:.2f})", flush=True)


# ---------------------------------------------------------------- report
def load(suite):
    return [json.loads(f.read_text()) for f in (HERE / "runs").glob("*/*.json")
            if not f.name.endswith(".mcp.json") and json.loads(f.read_text()).get("suite") == suite]


def grade_biz(runs):
    gf = HERE / "results/judge-biz.json"
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
        body = biz_report.PROMPT + "".join(f"### item key={alias[k(r)]}\nQUESTION: {r['q']}\nEXPECTED: {json.dumps(r['expected'], ensure_ascii=False)}\n"
                                           f"ANSWER: {r['answer']}\nANSWER ITEMS: {r['items']}\n\n" for r in batch)
        (WORK / "judge").mkdir(parents=True, exist_ok=True)
        p = subprocess.run(["claude", "-p", "--output-format", "json", "--no-session-persistence", "--setting-sources", "local", "--tools", "",
                            "--model", "sonnet", "--json-schema", biz_report.SCHEMA], input=body, capture_output=True, text=True,
                           cwd=WORK / "judge", timeout=900)
        inv = {v: kk for kk, v in alias.items()}
        for g in (json.loads(p.stdout).get("structured_output") or {}).get("grades", []):
            if g["key"] in inv:
                grades[inv[g["key"]]] = g
        print(f"judged {min(i + 12, len(todo))}/{len(todo)}", flush=True)
    gf.write_text(json.dumps(grades, ensure_ascii=False, indent=1))
    return grades


def report():
    (HERE / "results").mkdir(exist_ok=True)
    lines = []
    biz_runs = load("biz")
    if biz_runs:
        grades = grade_biz(biz_runs)
        ok = lambda r: bool((grades.get(f"{r['cfg']}-{r['qid']}") or {}).get("correct"))
        cfgs = sorted({r["cfg"] for r in biz_runs})
        cats = ["list", "version", "count", "facet", "sheet", "content"]
        lines += ["## biz-meta ×20 (48 questions)", "", "| Category | " + " | ".join(cfgs) + " |", "|---|" + "---|" * len(cfgs)]
        for cat in cats + ["all"]:
            row = []
            for c in cfgs:
                rs = [r for r in biz_runs if r["cfg"] == c and (cat == "all" or r["cat"] == cat)]
                row.append(f"{sum(map(ok, rs))}/{len(rs)}")
            lines.append(f"| {cat} | " + " | ".join(row) + " |")
        for label, fn in [("kb_query used (runs)", lambda rs: sum(any(t["tool"] == "kb_query" for t in r["tool_calls"]) for r in rs)),
                          ("data_query used on sheet questions", lambda rs: sum(any(t["tool"] == "data_query" for t in r["tool_calls"]) for r in rs if r["cat"] == "sheet")),
                          ("Skill invoked (runs)", lambda rs: sum(any(t["tool"] == "Skill" for t in r["tool_calls"]) for r in rs)),
                          ("mean cost", lambda rs: f"${sum(r['cost_usd'] or 0 for r in rs) / max(1, len(rs)):.3f}"),
                          ("mean turns", lambda rs: f"{sum(r['num_turns'] or 0 for r in rs) / max(1, len(rs)):.1f}"),
                          ("mean wall time", lambda rs: f"{sum(r['wall_s'] for r in rs) / max(1, len(rs)):.0f}s")]:
            lines.append(f"| {label} | " + " | ".join(str(fn([r for r in biz_runs if r["cfg"] == c])) for c in cfgs) + " |")
        lines.append("")
    s8_runs = load("s8")
    if s8_runs:
        for r in s8_runs:
            r["_ref"] = scale.QS[r["qid"]]["answer"]
        scale_report.ROOT = HERE  # judge cache in results/judge.json, cwd work/
        (HERE / "work").mkdir(exist_ok=True)
        grades = scale_report.judge(s8_runs, "sonnet")
        key = lambda r: f"{r['size']}-{r['cfg']}-{r['qid']}"
        cfgs = sorted({r["cfg"] for r in s8_runs})
        lines += ["## S8 adopt (okf-scale L, 30 questions, Read/Grep/Glob)", "", "| Metric | " + " | ".join(cfgs) + " |", "|---|" + "---|" * len(cfgs)]
        for label, fn in [("correct", lambda rs: f"{sum(1 for r in rs if (grades.get(key(r)) or {}).get('correct'))}/{len(rs)}"),
                          ("mean tool calls", lambda rs: f"{sum(len(r['tool_calls']) for r in rs) / max(1, len(rs)):.1f}"),
                          ("mean cost", lambda rs: f"${sum(r['cost_usd'] or 0 for r in rs) / max(1, len(rs)):.3f}"),
                          ("mean wall time", lambda rs: f"{sum(r['wall_s'] for r in rs) / max(1, len(rs)):.0f}s")]:
            lines.append(f"| {label} | " + " | ".join(fn([r for r in s8_runs if r["cfg"] == c]) for c in cfgs) + " |")
    text = "\n".join(lines) + "\n"
    (HERE / "results/table.md").write_text(text)
    print(text)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("suite", choices=["biz", "s8", "report"])
    ap.add_argument("--configs")
    ap.add_argument("--limit", type=int, default=0)
    ap.add_argument("--jobs", type=int, default=4)
    ap.add_argument("--model", default="sonnet")
    a = ap.parse_args()
    if not OKBASE.exists():
        sys.exit("build first: cargo build --release -p okbase-cli")
    if a.suite == "biz":
        state = prepare_biz()
        qs = json.loads((SPIKES / "biz-meta/questions-x20.json").read_text())[: a.limit or None]
        run_all([(c, q, a.model, state) for q in qs for c in (a.configs or "K,KS").split(",")], run_biz, a.jobs)
    elif a.suite == "s8":
        prepare_s8()
        qs = list(scale.QS.values())[: a.limit or None]
        run_all([(c, q, a.model) for q in qs for c in (a.configs or "R,A").split(",")], run_s8, a.jobs)
    else:
        report()


if __name__ == "__main__":
    main()
