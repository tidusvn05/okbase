#!/usr/bin/env python3
"""Sinh câu hỏi đánh giá từ 15 tài liệu gold (2 câu/tài liệu, xoay vòng vi/en/ja) bằng Claude CLI."""
import json, subprocess, concurrent.futures as cf
from pathlib import Path

ROOT = Path(__file__).resolve().parent
GOLD = json.loads((ROOT / "bundles/gold.json").read_text())
LANGS = ["vi", "en", "ja"]
LANG_NAME = {"vi": "Vietnamese", "en": "English", "ja": "Japanese"}
SCHEMA = json.dumps({"type": "object", "properties": {"questions": {"type": "array", "items": {"type": "object", "properties": {
    "lang": {"type": "string"}, "q": {"type": "string"}, "answer": {"type": "string"},
    "key_facts": {"type": "array", "items": {"type": "string"}}, "section": {"type": "string"}},
    "required": ["lang", "q", "answer", "key_facts", "section"]}}}, "required": ["questions"]})

PROMPT = """You create evaluation questions for a support chatbot whose knowledge base is the OpenClaw documentation (English).
Below is ONE document from that documentation. Write exactly 2 questions that a real user would ask, answerable from THIS document:
- Question 1 in {l1}, question 2 in {l2}. Write natively and naturally in that language (Vietnamese users may skip diacritics sometimes, Japanese may mix katakana tech terms). Do NOT translate the document title literally; paraphrase like a user would.
- Each question must target a SPECIFIC, non-obvious detail (a default value, a config key, a CLI command/flag, a limit, a precondition, a step) stated in the document — not a generic "what is X".
- The question must contain enough context (product/feature name) that it points to this document rather than to other OpenClaw docs.
- The two questions must ask about different details.
- answer: the correct answer in English (1-2 sentences). key_facts: 1-3 short atomic facts (English) that a correct answer MUST contain. section: heading where the answer is.

Document id: {id}
<document>
{text}
</document>"""


def gen(i, g):
    l1, l2 = LANGS[(2 * i) % 3], LANGS[(2 * i + 1) % 3]
    text = (ROOT / "bundles/S" / f"{g['id']}.md").read_text()
    p = subprocess.run(["claude", "-p", "--output-format", "json", "--no-session-persistence", "--setting-sources", "local",
                        "--tools", "", "--model", "sonnet", "--json-schema", SCHEMA],
                       input=PROMPT.format(l1=LANG_NAME[l1], l2=LANG_NAME[l2], id=g["id"], text=text),
                       capture_output=True, text=True, cwd=ROOT / "work", timeout=600)
    env = json.loads(p.stdout)
    qs = env["structured_output"]["questions"]
    for q, l in zip(qs, (l1, l2)):
        q["lang"], q["gold"] = l, g["id"]
    return qs, env.get("total_cost_usd", 0)


out, cost = [], 0
with cf.ThreadPoolExecutor(4) as ex:
    for qs, c in ex.map(lambda a: gen(*a), enumerate(GOLD)):
        out += qs
        cost += c
for n, q in enumerate(out):
    q["qid"] = n
(ROOT / "questions.json").write_text(json.dumps(out, ensure_ascii=False, indent=1))
print(f"{len(out)} questions, cost ${cost:.2f}")
from collections import Counter
print(Counter(q["lang"] for q in out))
