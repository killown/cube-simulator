# Telemetry Report: `fifo.json`

- **Target:** 165.0 Hz (6.0606 ms/frame)
- **Frames Analysed:** 9333
- **Session Duration:** 59.94 s

## Global Pacing

| Metric             | Value     | Evaluation                  |
| ------------------ | --------- | --------------------------- |
| Avg Delivery Time  | 6.4227 ms | GOOD (Consistent Half-Rate) |
| V-Sync Multiplier  | 1.06 x    |                             |
| Jitter (IFI delta) | 0.0731 ms | LOCKED                      |

## Phase Drift

| Metric          | Value      |
| --------------- | ---------- |
| Avg Phase Drift | +0.0056 ms |
| Max Phase Drift | -3.0302 ms |
| Drift Std Dev   | 1.7644 ms  |
| Avg Sync Score  | 49.93 %    |

## Stutter Events

- **Distinct events:** 3
- **Anomalous frames:** 82 (0.88% of session)
- **Vblanks lost:** 9

| IDX  | WORST Δ    | SZ  | MISSED | SEVERITY | RECOV. JITTER |
| ---- | ---------- | --- | ------ | -------- | ------------- |
| 2    | 19.0300 ms | 79  | 2      | CLUSTER  | 0.0906 ms     |
| 780  | 46.1856 ms | 2   | 6      | SEVERE   | 0.0747 ms     |
| 7406 | 12.9720 ms | 1   | 1      | MINOR    | 0.1511 ms     |

> `~` = fractional vblank slip (1.25×–2× ideal, no whole vblank missed)

## Session Phases

_Cadence regimes, keyed on global frame index_

| #   | GLOBAL IDX | MEAN Δ    | EFF. Hz  | JITTER    |
| --- | ---------- | --------- | -------- | --------- |
| 1   | 2–138      | 9.7786 ms | 102.3 Hz | 0.1498 ms |
| 2   | 139–779    | 6.5923 ms | 151.7 Hz | 0.0567 ms |
| 3   | 780–9332   | 6.3568 ms | 157.3 Hz | 0.0667 ms |

## Verdict

**ACCEPTABLE. Standard presentation timing.**
