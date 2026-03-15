# WGPU Cube Simulator

This project is a high-precision diagnostic tool built with **Rust** and **WGPU** to measure **frame pacing stability and presentation latency** under heavy GPU load. By utilizing a raymarched fragment shader rather than standard rasterization, it allows for granular control over GPU throughput to identify compositor bottlenecks and V-Sync implementation flaws.

---

> [!IMPORTANT]
> **Priority One:** For effective diagnostic testing, the workload should be increased (using the `--cubes` argument) until the **FPS drops below 60**.
>
> Saturating the GPU to this level is the only way to reliably expose frame pacing issues, as it removes any "buffer cushion" and forces the compositor's synchronization flaws to manifest as visible stutter or pacing spikes.

- **Pacing Detection:** Measures the statistical spread of frame delivery times to identify bunching and skipping invisible to raw FPS counters.
- **Compositor Benchmarking:** Highlights the architectural gap between modern compositors.
- **V-Sync Profiling:** Specifically targets the detection of "Back-Pressure" in the swapchain, where missed V-Blank intervals at high refresh rates cause cascading latency spikes.

### Installation and Usage

To get accurate metrics, you must compile with the release profile to minimize CPU-side scheduling interference and driver overhead:

    cargo build --release
    ./target/release/frame-test -c 120

### CLI Parameters

| Argument          | Description                                                                  | Default          |
| :---------------- | :--------------------------------------------------------------------------- | :--------------- |
| `-c, --cubes`     | Number of hollow cubes to march.                                             | 6                |
| `-z, --size`      | Radius/Scale of the objects.                                                 | 0.5              |
| `-s, --speed`     | Multiplier for rotation and oscillation.                                     | 1.0              |
| `--red`           | Red color component (0.0 to 1.0).                                            | 0.5              |
| `--green`         | Green color component (0.0 to 1.0).                                          | 0.8              |
| `--blue`          | Blue color component (0.0 to 1.0).                                           | 0.2              |
| `-t, --threshold` | Frame-time delta limit (ms) for MSD (Missed Frames).                         | 25.0             |
| `-f, --format`    | Force a specific `wgpu::TextureFormat`. Prints available options if invalid. | None             |
| `-m, --mode`      | Force a specific `wgpu::PresentMode` (`mailbox`, `immediate`, `fifo`).       | `mailbox` (auto) |
| `--steps`         | Maximum raymarching steps per fragment. Higher values increase GPU load.     | 80               |
| `--connector`     | DRM connector name (e.g. `DP-1`). Auto-selects if only one is active.        | Auto             |
| `--frame-log`     | Path to write high-res per-frame hardware telemetry (NDJSON).                | None             |
| `--csv`           | Path to write rolling window metrics as CSV.                                 | None             |
| `--json`          | Path to write rolling window metrics as NDJSON.                              | None             |

---

> **`TS_SOURCE` column** (CSV/JSON output only): Each row includes a `TS_SOURCE` field set to either `hw` or `cpu`. `hw` means jitter and FTV for that window were computed from WSI-domain KMS flip timestamps (compositor-resistant). `cpu` means the backend returned an invalid presentation timestamp and the metrics fell back to CPU-side `Instant` deltas.

### Low-Level Telemetry (`--frame-log`)

The `.json` frame log provides nanosecond-resolution insight for every scanout event using kernel-level timestamps (`CLOCK_MONOTONIC`). This bypasses compositor scheduling to reveal true hardware pacing.

Optional fields (`ipc_delta_ms`, `cpu_frame_ms`, `slack_ms`) are omitted from a record entirely when the backend cannot supply the required source data, never defaulted to zero.

- **`ts_ns`** — Absolute KMS/WSI presentation timestamp in nanoseconds (`CLOCK_MONOTONIC`). The exact moment the display hardware finished the page flip. Directly comparable to `wl_surface.frame` callback timestamps and `DRM_IOCTL_WAIT_VBLANK` reply timestamps, same clock epoch.

