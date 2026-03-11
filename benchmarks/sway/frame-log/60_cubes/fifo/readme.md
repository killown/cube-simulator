# Telemetry Report: `fifo.json`

- **Target:** 165.0 Hz (6.0606 ms/frame)
- **Frames Analysed:** 4159
- **Session Duration:** 59.94 s

## Global Pacing

| Metric             | Value      | Evaluation                          |
| ------------------ | ---------- | ----------------------------------- |
| Avg Delivery Time  | 14.4115 ms | PERFORMANCE LIMITED (Dropped Beats) |
| V-Sync Multiplier  | 2.38 x     |                                     |
| Jitter (IFI delta) | 0.2539 ms  | LOCKED                              |

## Phase Drift

| Metric          | Value      |
| --------------- | ---------- |
| Avg Phase Drift | +0.0258 ms |
| Max Phase Drift | +3.0283 ms |
| Drift Std Dev   | 1.7486 ms  |
| Avg Sync Score  | 50.11 %    |

## Stutter Events

- **Distinct events:** 1
- **Anomalous frames:** 4147 (99.71% of session)
- **Vblanks lost:** 5

| IDX | WORST Δ    | SZ   | MISSED | SEVERITY | RECOV. JITTER |
| --- | ---------- | ---- | ------ | -------- | ------------- |
| 2   | 41.1594 ms | 4147 | 5      | CLUSTER  | n/a           |

> `~` = fractional vblank slip (1.25×–2× ideal, no whole vblank missed)

## Verdict

**GPU BOUND. Throughput is significantly lower than refresh rate.**
