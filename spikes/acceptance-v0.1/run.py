#!/usr/bin/env python3
"""v0.1 acceptance (HANDOFF T11): the okf-scale G2 setup, with the MCP server
replaced by `okbase mcp serve --stdio`, with and without the okbase-answer skill.

Configs (bundle L, 30 questions, same system prompt, questions and judge as okf-scale G2):
  O   okbase      — G2 system prompt (root index.md + lexical strategy) + okbase MCP tools
  OS  okbase+skill — as O, plus the okbase-answer skill installed in the working directory

Target: >= 93% correct on bundle L (G2 in the spike).

Usage:
  cargo build --release -p okbase-cli
  python3 run.py [--configs O,OS] [--jobs 4] [--limit N] [--model sonnet] [--judge-model sonnet]
Costs Claude Code quota: about $0.02 per run plus judging (~$1.5 for both configs).
"""
import argparse, concurrent.futures as cf, json, subprocess, sys, time
from pathlib import Path

HERE = Path(__file__).resolve().parent
SCALE = HERE.parent / "okf-scale"
sys.path.insert(0, str(SCALE))
import run as scale  # noqa: E402  (BASE, LEXICAL, SCHEMA, QS, BUNDLES)
import report as scale_report  # noqa: E402  (PROMPT, judge)

OKBASE = HERE.parent.parent / "target/release/okbase"
SIZE = "L"
BUNDLE = scale.BUNDLES / SIZE
STATE = HERE / "work/state"
TOOLS = [f"mcp__kb__kb_{t}" for t in ("catalog", "list", "grep", "get", "query", "links")]


def prepare():
    if not OKBASE.exists():
        sys.exit(f"build the CLI first: cargo build --release -p okbase-cli ({OKBASE} missing)")
    if not BUNDLE.is_dir():
        sys.exit(f"{BUNDLE} missing: build the okf-scale bundles first (spikes/okf-scale/build_bundles.py)")
    STATE.mkdir(parents=True, exist_ok=True)
    # Index once so the parallel MCP servers start from a warm index.
    subprocess.run([str(OKBASE), "--state-dir", str(STATE), "-b", str(BUNDLE), "index"], check=True)
    skill_dir = HERE / "work/skill"
    skill_dir.mkdir(parents=True, exist_ok=True)
    subprocess.run([str(OKBASE), "-b", str(BUNDLE), "agent", "install", "--claude", "--project", str(skill_dir)], check=True,
                   capture_output=True)
    (skill_dir / ".mcp.json").unlink(missing_ok=True)  # the MCP server comes from --mcp-config below
    (HERE / "work/plain").mkdir(parents=True, exist_ok=True)


