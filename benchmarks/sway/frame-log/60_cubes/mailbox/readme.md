# Telemetry Report: `mailbox.json`

- **Target:** 165.0 Hz (6.0606 ms/frame)
- **Frames Analysed:** 4288
- **Session Duration:** 59.94 s

## Global Pacing

| Metric             | Value      | Evaluation                          |
| ------------------ | ---------- | ----------------------------------- |
| Avg Delivery Time  | 13.9784 ms | PERFORMANCE LIMITED (Dropped Beats) |
| V-Sync Multiplier  | 2.31 x     |                                     |
| Jitter (IFI delta) | 0.1483 ms  | LOCKED                              |

## Phase Drift

| Metric          | Value      |
| --------------- | ---------- |
| Avg Phase Drift | -0.0237 ms |
| Max Phase Drift | -3.0299 ms |
| Drift Std Dev   | 1.7419 ms  |
| Avg Sync Score  | 50.21 %    |

## Stutter Events

- **Distinct events:** 1
- **Anomalous frames:** 4282 (99.86% of session)
- **Vblanks lost:** 6

| IDX | WORST Δ    | SZ   | MISSED | SEVERITY | RECOV. JITTER |
| --- | ---------- | ---- | ------ | -------- | ------------- |
| 3   | 44.6654 ms | 4282 | 6      | CLUSTER  | n/a           |

> `~` = fractional vblank slip (1.25×–2× ideal, no whole vblank missed)

## Verdict

**GPU BOUND. Throughput is significantly lower than refresh rate.**
