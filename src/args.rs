use clap::Parser;

#[derive(Parser, Debug, Clone)]
#[command(author, version, about = "WGPU Cube Simulator")]
pub struct Args {
    #[arg(short, long, default_value_t = 6)]
    pub cubes: u32,
    #[arg(short = 'z', long, default_value_t = 0.5)]
    pub size: f32,
    #[arg(short, long, default_value_t = 1.0)]
    pub speed: f32,
    #[arg(long, default_value_t = 0.18)]
    pub red: f32,
    #[arg(long, default_value_t = 0.18)]
    pub green: f32,
    #[arg(long, default_value_t = 0.18)]
    pub blue: f32,
    #[arg(short = 't', long, default_value_t = 25.0)]
    pub threshold: f32,
    #[arg(short = 'f', long)]
    pub format: Option<String>,
    #[arg(short = 'm', long)]
    pub mode: Option<String>,
    #[arg(long, default_value_t = 80)]
    pub steps: u32,
    #[arg(long)]
    pub csv: Option<String>,
    #[arg(long)]
    pub json: Option<String>,
    #[arg(long)]
    pub frame_log: Option<String>,
    /// DRM connector to read the refresh rate from (e.g. `DP-1`, `HDMI-A-1`).
    /// Run without this flag to print all active connectors and their modes.
    #[arg(long)]
    pub connector: Option<String>,
    /// Swapchain frame latency (desired_maximum_frame_latency).
    ///
    /// Controls how many frames the compositor may buffer ahead of scanout.
    /// `2` (default) matches the wgpu default and gives the async GPU timestamp
    /// readback enough pipeline depth to complete before the next frame calls
    /// `gpu_timer.poll()`, keeping the GPU OSD field populated. `1` reduces
    /// input latency but risks the readback not completing in time, leaving
    /// `gpu_time_ms` as zero until a `device.poll()` catches up. Values above
    /// `3` inflate all delivery-pressure metrics with no practical benefit.
    #[arg(long, default_value_t = 2)]
    pub latency: u32,
    /// Enable compositor stress benchmark mode.
    ///
    /// Tests cube counts 1, 2, 3, … N seconds each. The first `--bench-warmup`
    /// seconds of every step are discarded so compositor startup jitter does not
    /// pollute the signal. Stops and reports results when `vblank_mul > 1`
    /// (yellow ring) or `EMA(vblank_mul) > 1.15` (red diamond) are triggered,
    /// or when the sweep completes cleanly.
    #[arg(long)]
    pub bench_secs: Option<u64>,
    /// Seconds to skip at the start of each benchmark step (compositor warmup).
    ///
    /// Defaults to `2` when `--bench-secs` is active. Must be strictly less than
    /// `--bench-secs`.
    #[arg(long, default_value_t = 2)]
    pub bench_warmup: u64,
    /// Maximum cube count to probe in benchmark mode (default: 64).
    #[arg(long, default_value_t = 64)]
    pub bench_max: u32,
    /// Override the automatic shader selection.
    ///
    /// Accepted values: `high` (raymarched SDF, full quality) or
    /// `low` (analytic raster, reduced workload).
    ///
    /// By default the shader is chosen automatically from adapter capabilities.
    /// Use `--shader low` on a high-end GPU to test the low-end path, or
    /// `--shader high` to force the full pipeline even on hardware that would
    /// normally be classified as low-end.
    ///
    /// The startup banner always shows which shader was actually loaded and
    /// whether the selection came from auto-detection or a manual override.
    /// Benchmark results are NOT comparable between the two variants.
    #[arg(long, value_name = "VARIANT", value_parser = ["high", "low"])]
    pub shader: Option<String>,
    /// Enable the software Phase-Locked Loop for vblank-synchronised frame submission.
    ///
    /// When active, a PI controller reads `phase_drift_ns` from the WSI pacing
    /// analyzer each frame and sleeps for a computed correction duration before
    /// calling `get_current_texture()`. This shifts the GPU submit instant so
    /// the finished buffer arrives at the compositor closer to the ideal vblank
    /// boundary, reducing `phase_drift_ms` and raising `sync_score` toward 100.
    ///
    /// # How it works
    ///
    /// The controller treats `phase_drift_ns` as the phase error of a PLL:
    ///
    /// ```text
    /// correction = Kp × drift + Ki × Σdrift
    /// ```
    ///
    /// A positive drift (frame presented late) produces a longer pre-submit sleep,
    /// shifting the next submit earlier relative to the vblank edge. A negative
    /// drift (frame presented early) reduces or eliminates the sleep.
    ///
    /// # Convergence
    ///
    /// After 8 consecutive frames with |drift| < 0.5 ms the controller enters
    /// `Locked` (tracking) mode and halves its gains to avoid injecting jitter
    /// into an already-stable loop. The lock state is logged to stderr at startup
    /// and visible in `--frame-log` as `pll_sleep_ns` and `pll_lock`.
    ///
    /// # Compatibility
    ///
    /// - `--mode mailbox` or `--mode immediate`: **recommended**, the pre-submit
    ///   sleep has direct control over the submit instant.
    /// - `--mode fifo`: the driver absorbs the vblank wait internally inside
    ///   `get_current_texture()`, so sleeping before it has minimal effect. The
    ///   controller still runs and its log fields remain valid for analysis, but
    ///   the sleep corrections are mostly consumed by the driver's own blocking.
    ///
    /// # Frame log fields added
    ///
    /// When `--frame-log` is also passed, each row gains three new fields:
    /// - `pll_error_ns` — phase error fed into this frame's PI iteration
    /// - `pll_sleep_ns` — sleep duration actually issued (0 when locked / early)
    /// - `pll_lock`     — `1` when the controller is in Locked tracking mode
    #[arg(long, default_value_t = false)]
    pub pll: bool,
}
