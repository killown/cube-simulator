# Telemetry Report: `mailbox.json`

- **Target:** 165.0 Hz (6.0606 ms/frame)
- **Frames Analysed:** 9885
- **Session Duration:** 59.94 s

## Global Pacing

| Metric             | Value     | Evaluation               |
| ------------------ | --------- | ------------------------ |
| Avg Delivery Time  | 6.0640 ms | PERFECT (Native Refresh) |
| V-Sync Multiplier  | 1.00 x    |                          |
| Jitter (IFI delta) | 7.7874 ms | STUTTERY                 |

## Phase Drift

| Metric          | Value      |
| --------------- | ---------- |
| Avg Phase Drift | -0.0099 ms |
| Max Phase Drift | -3.0303 ms |
| Drift Std Dev   | 1.7879 ms  |
| Avg Sync Score  | 48.53 %    |

## Stutter Events

- **Distinct events:** 50
- **Anomalous frames:** 3231 (32.69% of session)
- **Vblanks lost:** 44

| IDX  | WORST Δ     | SZ  | MISSED | SEVERITY | RECOV. JITTER |
| ---- | ----------- | --- | ------ | -------- | ------------- |
| 1    | 21.5153 ms  | 1   | 2      | MINOR    | 3.8904 ms     |
| 7    | 18.1111 ms  | 172 | 1      | CLUSTER  | 4.3447 ms     |
| 525  | 13.0974 ms  | 153 | 1      | CLUSTER  | 3.3110 ms     |
| 988  | 12.2620 ms  | 89  | 1      | CLUSTER  | 3.3621 ms     |
| 1259 | 17.7757 ms  | 162 | 1      | CLUSTER  | 4.2613 ms     |
| 1748 | 12.1260 ms  | 5   | 1      | MINOR    | 3.4538 ms     |
| 1767 | ~11.9869 ms | 1   | 0      | MINOR    | 3.5226 ms     |
| 1774 | 12.1928 ms  | 1   | 1      | MINOR    | 4.4298 ms     |
| 1780 | 12.5286 ms  | 5   | 1      | MINOR    | 4.3838 ms     |
| 1800 | 12.5524 ms  | 7   | 1      | MINOR    | 3.5900 ms     |
| 1825 | 12.9378 ms  | 7   | 1      | MINOR    | 4.4868 ms     |
| 1850 | 12.9423 ms  | 129 | 1      | CLUSTER  | 3.5540 ms     |
| 2241 | 12.5095 ms  | 59  | 1      | CLUSTER  | 4.1667 ms     |
| 2422 | ~12.0218 ms | 9   | 0      | SLIP     | 3.4221 ms     |
| 2453 | 12.7414 ms  | 137 | 1      | CLUSTER  | 4.3657 ms     |
| 2868 | 12.4174 ms  | 33  | 1      | CLUSTER  | 4.3234 ms     |
| 2971 | 13.1342 ms  | 65  | 1      | CLUSTER  | 3.8418 ms     |
| 3170 | 13.5613 ms  | 5   | 1      | MINOR    | 3.8517 ms     |
| 3189 | 13.4250 ms  | 87  | 1      | CLUSTER  | 3.1617 ms     |
| 3454 | 13.1327 ms  | 101 | 1      | CLUSTER  | 3.5509 ms     |
| 3761 | 12.2297 ms  | 1   | 1      | MINOR    | 3.5173 ms     |
| 3768 | 12.2268 ms  | 43  | 1      | CLUSTER  | 3.3836 ms     |
| 3901 | ~11.7685 ms | 3   | 0      | SLIP     | 3.3540 ms     |
| 3914 | ~11.8342 ms | 25  | 0      | CLUSTER  | 3.2810 ms     |
| 3993 | 12.3194 ms  | 71  | 1      | CLUSTER  | 3.4798 ms     |
| 4210 | 12.3479 ms  | 99  | 1      | CLUSTER  | 3.5849 ms     |
| 4511 | 12.9021 ms  | 47  | 1      | CLUSTER  | 3.5457 ms     |
| 4656 | 12.8676 ms  | 49  | 1      | CLUSTER  | 3.3118 ms     |
| 4807 | 12.8098 ms  | 87  | 1      | CLUSTER  | 4.4804 ms     |
| 5072 | 12.8226 ms  | 3   | 1      | MINOR    | 3.6290 ms     |
| 5085 | 12.5835 ms  | 1   | 1      | MINOR    | 3.6216 ms     |
| 5092 | 12.7430 ms  | 3   | 1      | MINOR    | 3.6637 ms     |
| 5105 | 12.7318 ms  | 31  | 1      | CLUSTER  | 3.3436 ms     |
| 5202 | 12.4964 ms  | 31  | 1      | CLUSTER  | 4.3309 ms     |
| 5299 | 12.3551 ms  | 19  | 1      | CLUSTER  | 4.2216 ms     |
| 5360 | ~12.0026 ms | 7   | 0      | SLIP     | 5.9336 ms     |
| 5384 | 17.7750 ms  | 101 | 1      | CLUSTER  | 4.0617 ms     |
| 5691 | 13.0676 ms  | 135 | 1      | CLUSTER  | 3.5368 ms     |
| 6100 | 12.7124 ms  | 9   | 1      | MINOR    | 4.4148 ms     |
| 6131 | 13.3728 ms  | 403 | 1      | CLUSTER  | 3.4130 ms     |
| 7344 | 13.2520 ms  | 185 | 1      | CLUSTER  | 3.5604 ms     |
| 7903 | 12.7893 ms  | 381 | 1      | CLUSTER  | 3.6347 ms     |
| 9050 | 12.6004 ms  | 3   | 1      | MINOR    | 3.6592 ms     |
| 9063 | 12.7128 ms  | 1   | 1      | MINOR    | 3.6413 ms     |
| 9070 | 12.8244 ms  | 147 | 1      | CLUSTER  | 3.4466 ms     |
| 9515 | ~11.9388 ms | 5   | 0      | SLIP     | 3.3881 ms     |
| 9534 | ~11.9316 ms | 3   | 0      | SLIP     | 3.6028 ms     |
| 9546 | 12.7845 ms  | 39  | 1      | CLUSTER  | 3.4087 ms     |
| 9668 | 12.2386 ms  | 17  | 1      | CLUSTER  | 3.4454 ms     |
| 9723 | 12.2602 ms  | 54  | 1      | CLUSTER  | 5.8081 ms     |

> `~` = fractional vblank slip (1.25×–2× ideal, no whole vblank missed)

## Verdict

**NATIVE BUT JITTERY. Correct speed, but delivery spacing is uneven.**
