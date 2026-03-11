# Telemetry Report: `fifo.json`

- **Target:** 165.0 Hz (6.0606 ms/frame)
- **Frames Analysed:** 4337
- **Session Duration:** 59.94 s

## Global Pacing

| Metric             | Value      | Evaluation                          |
| ------------------ | ---------- | ----------------------------------- |
| Avg Delivery Time  | 13.8215 ms | PERFORMANCE LIMITED (Dropped Beats) |
| V-Sync Multiplier  | 2.28 x     |                                     |
| Jitter (IFI delta) | 0.1560 ms  | LOCKED                              |

## Phase Drift

| Metric          | Value      |
| --------------- | ---------- |
| Avg Phase Drift | -0.0123 ms |
| Max Phase Drift | -3.0290 ms |
| Drift Std Dev   | 1.7414 ms  |
| Avg Sync Score  | 50.24 %    |

## Stutter Events

- **Distinct events:** 1
- **Anomalous frames:** 4331 (99.86% of session)
- **Vblanks lost:** 6

| IDX | WORST Δ    | SZ   | MISSED | SEVERITY | RECOV. JITTER |
| --- | ---------- | ---- | ------ | -------- | ------------- |
| 2   | 46.9162 ms | 4331 | 6      | CLUSTER  | n/a           |

> `~` = fractional vblank slip (1.25×–2× ideal, no whole vblank missed)

## Verdict

**GPU BOUND. Throughput is significantly lower than refresh rate.**
