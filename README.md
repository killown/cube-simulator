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

| Argument          | Description                                                                                       | Default          |
| :---------------- | :------------------------------------------------------------------------------------------------ | :--------------- |
| `-c, --cubes`     | Number of hollow cubes to march.                                                                  | 6                |
| `-s, --size`      | Radius/Scale of the objects.                                                                      | 0.5              |
| `-t, --threshold` | Frame-time delta limit (ms) for MSD (Missed Frames).                                              | 25.0             |
| `-f, --format`    | Force a specific `wgpu::TextureFormat` (e.g., `Rgba8Unorm`). Prints available options if invalid. | None             |
| `-m, --mode`      | Force a specific `wgpu::PresentMode` (`mailbox`, `immediate`, `fifo`).                            | `mailbox` (auto) |
| `--steps`         | Maximum raymarching steps per fragment. Higher values increase GPU load.                          | 80               |
| `--speed`         | Multiplier for rotation and oscillation.                                                          | 1.0              |
| `--red`           | Red color component (0.0 to 1.0).                                                                 | 0.5              |
| `--green`         | Green color component (0.0 to 1.0).                                                               | 0.8              |
| `--blue`          | Blue color component (0.0 to 1.0).                                                                | 0.2              |
| `--csv`           | Optional path to write metrics as CSV for offline analysis (e.g., `--csv out.csv`).               | None             |

### Present Mode Diagnostics

The simulator automatically selects the best available present mode (Mailbox > Immediate > Fifo) and prints the selection to the terminal with the following behavior:

- **Fifo (Standard VSync):** Standard VSync logic. Blocks the CPU to match the monitor's refresh rate. The driver and compositor handle synchronization internally; frame pacing is controlled via the display refresh cycle.
- **Mailbox (Triple Buffering):** A non-blocking mode that replaces the oldest frame in the queue. Ideal for measuring raw compositor scheduling behaviour.
- **Immediate (Uncapped):** Renders as fast as possible without sync, providing the rawest performance data but potentially causing screen tearing.

---

### Quick Usage Examples

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

- **JIT (Jitter)**
  The average variance (in milliseconds) between consecutive frame times. Calculated as the mean of `abs(frame_time[i] - frame_time[i-1])`. High jitter indicates inconsistent frame pacing. Even if the application averages a perfect 60 FPS (16.6ms), alternating between 10ms and 23ms frames will produce a visually unpleasant micro-stuttering experience.

- **MSD (Missed Frames)**
  A per-window counter of macro-stutters and severe application stalls. A frame is only evaluated here if its duration exceeds the configurable threshold (default: `25.0ms`). When a stall occurs, the total lost time is divided by the monitor's actual frame budget (queried at startup from the display's refresh rate) to calculate the discrete number of dropped presentation beats. This explicitly isolates true hardware/engine hitches from standard compositor noise.

- **FTV (Frame Time Variance %)**
  The coefficient of variation of frame times within the rolling window, expressed as a percentage (`stddev / mean * 100`). This metric directly captures how evenly frames are distributed across the 1000ms budget.

  A value near **0%** means all frames took approximately the same time perfectly uniform delivery. A high value means frame times are spread widely: some frames complete in a few milliseconds while others take tens of milliseconds. Even if the mean FPS looks acceptable, this imbalance causes frames to bunch together and then stall, which the eye perceives as judder or skipping.

  > **Example:** A sequence of `[5ms, 48ms, 6ms, 47ms]` averages to roughly 19 FPS, but the near-zero gaps between paired frames make the presentation look as if frames are being skipped entirely, because two frames arrive nearly simultaneously followed by a long gap. FTV will read high in this scenario while JIT and FPS alone may not tell the full story.

  > **Note:** FTV is measured entirely from CPU-side frame timestamps and is equally valid across all Wayland compositors (wlroots, Smithay, Mutter and so on) regardless of how each compositor internally schedules frame callbacks or swapchain synchronization.

---

### Performance Note: Why Raymarching?

Unlike triangle-based engines, raymarching is exponentially expensive based on the complexity of the `map()` function. Every pixel executes a distance field loop for every cube added. This creates a **purely GPU-bound** environment, which is the only way to accurately test if a compositor's V-Sync implementation can handle high-throughput scenarios without introducing artificial input lag or flickering.
