WGPU Cube Simulator
===================

This project is a high-precision diagnostic tool built with **Rust** and **WGPU** to measure **JIT (Just-In-Time) presentation latency** and frame pacing stability under heavy load. By utilizing a raymarched fragment shader rather than standard rasterization, it allows for granular control over GPU throughput to identify compositor bottlenecks and V-Sync implementation flaws.

* * *

> [!IMPORTANT]
> **Priority One:** For effective diagnostic testing, the workload should be increased (using the `--cubes` argument) until the **FPS drops below 60**.
>
> Saturating the GPU to this level is the only way to reliably expose frame pacing issues, as it removes any "buffer cushion" and forces the compositor's synchronization flaws to manifest as visible stutter or JIT spikes.

* **JIT Detection:** Identifies the delta between application-side render submission and hardware-side presentation.
* **Compositor Benchmarking:** Highlights the architectural gap between modern compositors.
* **V-Sync Profiling:** Specifically targets the detection of "Back-Pressure" in the swapchain, where missed V-Blank intervals at high refresh rates cause cascading latency spikes.

### Installation and Usage

To get accurate JIT metrics, you must compile with the release profile to minimize CPU-side scheduling interference and driver overhead:

    cargo build --release
    ./target/release/frame-test -c 120

### CLI Parameters

| Argument | Description | Default |
| :--- | :--- | :--- |
| `-c, --cubes` | Number of hollow cubes to march. | 120 |
| `-s, --size` | Radius/Scale of the objects. | 0.5 |
| `--speed` | Multiplier for rotation and oscillation. | 1.0 |
| `--red, --green, --blue` | RGB float components (0.0 to 1.0). | 0.5, 0.8, 0.2 |

***

### Technical Metrics

The on-screen display provides real-time telemetry used to detect jitter

### Performance Note: Why Raymarching?

Unlike triangle-based engines, raymarching is exponentially expensive based on the complexity of the `map()` function. Every pixel executes a distance field loop for every cube added. This creates a **purely GPU-bound** environment, which is the only way to accurately test if a compositor's V-Sync implementation can handle high-throughput scenarios without introducing artificial input lag or flickering.
