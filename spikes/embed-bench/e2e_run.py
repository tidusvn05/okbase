#!/usr/bin/env python3
"""Thí nghiệm end-to-end: agent CLI thật (claude) + MCP kb, so sánh 3 cách đưa knowledge.

  A = chỉ tool            (prompt không có knowledge)
  B = catalog + tool      (index.md trong prompt)
  C = pre-retrieval + catalog + tool (top-5 tài liệu + index.md)
  D = như C nhưng catalog nằm trong system prompt (phần ổn định, được cache giữa các lượt)
  E = "truyền thống": agent + thư mục bundle OKF, chỉ dùng tool đọc file có sẵn (Read/Grep/Glob), không qobot
  F = "truyền thống, không tool": nhồi toàn bộ bundle vào system prompt

Usage: python3 e2e_run.py [--configs A,B,C] [--limit N] [--jobs 3] [--model sonnet]
Kết quả: e2e/runs/<config>/<qid>.json, tổng hợp bằng e2e_report.py
"""
import argparse, json, os, subprocess, time, concurrent.futures as cf
from pathlib import Path

ROOT = Path(__file__).resolve().parent
E2E = ROOT / "e2e"
BIN = ROOT / "target/release/embed-bench"
DOCS = {d["id"]: d for d in json.loads((ROOT / "data/v2/docs.json").read_text())}
CATALOG = (E2E / "bundle/index.md").read_text()
TOOLS = ["mcp__kb__kb_search", "mcp__kb__kb_grep", "mcp__kb__kb_get", "mcp__kb__kb_list"]

SYSTEM = """You are the customer & employee support assistant of Hikari Home, a home-appliance company operating in Vietnam and Japan.
Rules:
- Answer ONLY from the Hikari knowledge base. Never guess policies, numbers or procedures.
- Reply in the same language as the user's question, concisely (max 3 sentences).
- Knowledge may already be provided in the prompt. If it is missing or does not clearly match the exact detail asked (which appliance, which country, which channel, which leave type...), use the kb tools:
  kb_search (semantic, any language), kb_grep (exact string such as an error code), kb_get (read a full document), kb_list (catalog).
- Put the ids of the documents your answer is based on in `sources` (most relevant first)."""

SYSTEM_E = SYSTEM.split("- Knowledge may already")[0] + """- The knowledge base is an OKF bundle in the current directory: `index.md` is the catalog (id, lang, title, description) and each document is `knowledge/<id>.md` (Markdown with YAML frontmatter). Documents are in Vietnamese, English and Japanese.
- Use your file tools (Glob, Grep, Read) to find and read the relevant documents before answering.
- Put the ids of the documents your answer is based on in `sources` (the file name without .md, most relevant first)."""

SYSTEM_F = SYSTEM.split("- Knowledge may already")[0] + """- The complete knowledge base (all documents) is included below. Put the ids of the documents your answer is based on in `sources` (most relevant first).

""" + "\n\n".join(f'<knowledge source="{i}" lang="{d["lang"]}">\n# {d["title"]}\n{d["text"]}\n</knowledge>' for i, d in DOCS.items())

SCHEMA = json.dumps({
    "type": "object",
    "properties": {"answer": {"type": "string"}, "sources": {"type": "array", "items": {"type": "string"}}},
    "required": ["answer", "sources"],
})


def doc_block(i):
    d = DOCS[i]
    return f'<knowledge source="{i}" lang="{d["lang"]}">\n# {d["title"]}\n{d["text"]}\n</knowledge>'


def build_prompt(cfg, item):
    parts = []
    if cfg in ("E", "F"):
        return f"User question:\n{item['q']}"
    if cfg in ("B", "C"):
        parts.append(CATALOG)
    if cfg in ("C", "D"):
        parts.append("Retrieved knowledge (top-5 by semantic search, may contain near-duplicates):\n\n"
                     + "\n\n".join(doc_block(i) for i in item["top5"]))
    parts.append(f"User question:\n{item['q']}")
    return "\n\n---\n\n".join(parts)


