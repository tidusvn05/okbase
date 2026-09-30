import json, collections, re, unicodedata
base = "/home/beebiz/workspace/qobot/spikes/embed-bench/data"
docs = json.load(open(f"{base}/v2/docs.json")); qs = json.load(open(f"{base}/v2/queries.json"))
old = json.load(open(f"{base}/docs.json"))
ids = [d["id"] for d in docs]
assert len(ids) == len(set(ids)), "dup ids"
assert all(set(d) == {"id","lang","title","text"} for d in docs)
assert all(set(q) == {"gold","lang","q"} for q in qs)
assert docs[:30] == old, "existing docs changed"
idset = set(ids)
assert all(q["gold"] in idset for q in qs), "missing gold"
per = collections.defaultdict(list)
for q in qs: per[q["gold"]].append(q["lang"])
bad = [i for i in ids if sorted(per[i]) != ["en","ja","vi"]]
assert not bad, bad
qtexts = [q["q"] for q in qs]; assert len(qtexts) == len(set(qtexts)), "dup query text"
warn = []
for d in docs[30:]:
    n = len(d["text"]) if d["lang"] == "ja" else len(d["text"].split())
    lo, hi = (120, 300) if d["lang"] == "ja" else (60, 150)
    if not lo <= n <= hi: warn.append((d["id"], n))
    if d["lang"] == "ja": assert re.search(r"[぀-ヿ]", d["text"])
print("docs:", len(docs), dict(collections.Counter(d["lang"] for d in docs)))
print("queries:", len(qs), dict(collections.Counter(q["lang"] for q in qs)))
print("queries per doc: all exactly 3 (vi/en/ja)")
viq = [q["q"] for q in qs if q["lang"] == "vi"]
nodia = [q for q in viq if all(ord(c) < 128 for c in q)]
print(f"vi queries without diacritics: {len(nodia)}/{len(viq)}")
lens = [len(d["text"]) if d["lang"]=="ja" else len(d["text"].split()) for d in docs[30:]]
for lg in ("vi","en","ja"):
    L=[len(d["text"]) if lg=="ja" else len(d["text"].split()) for d in docs[30:] if d["lang"]==lg]
    print(f"new {lg} doc length ({'chars' if lg=='ja' else 'words'}): min {min(L)} max {max(L)}")
print("length warnings:", warn)
print("OK")
