# Telemetry Report: `fifo.json`

- **Target:** 165.0 Hz (6.0606 ms/frame)
- **Frames Analysed:** 8951
- **Session Duration:** 59.92 s

## Global Pacing

| Metric             | Value     | Evaluation                  |
| ------------------ | --------- | --------------------------- |
| Avg Delivery Time  | 6.6944 ms | GOOD (Consistent Half-Rate) |
| V-Sync Multiplier  | 1.10 x    |                             |
| Jitter (IFI delta) | 1.2882 ms | STUTTERY                    |

## Phase Drift

| Metric          | Value      |
| --------------- | ---------- |
| Avg Phase Drift | -0.0265 ms |
| Max Phase Drift | +3.0297 ms |
| Drift Std Dev   | 1.8068 ms  |
| Avg Sync Score  | 47.93 %    |

## Stutter Events

- **Distinct events:** 66
- **Anomalous frames:** 901 (10.07% of session)
- **Vblanks lost:** 81

| IDX  | WORST Δ     | SZ  | MISSED | SEVERITY | RECOV. JITTER |
| ---- | ----------- | --- | ------ | -------- | ------------- |
| 2    | 26.8615 ms  | 7   | 3      | SEVERE   | 8.6503 ms     |
| 17   | 58.7367 ms  | 744 | 8      | CLUSTER  | 1.7430 ms     |
| 1510 | ~12.1194 ms | 1   | 0      | MINOR    | 3.4963 ms     |
| 1517 | 18.2198 ms  | 10  | 2      | CLUSTER  | 1.7956 ms     |
| 1541 | 12.1609 ms  | 1   | 1      | MINOR    | 1.7488 ms     |
| 1547 | 12.2636 ms  | 12  | 1      | CLUSTER  | 1.8826 ms     |
| 1575 | ~12.0886 ms | 1   | 0      | MINOR    | 0.0467 ms     |
| 1585 | 12.1810 ms  | 7   | 1      | MINOR    | 0.0399 ms     |
| 1606 | 18.2105 ms  | 7   | 2      | MINOR    | 0.0414 ms     |
| 1629 | 12.1899 ms  | 7   | 1      | MINOR    | 0.0442 ms     |
| 1649 | 12.1321 ms  | 1   | 1      | MINOR    | 0.0292 ms     |
| 1673 | 12.1886 ms  | 6   | 1      | MINOR    | 0.0360 ms     |
| 1731 | 25.0279 ms  | 1   | 3      | SEVERE   | 0.0193 ms     |
| 1748 | 25.1930 ms  | 1   | 3      | SEVERE   | 0.0536 ms     |
| 1766 | 25.6139 ms  | 1   | 3      | SEVERE   | 0.0262 ms     |
| 1783 | 24.9316 ms  | 1   | 3      | SEVERE   | 0.0431 ms     |
| 1835 | 12.2217 ms  | 7   | 1      | MINOR    | 0.0341 ms     |
| 1983 | 12.3281 ms  | 7   | 1      | MINOR    | 0.0416 ms     |
| 2012 | 12.1928 ms  | 7   | 1      | MINOR    | 0.1341 ms     |
| 2169 | 12.2074 ms  | 5   | 1      | MINOR    | 0.0779 ms     |
| 2313 | ~12.1173 ms | 1   | 0      | MINOR    | 0.0301 ms     |
| 2328 | 12.1918 ms  | 6   | 1      | MINOR    | 0.0482 ms     |
| 2347 | 12.1680 ms  | 1   | 1      | MINOR    | 0.0301 ms     |
| 2361 | 12.1920 ms  | 7   | 1      | MINOR    | 0.0383 ms     |
| 2477 | 12.2062 ms  | 6   | 1      | MINOR    | 0.0527 ms     |
| 2496 | 12.1601 ms  | 1   | 1      | MINOR    | 0.0534 ms     |
| 2516 | 12.1946 ms  | 6   | 1      | MINOR    | 0.0275 ms     |
| 2794 | 25.0764 ms  | 1   | 3      | SEVERE   | 0.0610 ms     |
| 2804 | 12.1278 ms  | 1   | 1      | MINOR    | 0.0989 ms     |
| 2823 | 12.1922 ms  | 1   | 1      | MINOR    | 7.3332 ms     |
| 2830 | 31.1091 ms  | 1   | 4      | SEVERE   | 2.2829 ms     |
| 2836 | 12.1924 ms  | 1   | 1      | MINOR    | 1.7935 ms     |
| 2842 | 12.2106 ms  | 1   | 1      | MINOR    | 0.0329 ms     |
| 2852 | 12.1555 ms  | 1   | 1      | MINOR    | 0.0312 ms     |
| 3646 | 12.1366 ms  | 1   | 1      | MINOR    | 0.0362 ms     |
| 3679 | 12.1402 ms  | 1   | 1      | MINOR    | 0.0279 ms     |
| 3788 | 12.1510 ms  | 1   | 1      | MINOR    | 0.0410 ms     |
| 3820 | 12.1390 ms  | 1   | 1      | MINOR    | 0.0292 ms     |
| 4167 | ~12.1104 ms | 1   | 0      | MINOR    | 0.0337 ms     |
| 4204 | 12.1695 ms  | 1   | 1      | MINOR    | 0.0528 ms     |
| 5039 | 12.1445 ms  | 1   | 1      | MINOR    | 0.0341 ms     |
| 5060 | 12.1520 ms  | 1   | 1      | MINOR    | 0.0522 ms     |
| 5083 | 12.1776 ms  | 1   | 1      | MINOR    | 0.0412 ms     |
| 5396 | 12.1222 ms  | 1   | 1      | MINOR    | 0.0609 ms     |
| 5410 | 12.1677 ms  | 1   | 1      | MINOR    | 0.0400 ms     |
| 5422 | ~12.0881 ms | 1   | 0      | MINOR    | 0.0393 ms     |
| 5438 | 12.1460 ms  | 1   | 1      | MINOR    | 0.0459 ms     |
| 5564 | ~12.1182 ms | 1   | 0      | MINOR    | 0.0614 ms     |
| 5585 | 12.1878 ms  | 1   | 1      | MINOR    | 0.0177 ms     |
| 5618 | 12.1459 ms  | 1   | 1      | MINOR    | 0.0380 ms     |
| 5679 | 12.1842 ms  | 1   | 1      | MINOR    | 0.0196 ms     |
| 6550 | 12.1291 ms  | 1   | 1      | MINOR    | 0.0308 ms     |
| 6736 | ~12.1168 ms | 1   | 0      | MINOR    | 0.0321 ms     |
| 6755 | 12.1632 ms  | 1   | 1      | MINOR    | 0.0655 ms     |
| 6775 | 12.1330 ms  | 1   | 1      | MINOR    | 0.0238 ms     |
| 6797 | 12.1432 ms  | 1   | 1      | MINOR    | 0.0293 ms     |
| 6903 | ~12.1095 ms | 1   | 0      | MINOR    | 0.0242 ms     |
| 6918 | 12.1361 ms  | 1   | 1      | MINOR    | 0.1022 ms     |
| 6932 | 12.1297 ms  | 1   | 1      | MINOR    | 0.0415 ms     |
| 6950 | 12.1260 ms  | 1   | 1      | MINOR    | 0.0410 ms     |
| 7267 | 12.1466 ms  | 1   | 1      | MINOR    | 0.0465 ms     |
| 8129 | 12.1548 ms  | 1   | 1      | MINOR    | 0.0196 ms     |
| 8189 | ~12.1093 ms | 1   | 0      | MINOR    | 0.1097 ms     |
| 8328 | 12.1253 ms  | 1   | 1      | MINOR    | 0.0368 ms     |
| 8553 | 12.1676 ms  | 1   | 1      | MINOR    | 0.0277 ms     |
| 8697 | 12.1391 ms  | 1   | 1      | MINOR    | 0.0435 ms     |

