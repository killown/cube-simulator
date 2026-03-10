# Cube Simulator Benchmarks

Performance analysis of the Cube Simulator using different Vulkan presentation modes.

---

## Environment Specs

### Hardware & Driver

- **GPU:** AMD Radeon RX 9060 XT (Discrete)
- **Architecture:** RADV GFX1200
- **Driver:** Mesa 26.1.0-devel (RADV)
- **Vulkan API:** 1.4.344
- **RAM:** 32GB DDR4 @ 3200 MT/s
  - **Configuration:** Asymmetric Flex Mode (8GB + 8GB + 16GB)
  - **Note:** First 24GB operates in dual-channel; remaining 8GB in single-channel.
- **CPU:** [AMD Ryzen 5 5600X 6-Core]

### Display Topology (Test Target)

- **Output:** HDMI-A-1
- **Resolution:** 1920x1080 @ 165Hz
- **Adaptive Sync:** Enabled (VRR Active)

### Software & Layers

- **OS:** Linux (Wayland)
- **Active Layers:**
  - `VK_LAYER_MESA_anti_lag`: Reduces input-to-display latency.
  - `VK_LAYER_MANGOHUD_overlay`: Performance monitoring.
  - `VK_LAYER_FROG_gamescope_wsi`: Optimized Wayland swapchain handling.

---

## Test Methodology

The benchmarks were executed under the following strict conditions to ensure data consistency:

- **Duration:** 1 minute per run (`timeout 1m`).
- **Workload:** 90 cubes (`-c 90`).
- **Data Capture:** Metrics sampled every 500ms.
- **Storage:** Every run generates a data file (`.csv` or `.json`) and a paired `-info.txt` file containing the command string, system stats, and hardware state at execution time.
- **Comparison:** Identical workloads applied to **FIFO** (Standard VSync) and **Mailbox** (Triple Buffering) to measure compositor back-pressure and frame pacing across different environments.

---

## Key Metrics Tracked

| Metric     | Description                  | Benchmark Significance                               |
| :--------- | :--------------------------- | :--------------------------------------------------- |
| **FPS**    | Average Frames Per Second    | Raw throughput capability.                           |
| **LOW_1**  | 1% Low FPS                   | Identifies micro-stutter and cache-miss hits.        |
| **JITTER** | Frame-to-frame variance (ms) | Measures the "smoothness" of frame delivery.         |
| **FTV**    | Frame Time Variance %        | Ratio of stddev to mean; indicates pacing stability. |

---

## Results

Detailed frame timing and jitter data can be found in the following reports:

- **[FIFO Benchmarks](./compositor-benchmarks-fifo.md):** Analysis of monitor-synchronized pacing (165Hz target).
- **[Mailbox Benchmarks](./compositor-benchmarks-mailbox.md):** Triple-buffering analysis focusing on uncapped internal framerates.

---
