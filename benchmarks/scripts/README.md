# frame_log_statistics.py

Static analyser for per-frame NDJSON pacing logs produced by the wgpu compositor
benchmark. Reads a single log file and emits a structured telemetry report covering
seven distinct analysis sections, each designed to answer a specific compositor
development question without requiring any external dependencies.

```
python frame_log_statistics.py <frame_log.json> [--markdown]
```

- Default output is aligned plain-text, readable in any terminal.
- `--markdown` emits a GitHub-flavoured Markdown report suitable for bug reports,
  CI artefacts, and regression tracking.

---

## Input Format

The script consumes the NDJSON format written by `write_frame_log_row` in
`metrics.rs`. Each line is a self-contained JSON object. A comment line at the
top documents the field set present in the file.

```
# frame,ts_ns,delta_ms,ideal_ms,drift_ms,drift_ns,vblank_mul,sync[,ipc_delta_ms][,cpu_frame_ms][,slack_ms]
{"frame":1,"ts_ns":22841896583172,"delta_ms":24.3482,"ideal_ms":6.0606,"drift_ms":0.0000,"drift_ns":0,"vblank_mul":4,"sync":100.00,"cpu_frame_ms":24.3297}
{"frame":4,"ts_ns":22841972922751,"delta_ms":24.6004,"ideal_ms":6.0606,"drift_ms":-2.4483,"drift_ns":-2448299,"vblank_mul":4,"sync":19.21,"ipc_delta_ms":18.7178,"cpu_frame_ms":24.5369,"slack_ms":29.4521}
```

### Field Reference

| Field          | Type  | Always present | Description                                                                                |
| -------------- | ----- | -------------- | ------------------------------------------------------------------------------------------ |
| `frame`        | int   | ✓              | Monotonic frame index since session start (0-based)                                        |
| `ts_ns`        | int   | ✓              | Absolute KMS/WSI presentation timestamp, nanoseconds, CLOCK_MONOTONIC                      |
| `delta_ms`     | float | ✓              | Measured inter-frame interval on the presentation clock (ms)                               |
| `ideal_ms`     | float | ✓              | Target vblank period from the monitor's reported refresh rate (ms)                         |
| `drift_ms`     | float | ✓              | Signed deviation from the nearest ideal vblank boundary (ms); `+` = late, `−` = early      |
| `drift_ns`     | int   | ✓              | Same drift at nanosecond precision, before float truncation                                |
| `vblank_mul`   | int   | ✓              | Vblank periods consumed by this frame; `1` = on-time, `2` = one missed, ≥`3` = stall       |
| `sync`         | float | ✓              | Sync quality score 0–100; `100` = perfectly on-vblank, `0` = half-period drift             |
| `ipc_delta_ms` | float | optional       | `delta_ms[n] − delta_ms[n−1]`, instantaneous jitter signal, absent on frame 1              |
| `cpu_frame_ms` | float | optional       | CPU-observed total frame time from `RedrawRequested` to `present()` return (ms)            |
| `slack_ms`     | float | optional       | `present_ts_ns − cpu_submit_ns`, GPU execution + flip pipeline depth as seen from CPU (ms) |

**Optional fields** are omitted from the JSON object when the backend cannot supply
the source data (e.g. `slack_ms` requires a valid previous WSI timestamp to compute
the CLOCK_MONOTONIC-domain submit instant). The script handles their absence
gracefully, sections that depend on them report "classification unavailable" rather
than failing.

**Concatenated sessions**: if you append multiple log files end-to-end, the loader
detects non-monotonic frame numbers and reassigns a session-global monotonic index
so that all downstream analysis remains correct. A note is printed when this occurs.

---

## Output Sections

### 1. Session Header

```
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  TELEMETRY REPORT  ·  testing.json
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

  TARGET            165.0 Hz  (6.0606 ms/frame)
  FRAMES ANALYSED   423
  SESSION DURATION  6.49 s
```

Orientation data. **TARGET** is derived from `ideal_ms` in the first frame record —
it is the monitor's reported refresh rate, not a configured constant. **SESSION
DURATION** is the sum of all `delta_ms` values, which measures time on the
presentation clock rather than wall-clock time, making it resistant to compositor
scheduling effects.

---

### 2. Global Pacing

```
  GLOBAL PACING
────────────────────────────────────────────────────────────────────────
  Avg Delivery Time      15.3458 ms   [PERFORMANCE LIMITED (Dropped Beats)]
  V-Sync Multiplier         2.53 x
  Jitter (IFI delta)      1.0549 ms   [STUTTERY]
```

