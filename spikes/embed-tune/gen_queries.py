#!/usr/bin/env python3
"""Spike S11 data: synthetic (query, passage) pairs for fine-tuning embeddings.

  ml  fixtures/multilingual: 100 short vi/en/ja documents -> 5 questions each (mixed languages, incl. cross-language)
  oc  okf-scale bundle L: 300 random chunks (from the okbase index) -> 2 questions each

The eval questions (S1 queries.json, okf-scale questions.json) are never shown to the generator;
generated questions that (nearly) repeat an eval question are dropped.
Costs Claude Code quota (~25 calls with sonnet).
"""
import json, random, re, sqlite3, subprocess, sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
WORK = HERE / "work"
SCHEMA = json.dumps({"type": "object", "properties": {"items": {"type": "array", "items": {"type": "object", "properties": {
    "key": {"type": "string"}, "queries": {"type": "array", "items": {"type": "object", "properties": {
        "lang": {"type": "string"}, "q": {"type": "string"}}, "required": ["lang", "q"]}}}, "required": ["key", "queries"]}}},
    "required": ["items"]})
PROMPT = """You write realistic search questions that people would type to find a given passage in a company / product knowledge base.
For EACH passage below write exactly {n} questions:
- {langs}
- Phrase them like real users: colloquial, sometimes vague or with typos, usually NOT reusing the passage's exact wording or title.
- Each question must be answerable from that passage alone.
Return one item per passage with the same key.

"""


def fm_split(text):
    m = re.match(r"^---\n(.*?)\n---\n?(.*)$", text, re.S)
    return (m.group(1), m.group(2)) if m else ("", text)


def title_of(fm, fallback):
    m = re.search(r'^title:\s*"?(.*?)"?\s*$', fm, re.M)
    return m.group(1) if m else fallback


def call(body):
    p = subprocess.run(["claude", "-p", "--output-format", "json", "--no-session-persistence", "--setting-sources", "local",
                        "--tools", "", "--model", "sonnet", "--json-schema", SCHEMA], input=body, capture_output=True, text=True,
                       cwd=WORK, timeout=900)
    env = json.loads(p.stdout)
    return (env.get("structured_output") or {}).get("items", []), env.get("total_cost_usd") or 0


def norm(s):
    return re.sub(r"\W+", " ", s.lower()).strip()


def main():
    WORK.mkdir(parents=True, exist_ok=True)
    rng = random.Random(7)
    passages = []  # (set, key, doc_id, title, text)
    for f in sorted((ROOT / "fixtures/multilingual/knowledge").glob("*.md")):
        fm, body = fm_split(f.read_text())
        passages.append(("ml", f.stem, f"knowledge/{f.stem}", title_of(fm, f.stem), body.strip()))
    db = ROOT / "spikes/acceptance-v0.3/work/s4-embeddinggemma-300m-q4/index.sqlite"
    rows = sqlite3.connect(db).execute(
        "SELECT c.id, c.doc_id, d.title, c.heading, c.text FROM chunks c JOIN docs d ON d.id = c.doc_id ORDER BY c.id").fetchall()
    for cid, doc, title, heading, text in rng.sample(rows, 300):
        passages.append(("oc", f"c{cid}", doc, f"{title} > {heading}" if heading else title, text))

    evalq = {norm(q["q"]) for q in json.loads((ROOT / "fixtures/multilingual/queries.json").read_text())}
    evalq |= {norm(q["q"]) for q in json.loads((ROOT / "spikes/okf-scale/questions.json").read_text())}

    out, cost, dropped = [], 0.0, 0
    for set_name, n, langs, batch in [
        ("ml", 5, "2 in Vietnamese, 2 in Japanese, 1 in English; at least 2 of them in a different language than the passage", 10),
        ("oc", 2, "1 in English and 1 in either Vietnamese or Japanese (alternate between passages)", 20),
    ]:
        group = [p for p in passages if p[0] == set_name]
        for i in range(0, len(group), batch):
            chunk = group[i:i + batch]
            body = PROMPT.format(n=n, langs=langs) + "".join(
                f"### passage key={p[1]}\nTITLE: {p[3]}\n{p[4][:3000]}\n\n" for p in chunk)
            items, c = call(body)
            cost += c
            by_key = {p[1]: p for p in chunk}
            for it in items:
                p = by_key.get(it["key"])
                if not p:
                    continue
                for q in it["queries"]:
                    if norm(q["q"]) in evalq:
                        dropped += 1
                        continue
                    out.append({"set": set_name, "doc_id": p[2], "title": p[3], "text": p[4], "lang": q["lang"], "query": q["q"]})
            print(f"{set_name} {i + len(chunk)}/{len(group)} pairs={len(out)} ${cost:.2f}", flush=True)
    (WORK / "pairs.jsonl").write_text("".join(json.dumps(r, ensure_ascii=False) + "\n" for r in out))
    print(f"wrote {len(out)} pairs, dropped {dropped} near-copies of eval questions, cost ${cost:.2f}")


if __name__ == "__main__":
    main()