def run_one(cfg, q, model):
    run_dir = HERE / "runs" / SIZE / cfg
    run_dir.mkdir(parents=True, exist_ok=True)
    out = run_dir / f"{q['qid']}.json"
    if out.exists():
        return json.loads(out.read_text())
    mcp = run_dir / f"{q['qid']}.mcp.json"
    mcp.write_text(json.dumps({"mcpServers": {"kb": {"command": str(OKBASE), "args": [
        "--state-dir", str(STATE), "--bundle", str(BUNDLE), "mcp", "serve", "--stdio"]}}}))
    system = scale.BASE + scale.LEXICAL + "\n" + (BUNDLE / "index.md").read_text()
    sp = run_dir / f"{q['qid']}.system.txt"
    sp.write_text(system)
    cwd = HERE / "work" / ("skill" if cfg == "OS" else "plain")
    sources = "project,local" if cfg == "OS" else "local"
    cmd = ["claude", "-p", "--output-format", "stream-json", "--verbose", "--no-session-persistence",
           "--setting-sources", sources, "--strict-mcp-config", "--mcp-config", str(mcp), "--model", model,
           "--json-schema", scale.SCHEMA, "--append-system-prompt-file", str(sp),
           "--tools", "Skill" if cfg == "OS" else "", "--allowedTools", ",".join(TOOLS + (["Skill"] if cfg == "OS" else []))]
    t = time.time()
    p = subprocess.run(cmd, input=f"User question:\n{q['q']}", capture_output=True, text=True, cwd=cwd, timeout=900)
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
    so = env.get("structured_output") or {}
    if not so and isinstance(env.get("result"), str):
        try:
            so = json.loads(env["result"])
        except Exception:
            so = {"answer": env.get("result", ""), "sources": []}
    rec = {"size": SIZE, "cfg": cfg, **q, "answer": so.get("answer", ""),
           "sources": [Path(str(x)).as_posix().removesuffix(".md").lstrip("./") for x in so.get("sources", [])],
           "wall_s": round(wall, 2), "num_turns": env.get("num_turns"), "cost_usd": env.get("total_cost_usd"),
           "usage": env.get("usage"), "is_error": env.get("is_error", True) if env else True,
           "tool_calls": calls, "system_chars": len(system), "stderr": p.stderr[-500:] if not env else ""}
    out.write_text(json.dumps(rec, ensure_ascii=False, indent=1))
    return rec


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--configs", default="O,OS")
    ap.add_argument("--jobs", type=int, default=4)
    ap.add_argument("--limit", type=int, default=0)
    ap.add_argument("--model", default="sonnet")
    ap.add_argument("--judge-model", default="sonnet")
    a = ap.parse_args()
    prepare()
    qs = list(scale.QS.values())[: a.limit or None]
    jobs = [(c, q) for q in qs for c in a.configs.split(",")]
    runs = []
    with cf.ThreadPoolExecutor(a.jobs) as ex:
        for f in cf.as_completed([ex.submit(run_one, c, q, a.model) for c, q in jobs]):
            r = f.result()
            runs.append(r)
            print(f"{len(runs)}/{len(jobs)} {r['cfg']} q{r['qid']} ${r['cost_usd'] or 0:.3f} {r['wall_s']}s", flush=True)

    # Judge with the okf-scale judge (same prompt and model), cached in results/judge.json.
    for r in runs:
        r["_ref"] = scale.QS[r["qid"]]["answer"]
    scale_report.ROOT = HERE
    (HERE / "results").mkdir(exist_ok=True)
    grades = scale_report.judge(runs, a.judge_model)
    key = lambda r: f"{r['size']}-{r['cfg']}-{r['qid']}"
    summary = {}
    lines = ["| Config | Correct | Facts | Turns (mean) | Tool calls (mean) | kb_query used | Cost (mean) |", "|---|---|---|---|---|---|---|"]
    for c in a.configs.split(","):
        rs = [r for r in runs if r["cfg"] == c]
        gs = [grades.get(key(r)) for r in rs]
        ok = sum(1 for g in gs if g and g["correct"])
        facts = sum(g["facts_hit"] for g in gs if g) / max(1, sum(len(r["key_facts"]) for r in rs))
        turns = sum(r["num_turns"] or 0 for r in rs) / max(1, len(rs))
        tc = sum(len(r["tool_calls"]) for r in rs) / max(1, len(rs))
        uq = sum(1 for r in rs if any(t["tool"] == "kb_query" for t in r["tool_calls"]))
        cost = sum(r["cost_usd"] or 0 for r in rs) / max(1, len(rs))
        summary[c] = {"n": len(rs), "correct": ok, "facts": round(facts, 3), "turns": round(turns, 2), "tool_calls": round(tc, 2),
                      "cost_usd": round(cost, 4)}
        lines.append(f"| {c} | {ok}/{len(rs)} ({100 * ok / max(1, len(rs)):.0f}%) | {100 * facts:.0f}% | {turns:.1f} | {tc:.1f} | {uq} | ${cost:.3f} |")
    table = "\n".join(lines)
    print("\n" + table + "\n\nTarget (HANDOFF T11): >= 93% on bundle L (okf-scale G2 = 93%).")
    (HERE / "results/summary.json").write_text(json.dumps(summary, indent=1))
    (HERE / "results/table.md").write_text(table + "\n")


if __name__ == "__main__":
    main()
