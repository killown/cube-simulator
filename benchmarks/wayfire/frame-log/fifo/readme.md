# Telemetry Report: `fifo.json`

- **Target:** 165.0 Hz (6.0606 ms/frame)
- **Frames Analysed:** 9289
- **Session Duration:** 59.94 s

## Global Pacing

| Metric             | Value     | Evaluation                  |
| ------------------ | --------- | --------------------------- |
| Avg Delivery Time  | 6.4523 ms | GOOD (Consistent Half-Rate) |
| V-Sync Multiplier  | 1.06 x    |                             |
| Jitter (IFI delta) | 0.1321 ms | LOCKED                      |

## Phase Drift

| Metric          | Value      |
| --------------- | ---------- |
| Avg Phase Drift | -0.0470 ms |
| Max Phase Drift | -3.0301 ms |
| Drift Std Dev   | 1.7674 ms  |
| Avg Sync Score  | 49.22 %    |

## Stutter Events

- **Distinct events:** 8
- **Anomalous frames:** 38 (0.41% of session)
- **Vblanks lost:** 19

| IDX  | WORST Δ     | SZ  | MISSED | SEVERITY | RECOV. JITTER |
| ---- | ----------- | --- | ------ | -------- | ------------- |
| 0    | 19.0513 ms  | 27  | 2      | CLUSTER  | 0.0543 ms     |
| 3832 | 36.8822 ms  | 2   | 5      | SEVERE   | 0.1008 ms     |
| 3863 | 43.8619 ms  | 3   | 6      | SEVERE   | 0.1313 ms     |
| 4050 | 40.7897 ms  | 2   | 5      | SEVERE   | 0.1182 ms     |
| 5122 | ~12.0440 ms | 1   | 0      | MINOR    | 0.0627 ms     |
| 5526 | ~11.8806 ms | 1   | 0      | MINOR    | 0.0606 ms     |
| 8890 | 12.6294 ms  | 1   | 1      | MINOR    | 0.0682 ms     |
| 9086 | ~11.8420 ms | 1   | 0      | MINOR    | 0.0705 ms     |

> `~` = fractional vblank slip (1.25×–2× ideal, no whole vblank missed)

## Session Phases

_Cadence regimes, keyed on global frame index_

| #   | GLOBAL IDX | MEAN Δ    | EFF. Hz  | JITTER    |
| --- | ---------- | --------- | -------- | --------- |
| 1   | 0–75       | 8.6543 ms | 115.5 Hz | 2.5050 ms |
| 2   | 76–3831    | 6.4244 ms | 155.7 Hz | 0.0737 ms |
| 3   | 3832–3862  | 7.5196 ms | 133.0 Hz | 1.4205 ms |
| 4   | 3863–3888  | 8.9064 ms | 112.3 Hz | 2.8748 ms |
| 5   | 3889–4049  | 6.3710 ms | 157.0 Hz | 0.0696 ms |
| 6   | 4050–9288  | 6.4243 ms | 155.7 Hz | 0.1018 ms |

## Verdict

**ACCEPTABLE. Standard presentation timing.**
