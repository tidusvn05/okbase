| | K0 | K1 | I1 |
|---|---|---|---|
| content | 8/8 | 8/8 | 8/8 |
| count | 8/8 | 8/8 | 8/8 |
| facet | 4/6 | 4/6 | 4/6 |
| list | 8/8 | 7/8 | 8/8 |
| sheet | 10/10 | 10/10 | 10/10 |
| version | 8/8 | 8/8 | 8/8 |
| **all** | **46/48** | **45/48** | **46/48** |
| mean cost | $0.0431 | $0.0439 | $0.0440 |
| mean input tokens | 34,920 | 33,929 | 34,592 |
| mean turns | 4.2 | 4.1 | 4.1 |
| mean wall time | 10.3 s | 11.1 s | 11.6 s |
| kb_query used (runs) | 31 | 32 | 32 |
| data_query on sheet questions | 10 | 9 | 10 |
| errors | 0 | 0 | 0 |

Missed:
- K0 q28 (facet): Gave 128 customers, only partial list; expected 122 named set
- K0 q29 (facet): Lists only 4 of 18 expected draft policies
- K1 q5 (list): missing=['products/air-purifier/hk-ai1425-1999', 'products/air-purifier/hk-ai2031-2549', 'products/air-purifier/hk-ai3280-1575', 'products/air-purifier/hk-ai435
- K1 q28 (facet): Says 127 customers; expected list has 123 entries
- K1 q29 (facet): Gave only 4; expected 18 draft policies
- I1 q28 (facet): Says 128 but lists 4+124; expected set has 4+... mismatch count; ACME-containing set differs
- I1 q29 (facet): Lists only 4 of 18 draft policies
