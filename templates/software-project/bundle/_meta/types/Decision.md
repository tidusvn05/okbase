---
type: Type Schema
title: "Decision"
description: "Fields used to organize Decision records in this knowledge base."
fields: {"decision_status":{"type":"string","required":true,"enum":["proposed","accepted","superseded"]},"owner":{"type":"string","required":true},"project_id":"string"}
---

Extend these fields for your own Decision records; keep OKF status separate from workflow state.
