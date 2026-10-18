---
type: Type Schema
title: "Task"
description: "Fields used to organize Task records in this knowledge base."
fields: {"due":"date","owner":{"type":"string","required":true},"priority":{"type":"string","enum":["high","normal","low"]},"project_id":{"type":"string","required":true},"task_status":{"type":"string","required":true,"enum":["todo","in_progress","blocked","done","cancelled"]}}
---

Extend these fields for your own Task records; keep OKF status separate from workflow state.
