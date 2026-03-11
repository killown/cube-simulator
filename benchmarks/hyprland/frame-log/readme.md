# Telemetry Report: `fifo.json`

- **Target:** 165.0 Hz (6.0606 ms/frame)
- **Frames Analysed:** 9696
- **Session Duration:** 59.94 s

## Global Pacing

| Metric             | Value     | Evaluation               |
| ------------------ | --------- | ------------------------ |
| Avg Delivery Time  | 6.1820 ms | PERFECT (Native Refresh) |
| V-Sync Multiplier  | 1.02 x    |                          |
| Jitter (IFI delta) | 0.2576 ms | LOCKED                   |

## Phase Drift

| Metric          | Value      |
| --------------- | ---------- |
| Avg Phase Drift | -0.0237 ms |
| Max Phase Drift | -3.0297 ms |
| Drift Std Dev   | 1.7653 ms  |
| Avg Sync Score  | 49.44 %    |

## Stutter Events

- **Distinct events:** 146
- **Anomalous frames:** 185 (1.91% of session)
- **Vblanks lost:** 114

| IDX  | WORST Δ     | SZ  | MISSED | SEVERITY | RECOV. JITTER |
| ---- | ----------- | --- | ------ | -------- | ------------- |
| 4    | 18.4517 ms  | 32  | 2      | CLUSTER  | 1.7520 ms     |
| 68   | ~12.1170 ms | 1   | 0      | MINOR    | 0.8961 ms     |
| 76   | 12.1624 ms  | 1   | 1      | MINOR    | 0.0131 ms     |
| 91   | ~12.1176 ms | 1   | 0      | MINOR    | 0.0381 ms     |
| 107  | 12.1446 ms  | 1   | 1      | MINOR    | 0.0285 ms     |
| 129  | ~12.1193 ms | 1   | 0      | MINOR    | 0.0354 ms     |
| 144  | 12.3372 ms  | 1   | 1      | MINOR    | 0.0750 ms     |
| 159  | ~12.1036 ms | 1   | 0      | MINOR    | 0.0214 ms     |
| 169  | 12.1629 ms  | 1   | 1      | MINOR    | 0.0316 ms     |
| 178  | 12.1503 ms  | 1   | 1      | MINOR    | 0.0431 ms     |
| 187  | 12.3165 ms  | 1   | 1      | MINOR    | 0.0723 ms     |
| 196  | 12.1448 ms  | 1   | 1      | MINOR    | 0.1321 ms     |
| 208  | ~12.1153 ms | 1   | 0      | MINOR    | 0.1176 ms     |
| 228  | 12.1301 ms  | 1   | 1      | MINOR    | 0.0135 ms     |
| 241  | 12.1779 ms  | 1   | 1      | MINOR    | 0.0244 ms     |
| 364  | ~12.1092 ms | 1   | 0      | MINOR    | 0.0176 ms     |
| 380  | 12.1649 ms  | 1   | 1      | MINOR    | 0.0515 ms     |
| 391  | ~12.1006 ms | 1   | 0      | MINOR    | 0.0410 ms     |
| 401  | 12.1356 ms  | 1   | 1      | MINOR    | 0.0285 ms     |
| 414  | 12.1620 ms  | 1   | 1      | MINOR    | 0.0464 ms     |
| 434  | 12.1532 ms  | 1   | 1      | MINOR    | 0.0274 ms     |
| 533  | ~12.1210 ms | 1   | 0      | MINOR    | 0.0358 ms     |
| 544  | 12.1659 ms  | 1   | 1      | MINOR    | 0.1212 ms     |
| 557  | 12.1650 ms  | 1   | 1      | MINOR    | 0.9070 ms     |
| 565  | 12.1753 ms  | 1   | 1      | MINOR    | 0.0401 ms     |
| 580  | 12.1475 ms  | 1   | 1      | MINOR    | 0.0338 ms     |
| 711  | 12.1362 ms  | 1   | 1      | MINOR    | 0.0579 ms     |
| 737  | 12.1318 ms  | 1   | 1      | MINOR    | 0.0477 ms     |
| 761  | 12.1241 ms  | 1   | 1      | MINOR    | 0.0297 ms     |
| 1076 | 12.1394 ms  | 1   | 1      | MINOR    | 0.0402 ms     |
| 1419 | ~12.0963 ms | 1   | 0      | MINOR    | 0.0225 ms     |
| 1437 | 12.1251 ms  | 1   | 1      | MINOR    | 0.0222 ms     |
| 1486 | 12.1723 ms  | 1   | 1      | MINOR    | 0.0230 ms     |
| 1523 | 12.1394 ms  | 1   | 1      | MINOR    | 0.0347 ms     |
| 1616 | 12.1553 ms  | 1   | 1      | MINOR    | 0.0281 ms     |
| 1744 | 12.1241 ms  | 1   | 1      | MINOR    | 0.0119 ms     |
| 1766 | ~12.0839 ms | 1   | 0      | MINOR    | 0.0274 ms     |
| 1780 | 12.1534 ms  | 1   | 1      | MINOR    | 0.0485 ms     |
| 1804 | ~12.1178 ms | 1   | 0      | MINOR    | 0.1416 ms     |
| 1821 | 12.1271 ms  | 2   | 1      | MINOR    | 0.0300 ms     |
| 1856 | 12.1642 ms  | 1   | 1      | MINOR    | 0.0460 ms     |
| 1890 | 12.1337 ms  | 1   | 1      | MINOR    | 0.0147 ms     |
| 1905 | 12.1883 ms  | 1   | 1      | MINOR    | 0.0509 ms     |
| 1917 | 12.1425 ms  | 1   | 1      | MINOR    | 0.0131 ms     |
| 1930 | 12.1426 ms  | 1   | 1      | MINOR    | 0.0204 ms     |
| 1939 | 12.1508 ms  | 1   | 1      | MINOR    | 0.0401 ms     |
| 1952 | 12.1291 ms  | 1   | 1      | MINOR    | 0.0465 ms     |
| 2116 | 12.1926 ms  | 1   | 1      | MINOR    | 0.0314 ms     |
| 2159 | 12.1655 ms  | 1   | 1      | MINOR    | 0.0564 ms     |
| 2219 | 12.1864 ms  | 2   | 1      | MINOR    | 0.0331 ms     |
| 2263 | ~12.0993 ms | 1   | 0      | MINOR    | 0.0317 ms     |
| 2747 | 12.3743 ms  | 1   | 1      | MINOR    | 0.0775 ms     |
| 2942 | 12.1397 ms  | 1   | 1      | MINOR    | 0.0954 ms     |
| 2953 | 12.1674 ms  | 1   | 1      | MINOR    | 0.1780 ms     |
| 2962 | 12.1291 ms  | 1   | 1      | MINOR    | 0.0340 ms     |
| 3065 | 12.1493 ms  | 1   | 1      | MINOR    | 0.0382 ms     |
| 3078 | 12.1494 ms  | 1   | 1      | MINOR    | 0.0314 ms     |
| 3088 | 12.1774 ms  | 1   | 1      | MINOR    | 0.0210 ms     |
| 3099 | 12.1538 ms  | 1   | 1      | MINOR    | 0.0498 ms     |
| 3109 | 12.1673 ms  | 1   | 1      | MINOR    | 0.0373 ms     |
| 3120 | 12.1474 ms  | 1   | 1      | MINOR    | 0.0292 ms     |
| 3132 | 12.1492 ms  | 1   | 1      | MINOR    | 0.0632 ms     |
| 3236 | 12.1505 ms  | 1   | 1      | MINOR    | 0.0402 ms     |
| 3253 | ~12.1079 ms | 1   | 0      | MINOR    | 0.0240 ms     |
| 3264 | 12.1684 ms  | 1   | 1      | MINOR    | 0.0381 ms     |
| 3279 | 12.1489 ms  | 1   | 1      | MINOR    | 0.0339 ms     |
| 3293 | 12.1565 ms  | 1   | 1      | MINOR    | 0.0306 ms     |
| 3477 | 12.1573 ms  | 2   | 1      | MINOR    | 0.0296 ms     |
| 3610 | ~12.1093 ms | 1   | 0      | MINOR    | 0.0300 ms     |
| 3626 | 12.1232 ms  | 1   | 1      | MINOR    | 0.0502 ms     |
| 3641 | 12.1675 ms  | 1   | 1      | MINOR    | 0.0267 ms     |
| 4413 | 12.1279 ms  | 1   | 1      | MINOR    | 0.0290 ms     |
| 4436 | 12.2216 ms  | 1   | 1      | MINOR    | 0.0256 ms     |
| 4460 | ~12.0969 ms | 1   | 0      | MINOR    | 0.0572 ms     |
| 4478 | ~12.1189 ms | 1   | 0      | MINOR    | 0.0395 ms     |
| 4496 | 12.1341 ms  | 1   | 1      | MINOR    | 0.0339 ms     |
| 4508 | 12.1768 ms  | 1   | 1      | MINOR    | 0.0349 ms     |
| 4566 | 12.1304 ms  | 1   | 1      | MINOR    | 0.0353 ms     |
| 4590 | 12.1494 ms  | 1   | 1      | MINOR    | 0.0256 ms     |
| 4610 | 12.1482 ms  | 1   | 1      | MINOR    | 0.1078 ms     |
| 4900 | ~12.1035 ms | 1   | 0      | MINOR    | 0.0437 ms     |
| 4933 | ~12.0857 ms | 1   | 0      | MINOR    | 0.0316 ms     |
| 4961 | 12.1242 ms  | 1   | 1      | MINOR    | 0.0525 ms     |
| 4985 | ~12.0978 ms | 1   | 0      | MINOR    | 0.4541 ms     |
| 5005 | 12.1430 ms  | 1   | 1      | MINOR    | 0.0407 ms     |
| 5167 | ~12.1209 ms | 1   | 0      | MINOR    | 0.0238 ms     |
| 5810 | 12.1582 ms  | 1   | 1      | MINOR    | 0.0309 ms     |
| 5827 | ~12.1036 ms | 1   | 0      | MINOR    | 0.0463 ms     |
| 5843 | 12.1265 ms  | 1   | 1      | MINOR    | 0.0361 ms     |
| 5860 | 12.1398 ms  | 1   | 1      | MINOR    | 0.0273 ms     |
| 5987 | 12.1478 ms  | 1   | 1      | MINOR    | 0.0504 ms     |
| 6146 | 12.1416 ms  | 2   | 1      | MINOR    | 0.0282 ms     |
| 6161 | 12.1434 ms  | 1   | 1      | MINOR    | 0.0437 ms     |
| 6177 | 12.1464 ms  | 1   | 1      | MINOR    | 0.0396 ms     |
| 6189 | 12.1369 ms  | 1   | 1      | MINOR    | 0.0202 ms     |
| 6200 | 12.1411 ms  | 1   | 1      | MINOR    | 0.0379 ms     |
| 6213 | 12.1459 ms  | 1   | 1      | MINOR    | 0.0338 ms     |
| 6249 | 12.1349 ms  | 1   | 1      | MINOR    | 0.0277 ms     |
| 6317 | 12.1245 ms  | 1   | 1      | MINOR    | 0.0260 ms     |
| 6337 | 12.1661 ms  | 1   | 1      | MINOR    | 0.0343 ms     |
| 6349 | 12.1561 ms  | 1   | 1      | MINOR    | 0.0385 ms     |
| 6369 | 12.1357 ms  | 1   | 1      | MINOR    | 0.0533 ms     |
| 6394 | 12.1618 ms  | 1   | 1      | MINOR    | 0.0439 ms     |
| 6527 | 12.1501 ms  | 1   | 1      | MINOR    | 0.0482 ms     |
| 6560 | 12.1514 ms  | 3   | 1      | MINOR    | 0.0485 ms     |
| 7070 | 12.1528 ms  | 1   | 1      | MINOR    | 0.0348 ms     |
| 7275 | 12.1809 ms  | 1   | 1      | MINOR    | 0.0240 ms     |
| 7292 | 12.1989 ms  | 1   | 1      | MINOR    | 0.0249 ms     |
| 7314 | ~12.1006 ms | 1   | 0      | MINOR    | 0.1656 ms     |
| 7331 | 12.1356 ms  | 1   | 1      | MINOR    | 0.0374 ms     |
| 7356 | 12.1409 ms  | 1   | 1      | MINOR    | 0.0268 ms     |
| 7426 | ~12.0955 ms | 1   | 0      | MINOR    | 0.0272 ms     |
| 7487 | ~12.1144 ms | 1   | 0      | MINOR    | 0.0274 ms     |
| 7502 | 12.1425 ms  | 1   | 1      | MINOR    | 0.0468 ms     |
| 7516 | ~12.0763 ms | 1   | 0      | MINOR    | 0.0292 ms     |
| 7531 | 12.1555 ms  | 1   | 1      | MINOR    | 0.0230 ms     |
| 7545 | 12.1416 ms  | 1   | 1      | MINOR    | 0.0116 ms     |
| 7554 | 12.2141 ms  | 2   | 1      | MINOR    | 0.0273 ms     |
| 7651 | 12.1707 ms  | 1   | 1      | MINOR    | 0.0467 ms     |
| 7667 | 12.1282 ms  | 1   | 1      | MINOR    | 0.0454 ms     |
| 7679 | 12.1407 ms  | 1   | 1      | MINOR    | 0.0327 ms     |
| 7690 | 12.1824 ms  | 1   | 1      | MINOR    | 0.0197 ms     |
| 7705 | 12.1681 ms  | 1   | 1      | MINOR    | 0.0155 ms     |
| 7725 | ~12.0988 ms | 1   | 0      | MINOR    | 0.0355 ms     |
| 7838 | 12.1628 ms  | 1   | 1      | MINOR    | 0.0202 ms     |
| 7879 | 12.1523 ms  | 1   | 1      | MINOR    | 0.0233 ms     |
| 7990 | ~12.0949 ms | 1   | 0      | MINOR    | 0.0292 ms     |
| 8016 | 12.1532 ms  | 1   | 1      | MINOR    | 0.0284 ms     |
| 8196 | 12.1441 ms  | 1   | 1      | MINOR    | 0.0206 ms     |
| 8542 | 12.4745 ms  | 1   | 1      | MINOR    | 0.0710 ms     |
| 8692 | ~12.1124 ms | 1   | 0      | MINOR    | 0.0458 ms     |
| 8709 | 12.1731 ms  | 2   | 1      | MINOR    | 0.0304 ms     |
| 8745 | ~12.0963 ms | 1   | 0      | MINOR    | 0.0567 ms     |
| 8852 | 12.1904 ms  | 1   | 1      | MINOR    | 0.0231 ms     |
| 8878 | 12.1455 ms  | 1   | 1      | MINOR    | 0.0469 ms     |
| 8907 | 12.1446 ms  | 1   | 1      | MINOR    | 0.0295 ms     |
| 8951 | 12.1413 ms  | 1   | 1      | MINOR    | 0.0233 ms     |
| 9008 | 12.3607 ms  | 1   | 1      | MINOR    | 0.0525 ms     |
| 9048 | ~11.9315 ms | 1   | 0      | MINOR    | 0.0341 ms     |
| 9063 | ~12.1154 ms | 1   | 0      | MINOR    | 0.0099 ms     |
| 9269 | 12.1682 ms  | 1   | 1      | MINOR    | 0.0436 ms     |
| 9285 | 12.1342 ms  | 1   | 1      | MINOR    | 0.0208 ms     |
| 9298 | ~12.1008 ms | 1   | 0      | MINOR    | 0.0325 ms     |
| 9401 | 12.1735 ms  | 1   | 1      | MINOR    | 0.0198 ms     |
| 9428 | ~12.1200 ms | 1   | 0      | MINOR    | 0.1648 ms     |
| 9458 | 12.1308 ms  | 1   | 1      | MINOR    | 0.0289 ms     |

> `~` = fractional vblank slip (1.25×–2× ideal, no whole vblank missed)

## Verdict

**NATIVE PERFORMANCE. GPU is perfectly tracking the monitor.**

> NOTE: Low avg sync score is expected over long sessions, the
> fixed phase origin precesses through all vblank phases as
> sub-microsecond residuals accumulate. Per-frame drift values
> and stutter events remain accurate.
