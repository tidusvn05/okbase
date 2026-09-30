---
type: Type Schema
title: Policy
description: Fields every Policy concept must carry.
fields:
  owner: {type: string, required: true}
  region: {type: string, enum: [VN, JP]}
  effective_from: date
---

Policies need an owner; region is VN or JP.
