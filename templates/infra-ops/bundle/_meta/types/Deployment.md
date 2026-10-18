---
type: Type Schema
title: "Deployment"
description: "Fields used to organize Deployment records in this knowledge base."
fields: {"deployment_status":{"type":"string","required":true,"enum":["planned","deployed","rolled_back"]},"environment":{"type":"string","required":true},"owner":"string","project_id":"string","service_id":{"type":"string","required":true},"version":{"type":"string","required":true}}
---

Extend these fields for your own Deployment records; keep OKF status separate from workflow state.