**Avg Delivery Time** is the mean of all `delta_ms` values, the average time
between consecutive hardware scan-outs. Because `delta_ms` is measured on the
KMS/WSI clock, this cannot be inflated by `max_render_time` or Wayland frame
callbacks.

**V-Sync Multiplier** is `avg_delta / ideal_ms`. It expresses throughput as a
multiple of the target refresh period:

| Range       | Label                               | Meaning                             |
| ----------- | ----------------------------------- | ----------------------------------- |
| 0.95 – 1.05 | PERFECT (Native Refresh)            | GPU is tracking the monitor exactly |
| 1.05 – 2.10 | GOOD (Consistent Half-Rate)         | Steady double-vblank delivery       |
| > 2.10      | PERFORMANCE LIMITED (Dropped Beats) | Significant frames being skipped    |

**Jitter (IFI delta)** is the mean of `|delta_ms[n] − delta_ms[n−1]|` across all
consecutive frame pairs, the mean absolute change in inter-frame interval. This
is a rolling average. The double-buffer ping-pong section (§5) provides the
instantaneous signal that this average can mask.

| Range        | Label    | Meaning                                       |
| ------------ | -------- | --------------------------------------------- |
| < 0.3 ms     | LOCKED   | Frame delivery is metronomic                  |
| 0.3 – 1.0 ms | STABLE   | Minor variation, not perceptible              |
| > 1.0 ms     | STUTTERY | Delivery spacing is irregular; likely visible |

---

### 3. Vblank Budget Distribution

```
  VBLANK BUDGET DISTRIBUTION
────────────────────────────────────────────────────────────────────────
   1× (on-time   )       2 frames    0.5%
   2× (1 dropped )     292 frames   69.0%  ██████████████████████████████████
   3× (2 dropped )     100 frames   23.6%  ███████████
   4× (3 dropped )      20 frames    4.7%  ██
   5× (4 dropped )       6 frames    1.4%
   7× (6 dropped )       1 frames    0.2%
   8× (7 dropped )       1 frames    0.2%
  10× (9 dropped )       1 frames    0.2%
```

A histogram of `vblank_mul` values across the session. Each row is one bucket:
the integer number of vblank periods that frame consumed, derived as
`max(1, round(delta_ms / ideal_ms))`.

This is the single most useful first-look summary for a compositor developer
because it turns the mean multiplier (a single lossy number) into the actual
delivery distribution. The example above has a mean of 2.53×, but the
distribution reveals that only **0.5%** of frames were actually on-time at 1×;
the dominant cadence was 2× (69%), with a long tail up to 10×. A mean of 2.53×
from a bimodal 1×/4× split looks identical in the header but has a completely
different perceptual character.

The bar chart (each `█` = 2%) makes regime transitions visible at a glance when
comparing two logs side by side.

---

### 4. Phase Drift

```
  PHASE DRIFT
────────────────────────────────────────────────────────────────────────
  Avg Phase Drift        +0.0638 ms
  Max Phase Drift        -3.0281 ms
  Drift Std Dev           1.8694 ms
  Avg Sync Score           46.63 %   [POOR]

  Percentile        drift_ns     drift_ms
  ──────────  ──────────────   ──────────
  min               -3028105 ns      -3.0281 ms
  p1                -2988577 ns      -2.9886 ms
  p5                -2806300 ns      -2.8063 ms
  p25               -1502481 ns      -1.5025 ms
  p50                 +75375 ns      +0.0754 ms
  p75               +1794716 ns      +1.7947 ms
  p95               +2859363 ns      +2.8594 ms
  p99               +3006594 ns      +3.0066 ms
  max               +3026456 ns      +3.0265 ms
```

Phase drift measures how far each frame's actual presentation timestamp landed
from the nearest ideal vblank boundary. It is always clamped to `±(ideal_ms / 2)`
so the value always names the _closest_ boundary regardless of accumulated offset.
Sign convention: positive = late, negative = early.

**Avg Phase Drift** near zero does not mean frames are landing on-vblank. It means
early and late errors are cancelling out. Always cross-reference with **Drift Std
Dev** and the percentile table.

**Avg Sync Score** is the session mean of the per-frame `sync` field,
`100 × (1 − |drift_ms| / (ideal_ms / 2))`, clamped to [0, 100].

