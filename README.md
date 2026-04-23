# WGPU Cube Simulator

This is a high-precision diagnostic tool for identifying how different compositors manage frame scheduling and presentation under load. Built with Rust and WGPU, it uses a heavy raymarched shader to stress-test your display stack, exposing which environments maintain a smooth, responsive pipeline and which ones collapse into stutter and lag, even when the hardware is still fast enough to keep up.

---

### Core Capabilities
- **Pacing Detection:** Captures micro-stutters invisible to raw FPS counters by measuring the precise gap between every hardware-level frame presentation.
- **Compositor Benchmarking:** Automatically identifies the "saturation threshold", the exact limit where the compositor fails to maintain perfect V-Blank alignment. The benchmark captures the moment a single hardware frame-slot is missed and report it.
- **V-Sync Profiling:** Catches "Back-Pressure", a glitch where missing one frame causes a pile-up that spikes input lag and makes the system feel sluggish.

For a detailed breakdown of the metrics and internal mechanics, see the Project Wiki:
* [**Technical Deep Dive: How it Works**](https://github.com/killown/cube-simulator/wiki/Technical-Deep-Dive:-How-it-Works)

### Installation and Usage

To get accurate metrics, you must compile with the release profile to minimize CPU-side scheduling interference and driver overhead:

    cargo build --release
    target/release/frame-test --connector DP-1 --bench-secs 3 --mode fifo


### CLI Parameters

| Argument          | Description                                                                  | Default          |
| :---------------- | :--------------------------------------------------------------------------- | :--------------- |
| `-c, --cubes`     | Number of hollow cubes to march.                                             | 6                |
| `-z, --size`      | Radius/Scale of the objects.                                                 | 0.5              |
| `-s, --speed`     | Multiplier for rotation and oscillation.                                     | 1.0              |
| `--red`           | Red color component (0.0 to 1.0).                                            | 0.18             |
| `--green`         | Green color component (0.0 to 1.0).                                          | 0.18             |
| `--blue`          | Blue color component (0.0 to 1.0).                                           | 0.18             |
| `-t, --threshold` | Frame-time delta limit (ms) for MSD (Missed Frames).                         | 25.0             |
| `-f, --format`    | Force a specific `wgpu::TextureFormat`. Prints available options if invalid. | None             |
| `-m, --mode`      | Force a specific `wgpu::PresentMode` (`mailbox`, `immediate`, `fifo`).       | `mailbox` (auto) |
| `--steps`         | Maximum raymarching steps per fragment. Higher values increase GPU load.     | 80               |
| `--connector`     | DRM connector name (e.g. `DP-1`). Auto-selects if only one is active.        | Auto             |
| `--frame-log`     | Path to write high-res per-frame hardware telemetry (NDJSON).                | None             |
| `--csv`           | Path to write rolling window metrics as CSV.                                 | None             |
| `--json`          | Path to write rolling window metrics as NDJSON.                              | None             |
| `--bench-secs`    | Seconds per step. Enables benchmark mode when set.                           | None             |
| `--bench-warmup`  | Seconds to skip at start of each benchmark step (compositor warmup).         | 2                |
| `--bench-max`     | Maximum cube count to probe in benchmark mode.                               | 64               |
| `--shader`        | Override shader selection (`high` for SDF, `low` for analytic).              | Auto             |
| `--pll`           | Enable PI controller to sync frame submission with hardware vblank.s.        | `false`          |

---

## License
This project is licensed under the **GNU General Public License v3.0**. See the [LICENSE](https://github.com/killown/cube-simulator/blob/master/LICENSE) file for the full text.
