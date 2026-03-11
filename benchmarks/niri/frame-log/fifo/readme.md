# Telemetry Report: `fifo.json`

- **Target:** 165.0 Hz (6.0606 ms/frame)
- **Frames Analysed:** 9782
- **Session Duration:** 59.94 s

## Global Pacing

| Metric             | Value     | Evaluation               |
| ------------------ | --------- | ------------------------ |
| Avg Delivery Time  | 6.1274 ms | PERFECT (Native Refresh) |
| V-Sync Multiplier  | 1.01 x    |                          |
| Jitter (IFI delta) | 0.0876 ms | LOCKED                   |

## Phase Drift

| Metric          | Value      |
| --------------- | ---------- |
| Avg Phase Drift | -0.0640 ms |
| Max Phase Drift | +3.0303 ms |
| Drift Std Dev   | 1.6300 ms  |
| Avg Sync Score  | 54.59 %    |

## Stutter Events

- **Distinct events:** 6
- **Anomalous frames:** 25 (0.26% of session)
- **Vblanks lost:** 20

| IDX  | WORST Δ    | SZ  | MISSED | SEVERITY | RECOV. JITTER |
| ---- | ---------- | --- | ------ | -------- | ------------- |
| 0    | 45.9503 ms | 19  | 6      | CLUSTER  | 0.4376 ms     |
| 2518 | 25.0056 ms | 1   | 3      | SEVERE   | 0.0210 ms     |
| 2536 | 18.9502 ms | 1   | 2      | MINOR    | 0.0670 ms     |
| 2554 | 26.8908 ms | 1   | 3      | SEVERE   | 0.0409 ms     |
| 2573 | 25.0672 ms | 1   | 3      | SEVERE   | 0.0415 ms     |
| 8249 | 25.0125 ms | 2   | 3      | SEVERE   | 0.0505 ms     |

> `~` = fractional vblank slip (1.25×–2× ideal, no whole vblank missed)

## Session Phases

_Cadence regimes, keyed on global frame index_

| #   | GLOBAL IDX | MEAN Δ    | EFF. Hz  | JITTER    |
| --- | ---------- | --------- | -------- | --------- |
| 1   | 0–66       | 8.9301 ms | 112.0 Hz | 4.0106 ms |
| 2   | 67–2574    | 6.1500 ms | 162.6 Hz | 0.0957 ms |
| 3   | 2575–9781  | 6.0935 ms | 164.1 Hz | 0.0488 ms |

## Verdict

**NATIVE PERFORMANCE. GPU is perfectly tracking the monitor.**

> NOTE: Low avg sync score is expected over long sessions, the
> fixed phase origin precesses through all vblank phases as
> sub-microsecond residuals accumulate. Per-frame drift values
> and stutter events remain accurate.
