---
type: SOP
title: "SOP Backup Policy – Hikari Home (VN & JP) – Draft v2"
description: "Draft proposal for backup schedules and a 90-day retention period, pending approval."
tags: [it, security, sop]
status: draft
lang: en
department: IT
updated: 2026-06-15
generated: {by: qobot-spike/0.1, at: 2026-06-15T09:00:00Z}
---
## Status

This document is a draft proposal pending approval by the IT Steering Committee. It was last updated on 15 June 2026 and is not yet in force. The current practice remains in effect until the proposal is formally approved and an effective date is announced.

## Proposed Backup Rules

- Production databases (ERP, CRM, order management) are backed up daily, with incremental snapshots every 4 hours.
- File servers and shared drives are backed up nightly.
- One copy is kept on-site and one encrypted copy is stored in a cloud region separate from the primary data center, covering both the Vietnam and Japan operations.

## Retention and Testing

The proposal sets a backup retention period of 90 days. Backups older than 90 days are deleted automatically, unless a legal hold applies.

Restore tests would be run every quarter on a sample of critical systems, and the results reported to the IT manager. Failed backup jobs must be investigated within one business day.

## Next Steps

Department heads are asked to send feedback on retention needs to IT before the review meeting. Once approved, the final version will replace this draft and be announced to all staff.
