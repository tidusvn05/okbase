---
type: Type Schema
title: "Incident"
description: "Fields used to organize Incident records in this knowledge base."
fields: {"incident_status":{"type":"string","required":true,"enum":["investigating","mitigating","resolved","closed"]},"owner":{"type":"string","required":true},"project_id":"string","service_id":{"type":"string","required":true}}
---

Extend these fields for your own Incident records; keep OKF status separate from workflow state.
