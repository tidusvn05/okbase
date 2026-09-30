---
type: Support Article
title: Partner API rate limits
description: The Hikari partner REST API allows 600 requests per minute per API key and a burst of 50 requests per second.
lang: en
---
# Partner API rate limits

The Hikari partner REST API allows 600 requests per minute per API key and a burst of 50 requests per second. When the limit is exceeded the API returns HTTP 429 with a Retry-After header indicating how many seconds to wait. Clients should implement exponential backoff. Enterprise partners can request a higher quota through their account manager. Webhook deliveries do not count toward the limit.
