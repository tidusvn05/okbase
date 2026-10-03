#!/usr/bin/env python3
"""Spike S11 eval, same protocol as okbase's retrieval_eval (S1 doc-level R@k; S4 gold in top-6 chunks,
at most 2 per document), with sentence-transformers so base and tuned models compare like for like.
Usage: .venv/bin/python evaluate.py <model path or HF id> [label]"""
import json, re, sqlite3, sys, time
from pathlib import Path

import numpy as np
from sentence_transformers import SentenceTransformer

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
Q = "task: search result | query: "


def doc(title, text):
    return f"title: {title or 'none'} | text: {text}"


def embed(model, texts, bs=4):  # small batches: 2048-token inputs, CPU RAM
    return model.encode(texts, batch_size=bs, normalize_embeddings=True, convert_to_numpy=True, show_progress_bar=False)


def main():
    name = sys.argv[1]
    label = sys.argv[2] if len(sys.argv) > 2 else name
    model = SentenceTransformer(name, device="cpu")
    model.max_seq_length = 2048
    res = {"model": label}
    # S1
    docs = []
    for f in sorted((ROOT / "fixtures/multilingual/knowledge").glob("*.md")):
        m = re.match(r"^---\n(.*?)\n---\n?(.*)$", f.read_text(), re.S)
        title = re.search(r'^title:\s*"?(.*?)"?\s*$', m.group(1), re.M).group(1)
        docs.append((f.stem, doc(title, m.group(2).strip())))
    D = embed(model, [d[1] for d in docs])
    qs = json.loads((ROOT / "fixtures/multilingual/queries.json").read_text())
    Qv = embed(model, [Q + q["q"] for q in qs])
    ranks = np.argsort(-(Qv @ D.T), axis=1)
    ids = [d[0] for d in docs]
    r1 = r3 = mrr = 0
    cross = [0, 0]
    for q, row in zip(qs, ranks):
        pos = [ids[i] for i in row].index(q["gold"])
        r1 += pos == 0
        r3 += pos < 3
        mrr += 1 / (pos + 1)
        if q["lang"] != q["gold"][:2]:
            cross[0] += pos == 0
            cross[1] += 1
    n = len(qs)
    res["s1"] = {"r@1": round(r1 / n, 3), "r@3": round(r3 / n, 3), "mrr": round(mrr / n, 3), "cross_r@1": round(cross[0] / cross[1], 3)}
    # S4
    db = ROOT / "spikes/acceptance-v0.3/work/s4-embeddinggemma-300m-q4/index.sqlite"
    rows = sqlite3.connect(db).execute("SELECT c.doc_id, d.title, c.heading, c.text FROM chunks c JOIN docs d ON d.id = c.doc_id ORDER BY c.id").fetchall()
    t = time.time()
    C = embed(model, [doc(f"{t_} > {h}" if h else t_, x) for _, t_, h, x in rows])
    res["s4_embed_s"] = round(time.time() - t)
    qs4 = json.loads((ROOT / "spikes/okf-scale/questions.json").read_text())
    S = embed(model, [Q + q["q"] for q in qs4]) @ C.T
    top6 = first = 0
    for q, sc in zip(qs4, S):
        seen, hits = {}, []
        for i in np.argsort(-sc):
            d = rows[i][0]
            seen[d] = seen.get(d, 0) + 1
            if seen[d] <= 2:
                hits.append(d)
            if len(hits) == 6:
                break
        top6 += q["gold"] in hits
        first += hits[0] == q["gold"]
    res["s4"] = {"top6": f"{top6}/30", "first": first}
    print(json.dumps(res, ensure_ascii=False))
    with (HERE / "work/results.jsonl").open("a") as f:
        f.write(json.dumps(res, ensure_ascii=False) + "\n")


if __name__ == "__main__":
    main()