- **`delta_ms`** — Actual time elapsed between consecutive hardware scan-outs, measured entirely on the presentation clock. The only timing value in the log that compositor scheduling policy (`max_render_time`, Wayland frame callbacks) cannot fabricate.

- **`ideal_ms`** — Target vblank period derived from the monitor's reported hardware refresh rate (e.g. `6.0606 ms` at 165 Hz). Constant within a session; sourced from DRM connector data when available, falling back to the winit monitor query.

- **`drift_ms`** — Signed deviation from the nearest ideal vblank boundary. `+` = late (frame missed its slot and was held to the next vblank). `−` = early (rare on vsync'd paths; possible in Immediate mode). Always clamped to `±(ideal_ms / 2)` so it names the _nearest_ boundary regardless of accumulated offset, a value of `+8 ms` on a 16 ms budget means one half-period late, not a frame that has been accumulating drift for 8 ms.

- **`drift_ns`** — The same signed drift as `drift_ms` but at raw nanosecond precision, before float truncation. `drift_ms` loses approximately 100 ns of precision per frame at 120 Hz. Use `drift_ns` when feeding this signal into a PLL or repaint-timer correction loop where sub-microsecond accuracy matters.

- **`vblank_mul`** — How many vblank periods this frame consumed, derived as `max(1, round(delta_ms / ideal_ms))`. `1` = on time. `2` = one vblank dropped (GPU overran its budget or compositor missed its deadline). `≥ 3` = severe stall (TTM eviction, GPU preemption, thermal throttle). Reading this directly from the log is faster than computing it manually from `delta_ms / ideal_ms`.

- **`sync`** — Frame sync quality score 0–100. Computed as `100 × (1 − |drift_ms| / (ideal_ms / 2))`, clamped to `[0, 100]`. `100` means the frame landed exactly on a vblank pulse. `0` means it landed at the worst possible point, exactly halfway between two vblank boundaries. Scores ≥ 95 are perceptually indistinguishable from perfect pacing on all display types.

- **`ipc_delta_ms`** _(optional)_ — Instantaneous inter-frame interval change: `delta_ms[n] − delta_ms[n−1]`. This is the raw per-frame jitter signal, distinct from the rolling jitter average reported in the tick log. Its primary use is detecting double-buffer ping-pong: a compositor locked into alternating fast/slow delivery (e.g. 7 ms / 11 ms / 7 ms / 11 ms) will produce a systematic sign-alternation in `ipc_delta_ms` that rolling averages partially cancel out but is clearly visible here. Absent on the first valid frame.

- **`cpu_frame_ms`** _(optional)_ — CPU-observed total frame time from `RedrawRequested` to `present()` return, in milliseconds. This is a `std::time::Instant` measurement, **not** the presentation clock. Cross-reference with `delta_ms` to diagnose buffer-hold behaviour:
  - `delta_ms ≈ cpu_frame_ms` → frame was scanned out immediately after the CPU finished work.
  - `delta_ms >> cpu_frame_ms` → the GPU finished on time but the compositor held the ready buffer, indicative of an overly conservative `max_render_time` policy, a triple-buffer queue deeper than 1, or Wayland frame-callback throttling.
  - `delta_ms < cpu_frame_ms` → impossible on vsync'd paths; indicates clock skew between the CPU and WSI domains.

- **`slack_ms`** _(optional)_ — Time from CPU-observed `queue.submit()` return to the hardware presentation timestamp, in milliseconds. Approximates GPU execution time plus driver flip pipeline depth as seen from the CPU timeline. On a healthy single-vblank Fifo path, `slack_ms ≈ ideal_ms`. Cross-reference with `drift_ms` to locate the bottleneck:

  | `drift_ms` | `slack_ms`   | Diagnosis                                                                                                                 |
  | ---------- | ------------ | ------------------------------------------------------------------------------------------------------------------------- |
  | High       | High         | GPU finished well before vblank; buffer sat in the compositor's queue, **compositor scheduling policy** is the bottleneck |
  | High       | ≈ 0          | GPU was still executing at vblank time, **GPU render budget** was exceeded                                                |
  | Low        | ≈ `ideal_ms` | Healthy one-vblank pipeline                                                                                               |

---

### Present Mode Diagnostics

The simulator automatically selects the best available present mode (Mailbox > Immediate > Fifo) and prints the selection to the terminal with the following behavior:

- **Fifo (Standard VSync):** Standard VSync logic. Blocks the CPU to match the monitor's refresh rate. The driver and compositor handle synchronization internally; frame pacing is controlled via the display refresh cycle.
- **Mailbox (Triple Buffering):** A non-blocking mode that replaces the oldest frame in the queue. Ideal for measuring raw compositor scheduling behaviour.
- **Immediate (Uncapped):** Renders as fast as possible without sync, providing the rawest performance data but potentially causing screen tearing.

---

### Benchmark Mode (`--bench-secs`)

Automatically sweeps cube counts from `1` up to `--bench-max`, holding each count for `--bench-secs` seconds. The first `--bench-warmup` seconds of every step are discarded so compositor startup jitter does not pollute the signal. The sweep stops and the window closes automatically when a pacing signal fires or the full range completes cleanly.

| Argument         | Description                                                              | Default |
| :--------------- | :----------------------------------------------------------------------- | :------ |
| `--bench-secs`   | Seconds per step. Enables benchmark mode when set.                       | None    |
| `--bench-warmup` | Seconds to discard at the start of each step (must be < `--bench-secs`). | `2`     |
| `--bench-max`    | Maximum cube count to probe before declaring a clean sweep.              | `64`    |

**Stop conditions** (checked only after the warmup window expires):

- **Yellow** — `vblank_mul > 1` on any single frame (amber ring fires). An isolated missed vblank slot.
- **Red** — `EMA(vblank_mul) > 1.15` (red diamond fires). Sustained compositor pressure, not just a spike.

The final report is printed to stdout before the window closes, showing the maximum clean cube count and the trigger that ended the sweep.

### Quick Usage Examples

#### Benchmark

```bash
# 5s per step, default 2s warmup, probe up to 32 cubes
target/release/frame-test --bench-secs 5 --bench-max 32 --connector DP-1

# Longer measurement window with tighter warmup
target/release/frame-test --bench-secs 10 --bench-warmup 3 --bench-max 64 --connector HDMI-A-1
```

#### Stress Test (120 Cubes, 10ms Target)

Runs the simulation with 120 cubes and flags any frame taking longer than 10ms as a "Missed Frame" (MSD).

```
target/release/frame-test -c 120 -t 10
```

#### Custom Color Profile (Purple)

Sets the RGB components manually to create a specific color output.

```
target/release/frame-test --red 0.6 --green 0.1 --blue 0.9
```

#### Visual Inspection (Slow & Large)

Increases cube size and slows down the rotation speed to inspect the raymarching edge detection.

```
target/release/frame-test --size 1.2 --speed 0.2
```

#### CSV Output

Writes per-window metrics to a file for offline analysis.

```
target/release/frame-test -c 120 --csv results.csv
```

#### Full Reset

Runs the simulator with all compiled default values.

```
target/release/frame-test
```

---

# WGPU Cube Simulator: Telemetry Metrics

## General Performance Throughput

- **FPS (Frames Per Second)**
  The rolling average of frames rendered over the last 500ms update window. This represents the baseline rendering throughput of the GPU and the application loop.

- **MIN (Minimum FPS)**
  The absolute lowest 500ms rolling average recorded since the application started. This highlights sustained worst-case performance under maximum load.

- **MAX (Maximum FPS)**
  The absolute highest 500ms rolling average recorded. This represents peak hardware capability when the raymarching shader is under minimal load (e.g., few overlapping cubes in the view frustum).

- **LOW (1% Low FPS)**
  The average frame rate calculated exclusively from the slowest 1% of frame times within the rolling window. This is the primary indicator of subjective smoothness. A high average FPS combined with a poor 1% Low indicates isolated, severe frame drops that the user will perceive as stutter.

## Advanced Pacing & Stability

- **Sustained Pressure** — An amber-yellow hollow ring appears in the top-right corner when `vblank_mul > 1` (any single frame missed its vblank slot). Fades over ~30 frames as a momentary ping.

- **Stutter Flash** — A red filled diamond overlays the ring when the rolling `vblank_mul` EMA exceeds `1.15`, the compositor is consistently missing deadlines, not just spiking. Lingers ~45 frames.

- **JIT (Jitter)**
  The average variance (in milliseconds) between consecutive frame times. Calculated as the mean of `abs(frame_time[i] - frame_time[i-1])`. High jitter indicates inconsistent frame pacing. Even if the application averages a perfect 60 FPS (16.6ms), alternating between 10ms and 23ms frames will produce a visually unpleasant micro-stuttering experience.

- **MSD (Missed Frames)**
  A per-window counter of macro-stutters and severe application stalls. A frame is only evaluated here if its duration exceeds the configurable threshold (default: `25.0ms`). When a stall occurs, the total lost time is divided by the monitor's actual frame budget (queried at startup from the display's refresh rate) to calculate the discrete number of dropped presentation beats. This explicitly isolates true hardware/engine hitches from standard compositor noise.

