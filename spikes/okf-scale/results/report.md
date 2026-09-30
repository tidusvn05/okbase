| Bundle | Cách | Đúng | Facts TB | Nguồn gold | p50 / p90 | Lượt | Tool call | Token vào TB | Chi phí TB | Lỗi |
|---|---|---|---|---|---|---|---|---|---|---|
| S | F full-context (không tool) | **100%** | 100% | 100% | 4.6s / 6.6s | 2.0 | 0.0 | 89,310 | $0.034 | 0 |
| S | D qobot: top-6 + catalog + tool | **97%** | 99% | 100% | 6.7s / 8.8s | 2.2 | 0.1 | 9,877 | $0.020 | 0 |
| S | D2 = D + quy tắc kiểm tra | **100%** | 100% | 100% | 6.3s / 8.2s | 2.1 | 0.1 | 9,923 | $0.020 | 0 |
| S | I = D không catalog | **100%** | 100% | 100% | 6.8s / 8.8s | 2.3 | 0.3 | 9,027 | $0.020 | 0 |
| S | A chỉ tool (search/get/grep/list) | **100%** | 100% | 100% | 10.2s / 11.3s | 5.5 | 3.5 | 17,015 | $0.021 | 0 |
| S | E agent + thư mục (Read/Grep/Glob) | **100%** | 100% | 100% | 7.1s / 10.1s | 4.1 | 2.1 | 17,704 | $0.019 | 0 |
| S | H RAG thuần (top-6, không tool) | **97%** | 97% | 100% | 4.3s / 5.0s | 2.0 | 0.0 | 6,422 | $0.018 | 0 |
| S | G duyệt cây index.md (không embedding) | **97%** | 98% | 100% | 7.0s / 9.2s | 4.9 | 2.9 | 15,732 | $0.019 | 0 |
| S | G2 lexical: index.md + kb_grep mạnh (không embedding) | **100%** | 100% | 100% | 6.5s / 8.3s | 4.3 | 2.3 | 15,145 | $0.017 | 0 |
| M | F full-context (không tool) | **100%** | 100% | 100% | 4.6s / 11.7s | 2.0 | 0.0 | 219,726 | $0.104 | 0 |
| M | D qobot: top-6 + catalog + tool | **100%** | 98% | 100% | 6.8s / 9.5s | 2.1 | 0.1 | 10,708 | $0.018 | 0 |
| M | D2 = D + quy tắc kiểm tra | **100%** | 100% | 100% | 6.4s / 7.8s | 2.1 | 0.1 | 11,332 | $0.019 | 0 |
| M | I = D không catalog | **100%** | 100% | 100% | 6.8s / 9.3s | 2.1 | 0.1 | 7,664 | $0.016 | 0 |
| M | A chỉ tool (search/get/grep/list) | **97%** | 98% | 100% | 10.2s / 13.1s | 5.5 | 3.5 | 18,309 | $0.022 | 0 |
| M | E agent + thư mục (Read/Grep/Glob) | **100%** | 100% | 100% | 7.4s / 9.8s | 4.2 | 2.2 | 17,731 | $0.018 | 0 |
| M | H RAG thuần (top-6, không tool) | **97%** | 95% | 100% | 4.2s / 5.4s | 2.0 | 0.0 | 6,048 | $0.016 | 0 |
| M | G duyệt cây index.md (không embedding) | **93%** | 97% | 100% | 6.9s / 8.7s | 5.0 | 3.0 | 15,888 | $0.019 | 0 |
| M | G2 lexical: index.md + kb_grep mạnh (không embedding) | **100%** | 100% | 100% | 6.2s / 9.1s | 4.5 | 2.5 | 15,821 | $0.018 | 0 |
| L | D qobot: top-6 + catalog + tool | **100%** | 99% | 100% | 7.1s / 16.2s | 2.2 | 0.2 | 23,688 | $0.023 | 0 |
| L | D2 = D + quy tắc kiểm tra | **97%** | 99% | 100% | 6.4s / 8.0s | 2.2 | 0.2 | 24,757 | $0.023 | 0 |
| L | I = D không catalog | **93%** | 97% | 100% | 7.1s / 14.9s | 2.2 | 0.2 | 8,005 | $0.018 | 0 |
| L | A chỉ tool (search/get/grep/list) | **90%** | 96% | 93% | 10.3s / 19.4s | 5.1 | 3.1 | 16,839 | $0.019 | 0 |
| L | E agent + thư mục (Read/Grep/Glob) | **97%** | 99% | 90% | 6.9s / 8.5s | 4.4 | 2.4 | 19,070 | $0.022 | 0 |
| L | H RAG thuần (top-6, không tool) | **97%** | 97% | 100% | 4.7s / 7.8s | 2.0 | 0.0 | 6,129 | $0.017 | 0 |
| L | G duyệt cây index.md (không embedding) | **90%** | 96% | 97% | 7.2s / 9.4s | 4.8 | 2.8 | 17,827 | $0.024 | 0 |
| L | G2 lexical: index.md + kb_grep mạnh (không embedding) | **93%** | 98% | 87% | 6.3s / 8.3s | 4.5 | 2.5 | 18,775 | $0.021 | 0 |
| XL | D qobot: top-6 + catalog + tool | **93%** | 97% | 87% | 6.9s / 9.2s | 2.0 | 0.0 | 8,030 | $0.018 | 0 |
| XL | D2 = D + quy tắc kiểm tra | **87%** | 93% | 80% | 6.6s / 7.5s | 2.2 | 0.2 | 9,583 | $0.019 | 0 |
| XL | I = D không catalog | **93%** | 98% | 83% | 6.9s / 8.8s | 2.0 | 0.0 | 7,030 | $0.016 | 0 |
| XL | A chỉ tool (search/get/grep/list) | **93%** | 97% | 87% | 10.2s / 12.8s | 5.1 | 3.1 | 16,614 | $0.021 | 0 |
| XL | E agent + thư mục (Read/Grep/Glob) | **93%** | 98% | 83% | 7.0s / 9.9s | 4.7 | 2.7 | 22,181 | $0.026 | 0 |
| XL | H RAG thuần (top-6, không tool) | **90%** | 94% | 80% | 4.7s / 5.4s | 2.0 | 0.0 | 6,164 | $0.016 | 0 |
| XL | G duyệt cây index.md (không embedding) | **90%** | 95% | 90% | 7.1s / 9.0s | 4.6 | 2.6 | 21,191 | $0.028 | 0 |
| XL | G2 lexical: index.md + kb_grep mạnh (không embedding) | **90%** | 97% | 83% | 7.4s / 10.2s | 4.4 | 2.4 | 19,175 | $0.022 | 0 |

