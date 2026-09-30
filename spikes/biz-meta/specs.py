#!/usr/bin/env python3
"""Sinh spec (metadata + facts) cho bundle business giả lập, sheet CSV, và câu hỏi có đáp án tính sẵn.

Output: specs.json, data/*.csv, questions.json
"""
import csv, json, os, random
from datetime import date, timedelta
from pathlib import Path

ROOT = Path(__file__).resolve().parent
SCALE = int(os.environ.get("SCALE", "1"))  # >1: thêm tài liệu "nhiễu" + sheet lớn hơn (đáp án tính lại)
SUF = "" if SCALE == 1 else f"-x{SCALE}"
rng = random.Random(2026)
TODAY = date(2026, 9, 30)

DEPTS = ["Sales", "CS", "Warehouse", "HR", "IT", "Legal", "Finance"]
DEPT = {"Sales": ("phòng Kinh doanh", "Sales", "営業部"), "CS": ("phòng CSKH", "Customer Support", "カスタマーサポート部"),
        "Warehouse": ("phòng Kho vận", "Warehouse", "倉庫部"), "HR": ("phòng Nhân sự", "HR", "人事部"),
        "IT": ("phòng IT", "IT", "IT部"), "Legal": ("phòng Pháp chế", "Legal", "法務部"), "Finance": ("phòng Tài chính", "Finance", "財務部")}
LI = {"vi": 0, "en": 1, "ja": 2}
specs = []


def add(**kw):
    kw.setdefault("tags", [])
    kw.setdefault("status", "stable")
    specs.append(kw)
    return kw


def rdate(a, b):
    a, b = date.fromisoformat(a), date.fromisoformat(b)
    return a + timedelta(days=rng.randint(0, (b - a).days))


# ---------------- Policies (có phiên bản) ----------------
POLICY = {
    "returns": (("chính sách đổi trả", "return policy", "返品ポリシー"), ["returns", "customer-support"],
                lambda: {"return_window_days": rng.choice([7, 14, 15, 30, 45]), "restocking_fee_pct": rng.choice([0, 5, 10, 15])}),
    "shipping": (("chính sách giao hàng", "shipping policy", "配送ポリシー"), ["shipping"],
                 lambda: {"free_shipping_threshold": rng.choice([300000, 500000, 800000, 1000000]), "standard_delivery_days": rng.choice([2, 3, 4, 5])}),
    "warranty-air-purifier": (("chính sách bảo hành máy lọc không khí", "air purifier warranty policy", "空気清浄機の保証ポリシー"), ["warranty", "air-purifier"],
                              lambda: {"warranty_months": rng.choice([12, 18, 24, 36])}),
    "warranty-rice-cooker": (("chính sách bảo hành nồi cơm điện", "rice cooker warranty policy", "炊飯器の保証ポリシー"), ["warranty", "rice-cooker"],
                             lambda: {"warranty_months": rng.choice([6, 12, 24])}),
    "installment": (("chính sách trả góp", "installment payment policy", "分割払いポリシー"), ["billing", "finance"],
                    lambda: {"min_order_amount": rng.choice([2000000, 3000000, 5000000]), "max_installment_months": rng.choice([6, 9, 12, 24])}),
    "loyalty": (("chính sách điểm thành viên", "loyalty points policy", "ポイントポリシー"), ["loyalty", "marketing"],
                lambda: {"points_expiry_months": rng.choice([6, 12, 18, 24])}),
    "privacy": (("chính sách bảo vệ dữ liệu cá nhân", "personal data protection policy", "個人情報保護ポリシー"), ["privacy", "legal", "security"],
                lambda: {"deletion_request_days": rng.choice([7, 14, 30])}),
    "gift-wrap": (("chính sách gói quà", "gift wrapping policy", "ギフト包装ポリシー"), ["customer-support", "marketing"],
                  lambda: {"gift_wrap_price": rng.choice([20000, 30000, 50000])}),
}
REGION_NAME = {"VN": ("Việt Nam", "Vietnam", "ベトナム"), "JP": ("Nhật Bản", "Japan", "日本")}
multi_version = []
for topic, (names, tags, fact) in POLICY.items():
    for region in ("VN", "JP"):
        lang = ("vi" if rng.random() < .8 else "en") if region == "VN" else ("ja" if rng.random() < .8 else "en")
        versions = [("v2", "stable")]
        if rng.random() < .55:
            versions.insert(0, ("v1", "deprecated"))
        if rng.random() < .2:
            versions.append(("v3", "draft"))
        prev = None
        for v, st in versions:
            f = fact()
            while prev and f == prev:
                f = fact()
            upd = {"v1": rdate("2024-01-01", "2025-06-30"), "v2": rdate("2025-07-01", "2026-08-31"), "v3": rdate("2026-09-01", "2026-09-29")}[v]
            s = add(id=f"policies/{region.lower()}/{topic}-{v}", kind="Policy", topic=topic, topic_names=names, region=region, lang=lang,
                    status=st, version=v, department="Legal" if topic == "privacy" else ("Finance" if topic == "installment" else "CS"),
                    tags=tags + ["policy", region.lower()], updated=str(upd), facts=f,
                    currency="VND" if region == "VN" else "JPY",
                    effective_from=str(upd + timedelta(days=14)) if st != "draft" else "2026-11-01")
            if prev is not None:
                s["supersedes"] = f"policies/{region.lower()}/{topic}-{versions[versions.index((v, st)) - 1][0]}"
            prev = f
        if len(versions) > 1:
            multi_version.append((topic, region))
