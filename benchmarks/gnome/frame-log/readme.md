# Telemetry Report: `fifo.json`

- **Target:** 165.0 Hz (6.0606 ms/frame)
- **Frames Analysed:** 9743
- **Session Duration:** 59.93 s

## Global Pacing

| Metric             | Value     | Evaluation               |
| ------------------ | --------- | ------------------------ |
| Avg Delivery Time  | 6.1508 ms | PERFECT (Native Refresh) |
| V-Sync Multiplier  | 1.01 x    |                          |
| Jitter (IFI delta) | 0.2517 ms | LOCKED                   |

## Phase Drift

| Metric          | Value      |
| --------------- | ---------- |
| Avg Phase Drift | +0.0199 ms |
| Max Phase Drift | +3.0300 ms |
| Drift Std Dev   | 1.7678 ms  |
| Avg Sync Score  | 49.17 %    |

## Stutter Events

- **Distinct events:** 106
- **Anomalous frames:** 132 (1.35% of session)
- **Vblanks lost:** 97

| IDX  | WORST Δ     | SZ  | MISSED | SEVERITY | RECOV. JITTER |
| ---- | ----------- | --- | ------ | -------- | ------------- |
| 2    | 36.4930 ms  | 27  | 5      | CLUSTER  | 1.7336 ms     |
| 67   | ~12.0927 ms | 1   | 0      | MINOR    | 0.0342 ms     |
| 78   | 12.5302 ms  | 1   | 1      | MINOR    | 0.0875 ms     |
| 95   | 12.1495 ms  | 1   | 1      | MINOR    | 0.1034 ms     |
| 113  | 12.1369 ms  | 1   | 1      | MINOR    | 0.0464 ms     |
| 139  | 12.1566 ms  | 1   | 1      | MINOR    | 0.0318 ms     |
| 157  | 12.1259 ms  | 1   | 1      | MINOR    | 0.0252 ms     |
| 170  | 12.1409 ms  | 1   | 1      | MINOR    | 0.0642 ms     |
| 182  | 12.1314 ms  | 1   | 1      | MINOR    | 0.1125 ms     |
| 194  | 12.1419 ms  | 1   | 1      | MINOR    | 0.1279 ms     |
| 207  | 12.1482 ms  | 1   | 1      | MINOR    | 0.0499 ms     |
| 223  | 12.1541 ms  | 1   | 1      | MINOR    | 0.0601 ms     |
| 385  | 12.1584 ms  | 1   | 1      | MINOR    | 0.0229 ms     |
| 399  | 12.1409 ms  | 1   | 1      | MINOR    | 0.9135 ms     |
| 407  | 12.1567 ms  | 1   | 1      | MINOR    | 0.0266 ms     |
| 416  | 12.1287 ms  | 1   | 1      | MINOR    | 0.0282 ms     |
| 430  | 12.1543 ms  | 1   | 1      | MINOR    | 0.1127 ms     |
| 524  | 12.1374 ms  | 1   | 1      | MINOR    | 0.1252 ms     |
| 540  | 12.1867 ms  | 1   | 1      | MINOR    | 0.0251 ms     |
| 550  | 12.1326 ms  | 1   | 1      | MINOR    | 0.0452 ms     |
| 560  | 12.1619 ms  | 1   | 1      | MINOR    | 0.9087 ms     |
| 568  | 12.1825 ms  | 1   | 1      | MINOR    | 0.9053 ms     |
| 576  | 12.1535 ms  | 1   | 1      | MINOR    | 0.1370 ms     |
| 585  | 12.1657 ms  | 1   | 1      | MINOR    | 0.0412 ms     |
| 719  | 12.1369 ms  | 1   | 1      | MINOR    | 0.0215 ms     |
| 742  | 12.1385 ms  | 1   | 1      | MINOR    | 0.0540 ms     |
| 763  | 12.1512 ms  | 1   | 1      | MINOR    | 0.0242 ms     |
| 1080 | 12.1709 ms  | 1   | 1      | MINOR    | 0.0136 ms     |
| 1424 | 12.1460 ms  | 1   | 1      | MINOR    | 0.0671 ms     |
| 1478 | 12.1919 ms  | 1   | 1      | MINOR    | 0.0703 ms     |
| 1571 | 13.5580 ms  | 1   | 1      | MINOR    | 0.2687 ms     |
| 1782 | ~12.1015 ms | 1   | 0      | MINOR    | 0.0541 ms     |
| 1809 | 12.1938 ms  | 1   | 1      | MINOR    | 0.0428 ms     |
| 1838 | 12.2174 ms  | 1   | 1      | MINOR    | 0.0690 ms     |
| 1903 | 12.1541 ms  | 1   | 1      | MINOR    | 0.0382 ms     |
| 1920 | 12.2154 ms  | 1   | 1      | MINOR    | 0.0771 ms     |
| 1935 | ~12.1079 ms | 1   | 0      | MINOR    | 0.1192 ms     |
| 1964 | 12.1218 ms  | 1   | 1      | MINOR    | 0.0269 ms     |
| 2765 | 12.2345 ms  | 1   | 1      | MINOR    | 0.0805 ms     |
| 2833 | 12.2640 ms  | 1   | 1      | MINOR    | 0.0522 ms     |
| 3089 | 12.1996 ms  | 1   | 1      | MINOR    | 0.0574 ms     |
| 3100 | 12.1460 ms  | 1   | 1      | MINOR    | 2.1850 ms     |
| 3107 | 12.1888 ms  | 1   | 1      | MINOR    | 0.0880 ms     |
| 3116 | 12.1525 ms  | 1   | 1      | MINOR    | 0.0649 ms     |
| 3125 | 12.1454 ms  | 1   | 1      | MINOR    | 0.0418 ms     |
| 3134 | 12.2046 ms  | 1   | 1      | MINOR    | 0.1185 ms     |
| 3144 | 12.1898 ms  | 1   | 1      | MINOR    | 0.0497 ms     |
| 3257 | 12.1594 ms  | 1   | 1      | MINOR    | 0.0700 ms     |
| 3269 | ~12.1095 ms | 1   | 0      | MINOR    | 0.0636 ms     |
| 3279 | 12.1936 ms  | 1   | 1      | MINOR    | 0.0471 ms     |
| 3290 | 12.1228 ms  | 1   | 1      | MINOR    | 0.1650 ms     |
| 3305 | 12.2269 ms  | 1   | 1      | MINOR    | 0.0667 ms     |
| 3516 | 12.2004 ms  | 1   | 1      | MINOR    | 0.0751 ms     |
| 3632 | 12.1665 ms  | 1   | 1      | MINOR    | 0.0629 ms     |
| 3646 | 12.1528 ms  | 1   | 1      | MINOR    | 0.0307 ms     |
| 3658 | 12.1233 ms  | 1   | 1      | MINOR    | 0.0707 ms     |
| 4438 | ~12.0897 ms | 1   | 0      | MINOR    | 0.0726 ms     |
| 4463 | 12.1489 ms  | 1   | 1      | MINOR    | 0.5572 ms     |
| 4488 | 12.1945 ms  | 1   | 1      | MINOR    | 0.0832 ms     |
| 4535 | 12.1489 ms  | 1   | 1      | MINOR    | 0.0262 ms     |
| 4604 | 12.2060 ms  | 1   | 1      | MINOR    | 0.0648 ms     |
| 4624 | 12.1438 ms  | 1   | 1      | MINOR    | 0.0217 ms     |
| 4836 | 12.9939 ms  | 1   | 1      | MINOR    | 0.1874 ms     |
| 4952 | 12.1843 ms  | 1   | 1      | MINOR    | 0.0617 ms     |
| 4980 | 12.2006 ms  | 1   | 1      | MINOR    | 0.0691 ms     |
| 5005 | 12.1938 ms  | 1   | 1      | MINOR    | 0.0233 ms     |
| 5028 | 12.2073 ms  | 1   | 1      | MINOR    | 0.0490 ms     |
| 5832 | 12.1450 ms  | 1   | 1      | MINOR    | 0.0203 ms     |
| 5847 | ~12.0967 ms | 1   | 0      | MINOR    | 0.0397 ms     |
| 5859 | 12.1400 ms  | 1   | 1      | MINOR    | 0.1115 ms     |
| 5870 | 12.1564 ms  | 1   | 1      | MINOR    | 0.0609 ms     |
| 5887 | 12.1495 ms  | 1   | 1      | MINOR    | 0.0680 ms     |
| 6012 | 12.1332 ms  | 1   | 1      | MINOR    | 0.0368 ms     |
| 6191 | 12.2158 ms  | 1   | 1      | MINOR    | 0.0462 ms     |
| 6202 | 12.1215 ms  | 1   | 1      | MINOR    | 0.0565 ms     |
| 6212 | ~12.0938 ms | 1   | 0      | MINOR    | 0.9190 ms     |
| 6220 | ~12.1060 ms | 1   | 0      | MINOR    | 0.1227 ms     |
| 6230 | 12.2227 ms  | 1   | 1      | MINOR    | 0.0581 ms     |
| 6242 | 12.1911 ms  | 1   | 1      | MINOR    | 0.0289 ms     |
| 6355 | 12.2218 ms  | 1   | 1      | MINOR    | 0.1326 ms     |
| 6371 | 12.1488 ms  | 1   | 1      | MINOR    | 0.0948 ms     |
| 6385 | 12.1885 ms  | 1   | 1      | MINOR    | 0.0310 ms     |
| 6407 | 12.2325 ms  | 1   | 1      | MINOR    | 0.0720 ms     |
| 6849 | 12.2607 ms  | 1   | 1      | MINOR    | 0.1726 ms     |
| 7329 | 12.1965 ms  | 1   | 1      | MINOR    | 0.1060 ms     |
| 7360 | 12.1430 ms  | 1   | 1      | MINOR    | 0.0542 ms     |
| 7523 | 12.2605 ms  | 1   | 1      | MINOR    | 0.0316 ms     |
| 7539 | ~12.0955 ms | 1   | 0      | MINOR    | 0.0646 ms     |
| 7552 | 12.2777 ms  | 1   | 1      | MINOR    | 0.0892 ms     |
| 7567 | 12.1435 ms  | 1   | 1      | MINOR    | 0.0869 ms     |
| 7581 | 12.2489 ms  | 1   | 1      | MINOR    | 0.1605 ms     |
| 7698 | 12.1243 ms  | 1   | 1      | MINOR    | 0.0450 ms     |
| 7710 | ~12.1017 ms | 1   | 0      | MINOR    | 0.0648 ms     |
| 7720 | 12.1950 ms  | 1   | 1      | MINOR    | 0.0402 ms     |
| 7732 | ~12.1091 ms | 1   | 0      | MINOR    | 0.0571 ms     |
| 7746 | 12.1707 ms  | 1   | 1      | MINOR    | 0.0578 ms     |
| 8051 | 12.1765 ms  | 1   | 1      | MINOR    | 0.0485 ms     |
| 8597 | 12.2494 ms  | 1   | 1      | MINOR    | 0.0387 ms     |
| 8772 | ~12.0566 ms | 1   | 0      | MINOR    | 0.0765 ms     |
| 8914 | ~12.0153 ms | 1   | 0      | MINOR    | 0.0632 ms     |
| 8939 | 12.2169 ms  | 1   | 1      | MINOR    | 0.0700 ms     |
| 9106 | 12.2152 ms  | 1   | 1      | MINOR    | 0.0586 ms     |
| 9329 | 12.2197 ms  | 1   | 1      | MINOR    | 0.0483 ms     |
| 9355 | 12.1471 ms  | 1   | 1      | MINOR    | 0.0864 ms     |
| 9476 | 12.1463 ms  | 1   | 1      | MINOR    | 0.0277 ms     |
| 9508 | 12.1924 ms  | 1   | 1      | MINOR    | 0.2021 ms     |

> `~` = fractional vblank slip (1.25×–2× ideal, no whole vblank missed)

## Verdict

**NATIVE PERFORMANCE. GPU is perfectly tracking the monitor.**

> NOTE: Low avg sync score is expected over long sessions, the
> fixed phase origin precesses through all vblank phases as
> sub-microsecond residuals accumulate. Per-frame drift values
> and stutter events remain accurate.
