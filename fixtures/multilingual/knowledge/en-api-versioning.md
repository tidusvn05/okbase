---
type: Support Article
title: API versioning and deprecation
description: The partner API is versioned in the URL path, for example /v2/orders.
lang: en
---
# API versioning and deprecation

The partner API is versioned in the URL path, for example /v2/orders. A new major version is released only for breaking changes, and each major version remains supported for at least 18 months after its successor goes live. Deprecated versions return a Sunset header with the shutdown date, and partners receive email reminders 6 months and 1 month beforehand. Additive changes, such as new optional fields, are shipped to the current version without notice, so clients must ignore unknown fields. Version 1 will be switched off on 31 March 2027.
