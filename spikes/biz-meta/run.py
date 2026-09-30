#!/usr/bin/env python3
"""Spike metadata/tag: so sánh 4 cách trên bundle business có metadata phong phú + sheet.

  E   — agent + thư mục (Read/Grep/Glob), bundle tổ chức theo thư mục + index.md
  V   — như E, bundle có thêm _views/ (danh sách theo tag/type/status/department/lang/customer, sinh tự động)
  Q   — như E + MCP kb_query (lọc metadata, facet, sum)
  QD  — như Q + MCP data_tables/data_query (SQL chỉ đọc trên sheet)

Usage: python3 run.py [--configs E,V,Q,QD] [--limit N] [--jobs 4]
"""
import argparse, json, subprocess, sys, time, concurrent.futures as cf
from pathlib import Path

ROOT = Path(__file__).resolve().parent
SUF = ""
SCHEMA = json.dumps({"type": "object", "properties": {
    "answer": {"type": "string"},
    "items": {"type": "array", "items": {"type": "string"}, "description": "for list/which questions: document ids (or entity names) that answer; otherwise empty"},
    "sources": {"type": "array", "items": {"type": "string"}}}, "required": ["answer", "items", "sources"]})

BASE = """You are the internal business assistant of Hikari Home (home appliances, Vietnam and Japan). Today is 2026-09-30.
Rules:
- Answer ONLY from the company knowledge bundle. Reply in the user's language, concisely.
- When the question asks to list or identify documents, put their document ids (path without .md, e.g. "sop/it/backup") in `items` and mention them in the answer. For "which departments/customers/categories" questions, put those names in `items`.
- Put ids of documents you used in `sources`.
Knowledge bundle (current directory, OKF format):
- Every directory has index.md (titles + one-line descriptions). Top level: policies/ (vn, jp; versioned v1/v2/v3), sop/<department>/, contracts/<customer>/, meetings/<department>/, products/<category>/, faq/, announcements/.
- Each document has YAML frontmatter: type, title, description, tags, status (stable = current/in force, deprecated = old/superseded, draft = not approved yet), lang, department, updated, and when relevant region, customer, version, supersedes, effective_from, effective_to, contract_value, currency, model, meeting_date.
- Spreadsheets imported from Google Sheets: data/inventory.csv, data/sales_2026.csv, data/price_list.csv (also as markdown tables in sources/sheets/).
"""
HINT = {
    "E": "",
    "V": "- _views/ contains auto-generated metadata tables: _views/tags/<tag>.md, _views/types/<type>.md, _views/status/<status>.md, _views/departments/<dept>.md, _views/lang/<lang>.md, _views/customers/<customer>.md, _views/contracts-all.md (effective dates and values), _views/recent.md. Start from _views/index.md for metadata questions.\n",
    "Q": "- Tool kb_query filters documents by frontmatter metadata (type, tags, status, lang, department, region, customer, dates, active_on) and returns counts, facets and sums. Prefer it for list/count/which questions, then read documents with Read for details.\n",
    "QD": "- Tool kb_query filters documents by frontmatter metadata (type, tags, status, lang, department, region, customer, dates, active_on) and returns counts, facets and sums. Prefer it for list/count/which questions, then read documents with Read for details.\n"
          "- Tools data_tables / data_query run read-only SQL over the spreadsheets (tables inventory, sales_2026, price_list). Use SQL aggregates for any numeric question about inventory, sales or prices.\n",
}


def run_one(cfg, q, model):
    d = ROOT / "runs" / f"{cfg}{SUF}"
    d.mkdir(parents=True, exist_ok=True)
    out = d / f"{q['qid']}.json"
    if out.exists():
        return json.loads(out.read_text())
    bundle = ROOT / (("bundle-V" if cfg == "V" else "bundle-E") + SUF)
    sysf = d / f"{q['qid']}.system.txt"
    sysf.write_text(BASE + HINT[cfg])
    tools = ["Read", "Grep", "Glob"]
    cmd = ["claude", "-p", "--output-format", "stream-json", "--verbose", "--no-session-persistence", "--setting-sources", "local",
           "--strict-mcp-config", "--model", model, "--json-schema", SCHEMA, "--append-system-prompt-file", str(sysf), "--tools", ",".join(tools)]
    log = d / f"{q['qid']}.tools.jsonl"
    log.unlink(missing_ok=True)
    if cfg in ("Q", "QD"):
        mcp = d / f"{q['qid']}.mcp.json"
        env = {"KB_LOG": str(log), "SUF": SUF}
        if cfg == "QD":
            env["DATA"] = "1"
        mcp.write_text(json.dumps({"mcpServers": {"kb": {"command": sys.executable, "args": [str(ROOT / "mcp_meta.py")], "env": env}}}))
        mt = ["mcp__kb__kb_query"] + (["mcp__kb__data_tables", "mcp__kb__data_query"] if cfg == "QD" else [])
        cmd += ["--mcp-config", str(mcp), "--allowedTools", ",".join(tools + mt)]
    else:
        cmd += ["--allowedTools", ",".join(tools)]
    t = time.time()
    p = subprocess.run(cmd, input=f"User question:\n{q['q']}", capture_output=True, text=True, cwd=bundle, timeout=900)
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
    norm = lambda x: str(x).strip().removeprefix("./").removesuffix(".md").lstrip("/")
    rec = {"cfg": cfg, "scale": SUF or "x1", **q, "answer": so.get("answer", ""), "items": [norm(x) for x in so.get("items", [])],
           "sources": [norm(x) for x in so.get("sources", [])], "wall_s": round(wall, 2), "num_turns": env.get("num_turns"),
           "cost_usd": env.get("total_cost_usd"), "usage": env.get("usage"), "is_error": env.get("is_error", True) if env else True,
           "tool_calls": calls, "stderr": "" if env else p.stderr[-500:]}
    out.write_text(json.dumps(rec, ensure_ascii=False, indent=1))
    return rec


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--configs", default="E,V,Q,QD")
    ap.add_argument("--limit", type=int, default=0)
    ap.add_argument("--jobs", type=int, default=4)
    ap.add_argument("--model", default="sonnet")
    ap.add_argument("--suffix", default="", help="'' hoặc '-x20'")
    a = ap.parse_args()
    global SUF
    SUF = a.suffix
    qs = json.loads((ROOT / f"questions{SUF}.json").read_text())[: a.limit or None]
    jobs = [(c, q) for q in qs for c in a.configs.split(",")]
    cost = 0.0
    with cf.ThreadPoolExecutor(a.jobs) as ex:
        futs = {ex.submit(run_one, c, q, a.model): (c, q) for c, q in jobs}
        for n, f in enumerate(cf.as_completed(futs), 1):
            c, q = futs[f]
            try:
                r = f.result()
                cost += r["cost_usd"] or 0
                print(f"[{n}/{len(jobs)}] {c} q{q['qid']} {q['cat']} {'ERR ' if r['is_error'] else ''}{r['wall_s']}s turns={r['num_turns']} "
                      f"tools={len(r['tool_calls'])} items={r['items'][:3]} (Σ ${cost:.2f})", flush=True)
            except Exception as e:
                print(f"[{n}/{len(jobs)}] {c} q{q['qid']} EXC {e}", flush=True)


if __name__ == "__main__":
    main()