| Range   | Label     |
| ------- | --------- |
| ≥ 90    | EXCELLENT |
| 70 – 90 | GOOD      |
| 50 – 70 | MARGINAL  |
| < 50    | POOR      |

> **Note on long sessions:** the phase origin is fixed at the first valid frame.
> Over long sessions, sub-microsecond residuals accumulate and the origin slowly
> precesses through all vblank phases. This causes avg sync score to trend toward
> ~50% even on a perfectly stable compositor. Per-frame drift values and stutter
> detection are unaffected because they use the nearest boundary, not the origin
> itself.

**Drift percentile table** is computed from `drift_ns` (raw nanosecond field) when
present, falling back to `drift_ms × 1,000,000` otherwise. Both columns are shown
for convenience. The nanosecond column matters for two use cases:

- **PLL / repaint-timer tuning**: a compositor feeding this drift signal into a
  phase-locked loop needs the un-truncated value. At 120 Hz, the float `drift_ms`
  loses ~100 ns of precision per frame.
- **Evaluating phase-lock quality**: if p5 and p95 are both close to `±(ideal_ms/2)`
  in nanoseconds (the half-period boundary), there is effectively no phase lock at
  all, the compositor is presenting at uniformly random phase. The example above
  shows exactly this: p5 = −2.8 ms and p95 = +2.86 ms on a 6.06 ms period, meaning
  the drift is almost uniformly distributed across the full ±3 ms range.

---

### 5. Compositor Bottleneck Analysis

```
  COMPOSITOR BOTTLENECK ANALYSIS
────────────────────────────────────────────────────────────────────────
  Classified frames      305
  GPU overrun                 0  (  0.0%)
  Compositor hold           305  (100.0%)
  Healthy                     0  (  0.0%)
  Mean hold gap          13.8915 ms
  P95  hold gap          15.0814 ms
  Mean slack             27.9190 ms
  P99  slack             30.2394 ms

  [Compositor hold dominant (100.0% of classified frames)]
```

This section is only populated when `slack_ms` **and** `cpu_frame_ms` are both
present in the log. It answers the most important compositor development question:
**when a frame is late, is it because the GPU ran over budget, or because the
compositor held a ready buffer too long?**

Each classified frame falls into exactly one category:

#### GPU Overrun

`slack_ms < ideal_ms × 0.25`

The GPU was still executing at vblank time. The command buffer submitted for this
frame had not finished by the time the display engine wanted to scan it out.
`slack_ms` is near zero because the submit-to-present gap is tiny, the CPU
observed presentation almost immediately after `queue.submit()` returned, meaning
the frame completed very late.

**What to look for:** high `vblank_mul` frames coinciding with GPU overrun. The fix
is in the render budget, reduce scene complexity, switch present modes, or
implement adaptive quality.

#### Compositor Hold

`(slack_ms − cpu_frame_ms) > ideal_ms × 0.5`

The GPU finished its work well before the presentation timestamp. The buffer sat
ready in the compositor's pipeline for longer than half a vblank period before
being flipped. The GPU is not the bottleneck.

**What to look for:**

- `max_render_time` policy set too conservatively, the compositor is artificially
  delaying flips to batch work, causing frames to miss the next vblank even though
  the GPU was done in time.
- Triple-buffer queue depth > 1, the compositor queued more frames than the
  display can consume per vblank, introducing pipeline latency.
- Wayland frame-callback throttling, the client was not sent a `wl_surface.frame`
  callback promptly, delaying the next submit.

**Hold gap** (`slack_ms − cpu_frame_ms`) quantifies how long the ready buffer
waited. Mean and P95 are reported. A P95 hold gap above `ideal_ms` (one full
vblank) is a strong signal of an overly conservative `max_render_time` setting.

#### Healthy

Neither condition is true. `slack_ms` is in the expected range for a one-vblank
pipeline (`≈ ideal_ms`) and the hold gap is within tolerance.

**Mean slack** and **P99 slack** describe the overall distribution of
`submit_to_present_ms` across all classified frames, independent of
category. On a healthy single-vblank Fifo path, mean slack should be
approximately equal to `ideal_ms`.

---

### 6. Double-Buffer Ping-Pong

```
  DOUBLE-BUFFER PING-PONG
────────────────────────────────────────────────────────────────────────
  Not detected  (sign-flip rate: 0.69)
```

Or, when detected:

