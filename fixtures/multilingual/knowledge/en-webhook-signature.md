---
type: Support Article
title: Verifying webhook signatures
description: Every webhook we send includes an X-Hikari-Signature header so you can confirm the request really came from us.
lang: en
---
# Verifying webhook signatures

Every webhook we send includes an X-Hikari-Signature header so you can confirm the request really came from us. The value is an HMAC-SHA256 hash of the raw request body, computed with your webhook signing secret and encoded in hex. Compute the same hash on your side and compare the two with a constant-time function. The X-Hikari-Timestamp header lets you reject requests older than 5 minutes to prevent replay attacks. Signing secrets are separate from API keys and can be rotated in the partner portal; the old secret stays valid for 24 hours.
