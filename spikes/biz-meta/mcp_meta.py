#!/usr/bin/env python3
"""MCP server (stdio, JSON-RPC) cho spike metadata:
  kb_query   — lọc tài liệu theo frontmatter (type, tags, status, lang, department, region, customer, ngày, hiệu lực...) + facets
  data_tables / data_query — SQL chỉ đọc trên các sheet (SQLite in-memory từ data/*.csv)   [bật bằng env DATA=1]
Metadata đọc từ metadata.json (bản production: bảng docs trong index.sqlite).
"""
import csv, json, os, sqlite3, sys
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parent
SUF = os.environ.get("SUF", "")
META = json.loads((ROOT / f"metadata{SUF}.json").read_text())
LOG = os.environ.get("KB_LOG")
WITH_DATA = os.environ.get("DATA") == "1"

db = None
if WITH_DATA:
    db = sqlite3.connect(":memory:")
    for f in sorted((ROOT / f"data{SUF}").glob("*.csv")):
        rows = list(csv.DictReader(open(f)))
        cols = list(rows[0])
        num = {c: all(r[c].lstrip("-").isdigit() for r in rows) for c in cols}
        db.execute(f"CREATE TABLE {f.stem} ({', '.join(f'{c} {'INTEGER' if num[c] else 'TEXT'}' for c in cols)})")
        db.executemany(f"INSERT INTO {f.stem} VALUES ({','.join('?' * len(cols))})", [[int(r[c]) if num[c] else r[c] for c in cols] for r in rows])
    db.execute("PRAGMA query_only = ON")

FIELDS = ["type", "status", "lang", "department", "region", "customer", "tags", "currency", "version"]
KB_QUERY = {"name": "kb_query", "description": (
    "Filter documents by frontmatter metadata and get counts/facets. All filters are optional and combined with AND. "
    "status: stable=current/in force, deprecated=old/superseded, draft=not yet approved. "
    "`active_on` (YYYY-MM-DD) keeps documents whose effective_from <= date <= effective_to (contracts) or that are stable and effective_from <= date (policies). "
    "Returns: total count, a table of matching documents (id, title, type, status, lang, department, updated, tags, and region/customer/effective/value when present), and optional facet counts."),
    "inputSchema": {"type": "object", "properties": {
        "type": {"type": "array", "items": {"type": "string"}, "description": "Policy, SOP, Contract, Meeting Note, Product Spec, FAQ, Announcement, Dataset"},
        "tags_all": {"type": "array", "items": {"type": "string"}}, "tags_any": {"type": "array", "items": {"type": "string"}},
        "status": {"type": "array", "items": {"type": "string"}}, "status_not": {"type": "array", "items": {"type": "string"}},
        "lang": {"type": "array", "items": {"type": "string"}}, "department": {"type": "array", "items": {"type": "string"}},
        "region": {"type": "array", "items": {"type": "string"}}, "customer": {"type": "array", "items": {"type": "string"}},
        "path": {"type": "string", "description": "id prefix, e.g. 'policies/jp/'"},
        "text": {"type": "string", "description": "case-insensitive substring in title/description/id"},
        "updated_from": {"type": "string"}, "updated_to": {"type": "string"},
        "effective_to_from": {"type": "string"}, "effective_to_to": {"type": "string"},
        "active_on": {"type": "string"},
        "sort": {"type": "string", "description": "field name, prefix '-' for descending, e.g. '-updated', 'effective_to'"},
        "limit": {"type": "integer", "default": 50}, "count_only": {"type": "boolean", "default": False},
        "facets": {"type": "array", "items": {"type": "string"}, "description": f"fields to count over the filtered set: {FIELDS}"},
        "sum_field": {"type": "string", "description": "numeric field to sum over the filtered set, e.g. contract_value"}}}}
DATA_TABLES = {"name": "data_tables", "description": "List SQL tables (imported Google Sheets) with columns and row counts.", "inputSchema": {"type": "object", "properties": {}}}
DATA_QUERY = {"name": "data_query", "description": "Run a read-only SQLite SELECT over the sheet tables (inventory, sales_2026, price_list). Use aggregates (SUM, COUNT, GROUP BY) instead of reading rows. Max 100 rows returned.",
              "inputSchema": {"type": "object", "properties": {"sql": {"type": "string"}}, "required": ["sql"]}}


def lower(x):
    return [str(v).lower() for v in (x or [])]


