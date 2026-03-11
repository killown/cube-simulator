# Telemetry Report: `fifo.json`

- **Target:** 165.0 Hz (6.0606 ms/frame)
- **Frames Analysed:** 9777
- **Session Duration:** 59.95 s

## Global Pacing

| Metric             | Value     | Evaluation               |
| ------------------ | --------- | ------------------------ |
| Avg Delivery Time  | 6.1317 ms | PERFECT (Native Refresh) |
| V-Sync Multiplier  | 1.01 x    |                          |
| Jitter (IFI delta) | 0.0918 ms | LOCKED                   |

## Phase Drift

| Metric          | Value      |
| --------------- | ---------- |
| Avg Phase Drift | +0.0124 ms |
| Max Phase Drift | +3.0300 ms |
| Drift Std Dev   | 1.7945 ms  |
| Avg Sync Score  | 48.36 %    |

## Stutter Events

- **Distinct events:** 1
- **Anomalous frames:** 44 (0.45% of session)
- **Vblanks lost:** 9

| IDX | WORST Δ    | SZ  | MISSED | SEVERITY | RECOV. JITTER |
| --- | ---------- | --- | ------ | -------- | ------------- |
| 2   | 66.3452 ms | 44  | 9      | CLUSTER  | 0.2601 ms     |

> `~` = fractional vblank slip (1.25×–2× ideal, no whole vblank missed)

## Verdict

**NATIVE PERFORMANCE. GPU is perfectly tracking the monitor.**

> NOTE: Low avg sync score is expected over long sessions, the
> fixed phase origin precesses through all vblank phases as
> sub-microsecond residuals accumulate. Per-frame drift values
> and stutter events remain accurate.
