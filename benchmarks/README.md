# Cube Simulator Benchmarks

Performance analysis of the Cube Simulator using different Vulkan presentation modes.

---

## Environment Specs

### Hardware & Driver

- **GPU:** AMD Radeon RX 9060 XT (Discrete)
- **Architecture:** RADV GFX1200
- **Driver:** Mesa 26.1.0-devel (RADV)
- **Vulkan API:** 1.4.344

### Software Layers

- **OS:** Linux
- **Key Layers:**
  - `VK_LAYER_MESA_anti_lag`
  - `VK_LAYER_MANGOHUD_overlay`
  - `VK_LAYER_FROG_gamescope_wsi`

---

## Test Methodology

The benchmarks were executed under the following strict conditions to ensure data consistency:

- **Duration:** 1 minute per run (`timeout 1m`).
- **Workload:** 90 cubes (`-c 90`).
- **Comparison:** Identical workloads applied to both **FIFO** (VSync enabled) and **Mailbox** (Triple Buffering/No Tear) presentation modes.

---

## Results

Detailed frame timing and jitter data can be found in the following reports:

- **[FIFO Benchmarks](./compositor-benchmarks-fifo.md):** Best for consistent frame pacing and power efficiency.
- **[Mailbox Benchmarks](./compositor-benchmarks-mailbox.md):** Best for lowest latency without screen tearing.

---

## Key Metrics Tracked

- **FPS:** Average frames per second.
- **LOW_1:** 1% Lows (Crucial for identifying stutter).
- **JITTER:** Variance in frame delivery times.
- **FTV:** Frame Time Variance.
