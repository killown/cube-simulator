# Telemetry Report: `mailbox.json`

- **Target:** 165.0 Hz (6.0606 ms/frame)
- **Frames Analysed:** 10033
- **Session Duration:** 59.95 s

## Global Pacing

| Metric             | Value     | Evaluation               |
| ------------------ | --------- | ------------------------ |
| Avg Delivery Time  | 5.9752 ms | PERFECT (Native Refresh) |
| V-Sync Multiplier  | 0.99 x    |                          |
| Jitter (IFI delta) | 0.0659 ms | LOCKED                   |

## Phase Drift

| Metric          | Value      |
| --------------- | ---------- |
| Avg Phase Drift | +0.0293 ms |
| Max Phase Drift | +3.0302 ms |
| Drift Std Dev   | 1.7359 ms  |
| Avg Sync Score  | 50.34 %    |

## Stutter Events

- **Distinct events:** 1
- **Anomalous frames:** 3 (0.03% of session)
- **Vblanks lost:** 2

| IDX | WORST Δ    | SZ  | MISSED | SEVERITY | RECOV. JITTER |
| --- | ---------- | --- | ------ | -------- | ------------- |
| 3   | 19.4525 ms | 3   | 2      | MINOR    | 0.1086 ms     |

> `~` = fractional vblank slip (1.25×–2× ideal, no whole vblank missed)

## Session Phases

_Cadence regimes, keyed on global frame index_

| #   | GLOBAL IDX | MEAN Δ    | EFF. Hz  | JITTER    |
| --- | ---------- | --------- | -------- | --------- |
| 1   | 0–20       | 7.0234 ms | 142.4 Hz | 2.5544 ms |
| 2   | 21–10032   | 5.9730 ms | 167.4 Hz | 0.0610 ms |

## Verdict

**NATIVE PERFORMANCE. GPU is perfectly tracking the monitor.**

> NOTE: Low avg sync score is expected over long sessions, the
> fixed phase origin precesses through all vblank phases as
> sub-microsecond residuals accumulate. Per-frame drift values
> and stutter events remain accurate.
