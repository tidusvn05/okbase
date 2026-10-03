#!/usr/bin/env python3
"""S7: lexical okfkit with Codex (and a small model), on the okf-scale L bundle (287 OpenClaw pages,
30 questions vi/en/ja): the setup of the v0.1 acceptance (config O) with `codex exec` instead of
Claude Code. Same system text (okf-scale G2: rules + lexical strategy + root index.md), questions,
structured answer and judge (okf-scale's, Claude Sonnet).

Codex has no system-prompt flag: the system text goes first in the prompt. It always has a shell;
it runs in an empty directory with a read-only sandbox, and shell use is recorded.

Usage: python3 s7.py run --models gpt-6.1-sol,gpt-6-luna [--limit N] [--jobs 4]
       python3 s7.py report
Costs Codex usage (ChatGPT plan) and a little Claude quota for the judge.
"""
import argparse, concurrent.futures as cf, json, os, subprocess, sys, time
from pathlib import Path

HERE = Path(__file__).resolve().parent
SPIKES = HERE.parent
sys.path.insert(0, str(SPIKES / "okf-scale"))
import run as scale  # noqa: E402  BASE, LEXICAL, SCHEMA, QS, BUNDLES
import report as scale_report  # noqa: E402  judge
sys.path.pop(0)

OKFKIT = Path(os.environ.get("OKFKIT_BIN", HERE / "work/okfkit"))
BUNDLE = scale.BUNDLES / "L"
WORK = HERE / "work"
# OpenAI structured outputs need a strict schema (additionalProperties: false).
SCHEMA = json.dumps({**json.loads(scale.SCHEMA), "additionalProperties": False})
STATE = WORK / "state-L"


def codex(prompt, model, cwd, extra_cfg, schema=None, timeout=900):
    """Runs `codex exec --json`; returns (final message, tool calls, usage, wall, stderr)."""
    cmd = ["codex", "exec", "--json", "--ephemeral", "--skip-git-repo-check", "--ignore-user-config",
           "-m", model, "-c", 'model_reasoning_effort="medium"', "-s", "read-only", "-C", str(cwd)]
    for c in extra_cfg:
        cmd += ["-c", c]
    last = cwd / ".last.txt"
    if schema:
        sf = cwd / ".schema.json"
        sf.write_text(schema)
        cmd += ["--output-schema", str(sf)]
    cmd += ["-o", str(last), "-"]
    t = time.time()
    p = subprocess.run(cmd, input=prompt, capture_output=True, text=True, timeout=timeout)
    wall = time.time() - t
    calls, usage, errors = [], {}, []
    for line in p.stdout.splitlines():
        try:
            ev = json.loads(line)
        except json.JSONDecodeError:
            continue
        item = ev.get("item") or {}
        if ev.get("type") == "item.completed":
            if item.get("type") == "mcp_tool_call":
                calls.append({"tool": item.get("tool"), "args": item.get("arguments")})
            elif item.get("type") == "command_execution":
                calls.append({"tool": "shell", "args": item.get("command")})
        if ev.get("type") in ("error", "turn.failed"):
            errors.append(str(ev.get("message") or ev.get("error"))[:300])
        if ev.get("type") == "turn.completed":
            for k, v in (ev.get("usage") or {}).items():
                usage[k] = usage.get(k, 0) + v
    final = last.read_text() if last.exists() else ""
    return final, calls, usage, wall, " | ".join(errors) + p.stderr[-500:]


SEARCH_STATE = WORK / "state-L-bge"
MODELS_DIR = os.environ.get("OKFKIT_MODELS_DIR", "/tmp/claude-1000/-home-beebiz-workspace-okfkit/7c824f32-bccf-4cb3-b57c-91551aa33e32/scratchpad/models")