> `~` = fractional vblank slip (1.25×–2× ideal, no whole vblank missed)

## Session Phases

_Cadence regimes, keyed on global frame index_

| #   | GLOBAL IDX | MEAN Δ    | EFF. Hz  | JITTER    |
| --- | ---------- | --------- | -------- | --------- |
| 1   | 2–1589     | 9.0915 ms | 110.0 Hz | 6.0629 ms |
| 2   | 1590–1654  | 7.9325 ms | 126.1 Hz | 3.5941 ms |
| 3   | 1655–1695  | 6.9537 ms | 143.8 Hz | 1.8576 ms |
| 4   | 1696–1786  | 6.9028 ms | 144.9 Hz | 1.7352 ms |
| 5   | 1787–1858  | 6.6558 ms | 150.2 Hz | 1.0606 ms |
| 6   | 1859–2035  | 6.5460 ms | 152.8 Hz | 0.8922 ms |
| 7   | 2036–2189  | 6.2631 ms | 159.7 Hz | 0.4364 ms |
| 8   | 2190–2384  | 6.5324 ms | 153.1 Hz | 0.8510 ms |
| 9   | 2385–2500  | 6.4319 ms | 155.5 Hz | 0.6872 ms |
| 10  | 2501–2537  | 7.0512 ms | 141.8 Hz | 1.7169 ms |
| 11  | 2538–2829  | 6.1726 ms | 162.0 Hz | 0.2586 ms |
| 12  | 2830–8950  | 6.1048 ms | 163.8 Hz | 0.1159 ms |

## Verdict

**ACCEPTABLE. Standard presentation timing.**
