---
type: Type Schema
title: "Memory"
description: "Fields used to organize Memory records in this knowledge base."
fields: {"memory_kind":{"type":"string","required":true,"enum":["preference","fact","decision"]},"observed_at":{"type":"date","required":true},"owner":"string","project_id":"string","review_after":"date"}
---

Extend these fields for your own Memory records; keep OKF status separate from workflow state.