def run_one(cfg, q):
    """`cfg` is a Codex model, or `<model>+search`: kb_search on (bge-m3 int8, the v0.3 vectors)."""
    model, search = cfg.removesuffix("+search"), cfg.endswith("+search")
    d = HERE / "runs/s7" / cfg
    d.mkdir(parents=True, exist_ok=True)
    out = d / f"{q['qid']}.json"
    if out.exists():
        return json.loads(out.read_text())
    cwd = WORK / "empty" / cfg / str(q["qid"])
    cwd.mkdir(parents=True, exist_ok=True)
    system = scale.BASE + (scale.TOOLS_HINT if search else scale.LEXICAL) + "\n" + (BUNDLE / "index.md").read_text()
    mcp = [f'mcp_servers.kb.command="{OKFKIT}"',
           "mcp_servers.kb.args=" + json.dumps(["--state-dir", str(SEARCH_STATE if search else STATE), "--bundle", str(BUNDLE),
                                                "mcp", "serve", "--stdio"])]
    if search:
        mcp.append(f'mcp_servers.kb.env={{OKFKIT_MODELS_DIR="{MODELS_DIR}", XDG_CACHE_HOME="{WORK / "xdg"}"}}')
        mcp.append("mcp_servers.kb.startup_timeout_sec=60")
    final, calls, usage, wall, err = codex(f"{system}\n\nUser question:\n{q['q']}", model, cwd, mcp, SCHEMA)
    try:
        so = json.loads(final)
    except json.JSONDecodeError:
        so = {"answer": final, "sources": []}
    rec = {"size": "L", "cfg": cfg, **q, "answer": so.get("answer", ""),
           "sources": [Path(str(x)).as_posix().removesuffix(".md").lstrip("./") for x in so.get("sources", [])],
           "wall_s": round(wall, 2), "usage": usage, "tool_calls": calls, "is_error": not final, "stderr": "" if final else err}
    out.write_text(json.dumps(rec, ensure_ascii=False, indent=1))
    return rec


def run(models, limit, jobs):
    STATE.mkdir(parents=True, exist_ok=True)
    subprocess.run([str(OKFKIT), "--state-dir", str(STATE), "-b", str(BUNDLE), "index"], check=True, capture_output=True)
    qs = list(scale.QS.values())[:limit]
    todo = [(m, q) for q in qs for m in models]
    done = 0
    with cf.ThreadPoolExecutor(jobs) as ex:
        for f in cf.as_completed([ex.submit(run_one, m, q) for m, q in todo]):
            r = f.result()
            done += 1
            print(f"[{done}/{len(todo)}] {r['cfg']} q{r['qid']} {r['wall_s']}s tools={len(r['tool_calls'])} "
                  f"in={r['usage'].get('input_tokens', 0)} err={r['is_error']}", flush=True)


def report():
    runs = [json.loads(f.read_text()) for f in (HERE / "runs/s7").glob("*/*.json")]
    for r in runs:
        r["_ref"] = scale.QS[r["qid"]]["answer"]
    scale_report.ROOT = HERE
    (HERE / "results").mkdir(exist_ok=True)
    grades = scale_report.judge(runs, "sonnet")
    key = lambda r: f"{r['size']}-{r['cfg']}-{r['qid']}"
    mean = lambda xs: sum(xs) / len(xs) if xs else 0
    lines = ["| Model | Correct | Key facts | vi / en / ja | Tool calls (mean) | Shell calls | Input tokens (mean) | Wall (mean) |",
             "|---|---|---|---|---|---|---|---|"]
    for m in sorted({r["cfg"] for r in runs}):
        rs = [r for r in runs if r["cfg"] == m]
        gs = {r["qid"]: grades.get(key(r)) or {} for r in rs}
        ok = sum(bool(g.get("correct")) for g in gs.values())
        facts = sum(g.get("facts_hit", 0) for g in gs.values()) / max(1, sum(len(r["key_facts"]) for r in rs))
        lang = " / ".join(f"{sum(bool(gs[r['qid']].get('correct')) for r in rs if r['lang'] == l)}/{sum(r['lang'] == l for r in rs)}"
                          for l in ("vi", "en", "ja"))
        lines.append(f"| {m} | {ok}/{len(rs)} ({100 * ok / len(rs):.0f}%) | {100 * facts:.0f}% | {lang} | "
                     f"{mean([len(r['tool_calls']) for r in rs]):.1f} | {sum(t['tool'] == 'shell' for r in rs for t in r['tool_calls'])} | "
                     f"{mean([r['usage'].get('input_tokens', 0) for r in rs]):,.0f} | {mean([r['wall_s'] for r in rs]):.0f} s |")
        lines += [f"  - missed q{r['qid']} ({r['lang']}): {(gs[r['qid']].get('reason') or '')[:140]}" for r in sorted(rs, key=lambda r: r['qid'])
                  if not gs[r['qid']].get('correct')]
    text = "\n".join(lines) + "\n"
    (HERE / "results/s7-table.md").write_text(text)
    print(text)


if __name__ == "__main__":
    ap = argparse.ArgumentParser()
    ap.add_argument("cmd", choices=["run", "report"])
    ap.add_argument("--models", default="gpt-6.1-sol,gpt-6-luna")
    ap.add_argument("--limit", type=int, default=None)
    ap.add_argument("--jobs", type=int, default=4)
    a = ap.parse_args()
    run(a.models.split(","), a.limit, a.jobs) if a.cmd == "run" else report()
