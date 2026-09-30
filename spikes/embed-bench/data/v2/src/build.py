import json, sys, os
sys.path.insert(0, os.path.dirname(__file__))
from new_docs import NEW_DOCS
from queries_src import Q
base = "/home/beebiz/workspace/qobot/spikes/embed-bench/data"
old = json.load(open(f"{base}/docs.json"))
docs = old + [{"id": i, "lang": l, "title": t, "text": x} for i, l, t, x in NEW_DOCS]
queries = []
for d in docs:
    vi, en, ja = Q[d["id"]]
    for lang, q in (("vi", vi), ("en", en), ("ja", ja)):
        queries.append({"gold": d["id"], "lang": lang, "q": q})
os.makedirs(f"{base}/v2", exist_ok=True)
def dump(path, rows):
    with open(path, "w", encoding="utf-8") as f:
        f.write("[\n" + ",\n".join(json.dumps(r, ensure_ascii=False) for r in rows) + "\n]\n")
dump(f"{base}/v2/docs.json", docs)
dump(f"{base}/v2/queries.json", queries)
extra = set(Q) - {d["id"] for d in docs}
print("unused query keys:", extra)
