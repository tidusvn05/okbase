---
type: Type Schema
title: "Risk"
description: "Fields used to organize Risk records in this knowledge base."
fields: {"owner":{"type":"string","required":true},"project_id":"string","risk_status":{"type":"string","required":true,"enum":["open","mitigating","accepted","closed"]}}
---

Extend these fields for your own Risk records; keep OKF status separate from workflow state.
