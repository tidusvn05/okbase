---
type: Type Schema
title: "Investigation"
description: "Fields used to organize Investigation records in this knowledge base."
fields: {"finding_kind":{"type":"string","enum":["hypothesis","confirmed","inconclusive"]},"investigation_status":{"type":"string","required":true,"enum":["open","investigating","concluded"]},"owner":{"type":"string","required":true},"project_id":"string"}
---

Extend these fields for your own Investigation records; keep OKF status separate from workflow state.