def match(d, a):
    tags = [t.lower() for t in d.get("tags", [])]
    if a.get("type") and d["kind"].lower() not in lower(a["type"]):
        return False
    if a.get("tags_all") and not all(t in tags for t in lower(a["tags_all"])):
        return False
    if a.get("tags_any") and not any(t in tags for t in lower(a["tags_any"])):
        return False
    if a.get("status") and d["status"] not in lower(a["status"]):
        return False
    if a.get("status_not") and d["status"] in lower(a["status_not"]):
        return False
    for f in ("lang", "department", "region", "customer"):
        if a.get(f) and str(d.get(f, "")).lower() not in lower(a[f]):
            return False
    if a.get("path") and not d["id"].startswith(a["path"].strip("/")):
        return False
    if a.get("text") and a["text"].lower() not in f"{d['id']} {d['title']} {d['description']}".lower():
        return False
    if a.get("updated_from") and d["updated"] < a["updated_from"]:
        return False
    if a.get("updated_to") and d["updated"] > a["updated_to"]:
        return False
    ef, et = d.get("effective_from"), d.get("effective_to")
    if a.get("effective_to_from") and (not et or et < a["effective_to_from"]):
        return False
    if a.get("effective_to_to") and (not et or et > a["effective_to_to"]):
        return False
    if a.get("active_on"):
        day = a["active_on"]
        if d["kind"] == "Contract":
            return bool(ef and et and ef <= day <= et)
        return d["status"] == "stable" and (not ef or ef <= day)
    return True


def kb_query(a):
    res = [d for d in META if match(d, a)]
    sort = a.get("sort") or "id"
    key = sort.lstrip("-").replace("type", "kind")
    res.sort(key=lambda d: str(d.get(key, "")), reverse=sort.startswith("-"))
    out = [f"total: {len(res)} documents"]
    if a.get("sum_field"):
        vals = [d.get(a["sum_field"]) for d in res if isinstance(d.get(a["sum_field"]), (int, float))]
        cur = Counter(d.get("currency") for d in res if d.get(a["sum_field"]) is not None)
        out.append(f"sum({a['sum_field']}) = {sum(vals)} over {len(vals)} docs (currencies: {dict(cur)})")
    for f in a.get("facets") or []:
        c = Counter()
        for d in res:
            v = d.get("kind" if f == "type" else f)
            for x in (v if isinstance(v, list) else [v]):
                if x is not None:
                    c[x] += 1
        out.append(f"facet {f}: " + ", ".join(f"{k}={n}" for k, n in c.most_common()))
    if not a.get("count_only"):
        out.append("| id | title | type | status | lang | department | updated | tags | extra |")
        for d in res[: int(a.get("limit") or 50)]:
            extra = " ".join(f"{k}={d[k]}" for k in ("region", "customer", "version", "effective_from", "effective_to", "contract_value", "currency") if k in d)
            out.append(f"| {d['id']} | {d['title']} | {d['kind']} | {d['status']} | {d['lang']} | {d['department']} | {d['updated']} | {', '.join(d['tags'])} | {extra} |")
        if len(res) > int(a.get("limit") or 50):
            out.append(f"... {len(res) - int(a.get('limit') or 50)} more (raise limit or narrow filters)")
    return "\n".join(out)


def data_tables(_):
    out = []
    for (t,) in db.execute("SELECT name FROM sqlite_master WHERE type='table'"):
        cols = [f"{r[1]} {r[2]}" for r in db.execute(f"PRAGMA table_info({t})")]
        n = db.execute(f"SELECT COUNT(*) FROM {t}").fetchone()[0]
        out.append(f"{t} ({n} rows): {', '.join(cols)}")
    return "\n".join(out)


def data_query(a):
    sql = a.get("sql", "").strip().rstrip(";")
    if not sql.lower().startswith(("select", "with")):
        return "error: only SELECT queries are allowed"
    try:
        cur = db.execute(sql)
    except Exception as e:
        return f"SQL error: {e}"
    cols = [c[0] for c in cur.description]
    rows = cur.fetchmany(101)
    out = ["| " + " | ".join(cols) + " |"] + ["| " + " | ".join(str(v) for v in r) + " |" for r in rows[:100]]
    if len(rows) > 100:
        out.append("... truncated at 100 rows")
    return "\n".join(out)


TOOLS = {"kb_query": (KB_QUERY, kb_query)}
if WITH_DATA:
    TOOLS.update({"data_tables": (DATA_TABLES, data_tables), "data_query": (DATA_QUERY, data_query)})

for line in sys.stdin:
    try:
        msg = json.loads(line)
    except json.JSONDecodeError:
        continue
    if "id" not in msg:
        continue
    m, p = msg.get("method"), msg.get("params") or {}
    try:
        if m == "initialize":
            r = {"protocolVersion": p.get("protocolVersion", "2025-06-18"), "capabilities": {"tools": {}}, "serverInfo": {"name": "kb", "version": "0.3"}}
        elif m == "ping":
            r = {}
        elif m == "tools/list":
            r = {"tools": [t for t, _ in TOOLS.values()]}
        elif m == "tools/call":
            name, args = p.get("name"), p.get("arguments") or {}
            if LOG:
                with open(LOG, "a") as f:
                    f.write(json.dumps({"tool": name, "args": args}, ensure_ascii=False) + "\n")
            r = {"content": [{"type": "text", "text": TOOLS[name][1](args)}]}
        else:
            raise ValueError(f"method not found: {m}")
        resp = {"jsonrpc": "2.0", "id": msg["id"], "result": r}
    except Exception as e:
        resp = {"jsonrpc": "2.0", "id": msg["id"], "error": {"code": -32601, "message": str(e)}}
    sys.stdout.write(json.dumps(resp, ensure_ascii=False) + "\n")
    sys.stdout.flush()