def run_one(cfg, item, model):
    out_dir = E2E / "runs" / cfg
    out_dir.mkdir(parents=True, exist_ok=True)
    out = out_dir / f"{item['qid']}.json"
    if out.exists():
        return json.loads(out.read_text())
    log = out_dir / f"{item['qid']}.tools.jsonl"
    log.unlink(missing_ok=True)
    mcp = out_dir / f"{item['qid']}.mcp.json"
    mcp.write_text(json.dumps({"mcpServers": {"kb": {"command": str(BIN), "args": ["mcp"],
                                                   "env": {"DATA": "v2", "KB_LOG": str(log)}}}}))
    work = E2E / "work"
    work.mkdir(exist_ok=True)
    base = ["claude", "-p", "--no-session-persistence", "--setting-sources", "local", "--strict-mcp-config",
            "--model", model, "--json-schema", SCHEMA]
    if cfg == "E":
        cmd = base + ["--output-format", "stream-json", "--verbose", "--tools", "Read,Grep,Glob",
                      "--allowedTools", "Read,Grep,Glob", "--append-system-prompt", SYSTEM_E]
        cwd = E2E / "bundle"
    elif cfg == "F":
        cmd = base + ["--output-format", "stream-json", "--verbose", "--tools", "", "--append-system-prompt", SYSTEM_F]
        cwd = work
    else:
        cmd = base + ["--output-format", "json", "--mcp-config", str(mcp), "--tools", "",
                      "--allowedTools", ",".join(TOOLS),
                      "--append-system-prompt", SYSTEM + ("\n\n" + CATALOG if cfg == "D" else "")]
        cwd = work
    t = time.time()
    p = subprocess.run(cmd, input=build_prompt(cfg, item), capture_output=True, text=True, cwd=cwd, timeout=600)
    wall = time.time() - t
    native_tools = []
    try:
        if "stream-json" in cmd:
            env = {}
            for line in p.stdout.splitlines():
                ev = json.loads(line)
                if ev.get("type") == "assistant":
                    for c in ev["message"].get("content", []):
                        if c.get("type") == "tool_use" and c.get("name") != "StructuredOutput":
                            native_tools.append({"tool": c["name"], "args": c.get("input")})
                elif ev.get("type") == "result":
                    env = ev
        else:
            env = json.loads(p.stdout)
    except json.JSONDecodeError:
        env = {"is_error": True, "raw": p.stdout[-2000:], "stderr": p.stderr[-2000:]}
    so = env.get("structured_output") or {}
    if not so and isinstance(env.get("result"), str):
        try:
            so = json.loads(env["result"])
        except Exception:
            so = {"answer": env.get("result", ""), "sources": []}
    so["sources"] = [Path(str(x)).name.removesuffix(".md") for x in so.get("sources", [])]
    tools = [json.loads(l) for l in log.read_text().splitlines()] if log.exists() else native_tools
    rec = {
        "cfg": cfg, **item, "answer": so.get("answer", ""), "sources": so.get("sources", []),
        "wall_s": round(wall, 2), "duration_ms": env.get("duration_ms"), "num_turns": env.get("num_turns"),
        "cost_usd": env.get("total_cost_usd"), "usage": env.get("usage"), "is_error": env.get("is_error"),
        "tool_calls": tools, "prompt_chars": len(build_prompt(cfg, item)),
    }
    out.write_text(json.dumps(rec, ensure_ascii=False, indent=1))
    return rec


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--configs", default="A,B,C")
    ap.add_argument("--limit", type=int, default=0)
    ap.add_argument("--jobs", type=int, default=3)
    ap.add_argument("--model", default="sonnet")
    a = ap.parse_args()
    items = json.loads((E2E / "queries.json").read_text())
    if a.limit:
        items = items[: a.limit]
    jobs = [(c, it) for it in items for c in a.configs.split(",")]
    done = 0
    with cf.ThreadPoolExecutor(a.jobs) as ex:
        futs = {ex.submit(run_one, c, it, a.model): (c, it) for c, it in jobs}
        for f in cf.as_completed(futs):
            c, it = futs[f]
            done += 1
            try:
                r = f.result()
                ok = it["gold"] in (r["sources"] or [])[:1]
                print(f"[{done}/{len(jobs)}] {c} q{it['qid']} {'OK ' if ok else 'MISS'} "
                      f"{r['wall_s']}s turns={r['num_turns']} tools={len(r['tool_calls'])} src={r['sources'][:3]}", flush=True)
            except Exception as e:
                print(f"[{done}/{len(jobs)}] {c} q{it['qid']} ERROR {e}", flush=True)


if __name__ == "__main__":
    main()