# JPY cho policy Nhật: đổi giá trị tiền cho hợp lý
for s in specs:
    if s["kind"] == "Policy" and s["region"] == "JP":
        for k in ("free_shipping_threshold", "min_order_amount", "gift_wrap_price"):
            if k in s["facts"]:
                s["facts"][k] = s["facts"][k] // 200

# ---------------- SOPs ----------------
SOP = {
    "Warehouse": [("receiving", ("nhận hàng nhập kho", "inbound receiving", "入荷受付"), {"inspection_deadline_hours": [4, 8, 24]}),
                  ("stock-count", ("kiểm kê tồn kho", "stock count", "棚卸し"), {"count_frequency_days": [7, 14, 30]}),
                  ("packing", ("đóng gói đơn hàng", "order packing", "梱包"), {"max_package_weight_kg": [20, 25, 30]}),
                  ("forklift", ("an toàn xe nâng", "forklift safety", "フォークリフト安全"), {"max_speed_kmh": [5, 8, 10]}),
                  ("returns-processing", ("xử lý hàng hoàn", "returns processing", "返品処理"), {"processing_days": [2, 3, 5]}),
                  ("cold-storage", ("kho lạnh", "cold storage", "冷蔵保管"), {"temperature_c": [2, 4, 6]}),
                  ("dispatch", ("xuất kho", "dispatch", "出荷"), {"cutoff_hour": [14, 15, 16]}),
                  ("damaged-goods", ("hàng hư hỏng", "damaged goods", "破損品"), {"report_within_hours": [2, 12, 24]}),
                  ("labeling", ("dán nhãn", "labeling", "ラベル貼付"), {"label_retention_days": [30, 90]}),
                  ("hazmat", ("hàng nguy hiểm", "hazardous materials", "危険物"), {"training_hours": [4, 8]})],
    "CS": [("ticket-triage", ("phân loại ticket", "ticket triage", "チケット振り分け"), {"first_response_hours": [1, 2, 4]}),
           ("refund-approval", ("duyệt hoàn tiền", "refund approval", "返金承認"), {"auto_approve_limit": [500000, 1000000]}),
           ("escalation", ("chuyển cấp xử lý", "escalation", "エスカレーション"), {"escalate_after_hours": [24, 48]}),
           ("vip-customers", ("khách hàng VIP", "VIP customers", "VIP顧客対応"), {"callback_minutes": [15, 30]}),
           ("chat-handling", ("xử lý chat", "chat handling", "チャット対応"), {"max_concurrent_chats": [3, 4, 5]}),
           ("complaint", ("khiếu nại", "complaint handling", "苦情対応"), {"resolution_days": [3, 5, 7]}),
           ("warranty-claims", ("tiếp nhận bảo hành", "warranty claims", "保証請求受付"), {"required_photos": [2, 3]}),
           ("call-recording", ("ghi âm cuộc gọi", "call recording", "通話録音"), {"retention_days": [90, 180]})],
    "IT": [("password", ("mật khẩu", "password management", "パスワード管理"), {"rotation_days": [60, 90, 180]}),
           ("access-request", ("cấp quyền truy cập", "access requests", "アクセス申請"), {"approval_sla_hours": [8, 24]}),
           ("backup", ("sao lưu dữ liệu", "data backup", "バックアップ"), {"backup_retention_days": [30, 90]}),
           ("incident", ("sự cố bảo mật", "security incident", "セキュリティインシデント"), {"report_within_minutes": [15, 30, 60]}),
           ("laptop", ("cấp phát laptop", "laptop provisioning", "PC支給"), {"replacement_years": [3, 4]}),
           ("vpn", ("sử dụng VPN", "VPN usage", "VPN利用"), {"session_timeout_hours": [8, 12]})],
    "HR": [("leave", ("nghỉ phép", "leave requests", "休暇申請"), {"notice_days": [3, 5, 7]}),
           ("onboarding", ("tiếp nhận nhân viên mới", "onboarding", "入社手続き"), {"buddy_weeks": [2, 4]}),
           ("overtime", ("làm thêm giờ", "overtime", "残業"), {"monthly_cap_hours": [30, 40, 45]}),
           ("expense", ("hoàn ứng chi phí", "expense claims", "経費精算"), {"submit_within_days": [7, 15, 30]}),
           ("remote-work", ("làm việc từ xa", "remote work", "リモートワーク"), {"max_days_per_week": [2, 3]}),
           ("offboarding", ("nghỉ việc", "offboarding", "退職手続き"), {"notice_days": [30, 45]})],
}
SOP_TAGS = {"Warehouse": ["warehouse", "inventory"], "CS": ["customer-support"], "IT": ["it", "security"], "HR": ["hr"]}
for dept, items in SOP.items():
    for topic, names, fact in items:
        st = rng.choices(["stable", "draft", "deprecated"], [22, 5, 3])[0]
        lang = {"Warehouse": rng.choice(["vi", "vi", "ja"]), "CS": rng.choice(["vi", "en", "ja"]), "IT": rng.choice(["en", "en", "vi"]), "HR": rng.choice(["vi", "ja"])}[dept]
        extra = []
        if topic in ("incident", "password", "access-request", "call-recording"):
            extra.append("security")
        if topic in ("call-recording", "vip-customers"):
            extra.append("privacy")
        add(id=f"sop/{dept.lower()}/{topic}", kind="SOP", topic=topic, topic_names=names, department=dept, lang=lang, status=st,
            tags=sorted(set(SOP_TAGS[dept] + ["sop"] + extra)), updated=str(rdate("2024-03-01", "2026-09-25")),
            facts={k: rng.choice(v) for k, v in fact.items()})

