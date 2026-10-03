"""Builds the lexical eval suites from the tool calls agents made in the spikes.

For each recorded run, the discovery calls (okbase `kb_grep`/`kb_query`/`kb_list`/`data_query`,
and Claude Code `Grep`, mapped to `kb_grep`) become one candidate case. Reads (`kb_get`, `Read`)
are left out: they only fetch what the agent had already found. Semantic `kb_search` needs a
model and is left to the retrieval eval. Every candidate is run with `okbase-eval`; per question
the first passing candidate is kept (else the first one, recorded as `fail`).

Usage (from the repository root): python3 fixtures/eval/mine.py
Sources: spikes/biz-meta (S5, ×1 runs: configs E, Q, QD) on fixtures/business, and
spikes/embed-bench round 4 (S3, configs A-F) on fixtures/multilingual.
"""
import gzip
import json
import re
import subprocess
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / "fixtures/eval"
EVAL = ["cargo", "run", "-q", "--release", "-p", "okbase-eval", "--", "lexical", "--json"]


def rel_path(path, markers):
    """A recorded absolute path made relative to the bundle (None for the bundle root)."""
    if not path:
        return None
    for m in markers:
        if m in path:
            path = path.split(m, 1)[1]
            break
    path = path.strip("/")
    if path.startswith("/"):
        return None
    return path or None


def to_okbase(tc, markers):
    """A recorded call as an okbase MCP call, or None when it is not a discovery call."""
    tool, args = tc["tool"], tc.get("args", {})
    if tool in ("kb_grep", "kb_query", "kb_list", "data_query"):
        return {"tool": tool, "args": args}
    if tool == "Grep":
        a = {"pattern": args["pattern"]}
        p = rel_path(args.get("path"), markers)
        if args.get("glob") and args["glob"] not in ("*.md", "**/*.md"):
            a["path"] = args["glob"]
        elif p:
            a["path"] = p if p.endswith(".md") else p.rstrip("/") + "/"
        if args.get("output_mode") == "files_with_matches":
            a["files_only"] = True
        ctx = args.get("-C") or args.get("-A") or args.get("-B")
        if ctx:
            a["context"] = min(int(ctx), 5)
        if args.get("head_limit"):
            a["limit"] = min(int(args["head_limit"]), 200)
        return {"tool": "kb_grep", "args": a}
    return None


def business():
    qs = {q["qid"]: q for q in json.load(open(ROOT / "spikes/biz-meta/questions.json"))}
    runs = [json.loads(line) for line in gzip.open(ROOT / "spikes/biz-meta/results/runs.jsonl.gz")]
    runs = [r for r in runs if r.get("scale") in (None, "") and r["cfg"] in ("QD", "Q", "E")]
    order = {"QD": 0, "Q": 1, "E": 2}
    runs.sort(key=lambda r: (r["qid"], order[r["cfg"]]))
    cands = {}
    for r in runs:
        q = qs[r["qid"]]
        calls = [c for tc in r["tool_calls"] if (c := to_okbase(tc, ["/bundle-E/", "/bundle-E-x20/"]))]
        if not calls:
            continue
        exp, cat = q["expected"], q["cat"]
        if cat == "list":
            expect = {"ids": exp}
        elif cat == "version":
            expect = {"ids": [exp["current_doc"]]}
        elif cat == "content":
            expect = {"ids": [exp["doc"]]}
        elif cat == "facet" and isinstance(exp, dict):
            expect = {"values": exp["customers"] + [exp["count"]]}
        elif isinstance(exp, dict) and "value" in exp:
            expect = {"values": [exp["value"]]}
        else:
            expect = {"values": exp if isinstance(exp, list) else [exp]}
        cands.setdefault(q["qid"], []).append({
            "id": f"q{q['qid']}-{r['cfg']}", "cat": cat, "lang": q["lang"], "q": q["q"],
            "calls": calls, "expect": expect, "baseline": "fail",
        })
    return pick("business", "business",
                "spikes/biz-meta (S5) x1 runs, configs QD, Q, E: Claude Sonnet's discovery calls "
                "(Grep mapped to kb_grep) and the computed answers", cands)


def multilingual():
    runs = [json.loads(line) for line in gzip.open(ROOT / "spikes/embed-bench/results/e2e/runs.jsonl.gz")]
    runs.sort(key=lambda r: (r["qid"], r["cfg"]))
    cands = {}
    for r in runs:
        calls = [c for tc in r["tool_calls"] if (c := to_okbase(tc, ["/e2e/bundle/", "/e2e/bundle"]))]
        if not calls:
            continue
        # The spike bundle was flat; the fixture keeps documents under knowledge/.
        for c in calls:
            p = c["args"].get("path")
            if p and p != "index.md":
                c["args"]["path"] = "knowledge/" + p
        cands.setdefault(r["qid"], []).append({
            "id": f"q{r['qid']}-{r['cfg']}", "cat": "lookup", "lang": r["lang"], "q": r["q"],
            "calls": calls, "expect": {"ids": [f"knowledge/{r['gold']}"]}, "baseline": "fail",
        })
    return pick("multilingual", "multilingual",
                "spikes/embed-bench round 4 (S3), configs A-F: Claude Sonnet's grep calls "
                "(kb_grep, or Grep mapped to it) and the gold document", cands)


def pick(name, bundle, about, cands):
    flat = [c for cs in cands.values() for c in cs]
    with tempfile.NamedTemporaryFile("w", suffix=".json", delete=False) as f:
        json.dump({"suite": name, "bundle": bundle, "about": about, "cases": flat}, f, ensure_ascii=False)
    report = json.loads(subprocess.run(EVAL + [f.name], cwd=ROOT, check=True, capture_output=True, text=True).stdout)[0]
    passed = {c["id"] for c in report["cases"] if c["outcome"] == "pass"}
    cases = []
    for qid in sorted(cands):
        cs = cands[qid]
        best = next((c for c in cs if c["id"] in passed), cs[0])
        best = dict(best, id=f"q{qid}", baseline="pass" if best["id"] in passed else "fail")
        cases.append(best)
    suite = {"suite": name, "bundle": bundle, "about": about, "cases": cases}
    (OUT / f"{name}.json").write_text(json.dumps(suite, ensure_ascii=False, indent=1) + "\n")
    n = sum(c["baseline"] == "pass" for c in cases)
    print(f"{name}: {n}/{len(cases)} pass ({len(flat)} candidates)")


business()
multilingual()