- **FTV (Frame Time Variance %)**
  The coefficient of variation of frame times within the rolling window, expressed as a percentage (`stddev / mean * 100`). This metric directly captures how evenly frames are distributed across the 1000ms budget.

  A value near **0%** means all frames took approximately the same time perfectly uniform delivery. A high value means frame times are spread widely: some frames complete in a few milliseconds while others take tens of milliseconds. Even if the mean FPS looks acceptable, this imbalance causes frames to bunch together and then stall, which the eye perceives as judder or skipping.

- **CPU (CPU Time)**
  The total time in milliseconds spent on the host processor to prepare and submit a frame. This includes input processing, world state updates, and encoding the `wgpu` command buffer. High CPU time relative to the frame budget suggests the application is "CPU bound," which can lead to input lag and frame drops even if the GPU is idle.

- **GPU (GPU Time)**
  The actual hardware execution time on the graphics card, measured via `TIMESTAMP_QUERY`. This represents the duration from when the GPU starts processing the first draw call to when the final render pass completes. By comparing this against the frame budget, you can determine if the shader complexity or cube count is exceeding the hardware's fill rate or compute throughput.

- **SYN (Sync Score)**
  A phase-locked alignment metric (0–100) representing how consistently the application's presentation hits the monitor's VSync intervals. A score of **100** indicates perfect phase-alignment with the display's hardware clock. A dropping score indicates "VBlank drift," where the application is losing its synchronization with the compositor, often a precursor to dropped frames or fluctuating latency.

