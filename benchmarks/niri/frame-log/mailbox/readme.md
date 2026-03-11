# Telemetry Report: `mailbox.json`

- **Target:** 165.0 Hz (6.0606 ms/frame)
- **Frames Analysed:** 10001
- **Session Duration:** 59.94 s

## Global Pacing

| Metric             | Value     | Evaluation               |
| ------------------ | --------- | ------------------------ |
| Avg Delivery Time  | 5.9936 ms | PERFECT (Native Refresh) |
| V-Sync Multiplier  | 0.99 x    |                          |
| Jitter (IFI delta) | 0.3998 ms | STABLE                   |

## Phase Drift

| Metric          | Value      |
| --------------- | ---------- |
| Avg Phase Drift | -0.0042 ms |
| Max Phase Drift | +3.0301 ms |
| Drift Std Dev   | 1.7433 ms  |
| Avg Sync Score  | 50.39 %    |

## Stutter Events

- **Distinct events:** 55
- **Anomalous frames:** 75 (0.75% of session)
- **Vblanks lost:** 49

| IDX  | WORST Δ     | SZ  | MISSED | SEVERITY | RECOV. JITTER |
| ---- | ----------- | --- | ------ | -------- | ------------- |
| 3    | 34.1315 ms  | 21  | 4      | CLUSTER  | 0.0558 ms     |
| 57   | 12.1478 ms  | 1   | 1      | MINOR    | 0.8959 ms     |
| 65   | 12.1548 ms  | 1   | 1      | MINOR    | 1.0578 ms     |
| 73   | 12.2035 ms  | 1   | 1      | MINOR    | 0.0427 ms     |
| 83   | 12.1267 ms  | 1   | 1      | MINOR    | 0.0241 ms     |
| 100  | ~12.0976 ms | 1   | 0      | MINOR    | 0.0196 ms     |
| 124  | 12.4175 ms  | 1   | 1      | MINOR    | 0.1262 ms     |
| 162  | 12.1880 ms  | 1   | 1      | MINOR    | 0.0277 ms     |
| 182  | 12.1806 ms  | 1   | 1      | MINOR    | 0.0240 ms     |
| 197  | 12.1404 ms  | 1   | 1      | MINOR    | 0.1403 ms     |
| 212  | 12.1745 ms  | 1   | 1      | MINOR    | 0.1349 ms     |
| 229  | 12.5002 ms  | 1   | 1      | MINOR    | 0.1665 ms     |
| 426  | 12.1584 ms  | 1   | 1      | MINOR    | 0.0183 ms     |
| 454  | 12.1313 ms  | 1   | 1      | MINOR    | 0.0463 ms     |
| 576  | ~12.1008 ms | 1   | 0      | MINOR    | 0.0218 ms     |
| 591  | 12.1377 ms  | 1   | 1      | MINOR    | 0.0368 ms     |
| 605  | 12.1559 ms  | 1   | 1      | MINOR    | 0.0470 ms     |
| 1520 | 12.3246 ms  | 1   | 1      | MINOR    | 0.1009 ms     |
| 1855 | 12.1448 ms  | 1   | 1      | MINOR    | 0.0176 ms     |
| 1878 | 12.1715 ms  | 1   | 1      | MINOR    | 0.0457 ms     |
| 1912 | 12.2402 ms  | 1   | 1      | MINOR    | 0.0413 ms     |
| 1951 | 12.3484 ms  | 1   | 1      | MINOR    | 0.0629 ms     |
| 1971 | 12.2430 ms  | 1   | 1      | MINOR    | 0.0354 ms     |
| 1990 | 12.1340 ms  | 1   | 1      | MINOR    | 0.0327 ms     |
| 2020 | 12.1520 ms  | 1   | 1      | MINOR    | 0.0216 ms     |
| 2334 | 12.2268 ms  | 1   | 1      | MINOR    | 0.0930 ms     |
| 2563 | ~10.4037 ms | 1   | 0      | MINOR    | 1.8307 ms     |
| 2631 | ~9.6247 ms  | 1   | 0      | MINOR    | 1.1572 ms     |
| 3188 | ~12.1148 ms | 1   | 0      | MINOR    | 0.0330 ms     |
| 3201 | 12.1472 ms  | 1   | 1      | MINOR    | 0.0379 ms     |
| 3213 | 12.1278 ms  | 1   | 1      | MINOR    | 0.0548 ms     |
| 3227 | 12.1788 ms  | 1   | 1      | MINOR    | 0.0550 ms     |
| 3360 | 12.1702 ms  | 1   | 1      | MINOR    | 0.0445 ms     |
| 3376 | 12.1370 ms  | 1   | 1      | MINOR    | 0.0359 ms     |
| 3399 | 12.3116 ms  | 1   | 1      | MINOR    | 0.0308 ms     |
| 3749 | 12.3231 ms  | 1   | 1      | MINOR    | 0.0502 ms     |
| 4603 | 12.1971 ms  | 1   | 1      | MINOR    | 0.0432 ms     |
| 4636 | 12.2578 ms  | 1   | 1      | MINOR    | 0.1099 ms     |
| 4742 | ~12.0942 ms | 1   | 0      | MINOR    | 0.0429 ms     |
| 5144 | 12.1238 ms  | 1   | 1      | MINOR    | 0.0388 ms     |
| 6013 | ~12.1208 ms | 1   | 0      | MINOR    | 0.0274 ms     |
| 6033 | 12.1560 ms  | 1   | 1      | MINOR    | 0.0503 ms     |
| 6383 | 12.2238 ms  | 1   | 1      | MINOR    | 0.0542 ms     |
| 6395 | 12.1426 ms  | 1   | 1      | MINOR    | 0.0396 ms     |
| 6410 | ~12.1129 ms | 1   | 0      | MINOR    | 0.0220 ms     |
| 6543 | 12.2821 ms  | 1   | 1      | MINOR    | 0.0613 ms     |
| 6566 | 12.1408 ms  | 1   | 1      | MINOR    | 0.0240 ms     |
| 6604 | 12.2890 ms  | 1   | 1      | MINOR    | 0.1011 ms     |
| 7755 | 12.3009 ms  | 1   | 1      | MINOR    | 0.0487 ms     |
| 7777 | 12.1483 ms  | 1   | 1      | MINOR    | 0.0491 ms     |
| 7799 | 12.3518 ms  | 1   | 1      | MINOR    | 0.0399 ms     |
| 7920 | 12.1275 ms  | 1   | 1      | MINOR    | 0.0372 ms     |
| 7934 | 12.1615 ms  | 1   | 1      | MINOR    | 0.1009 ms     |
| 7952 | ~12.0732 ms | 1   | 0      | MINOR    | 0.1719 ms     |
| 9371 | 12.2423 ms  | 1   | 1      | MINOR    | 0.0542 ms     |

> `~` = fractional vblank slip (1.25×–2× ideal, no whole vblank missed)

## Verdict

**NATIVE PERFORMANCE. GPU is perfectly tracking the monitor.**

> NOTE: Low avg sync score is expected over long sessions, the
> fixed phase origin precesses through all vblank phases as
> sub-microsecond residuals accumulate. Per-frame drift values
> and stutter events remain accurate.
