# Telemetry Report: `mailbox.json`

- **Target:** 165.0 Hz (6.0606 ms/frame)
- **Frames Analysed:** 9793
- **Session Duration:** 59.95 s

## Global Pacing

| Metric             | Value     | Evaluation               |
| ------------------ | --------- | ------------------------ |
| Avg Delivery Time  | 6.1213 ms | PERFECT (Native Refresh) |
| V-Sync Multiplier  | 1.01 x    |                          |
| Jitter (IFI delta) | 0.0752 ms | LOCKED                   |

## Phase Drift

| Metric          | Value      |
| --------------- | ---------- |
| Avg Phase Drift | +0.1745 ms |
| Max Phase Drift | -3.0299 ms |
| Drift Std Dev   | 1.7813 ms  |
| Avg Sync Score  | 47.92 %    |

## Stutter Events

- **Distinct events:** 1
- **Anomalous frames:** 39 (0.40% of session)
- **Vblanks lost:** 6

| IDX | WORST Δ    | SZ  | MISSED | SEVERITY | RECOV. JITTER |
| --- | ---------- | --- | ------ | -------- | ------------- |
| 2   | 47.9051 ms | 39  | 6      | CLUSTER  | 0.0971 ms     |

> `~` = fractional vblank slip (1.25×–2× ideal, no whole vblank missed)

## Verdict

**NATIVE PERFORMANCE. GPU is perfectly tracking the monitor.**

> NOTE: Low avg sync score is expected over long sessions, the
> fixed phase origin precesses through all vblank phases as
> sub-microsecond residuals accumulate. Per-frame drift values
> and stutter events remain accurate.