# ---------------- Contracts ----------------
CUSTOMERS = {"ACME Corp": "VN", "Sakura Trading": "JP", "Minh Phat JSC": "VN", "Tokyo Denki Retail": "JP", "Saigon Hotels Group": "VN", "Osaka Office Supply": "JP"}
CONTRACT_TYPES = [("distribution", ("hợp đồng phân phối", "distribution agreement", "販売代理店契約")),
                  ("supply", ("hợp đồng cung cấp", "supply agreement", "供給契約")),
                  ("maintenance", ("hợp đồng bảo trì", "maintenance agreement", "保守契約")),
                  ("installation", ("hợp đồng lắp đặt", "installation agreement", "設置契約"))]
cust_list = list(CUSTOMERS)
for i in range(30):
    cust = cust_list[i % 6] if i < 24 else rng.choice(cust_list[:3])
    region = CUSTOMERS[cust]
    ctype, cnames = rng.choice(CONTRACT_TYPES)
    start = rdate("2023-06-01", "2026-08-01")
    months = rng.choice([12, 12, 24, 36])
    end = date(start.year + (start.month - 1 + months) // 12, (start.month - 1 + months) % 12 + 1, min(start.day, 28)) - timedelta(days=1)
    value = rng.randrange(200, 5000) * (1_000_000 if region == "VN" else 10_000)
    slug = cust.lower().replace(" ", "-")
    add(id=f"contracts/{slug}/{ctype}-{start.year}{start.month:02d}", kind="Contract", topic=ctype, topic_names=cnames, customer=cust, region=region,
        lang=("vi" if region == "VN" else "ja") if rng.random() < .7 else "en", department="Sales", status="stable",
        tags=["contract", "b2b", "sales", region.lower()], updated=str(start - timedelta(days=rng.randint(3, 20))),
        effective_from=str(start), effective_to=str(end), contract_value=value, currency="VND" if region == "VN" else "JPY",
        facts={"payment_terms_days": rng.choice([30, 45, 60]), "renewal_notice_days": rng.choice([30, 60, 90])})

# ---------------- Meeting notes ----------------
MEET = [("budget", ("ngân sách", "budget review", "予算レビュー"), ["finance"]), ("hiring", ("tuyển dụng", "hiring plan", "採用計画"), ["hr"]),
        ("launch", ("ra mắt sản phẩm", "product launch", "製品発売"), ["product", "marketing"]), ("kpi", ("KPI", "KPI review", "KPIレビュー"), ["sales"]),
        ("security-review", ("rà soát bảo mật", "security review", "セキュリティレビュー"), ["security", "it"]),
        ("inventory-planning", ("kế hoạch tồn kho", "inventory planning", "在庫計画"), ["inventory", "warehouse"])]
for i in range(25):
    topic, names, tags = MEET[i % 6]
    dept = {"budget": "Finance", "hiring": "HR", "launch": "Sales", "kpi": "Sales", "security-review": "IT", "inventory-planning": "Warehouse"}[topic]
    if i >= 18:
        dept = rng.choice(["Sales", "HR"])
    d = rdate("2025-10-01", "2026-09-28")
    add(id=f"meetings/{dept.lower()}/{d.isoformat()}-{topic}", kind="Meeting Note", topic=topic, topic_names=names, department=dept,
        lang=rng.choice(["vi", "en", "ja"]), status="stable", tags=sorted(set(tags + ["meeting"])), updated=str(d), meeting_date=str(d),
        facts={"decided_budget_million_vnd": rng.randrange(50, 900, 10), "next_meeting_in_weeks": rng.choice([1, 2, 4])})

# ---------------- Product specs ----------------
CATS = {"air-purifier": ("máy lọc không khí", "air purifier", "空気清浄機"), "rice-cooker": ("nồi cơm điện", "rice cooker", "炊飯器"),
        "aircon": ("điều hòa", "air conditioner", "エアコン"), "fridge": ("tủ lạnh", "refrigerator", "冷蔵庫"), "washer": ("máy giặt", "washing machine", "洗濯機")}
for i in range(15):
    cat = list(CATS)[i % 5]
    model = f"HK-{cat[:2].upper()}{rng.randint(100, 999)}"
    st = "deprecated" if rng.random() < .3 else "stable"
    add(id=f"products/{cat}/{model.lower()}", kind="Product Spec", topic=cat, topic_names=CATS[cat], model=model, department="Sales",
        lang=rng.choice(["vi", "en", "ja"]), status=st, tags=["product", cat], updated=str(rdate("2024-01-01", "2026-09-01")),
        facts={"power_watts": rng.randrange(40, 2400, 10), "launch_year": rng.choice([2021, 2022, 2023, 2024, 2025])})

# ---------------- FAQ / Announcements ----------------
for i in range(10):
    t = rng.choice(["returns", "shipping", "warranty", "loyalty", "billing", "privacy"])
    add(id=f"faq/{t}-{i}", kind="FAQ", topic=t, topic_names=(t, t, t), department="CS", lang=rng.choice(["vi", "en", "ja"]),
        status="stable", tags=["faq", "customer-support", t], updated=str(rdate("2025-01-01", "2026-09-20")), facts={})
for i in range(10):
    t = rng.choice(["office-move", "holiday", "new-system", "price-change", "security-training"])
    tags = {"office-move": ["hr"], "holiday": ["hr"], "new-system": ["it"], "price-change": ["pricing", "sales"], "security-training": ["security", "it"]}[t]
    add(id=f"announcements/{t}-{i}", kind="Announcement", topic=t, topic_names=(t, t, t), department=rng.choice(DEPTS), lang=rng.choice(["vi", "en", "ja"]),
        status="stable", tags=["announcement"] + tags, updated=str(rdate("2025-06-01", "2026-09-29")), facts={})

# ---------------- Filler (chỉ khi SCALE > 1): tài liệu thật-giả cùng schema, nội dung sinh bằng template ----------------
if SCALE > 1:
    r2 = random.Random(99)
    n_extra = 151 * (SCALE - 1)
    extra_customers = [f"Customer {i:03d}" for i in range(1, 201)]
    kinds = r2.choices(["Contract", "Meeting Note", "SOP", "Product Spec", "FAQ", "Announcement", "Policy"], [27, 30, 20, 10, 6, 5, 2], k=n_extra)
    for n, kind in enumerate(kinds):
        lang = r2.choice(["vi", "en", "ja"])
        upd = str(date(2023, 1, 1) + timedelta(days=r2.randint(0, 1368)))
        if kind == "Contract":
            cust = r2.choice(extra_customers)
            region = r2.choice(["VN", "JP"])
            start = date.fromisoformat(upd) + timedelta(days=10)
            months = r2.choice([12, 24, 36])
            end = date(start.year + (start.month - 1 + months) // 12, (start.month - 1 + months) % 12 + 1, min(start.day, 28)) - timedelta(days=1)
            ctype, cn = r2.choice(CONTRACT_TYPES)
            add(id=f"contracts/{cust.lower().replace(' ', '-')}/{ctype}-{start.year}{start.month:02d}-{n}", kind="Contract", topic=ctype, topic_names=cn,
                customer=cust, region=region, lang=lang, department="Sales", tags=["contract", "b2b", "sales", region.lower()], updated=upd,
                effective_from=str(start), effective_to=str(end), contract_value=r2.randrange(200, 5000) * (1_000_000 if region == "VN" else 10_000),
                currency="VND" if region == "VN" else "JPY", facts={"payment_terms_days": r2.choice([30, 45, 60])}, filler=True)
        elif kind == "Meeting Note":
            topic, names, tags = r2.choice(MEET)
            dept = r2.choice(DEPTS)
            add(id=f"meetings/{dept.lower()}/{upd}-{topic}-{n}", kind=kind, topic=topic, topic_names=names, department=dept, lang=lang,
                tags=sorted(set(tags + ["meeting"])), updated=upd, meeting_date=upd, facts={"decided_budget_million_vnd": r2.randrange(50, 900, 10)}, filler=True)
        elif kind == "SOP":
            dept = r2.choice(list(SOP))
            topic, names, fact = r2.choice(SOP[dept])
            branch = r2.randint(1, 40)
            add(id=f"sop/{dept.lower()}/branch-{branch:02d}/{topic}-{n}", kind=kind, topic=topic, topic_names=names, department=dept, lang=lang,
                status=r2.choices(["stable", "draft", "deprecated"], [22, 5, 3])[0], tags=sorted(set(SOP_TAGS[dept] + ["sop", "branch"])), updated=upd,
                facts={k: r2.choice(v) for k, v in fact.items()}, filler=True)
        elif kind == "Product Spec":
            cat = r2.choice(list(CATS))
            model = f"HK-{cat[:2].upper()}{r2.randint(1000, 9999)}"
            add(id=f"products/{cat}/{model.lower()}-{n}", kind=kind, topic=cat, topic_names=CATS[cat], model=model, department="Sales", lang=lang,
                status="deprecated" if r2.random() < .3 else "stable", tags=["product", cat], updated=upd,
                facts={"power_watts": r2.randrange(40, 2400, 10)}, filler=True)
        elif kind == "Policy":
            topic = r2.choice(list(POLICY))
            region = r2.choice(["TH", "SG", "KR"])
            add(id=f"policies/{region.lower()}/{topic}-{n}", kind=kind, topic=topic, topic_names=POLICY[topic][0], region=region, lang="en",
                status=r2.choice(["stable", "deprecated", "draft"]), version=r2.choice(["v1", "v2", "v3"]), department="CS",
                tags=POLICY[topic][1] + ["policy", region.lower()], updated=upd, facts=POLICY[topic][2](), currency="USD", effective_from=upd)
            specs[-1]["filler"] = True
        else:
            t = r2.choice(["returns", "shipping", "warranty", "office-move", "holiday", "new-system"])
            add(id=f"{'faq' if kind == 'FAQ' else 'announcements'}/{t}-x{n}", kind=kind, topic=t, topic_names=(t, t, t), department=r2.choice(DEPTS),
                lang=lang, tags=[kind.lower(), t], updated=upd, facts={}, filler=True)

(ROOT / f"specs{SUF}.json").write_text(json.dumps(specs, ensure_ascii=False, indent=1))

# ---------------- Sheets ----------------
DATA_DIR = ROOT / f"data{SUF}"
DATA_DIR.mkdir(exist_ok=True)
PRODUCTS = [(f"SKU-{c[:2].upper()}{n:03d}", f"Hikari {CATS[c][1].title()} {n}", c) for c in CATS for n in range(1, 12 * SCALE + 1)]
WAREHOUSES = ["HN", "HCM", "Tokyo", "Osaka"]
inv = []
for sku, name, cat in PRODUCTS:
    for w in WAREHOUSES:
        inv.append({"sku": sku, "product_name": name, "category": cat, "warehouse": w, "qty": rng.randint(0, 400), "reorder_level": rng.choice([20, 30, 50])})
sales = []
for m in range(1, 10):
    for region in ("VN", "JP"):
        for sku, name, cat in PRODUCTS:
            if rng.random() < .45:
                u = rng.randint(1, 120)
                price = rng.randint(1_500_000, 25_000_000) if region == "VN" else rng.randint(8_000, 150_000)
                sales.append({"month": f"2026-{m:02d}", "region": region, "sku": sku, "category": cat, "units": u,
                              "revenue": u * price, "currency": "VND" if region == "VN" else "JPY"})
prices = [{"sku": sku, "product_name": name, "category": cat, "price_vnd": rng.randrange(1_500_000, 25_000_000, 10_000),
           "price_jpy": rng.randrange(8_000, 150_000, 100), "status": rng.choice(["active"] * 5 + ["discontinued"])} for sku, name, cat in PRODUCTS]
for name, rows in (("inventory", inv), ("sales_2026", sales), ("price_list", prices)):
    with open(DATA_DIR / f"{name}.csv", "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=list(rows[0]))
        w.writeheader()
        w.writerows(rows)
print(f"specs: {len(specs)}  inventory={len(inv)} sales={len(sales)} prices={len(prices)}  multi-version policies={len(multi_version)}")

# ---------------- Questions (đáp án tính từ spec) ----------------
Q = []
langs = ["vi", "en", "ja"]


def q(cat, lang, text, answer, kind="set"):
    Q.append({"qid": len(Q), "cat": cat, "lang": lang, "q": text, "expected": answer, "answer_kind": kind})


def active(s):
    return s["effective_from"] <= str(TODAY) <= s["effective_to"]


def titles(ss):
    return [s["id"] for s in ss]


C = [s for s in specs if s["kind"] == "Contract"]
P = [s for s in specs if s["kind"] == "Policy"]
S = [s for s in specs if s["kind"] == "SOP"]
M = [s for s in specs if s["kind"] == "Meeting Note"]
PR = [s for s in specs if s["kind"] == "Product Spec"]

# C1 — liệt kê theo bộ lọc
q("list", "vi", "Liệt kê các hợp đồng với Sakura Trading còn hiệu lực tại thời điểm hôm nay (30/09/2026).", titles([s for s in C if s["customer"] == "Sakura Trading" and active(s)]))
q("list", "en", "Which Warehouse SOPs are still in draft status?", titles([s for s in S if s["department"] == "Warehouse" and s["status"] == "draft"]))
q("list", "ja", "日本向けの現行（有効な）ポリシーをすべて挙げてください。ドラフトや廃止版は除いてください。", titles([s for s in P if s["region"] == "JP" and s["status"] == "stable"]))
q("list", "vi", "Những tài liệu nào gắn tag security được cập nhật trong năm 2026?", titles([s for s in specs if "security" in s["tags"] and s["updated"] >= "2026-01-01"]))
q("list", "en", "List all contracts that expire between 2026-10-01 and 2027-06-30.", titles([s for s in C if "2026-10-01" <= s["effective_to"] <= "2027-06-30"]))
q("list", "ja", "販売終了（廃止）になった製品モデルの一覧を教えてください。", titles([s for s in PR if s["status"] == "deprecated"]))
q("list", "vi", "Liệt kê các biên bản họp của phòng Nhân sự (HR) trong năm 2026.", titles([s for s in M if s["department"] == "HR" and s["meeting_date"] >= "2026-01-01"]))
q("list", "en", "Which Customer Support (CS) SOPs are written in Japanese?", titles([s for s in S if s["department"] == "CS" and s["lang"] == "ja"]))

# C2 — chi tiết theo phiên bản hiện hành
FACT_Q = {"return_window_days": ("được đổi trả trong bao nhiêu ngày", "how many days is the return window", "返品期限は何日"),
          "free_shipping_threshold": ("đơn từ bao nhiêu thì được miễn phí vận chuyển", "what is the free-shipping threshold", "送料無料になる注文金額はいくら"),
          "warranty_months": ("thời hạn bảo hành là bao nhiêu tháng", "how many months is the warranty", "保証期間は何か月"),
          "min_order_amount": ("đơn tối thiểu bao nhiêu để được trả góp", "what is the minimum order amount for installments", "分割払いの最低注文金額はいくら"),
          "points_expiry_months": ("điểm hết hạn sau bao nhiêu tháng", "after how many months do points expire", "ポイントは何か月で失効"),
          "deletion_request_days": ("yêu cầu xoá dữ liệu được xử lý trong bao nhiêu ngày", "within how many days are deletion requests handled", "削除依頼は何日以内に対応"),
          "gift_wrap_price": ("phí gói quà là bao nhiêu", "how much does gift wrapping cost", "ギフト包装の料金はいくら")}
for n, (topic, region) in enumerate(multi_version[:8]):
    cur = next(s for s in P if s["topic"] == topic and s["region"] == region and s["status"] == "stable")
    key = next(iter(cur["facts"]))
    lang = langs[n % 3]
    li = LI[lang]
    text = {"vi": f"Theo {POLICY[topic][0][0]} hiện hành tại {REGION_NAME[region][0]}, {FACT_Q[key][0]}?",
            "en": f"Under the current {POLICY[topic][0][1]} for {REGION_NAME[region][1]}, {FACT_Q[key][1]}?",
            "ja": f"{REGION_NAME[region][2]}の現行の{POLICY[topic][0][2]}では、{FACT_Q[key][2]}ですか？"}[lang]
    q("version", lang, text, {"value": cur["facts"][key], "fact": key, "current_doc": cur["id"],
                               "other_versions": {s["id"]: s["facts"][key] for s in P if s["topic"] == topic and s["region"] == region and s is not cur}}, "value")

# C3 — đếm / tổng hợp theo metadata
q("count", "vi", "Có bao nhiêu hợp đồng hết hạn trong năm 2026?", len([s for s in C if s["effective_to"].startswith("2026")]), "number")
q("count", "en", "How many meeting notes did the Sales department record in Q2 2026 (April–June)?", len([s for s in M if s["department"] == "Sales" and "2026-04-01" <= s["meeting_date"] <= "2026-06-30"]), "number")
q("count", "ja", "2026年に更新された倉庫部のSOPはいくつありますか？", len([s for s in S if s["department"] == "Warehouse" and s["updated"] >= "2026-01-01"]), "number")
q("count", "vi", "Hiện có bao nhiêu chính sách đang có hiệu lực (stable) cho thị trường Việt Nam?", len([s for s in P if s["region"] == "VN" and s["status"] == "stable"]), "number")
q("count", "en", "How many documents are tagged 'privacy'?", len([s for s in specs if "privacy" in s["tags"]]), "number")
tot = sum(s["contract_value"] for s in C if s["customer"] == "ACME Corp" and active(s))
q("count", "ja", "ACME Corpとの現在有効な契約の契約金額の合計はいくらですか？", {"value": tot, "currency": "VND"}, "number")
q("count", "vi", "Có bao nhiêu SOP đang ở trạng thái nháp (draft) trên toàn công ty?", len([s for s in S if s["status"] == "draft"]), "number")
q("count", "en", "How many contracts with Japanese customers are currently active?", len([s for s in C if s["region"] == "JP" and active(s)]), "number")

# C4 — khám phá / facet
q("facet", "vi", "Những phòng ban nào có tài liệu gắn tag privacy?", sorted({s["department"] for s in specs if "privacy" in s["tags"]}))
cnt = {}
for s in C:
    if active(s):
        cnt[s["customer"]] = cnt.get(s["customer"], 0) + 1
best = max(cnt.values())
q("facet", "en", "Which customer has the most currently active contracts, and how many?", {"customers": sorted(k for k, v in cnt.items() if v == best), "count": best})
q("facet", "ja", "廃止（販売終了）モデルがある製品カテゴリはどれですか？", sorted({s["topic"] for s in PR if s["status"] == "deprecated"}))
q("facet", "vi", "Những phòng ban nào đang có SOP ở trạng thái nháp?", sorted({s["department"] for s in S if s["status"] == "draft"}))
q("facet", "en", "Which customers have at least one contract expiring in 2026?", sorted({s["customer"] for s in C if s["effective_to"].startswith("2026")}))
q("facet", "ja", "新しいバージョンがドラフト（未承認）として準備中のポリシーはどれですか？", titles([s for s in P if s["status"] == "draft"]))

# C5 — sheet
def s_inv(cat=None, wh=None):
    return sum(r["qty"] for r in inv if (cat is None or r["category"] == cat) and (wh is None or r["warehouse"] == wh))
q("sheet", "vi", "Tổng số lượng tồn kho máy lọc không khí (air-purifier) ở kho HCM là bao nhiêu?", s_inv("air-purifier", "HCM"), "number")
q("sheet", "en", "How many inventory rows in the Tokyo warehouse are below their reorder level?", len([r for r in inv if r["warehouse"] == "Tokyo" and r["qty"] < r["reorder_level"]]), "number")
q("sheet", "ja", "大阪倉庫の炊飯器（rice-cooker）の在庫合計は何台ですか？", s_inv("rice-cooker", "Osaka"), "number")
rev = {}
for r in sales:
    if r["region"] == "JP":
        rev[r["month"]] = rev.get(r["month"], 0) + r["revenue"]
q("sheet", "vi", "Trong năm 2026, tháng nào thị trường Nhật (JP) có doanh thu cao nhất và bao nhiêu?", {"month": max(rev, key=rev.get), "revenue_jpy": max(rev.values())}, "value")
q("sheet", "en", "What was the total VN revenue (VND) for washing machines (washer) in Q1 2026?",
  sum(r["revenue"] for r in sales if r["region"] == "VN" and r["category"] == "washer" and r["month"] <= "2026-03"), "number")
units = {}
for r in sales:
    if r["region"] == "VN" and "2026-04" <= r["month"] <= "2026-06":
        units[r["sku"]] = units.get(r["sku"], 0) + r["units"]
q("sheet", "ja", "2026年第2四半期（4〜6月）にベトナムで最も販売台数が多かったSKUはどれで、何台ですか？", {"sku": max(units, key=units.get), "units": max(units.values())}, "value")
q("sheet", "vi", "Có bao nhiêu sản phẩm trong bảng giá đang ở trạng thái discontinued?", len([p for p in prices if p["status"] == "discontinued"]), "number")
pp = rng.choice(prices)
q("sheet", "en", f"What is the JPY price of {pp['sku']} in the price list?", pp["price_jpy"], "number")
q("sheet", "ja", "全倉庫の冷蔵庫（fridge）の在庫合計は？", s_inv("fridge"), "number")
q("sheet", "vi", "Tổng số lượng tồn kho của tất cả sản phẩm ở kho HN là bao nhiêu?", s_inv(None, "HN"), "number")

# C6 — nội dung thường (không cần metadata)
FACT_LABEL = {"inspection_deadline_hours": ("thời hạn kiểm tra (giờ)", "inspection deadline (hours)", "検品期限（時間）"),
              "count_frequency_days": ("tần suất kiểm kê (ngày)", "count frequency (days)", "棚卸しの頻度（日）"),
              "max_package_weight_kg": ("khối lượng kiện tối đa (kg)", "maximum package weight (kg)", "梱包の最大重量（kg）"),
              "max_speed_kmh": ("tốc độ tối đa (km/h)", "maximum speed (km/h)", "最高速度（km/h）"),
              "processing_days": ("thời gian xử lý (ngày)", "processing time (days)", "処理日数"),
              "temperature_c": ("nhiệt độ bảo quản (°C)", "storage temperature (°C)", "保管温度（℃）"),
              "cutoff_hour": ("giờ chốt xuất kho", "dispatch cutoff hour", "出荷締め時刻"),
              "report_within_hours": ("thời hạn báo cáo (giờ)", "reporting deadline (hours)", "報告期限（時間）"),
              "label_retention_days": ("thời gian lưu nhãn (ngày)", "label retention (days)", "ラベル保管期間（日）"),
              "training_hours": ("số giờ đào tạo", "training hours", "研修時間")}
picks = [s for s in S if s["status"] == "stable" and s["department"] == "Warehouse"][:4] + [s for s in C if active(s)][:2] + [s for s in PR if s["status"] == "stable"][:2]
for n, s in enumerate(picks):
    lang = langs[n % 3]
    key, val = next(iter(s["facts"].items()))
    name = s["topic_names"][LI[lang]]
    if s["kind"] == "SOP":
        lab = FACT_LABEL[key][LI[lang]]
        text = {"vi": f"Theo quy trình {name} của {DEPT[s['department']][0]}, {lab} được quy định là bao nhiêu?",
                "en": f"In the {DEPT[s['department']][1]} {name} SOP, what is the required {lab}?",
                "ja": f"{DEPT[s['department']][2]}の{name}の手順で、{lab}はいくつと定められていますか？"}[lang]
    elif s["kind"] == "Contract":
        text = {"vi": f"Trong {name} với {s['customer']} ký tháng {s['effective_from'][:7]}, điều khoản thanh toán là bao nhiêu ngày?",
                "en": f"What are the payment terms (days) in the {name} with {s['customer']} starting {s['effective_from'][:7]}?",
                "ja": f"{s['customer']}との{s['effective_from'][:7]}開始の{name}の支払条件は何日ですか？"}[lang]
    else:
        text = {"vi": f"Model {s['model']} ({name}) có công suất bao nhiêu W?", "en": f"What is the power rating (W) of model {s['model']} ({name})?",
                "ja": f"モデル{s['model']}（{name}）の消費電力は何Wですか？"}[lang]
    q("content", lang, text, {"value": val, "doc": s["id"]}, "value")

(ROOT / f"questions{SUF}.json").write_text(json.dumps(Q, ensure_ascii=False, indent=1))
from collections import Counter
print(len(Q), "questions", Counter(x["cat"] for x in Q), Counter(x["lang"] for x in Q))