| Cách | vi | en | ja |
|---|---|---|---|
| F full-context (không tool) | 100% | 100% | 100% |
| D qobot: top-6 + catalog + tool | 98% | 95% | 100% |
| D2 = D + quy tắc kiểm tra | 98% | 92% | 98% |
| I = D không catalog | 98% | 92% | 100% |
| A chỉ tool (search/get/grep/list) | 90% | 95% | 100% |
| E agent + thư mục (Read/Grep/Glob) | 100% | 92% | 100% |
| H RAG thuần (top-6, không tool) | 98% | 88% | 100% |
| G duyệt cây index.md (không embedding) | 85% | 92% | 100% |
| G2 lexical: index.md + kb_grep mạnh (không embedding) | 95% | 92% | 100% |

Sai (theo bundle/cách):
- S/D q28 [en] src=['tools/reactions'] — Omits allowlist option; only off and all mentioned.
- S/G q18 [vi] src=['gateway/multiple-gateways'] — 120 gap correct; derived ports +108 vs base+110 not stated
- S/H q28 [en] src=['tools/reactions'] — Said don't know; no facts given.
- M/A q18 [vi] src=['gateway/multiple-gateways'] — Says 120 spacing but omits base+110 derived port range.
- M/G q15 [vi] src=['concepts/memory-honcho'] — Missing 'LLM-powered Q&A' framing; only says asks about user.
- M/G q18 [vi] src=['gateway/multiple-gateways'] — Says 120 gap but omits base+110 derived port reach.
- M/H q28 [en] src=['tools/reactions'] — Said don't know; no facts provided.
- L/A q6 [vi] src=['gateway/security/tool-permissions', 'help/faq/chat-commands-and-stopping'] — Restart not confirmed as unnecessary; hedged and conditional.
- L/A q18 [vi] src=['gateway/multiple-gateways', 'gateway/config-gateway'] — Gives 120 spacing but omits base+110 reach.
- L/A q22 [en] src=['gateway/telemetry', 'gateway/config-observability'] — Missing the no prompts/messages/keys/paths fact; adds request surface.
- L/D2 q22 [en] src=['gateway/telemetry', 'help/faq/what-is-openclaw'] — Omits fact that no prompts, messages, keys, or paths are sent.
- L/E q22 [en] src=['gateway/telemetry'] — Missing statement that no prompts/messages/keys/paths are sent.
- L/G q10 [en] src=['providers/lmstudio'] — Doesn't clearly state preload is enabled by default.
- L/G q18 [vi] src=['gateway/multiple-gateways'] — Has 120 spacing but omits base+110 fact.
- L/G q22 [en] src=['gateway/telemetry', 'help/faq/what-is-openclaw'] — Missing statement that no prompts/messages/keys/paths are sent
- L/G2 q6 [vi] src=['help/faq/chat-commands-and-stopping', 'gateway/security/tool-permissions'] — Hedges on restart; says restart if still failing, not definitive no.
- L/G2 q22 [en] src=['gateway/telemetry'] — Missing statement that no prompts/messages/keys/paths are sent.
- L/H q28 [en] src=['channels/signal', 'tools/reactions'] — Said don't know; missed default 'own'.
- L/I q10 [en] src=['providers/lmstudio'] — Missing that preload is enabled by default.
- L/I q22 [en] src=['gateway/telemetry', 'help/faq/what-is-openclaw'] — Omits that no prompts/messages/keys/paths are sent.
- XL/A q18 [vi] src=['gateway/multiple-gateways'] — 120 spacing stated; base+110 fact missing.
- XL/A q22 [en] src=['gateway/telemetry', 'gateway/config-runtime'] — Omits the fact that no prompts, messages, keys, or paths are sent.
- XL/D q9 [vi] src=['plugins/codex-harness/troubleshooting', 'plugins/codex-harness-runtime/recovery'] — Says no broadening; misses host load check and retry remaining registrations.
- XL/D q22 [en] src=['gateway/config-runtime', 'gateway/telemetry'] — Omits explicit statement that no prompts/messages/keys/paths sent.
- XL/D2 q9 [vi] src=['plugins/codex-harness/troubleshooting'] — Says no broaden permissions; lacks check host load/logs, retry remaining.
- XL/D2 q22 [en] src=['gateway/config-runtime', 'gateway/telemetry'] — Omits that no prompts, messages, keys, paths are sent.
- XL/D2 q23 [ja] src=['gateway/security/trust-model', 'start/teams'] — Omits 'share only with mutually trusting people' framing partially; mentions trust team
- XL/D2 q28 [en] src=['channels/signal', 'gateway/config-channels/personal-messaging'] — Default own and options given, but hedged on what own does.
- XL/E q22 [en] src=['gateway/telemetry'] — Missing statement that no prompts, messages, keys, paths are sent.
- XL/E q28 [en] src=['channels/signal'] — Default own and options listed, but lacks bot-message-only emission detail.
- XL/G q9 [vi] src=['cli/triage'] — Says don't retry; contradicts retry guidance; omits host load check.
- XL/G q18 [vi] src=['gateway/multiple-gateways'] — Missing base+110 derived port reach fact
- XL/G q22 [en] src=['gateway/telemetry', 'install/updating/automatic-updates'] — Missing 'no prompts, messages, keys, paths sent'; adds location claims.
- XL/G2 q21 [vi] src=['tools/exa-search'] — Missing endpoint being part of the cache key.
- XL/G2 q22 [en] src=['gateway/telemetry'] — Omits that no prompts, messages, keys, paths are sent.
- XL/G2 q28 [en] src=['channels/signal'] — Doesn't state own means reactions to bot's messages.
- XL/H q9 [vi] src=['plugins/codex-harness/troubleshooting'] — Mentions logs, but says retry differently; misses key points partially.
- XL/H q22 [en] src=['gateway/config-runtime', 'gateway/telemetry'] — Doesn't state no prompts/messages/keys/paths are sent.
- XL/H q28 [en] src=['gateway/config-channels/personal-messaging', 'channels/signal'] — Default own and other options right, but omits what own does.
- XL/I q9 [vi] src=['plugins/codex-harness/troubleshooting'] — No retry to finish registrations; host load not clearly stated.
- XL/I q22 [en] src=['gateway/config-runtime', 'gateway/telemetry'] — Missing explicit statement that no prompts/messages/keys/paths sent.