> **Example:** A sequence of `[5ms, 48ms, 6ms, 47ms]` averages to roughly 19 FPS, but the near-zero gaps between paired frames make the presentation look as if frames are being skipped entirely, because two frames arrive nearly simultaneously followed by a long gap. FTV will read high in this scenario while JIT and FPS alone may not tell the full story.

### Performance Note: Why Raymarching?

Unlike triangle-based engines, raymarching is exponentially expensive based on the complexity of the `map()` function. Every pixel executes a distance field loop for every cube added. This creates a **purely GPU-bound** environment, which is the only way to accurately test if a compositor's V-Sync implementation can handle high-throughput scenarios without introducing artificial input lag or flickering.

### GPU Micro-Stutter Diagnostics (`[MICRO]` output)

When a vblank miss is detected, a `[MICRO]` line is printed to stderr in real
time. These lines decompose the missed frame into three distinct latency
sources so you know immediately whether to blame the shader, the driver, or
the DMA subsystem.

```
[MICRO] vblank×3 — SHADER OVERRUN (driver 0.32ms  shader 23.72ms  resolve 0.05ms  total 24.08ms)
```

#### Fields

| Field         | What it measures                                                                 |
| :------------ | :------------------------------------------------------------------------------- |
| `vblank×N`    | How many vblank periods this frame consumed. `×2` = one missed slot. `×3` = two. |
| `driver Nms`  | Time from CPU submit to first GPU instruction. Normal: < 1 ms.                   |
| `shader Nms`  | True GPU render-pass execution time from hardware timestamp queries.             |
| `resolve Nms` | GPU DMA copy overhead for the timestamp readback buffer. Normal: < 0.1 ms.       |
| `total Nms`   | Full GPU pipeline span from submit to resolve complete.                          |

