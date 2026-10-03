| Model | Correct | Key facts | vi / en / ja | Tool calls (mean) | Shell calls | Input tokens (mean) | Wall (mean) |
|---|---|---|---|---|---|---|---|
| gpt-6-luna | 21/30 (70%) | 88% | 8/10 / 6/10 / 7/10 | 2.9 | 0 | 85,677 | 14 s |
  - missed q1 (en): Omits one Graph lookup per HTML activity detail.
  - missed q2 (ja): Doesn't mention dedup by bridge event id.
  - missed q4 (en): Omits that default error is terminal for whole chain.
  - missed q8 (ja): Missing boot ID and start ticks detail.
  - missed q21 (vi): Missing endpoint being part of cache key.
  - missed q22 (en): Misses 'no prompts/messages/keys/paths sent'; adds request surface.
  - missed q26 (ja): Doesn't mention DashScope's accepted default.
  - missed q27 (vi): Missing default is international endpoint.
  - missed q28 (en): Missing other options off, all, allowlist.
| gpt-6-luna+search | 21/30 (70%) | 88% | 6/10 / 7/10 / 8/10 | 1.9 | 0 | 65,280 | 14 s |
  - missed q2 (ja): Omits dedup by bridge event id.
  - missed q3 (vi): Missing statement that final text isn't auto-sent
  - missed q8 (ja): Missing boot ID and start ticks.
  - missed q10 (en): Missing that preload is enabled by default.
  - missed q12 (vi): Said per-peer, not per-channel-peer; multi-account correct.
  - missed q21 (vi): Missing endpoint being part of cache key.
  - missed q22 (en): Missing explicit statement that no prompts/messages/keys/paths are sent.
  - missed q27 (vi): Missing default international endpoint.
  - missed q28 (en): Missing other options off, all, allowlist.
| gpt-6.1-sol | 26/30 (87%) | 94% | 9/10 / 7/10 / 10/10 | 2.4 | 1 | 72,900 | 18 s |
  - missed q10 (en): Gives preload false but doesn't clearly state preload default via native load endpoint; borderline.
  - missed q22 (en): Doesn't state no prompts/messages/keys/paths are sent.
  - missed q27 (vi): Missing that default is international endpoint.
  - missed q28 (en): Omits other options: off, all, allowlist.
| gpt-6.1-sol+search | 26/30 (87%) | 94% | 9/10 / 8/10 / 9/10 | 2.5 | 8 | 78,208 | 19 s |
  - missed q22 (en): Omits no prompts/messages/keys/paths sent fact
  - missed q26 (ja): Says 5 seconds but omits DashScope default attribution.
  - missed q27 (vi): Omits default international endpoint
  - missed q28 (en): Missing other options off, all, allowlist.
