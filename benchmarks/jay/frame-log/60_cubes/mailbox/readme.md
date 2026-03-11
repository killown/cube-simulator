# Telemetry Report: `mailbox.json`

- **Target:** 165.0 Hz (6.0606 ms/frame)
- **Frames Analysed:** 4343
- **Session Duration:** 59.94 s

## Global Pacing

| Metric             | Value      | Evaluation                          |
| ------------------ | ---------- | ----------------------------------- |
| Avg Delivery Time  | 13.8025 ms | PERFORMANCE LIMITED (Dropped Beats) |
| V-Sync Multiplier  | 2.28 x     |                                     |
| Jitter (IFI delta) | 0.1226 ms  | LOCKED                              |

## Phase Drift

| Metric          | Value      |
| --------------- | ---------- |
| Avg Phase Drift | -0.0092 ms |
| Max Phase Drift | -3.0301 ms |
| Drift Std Dev   | 1.7445 ms  |
| Avg Sync Score  | 50.26 %    |

## Stutter Events

- **Distinct events:** 1
- **Anomalous frames:** 4337 (99.86% of session)
- **Vblanks lost:** 7

| IDX | WORST Δ    | SZ   | MISSED | SEVERITY | RECOV. JITTER |
| --- | ---------- | ---- | ------ | -------- | ------------- |
| 3   | 49.8477 ms | 4337 | 7      | CLUSTER  | n/a           |

> `~` = fractional vblank slip (1.25×–2× ideal, no whole vblank missed)

## Verdict

**GPU BOUND. Throughput is significantly lower than refresh rate.**
