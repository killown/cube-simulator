# WGPU Cube Simulator

This project is a high-precision diagnostic tool built with **Rust** and **WGPU** to measure **JIT (Just-In-Time) presentation latency** and frame pacing stability under heavy load. By utilizing a raymarched fragment shader rather than standard rasterization, it allows for granular control over GPU throughput to identify compositor bottlenecks and V-Sync implementation flaws.

---

> [!IMPORTANT]
> **Priority One:** For effective diagnostic testing, the workload should be increased (using the `--cubes` argument) until the **FPS drops below 60**.
>
> Saturating the GPU to this level is the only way to reliably expose frame pacing issues, as it removes any "buffer cushion" and forces the compositor's synchronization flaws to manifest as visible stutter or JIT spikes.

- **JIT Detection:** Identifies the delta between application-side render submission and hardware-side presentation.
- **Compositor Benchmarking:** Highlights the architectural gap between modern compositors.
- **V-Sync Profiling:** Specifically targets the detection of "Back-Pressure" in the swapchain, where missed V-Blank intervals at high refresh rates cause cascading latency spikes.

### Installation and Usage

To get accurate JIT metrics, you must compile with the release profile to minimize CPU-side scheduling interference and driver overhead:

    cargo build --release
    ./target/release/frame-test -c 120

### CLI Parameters

| Argument          | Description                                          | Default |
| ----------------- | ---------------------------------------------------- | ------- |
| `-c, --cubes`     | Number of hollow cubes to march.                     | 120     |
| `-s, --size`      | Radius/Scale of the objects.                         | 0.5     |
| `-t, --threshold` | Frame-time delta limit (ms) for MSD (Missed Frames). | 25.0    |
| `--speed`         | Multiplier for rotation and oscillation.             | 1.0     |
| `--red`           | Red color component (0.0 to 1.0).                    | 0.5     |
| `--green`         | Green color component (0.0 to 1.0).                  | 0.8     |
| `--blue`          | Blue color component (0.0 to 1.0).                   | 0.2     |

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
  The average frame rate calculated exclusively from the slowest 1% of frame times within the 500ms window. This is the primary indicator of subjective smoothness. A high average FPS combined with a poor 1% Low indicates isolated, severe frame drops that the user will perceive as stutter.

## Advanced Pacing & Stability

- **JIT (Jitter)**
  The average variance (in milliseconds) between consecutive frame times. Calculated mathematically as the average of `abs(frame_time[i] - frame_time[i-1])`. High jitter indicates inconsistent frame pacing. Even if the application averages a perfect 60 FPS (16.6ms), alternating between 10ms and 23ms frames will produce a visually unpleasant, "micro-stuttering" experience.

- **MSD (Missed Frames)**
  A cumulative counter of macro-stutters and severe application stalls. A frame is only evaluated here if its duration exceeds the configurable threshold (default: `25.0ms`). When a stall occurs, the total lost time is divided by the universal VSync interval (`16.66ms`) to calculate the discrete number of dropped presentation beats. This explicitly isolates true hardware/engine hitches from standard compositor noise.

- **ACQ (Acquire Time)**
  The duration (in milliseconds) the application thread is blocked waiting for `surface.get_current_texture()`. This metric maps directly to presentation back-pressure from the display server or Wayland compositor. If the compositor's buffers are saturated, or it is intentionally throttling the application to maintain desktop stability, `ACQ` will spike. This proves the bottleneck exists in the OS presentation layer, not the application's internal GPU work submission.

### Performance Note: Why Raymarching?

Unlike triangle-based engines, raymarching is exponentially expensive based on the complexity of the `map()` function. Every pixel executes a distance field loop for every cube added. This creates a **purely GPU-bound** environment, which is the only way to accurately test if a compositor's V-Sync implementation can handle high-throughput scenarios without introducing artificial input lag or flickering.
