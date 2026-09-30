#!/usr/bin/env python3
"""Spike quy mô: so sánh các cách cho agent CLI dùng bundle OKF lớn (docs OpenClaw).

Cách (config):
  F  full      — toàn bộ bundle trong system prompt, không tool
  D  qobot     — top-6 chunk (EmbeddingGemma) + catalog trong system prompt + MCP (search/get/grep/list)
  A  tools     — chỉ MCP (search/get/grep/list), không có gì trong prompt
  E  folder    — agent + thư mục bundle, dùng Read/Grep/Glob có sẵn của CLI
  H  rag       — RAG thuần: top-6 chunk trong prompt, không tool
  G  tree      — không embedding: index.md gốc trong system prompt + MCP get/grep/list (duyệt cây OKF)
  I  qobot-nocat — như D nhưng không có catalog (top-6 + MCP)
  G2 lexical v2 — như G, kb_grep nâng cấp (regex, bỏ dấu, path, context, files_only) + hướng dẫn chiến lược tìm
  D2 qobot v2   — như D + quy tắc kiểm tra (chi tiết cụ thể phải có trong văn bản đã đọc; đủ mọi ý)

Usage: python3 run.py --sizes S,M --configs F,D,A,E,H,G [--jobs 4] [--limit N]
"""
import argparse, json, subprocess, time, concurrent.futures as cf
from pathlib import Path

ROOT = Path(__file__).resolve().parent
BUNDLES = ROOT / "bundles"
BIN = ROOT.parent / "embed-bench/target/release/embed-bench"
QS = {q["qid"]: q for q in json.loads((ROOT / "questions.json").read_text())}

BASE = """You are the support assistant for OpenClaw (an open-source personal AI agent gateway). Rules:
- Answer ONLY from the OpenClaw documentation knowledge base. Never guess config keys, default values, limits or commands.
- Reply in the same language as the user's question, concisely (max 4 sentences). Keep config keys, commands and values verbatim.
- Put the ids of the documents your answer is based on in `sources` (document id = path without .md, most relevant first).
"""
VERIFY = """- Verification rule: config keys, default values, limits and commands must be literally supported by text you have read. Retrieved sections come from semantic search and may belong to a similar-but-different feature. If they don't state the exact detail, or the question asks several things and some are not covered, use kb_grep / kb_get / kb_search to find the rest before answering. Related details are often spread across a guide, a reference page and a FAQ.
"""
LEXICAL = """- Knowledge tools: kb_list (directory index.md), kb_grep (regex, case/accent-insensitive; `files_only` to rank documents; `path` to narrow; `context` for surrounding lines), kb_get (read a document or one `section`).
- Strategy: the docs are English — translate the question's key terms to English first; kb_grep with alternation of synonyms/likely config key spellings (e.g. 'dmScope|dm scope|direct message') and files_only to find candidate documents; read the best ones with kb_get (use `section` for long docs); use the index to understand structure.
- Answers may need details from several documents; if the question asks several things, make sure each is covered.
"""
TOOLS_HINT = """- Knowledge tools: kb_search (semantic search, any language → English docs), kb_grep (exact string: config key, flag, error text), kb_get (read a document or one `section`), kb_list (directory index.md).
"""
SCHEMA = json.dumps({"type": "object", "properties": {"answer": {"type": "string"}, "sources": {"type": "array", "items": {"type": "string"}}}, "required": ["answer", "sources"]})
MCP_TOOLS = ["mcp__kb__kb_search", "mcp__kb__kb_get", "mcp__kb__kb_grep", "mcp__kb__kb_list"]


def full_bundle(size):
    parts = []
    for p in sorted((BUNDLES / size).rglob("*.md")):
        if p.name == "index.md" or ".qobot" in p.parts:
            continue
        doc_id = p.relative_to(BUNDLES / size).with_suffix("").as_posix()
        parts.append(f'<document id="{doc_id}">\n{p.read_text()}\n</document>')
    return "\n\n".join(parts)


def retrieved(size, qid):
    r = json.loads((BUNDLES / f"{size}.retrieval.json").read_text())
    ch = next(x for x in r if x["qid"] == qid)["chunks"]
    return "Retrieved sections (semantic search, top-6; may be partially relevant):\n\n" + "\n\n".join(
        f'<knowledge source="{c["doc"]}" section="{c["heading"]}">\n{c["text"]}\n</knowledge>' for c in ch)


def catalog(size):
    if size == "XL":  # catalog phẳng quá lớn (~44k token) → chỉ index.md gốc
        return "Knowledge bundle root index (use kb_list to browse directories):\n" + (BUNDLES / "XL/index.md").read_text()
    return (BUNDLES / f"{size}.catalog.md").read_text()


