| Nhóm câu hỏi (số câu) | E | V | Q | QD | E | V | QD |
|---|---|---|---|---|---|---|---|
| Liệt kê theo bộ lọc (8) | 8/8 | 8/8 | 8/8 | 8/8 | 7/8 | 8/8 | 8/8 |
| Chi tiết theo bản hiện hành (8) | 8/8 | 8/8 | 8/8 | 8/8 | 8/8 | 8/8 | 8/8 |
| Đếm / tổng theo metadata (8) | 8/8 | 8/8 | 8/8 | 8/8 | 8/8 | 8/8 | 8/8 |
| Khám phá (phòng ban/khách hàng nào…) (6) | 6/6 | 6/6 | 6/6 | 6/6 | 4/6 | 5/6 | 5/6 |
| Số liệu trên sheet (10) | 10/10 | 9/10 | 10/10 | 10/10 | 9/10 | 8/10 | 10/10 |
| Nội dung thường (đối chứng) (8) | 8/8 | 8/8 | 8/8 | 8/8 | 8/8 | 8/8 | 7/8 |
| **Tổng** | **100%** | **98%** | **100%** | **100%** | **92%** | **94%** | **96%** |

| Cách | Đúng | p50 / p90 | Lượt | Tool call | Token vào | Chi phí TB | List: precision / recall |
|---|---|---|---|---|---|---|---|
| E agent + thư mục (Read/Grep/Glob) | 100% | 7.9s / 10.8s | 4.3 | 2.3 | 18,036 | $0.024 | 100% / 100% |
| V = E + _views/ (trang liệt kê theo metadata) | 98% | 7.4s / 10.6s | 4.1 | 2.1 | 17,828 | $0.026 | 100% / 100% |
| Q = E + kb_query (lọc metadata) | 100% | 7.5s / 11.7s | 4.0 | 1.9 | 19,945 | $0.022 | 100% / 100% |
| QD = Q + data_query (SQL trên sheet) | 100% | 6.7s / 8.6s | 3.8 | 1.8 | 19,508 | $0.017 | 100% / 100% |
| E ×20 (3.020 tài liệu, sheet ~10k dòng) | 92% | 10.3s / 26.4s | 5.4 | 3.4 | 33,210 | $0.056 | 100% / 89% |
| V ×20 | 94% | 9.6s / 36.6s | 5.1 | 3.1 | 43,784 | $0.070 | 100% / 100% |
| QD ×20 | 96% | 7.3s / 13.9s | 4.1 | 2.1 | 24,966 | $0.032 | 100% / 100% |

Câu sai:
- q7 list [en] E-x20: missing=['sop/cs/branch-02/escalation-2089', 'sop/cs/branch-04/complaint-165', 'sop/cs/branch-05/chat-handling-476', 'sop/cs/branch-08/complaint-2291', 'sop/cs/branch-09/complaint-1981', 'sop/cs/branch-09/warranty-claims-635', 'sop/cs/branch-17/call-recording-886', 'sop/cs/branch-17/ticket-triage-17', 'sop/cs/branch-18/vip-customers-2667', 'sop/cs/branch-19/call-recording-1328', 'sop/cs/branch-20/complaint-1256', 'sop/cs/branch-20/complaint-548', 'sop/cs/branch-20/warranty-claims-882', 'sop/cs/branch-21/refund-approval-1126', 'sop/cs/branch-21/warranty-claims-191', 'sop/cs/branch-22/chat-handling-551', 'sop/cs/branch-22/escalation-2157', 'sop/cs/branch-23/call-recording-1813', 'sop/cs/branch-25/chat-handling-2076', 'sop/cs/branch-25/complaint-1936', 'sop/cs/branch-25/escalation-2465', 'sop/cs/branch-26/vip-customers-1827', 'sop/cs/branch-27/call-recording-1400', 'sop/cs/branch-27/call-recording-759', 'sop/cs/branch-27/refund-approval-1716', 'sop/cs/branch-28/ticket-triage-1251', 'sop/cs/branch-29/call-recording-1111', 'sop/cs/branch-29/escalation-1541', 'sop/cs/branch-30/refund-approval-359', 'sop/cs/branch-31/ticket-triage-1705', 'sop/cs/branch-33/refund-approval-1393', 'sop/cs/branch-35/chat-handling-1360', 'sop/cs/branch-35/warranty-claims-458', 'sop/cs/branch-36/escalation-1050', 'sop/cs/branch-36/vip-customers-1095', 'sop/cs/branch-38/warranty-claims-987', 'sop/cs/branch-39/refund-approval-2728', 'sop/cs/branch-39/vip-customers-1630', 'sop/cs/branch-40/call-recording-352', 'sop/cs/branch-40/complaint-301', 'sop/cs/branch-40/warranty-claims-1202'] extra=[]
- q28 facet [en] E-x20: Answer says 127 customers; expected list has 123 names
- q29 facet [ja] E-x20: Only 4 listed; excluded 14 expected drafts
- q29 facet [ja] QD-x20: Only 4 listed; expected 18 policies
- q29 facet [ja] V-x20: Lists only 4 of 18 expected draft policies
- q33 sheet [vi] E-x20: No answer given
- q33 sheet [vi] V-x20: No answer given
- q35 sheet [ja] V-x20: Gave SKU-AI156 307; expected SKU-AI009 406 as final.
- q37 sheet [en] V: Gave two prices, hedged; didn't commit to 77,600
- q41 content [en] QD-x20: Did not give 30; listed branch variants