#### Cause labels and what to do

| Label               | Meaning                                                                                                        | Action                                                                                         |
| :------------------ | :------------------------------------------------------------------------------------------------------------- | :--------------------------------------------------------------------------------------------- |
| `SHADER OVERRUN`    | `shader_ms > frame_budget_ms`. The fragment workload exceeded the budget.                                      | Reduce `--cubes` or `--steps`.                                                                 |
| `DRIVER STALL`      | `driver_ms > 2ms`. The Vulkan driver held the command buffer unusually long before scheduling it onto the GPU. | Usually transient. Persistent stalls may indicate driver or thermal issues.                    |
| `RESOLVE/DMA SPIKE` | `resolve_ms > 0.3ms`. The DMA copy of timestamp data spiked.                                                   | Usually a one-off TTM eviction or PCIe contention. Persistent spikes indicate memory pressure. |

#### Reading a session

The first one or two `[MICRO]` lines after startup will show `shader 0.00ms`
and may be mislabelled as `DRIVER STALL`. This is expected: the outer GPU
timer lags one frame, so shader time is not yet available on the very first
readback. Discard these cold-start lines.

Once shader time is populated, the labels are accurate. A healthy session
under manageable load shows no `[MICRO]` output at all. Lines only appear
when a vblank is actually missed.

### Why FPS is unreliable with the low-end shader

The low-end shader uses an analytic ray-box intersection to render each cube.
Unlike the high-end raymarcher, which always evaluates every cube for every
pixel regardless of what the scene looks like, the low-end shader can exit
early when a ray misses a cube's bounding box entirely.

This sounds like an optimisation, and it is, but it makes the GPU cost
**dependent on the current animation frame**, not just the cube count.

When cubes are spread far apart, most rays miss most cubes and the early exits
fire frequently. When cubes happen to cluster together or align with the camera,
more rays enter more bounding boxes and the full intersection math runs. The
cost of a single frame can swing by 20–40% between these two extremes.

Since the cubes animate continuously, this variance shows up directly as FPS
instability. Two consecutive seconds at the same `-c` value can produce
noticeably different frame times just because the scene moved. More
counterintuitively, **`-c 5` can be slower than `-c 6`** at a given moment
because five widely-spaced cubes force more rays to traverse the full scene
depth than six cubes that happen to cluster and occlude each other.

**This makes the LOW 1% FPS figure meaningless as a compositor diagnostic.**
The 1% lows will reflect animation configuration as much as compositor
behaviour, so two runs with identical hardware and compositors can produce
different LOW scores just because they started at different times.

The high-end raymarcher does not have this problem. Its SDF loop always runs
`--steps` iterations for every pixel and evaluates every cube on every
iteration, the cost is strictly `steps × cube_count × pixel_count`, constant
regardless of scene geometry. This is why it is the correct shader for any
real compositor pressure test or benchmark.

The low-end shader exists only to provide a usable visual on hardware that
cannot sustain the raymarcher at even `-c 1`. If your hardware can run the
high-end shader at all, use it.