def setup(cfg, size, q, run_dir):
    """Trả về (system_prompt, user_prompt, extra_cli_args, cwd)."""
    qtext = f"User question:\n{q['q']}"
    mcp = run_dir / f"{q['qid']}.mcp.json"
    log = run_dir / f"{q['qid']}.tools.jsonl"
    env = {"KB_LOG": str(log)}
    if cfg in ("G", "G2"):
        env["NOSEARCH"] = "1"
    mcp.write_text(json.dumps({"mcpServers": {"kb": {"command": str(BIN), "args": ["bundle-mcp", str(BUNDLES / size)], "env": env}}}))
    mcp_args = ["--mcp-config", str(mcp), "--tools", "", "--allowedTools", ",".join(MCP_TOOLS)]
    work = ROOT / "work"
    if cfg == "F":
        return BASE + "\nThe complete documentation is included below.\n\n" + full_bundle(size), qtext, ["--tools", ""], work
    if cfg == "D":
        return BASE + TOOLS_HINT + "- Relevant sections may already be in the prompt; if they don't clearly answer the exact detail, use the tools.\n\n" + catalog(size), retrieved(size, q["qid"]) + "\n\n---\n\n" + qtext, mcp_args, work
    if cfg == "A":
        return BASE + TOOLS_HINT, qtext, mcp_args, work
    if cfg == "D2":
        return BASE + TOOLS_HINT + VERIFY + "\n" + catalog(size), retrieved(size, q["qid"]) + "\n\n---\n\n" + qtext, mcp_args, work
    if cfg == "G2":
        return BASE + LEXICAL + "\n" + (BUNDLES / size / "index.md").read_text(), qtext, mcp_args, work
    if cfg == "I":
        return BASE + TOOLS_HINT + "- Relevant sections may already be in the prompt; if they don't clearly answer the exact detail, use the tools.\n", retrieved(size, q["qid"]) + "\n\n---\n\n" + qtext, mcp_args, work
    if cfg == "E":
        return (BASE + "- The documentation is an OKF bundle in the current directory: every directory has an index.md (catalog with one-line descriptions); documents are Markdown files with YAML frontmatter. Use Glob/Grep/Read to find and read the relevant documents before answering.\n",
                qtext, ["--tools", "Read,Grep,Glob", "--allowedTools", "Read,Grep,Glob"], BUNDLES / size)
    if cfg == "H":
        return BASE + "- Answer only from the retrieved sections in the prompt. If they do not contain the answer, say you don't know.\n", retrieved(size, q["qid"]) + "\n\n---\n\n" + qtext, ["--tools", ""], work
    if cfg == "G":
        return (BASE + "- Knowledge tools: kb_list (directory index.md), kb_get (read a document or one `section`), kb_grep (exact string search). Browse the index to find documents.\n\n"
                + (BUNDLES / size / "index.md").read_text(), qtext, mcp_args, work)
    raise ValueError(cfg)


def run_one(size, cfg, q, model):
    run_dir = ROOT / "runs" / size / cfg
    run_dir.mkdir(parents=True, exist_ok=True)
    out = run_dir / f"{q['qid']}.json"
    if out.exists():
        return json.loads(out.read_text())
    system, user, extra, cwd = setup(cfg, size, q, run_dir)
    sp = run_dir / f"{q['qid']}.system.txt"
    sp.write_text(system)
    (ROOT / "work").mkdir(exist_ok=True)
    cmd = ["claude", "-p", "--output-format", "stream-json", "--verbose", "--no-session-persistence",
           "--setting-sources", "local", "--strict-mcp-config", "--model", model, "--json-schema", SCHEMA,
           "--append-system-prompt-file", str(sp)] + extra
    log = run_dir / f"{q['qid']}.tools.jsonl"
    log.unlink(missing_ok=True)
    t = time.time()
    p = subprocess.run(cmd, input=user, capture_output=True, text=True, cwd=cwd, timeout=900)
    wall = time.time() - t
    env, native = {}, []
    for line in p.stdout.splitlines():
        try:
            ev = json.loads(line)
        except json.JSONDecodeError:
            continue
        if ev.get("type") == "assistant":
            for c in ev["message"].get("content", []):
                if c.get("type") == "tool_use" and c.get("name") != "StructuredOutput":
                    native.append({"tool": c["name"].removeprefix("mcp__kb__"), "args": c.get("input")})
        elif ev.get("type") == "result":
            env = ev
    so = env.get("structured_output") or {}
    if not so and isinstance(env.get("result"), str):
        try:
            so = json.loads(env["result"])
        except Exception:
            so = {"answer": env.get("result", ""), "sources": []}
    rec = {"size": size, "cfg": cfg, **q, "answer": so.get("answer", ""),
           "sources": [Path(str(x)).as_posix().removesuffix(".md").lstrip("./") for x in so.get("sources", [])],
           "wall_s": round(wall, 2), "num_turns": env.get("num_turns"), "cost_usd": env.get("total_cost_usd"),
           "usage": env.get("usage"), "is_error": env.get("is_error", True) if env else True,
           "tool_calls": native, "system_chars": len(system), "stderr": p.stderr[-500:] if not env else ""}
    out.write_text(json.dumps(rec, ensure_ascii=False, indent=1))
    return rec


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--sizes", default="S")
    ap.add_argument("--configs", default="F,D,A,E,H,G")
    ap.add_argument("--jobs", type=int, default=4)
    ap.add_argument("--limit", type=int, default=0)
    ap.add_argument("--model", default="sonnet")
    a = ap.parse_args()
    qs = list(QS.values())[: a.limit or None]
    jobs = [(s, c, q) for s in a.sizes.split(",") for q in qs for c in a.configs.split(",")]
    cost, done = 0.0, 0
    with cf.ThreadPoolExecutor(a.jobs) as ex:
        futs = {ex.submit(run_one, s, c, q, a.model): (s, c, q) for s, c, q in jobs}
        for f in cf.as_completed(futs):
            s, c, q = futs[f]
            done += 1
            try:
                r = f.result()
                cost += r["cost_usd"] or 0
                print(f"[{done}/{len(jobs)}] {s}/{c} q{q['qid']} {'ERR ' if r['is_error'] else ''}{r['wall_s']}s turns={r['num_turns']} "
                      f"tools={len(r['tool_calls'])} src={r['sources'][:2]} ${r['cost_usd'] or 0:.3f} (Σ ${cost:.2f})", flush=True)
            except Exception as e:
                print(f"[{done}/{len(jobs)}] {s}/{c} q{q['qid']} EXC {e}", flush=True)


if __name__ == "__main__":
    main()