```
  DOUBLE-BUFFER PING-PONG
────────────────────────────────────────────────────────────────────────
  DETECTED  (sign-flip rate: 0.91)
  Mean fast cadence        7.0142 ms
  Mean slow cadence       11.1063 ms
  Frame-time spread        4.0921 ms

  Systematic alternation: compositor locked to two delivery slots.
  Visible judder likely even at nominal FPS.
```

Ping-pong is a pathological delivery pattern where the compositor alternates
between two fixed frame-time slots, e.g. 7 ms / 11 ms / 7 ms / 11 ms on a
6.06 ms target. The mean frame time may look acceptable (9 ms ≈ 1.5× ideal),
but the eye perceives the alternation as judder because it never settles into a
stable cadence.

**Why rolling jitter misses this:** the rolling jitter average in the Global
Pacing section computes `mean(|delta[n] − delta[n−1]|)`. In a perfect ping-pong
pattern with spread `S`, every consecutive pair flips by exactly `S`, so the
rolling jitter accurately reads `S`. However, in practice the ping-pong is rarely
perfect, there is noise on top of the alternation, and the rolling average
conflates the systematic alternation with random noise. The ping-pong detector
uses `ipc_delta_ms` (the raw instantaneous `delta[n] − delta[n−1]` stored per
frame) to measure the **sign-flip rate** of consecutive values.

**Sign-flip rate:** fraction of consecutive `ipc_delta_ms` pairs where the sign
alternates (positive → negative or negative → positive). In a pure ping-pong,
every pair flips, giving a rate of 1.0. The detection threshold is **0.70**: more
than 70% of consecutive pairs alternating means the pattern is systematic rather
than random (random noise gives a rate of ~0.50).

**Frame-time spread** is `mean_slow − mean_fast`. Values above 2 ms are
perceptually salient on most displays. The example above at 4.09 ms spread would
produce clearly visible judder at 165 Hz.

---

### 7. Stutter Events

```
  STUTTER EVENTS
────────────────────────────────────────────────────────────────────────
  Distinct events    1
  Anomalous frames   421  (99.53% of session)
  Vblanks lost       9

      IDX     WORST Δ    SZ  MISSED  SEVERITY   RECOV. JITTER
  ───────  ──────────  ────  ──────  ─────────  ─────────────
        0    62.6715ms   421       9  CLUSTER              n/a

  ~ = fractional vblank slip (1.25×–2× ideal, no whole vblank missed)
```

A stutter event is any frame whose `delta_ms ≥ 1.5 × ideal_ms`. Consecutive
anomalous frames within 5 global indices of each other are merged into a single
compound **cluster** so that a sustained throughput problem is not reported as
hundreds of independent minor events.

#### Event Table Columns

| Column        | Description                                                                |
| ------------- | -------------------------------------------------------------------------- | --------------------- | ------------------------------------------------------------------------------------------------- |
| IDX           | Global frame index of the first anomalous frame in the cluster             |
| WORST Δ       | `delta_ms` of the single worst frame in the cluster                        |
| SZ            | Number of anomalous frames in the cluster                                  |
| MISSED        | Whole vblanks skipped by the worst frame: `floor(worst_delta / ideal) − 1` |
| SEVERITY      | See table below                                                            |
| RECOV. JITTER | Mean `                                                                     | delta[i] − delta[i−1] | ` over the 8 frames following the event; measures how quickly delivery stabilises after the stall |

A `~` prefix on WORST Δ marks a **fractional slip**: the frame landed between
1.25× and 2× ideal without skipping a whole vblank. These are caused by
sub-vblank scheduling noise rather than a full missed deadline, and are
visually distinct from whole-vblank drops.

#### Severity Levels

| Level    | Condition                        | Typical cause                                                     |
| -------- | -------------------------------- | ----------------------------------------------------------------- |
| MINOR    | Single frame, < 3 vblanks missed | Transient scheduling noise                                        |
| SLIP     | Fractional slip, cluster > 1     | Recurring sub-vblank scheduling variation                         |
| SEVERE   | ≥ 3 vblanks missed               | GPU overrun, TTM eviction, shader compilation stall               |
| CRITICAL | ≥ 7 vblanks missed               | GPU preemption, thermal throttle, device reset                    |
| CLUSTER  | Cluster size ≥ 10                | Sustained throughput degradation; treat as a regime, not an event |

**Recovery jitter** is reported as `n/a` when the event extends to the end of
the log (no recovery frames available) or when the cluster consumed all remaining
frames, as in the example above where 99.5% of the session was anomalous.

---

### 8. Session Phases

```
  SESSION PHASES  (cadence regimes, keyed on global frame index)
────────────────────────────────────────────────────────────────────────
    #      GLOBAL IDX     MEAN Δ   EFF. Hz    JITTER  DOM.×    SYNC
  ───  ──────────────  ─────────  ────────  ─────────  ──────  ──────
    1           0–142   24.2902ms    41.2Hz   4.2831ms      4×  13.9%
    2         143–290   14.1234ms    70.8Hz   1.1020ms      2×  48.2%
    3         291–422    6.1182ms   163.5Hz   0.3104ms      1×  91.7%
```

Session phases are printed only when the analyser detects more than one distinct
delivery-cadence regime. A phase boundary is triggered when the 20-frame rolling
mean of `delta_ms` crosses `1.2 × ideal_ms` in either direction, sustained for at
least 20 frames (to suppress false boundaries from transient stutter events).

This section is most useful for logs that were captured during a warm-up or ramp-up
period, e.g. a benchmark run where shader compilation or asset streaming causes
degraded throughput for the first several seconds before the GPU settles into its
steady-state cadence. Without phase segmentation, the early degraded frames drag
down the session-wide statistics and obscure the steady-state behaviour.

#### Phase Table Columns

| Column     | Description                                                 |
| ---------- | ----------------------------------------------------------- | --------------------- | ------------------- |
| #          | Phase number, chronological                                 |
| GLOBAL IDX | Frame index range (session-global, handles counter resets)  |
| MEAN Δ     | Mean `delta_ms` within this phase                           |
| EFF. Hz    | `1000 / mean_delta`, actual delivery rate during this phase |
| JITTER     | Mean `                                                      | delta[n] − delta[n−1] | ` within this phase |
| DOM.×      | Most common `vblank_mul` in this phase                      |
| SYNC       | Mean sync score within this phase                           |

---

### 9. Verdict

```
  VERDICT: GPU BOUND. Throughput is significantly lower than refresh rate.
```

A single-line summary derived from the three session-wide scalars: V-Sync
multiplier, jitter, and avg sync score.

| Condition                             | Verdict                                                            |
| ------------------------------------- | ------------------------------------------------------------------ |
| Multiplier 0.95–1.05, jitter < 0.5 ms | NATIVE PERFORMANCE. GPU is perfectly tracking the monitor.         |
| Multiplier 0.95–1.05, jitter ≥ 0.5 ms | NATIVE BUT JITTERY. Correct speed, but delivery spacing is uneven. |
| Multiplier > 2.0                      | GPU BOUND. Throughput is significantly lower than refresh rate.    |
| All other                             | ACCEPTABLE. Standard presentation timing.                          |

When the verdict is NATIVE PERFORMANCE but avg sync score is below 70%, an
additional note explains the phase-origin precession effect so it is not
misread as a pacing problem.

---

## Cross-Referencing Sections

The sections are designed to be read together. Common diagnostic workflows:

**"Frames are late but I don't know why"**
→ Check §5 (Bottleneck Analysis). If compositor hold is dominant, examine
`max_render_time` policy and buffer queue depth. If GPU overrun is dominant,
the render budget needs reduction.

**"Average FPS looks fine but it feels choppy"**
→ Check §6 (Ping-Pong). A sign-flip rate above 0.70 with a spread above 2 ms
will produce perceptible judder at any nominal FPS.
→ Check §3 (Vblank Distribution). A mean of 1.5× from a 50/50 split of 1× and
2× frames looks fine in the header but produces stuttery delivery.

**"Performance degrades over time"**
→ Check §8 (Session Phases). If phases show declining EFF. Hz over the session,
look for thermal throttling (sudden CRITICAL stutter events in §7) or
progressive TTM eviction pressure.

**"I want to tune my repaint timer"**
→ Check the drift percentile table in §4. The p5–p95 spread in nanoseconds is
your PLL error budget. If p50 (`drift_ns`) is consistently non-zero, your
repaint timer has a systematic phase offset that can be corrected.

**"Stutter events look infrequent but recovery jitter is high"**
→ The compositor is recovering from stalls slowly. High recovery jitter means
delivery spacing is still irregular for several frames after the stall. Check
whether `max_render_time` is causing the compositor to over-correct after a
stall by holding subsequent frames longer than necessary.

---
