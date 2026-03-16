use std::collections::VecDeque;
use std::fs::File;
use std::io::Write;

pub struct FrameMetrics {
    pub frame_times: VecDeque<f32>,
    pub sync_scores: VecDeque<f32>,
    pub frame_budget_ms: f32,
    pub frame_count: u32,
    pub dropped_frames: u32,
    pub current_fps: f32,
    pub min_fps: f32,
    pub max_fps: f32,
    pub last_fps_update: std::time::Instant,
    /// Ring of raw hardware presentation timestamps (nanoseconds).
    /// `None` slots mean the backend returned `UNSUPPORTED`.
    presentation_timestamps: VecDeque<Option<u64>>,
    /// Whether the backend has ever returned a valid hardware timestamp.
    hw_timestamps_available: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct TickStats {
    pub current_fps: f32,
    pub min_fps: f32,
    pub max_fps: f32,
    pub low_1_fps: f32,
    pub jitter: f32,
    pub dropped_frames: u32,
    pub ftv: f32,
    pub sync_var: f32,
    /// `true` when jitter/ftv are derived from KMS flip timestamps rather than
    /// CPU-side `Instant` deltas. When `false`, `max_render_time` on a Mailbox
    /// compositor can artificially deflate these values.
    pub hw_verified: bool,
}

/// A single frame's pacing record derived from hardware presentation timestamps.
///
/// Every field is annotated with the compositor-development question it answers.
/// CPU-domain fields (`cpu_frame_ms`, `slack_ms`) are explicitly marked; all
/// timestamp fields originate from the WSI/KMS clock (CLOCK_MONOTONIC domain).
#[derive(Debug, Clone, Copy)]
pub struct FramePacingRecord {
    /// Frame sequence number since the analyzer was created (0-based).
    pub frame_index: u64,

    // ── Presentation clock ───────────────────────────────────────────────────
    /// Absolute KMS/WSI presentation timestamp for this frame (nanoseconds).
    ///
    /// Directly comparable to `wl_surface.frame` callback timestamps and
    /// `DRM_IOCTL_WAIT_VBLANK` reply timestamps, same CLOCK_MONOTONIC epoch.
    pub timestamp_ns: u64,

    /// Measured inter-frame interval on the presentation clock (milliseconds).
    ///
    /// The only timing metric that cannot be fabricated by compositor scheduling.
    pub delta_ms: f32,

    /// Target vblank period from the monitor's reported refresh rate (milliseconds).
    pub ideal_ms: f32,

    // ── Vblank grid alignment ────────────────────────────────────────────────
    /// Signed deviation from the nearest ideal vblank boundary (milliseconds).
    ///
    /// Positive = frame arrived late (missed its slot, presented on next vblank).
    /// Negative = frame arrived early (unlikely on vsync'd paths; possible on Immediate).
    /// Clamped to `±(ideal_ms / 2)` so it always names the *nearest* boundary, not
    /// an accumulated offset, a value of +8 ms on a 16 ms budget means one half-period
    /// late, regardless of how many previous frames drifted.
    pub phase_drift_ms: f32,

    /// Raw signed drift in nanoseconds before float truncation.
    ///
    /// Retains sub-microsecond precision lost in `phase_drift_ms` (~100 ns at 120 Hz).
    /// Use this when feeding drift into a PLL or repaint-timer correction loop.
    pub phase_drift_ns: i64,

    /// How many vblank periods this frame consumed.
    ///
    /// `1` = on time.  `2` = one vblank dropped (GPU overran or compositor missed
    /// deadline).  `≥ 3` = severe stall (TTM eviction, GPU preemption, thermal
    /// throttle).
    ///
    /// Uses a 1.25× hysteresis band instead of a bare `round()`: a frame must
    /// consume at least 125% of `ideal_ms` before being promoted to `vblank_mul = 2`.
    /// This eliminates false positives at 120–165 Hz where `CLOCK_MONOTONIC` jitter
    /// on tickless kernels is large enough to push a clean frame over the 0.5×
    /// midpoint that `round()` would trip on.
    pub vblank_mul: u32,

    /// Normalised frame sync quality: 100 = perfectly on-vblank, 0 = half-period drift.
    ///
    /// `100 × (1 − |phase_drift_ms| / (ideal_ms / 2))`, clamped to `[0, 100]`.
    /// ≥ 95 is perceptually indistinguishable from perfect pacing on all display types.
    pub sync_score: f32,

    // ── Instantaneous jitter ─────────────────────────────────────────────────
    /// Change in inter-frame interval relative to the previous frame (milliseconds).
    ///
    /// `delta_ms[n] − delta_ms[n−1]`.  Detects double-buffer ping-pong
    /// (alternating fast/slow frames that cancel out in the rolling jitter average
    /// but are clearly visible on screen).  `None` on the first valid frame.
    pub ipc_delta_ms: Option<f32>,

    // ── CPU-domain work cycle ─────────────────────────────────────────────────
    /// Total CPU-observed frame time: `RedrawRequested` → `present()` return
    /// (milliseconds, CPU `Instant`, not the presentation clock).
    ///
    /// Compare against `delta_ms` (presentation clock) to diagnose buffer-hold policy:
    /// - `delta_ms ≈ cpu_frame_ms` → frame was scanned out immediately after CPU work.
    /// - `delta_ms >> cpu_frame_ms` → compositor held the buffer (`max_render_time`
    ///   inflation, triple-buffer queue depth > 1, Wayland frame-callback throttling).
    /// - `delta_ms < cpu_frame_ms` → impossible on vsync'd paths; indicates clock skew.
    ///
    /// `None` when not supplied to [`PacingAnalyzer::push`].
    pub cpu_frame_ms: Option<f32>,

    /// Time from CPU-observed `queue.submit()` return to the hardware presentation
    /// timestamp (milliseconds, mixed CPU + HW domain).
    ///
    /// Approximates GPU execution + driver flip pipeline depth as seen from the CPU.
    /// On a healthy single-vblank Fifo path: `slack_ms ≈ ideal_ms`.
    ///
    /// Cross-reference with `phase_drift_ms` to locate the bottleneck:
    /// - `drift high` + `slack high` → GPU finished well before vblank; buffer sat in
    ///   the compositor's queue, **compositor scheduling policy** is the bottleneck.
    /// - `drift high` + `slack ≈ 0`  → GPU was still executing at vblank time —
    ///   **GPU render budget** was exceeded.
    /// - `drift low`  + `slack ≈ ideal_ms` → healthy, one-vblank pipeline.
    ///
    /// `None` when `cpu_submit_ns` was not supplied to [`PacingAnalyzer::push`].
    pub slack_ms: Option<f32>,

    /// True GPU execution time measured by hardware timestamp queries (milliseconds).
    ///
    /// Obtained from a `wgpu::QuerySet` with `TIMESTAMP_QUERY` capability, written
    /// by the render pass begin/end timestamps and resolved via `resolve_query_set`.
    /// This is the only signal that unambiguously answers "did the GPU overrun its
    /// budget?", `slack_ms` cannot distinguish GPU overrun from compositor hold.
    ///
    /// Cross-reference:
    /// - `gpu_time_ms > ideal_ms`         → GPU render budget exceeded; reduce load.
    /// - `gpu_time_ms < ideal_ms` + high drift → compositor scheduling bottleneck.
    /// - `gpu_time_ms ≈ 0`                → query not supported or not yet resolved.
    ///
    /// `None` when `TIMESTAMP_QUERY` is not available on the adapter or the readback
    /// buffer has not yet been mapped (the value from the previous frame is used
    /// until the mapping completes).
    pub gpu_time_ms: Option<f32>,

    // ── Micro-stutter breakdown (TIMESTAMP_QUERY_INSIDE_ENCODERS) ────────────
    /// Time from CPU `queue.submit()` to first GPU instruction (milliseconds).
    ///
    /// Measured by a `write_timestamp` probe placed immediately before the render
    /// pass on the command encoder.  Isolates Vulkan driver command-buffer
    /// scheduling latency from actual shader execution time.
    ///
    /// Healthy range: < 1 ms.  A persistent spike (> 2 ms) indicates driver-level
    /// stall, not a GPU render budget problem and not fixable by reducing shader
    /// complexity.
    ///
    /// `None` when `TIMESTAMP_QUERY_INSIDE_ENCODERS` is unavailable.
    pub micro_driver_ms: Option<f32>,

    /// GPU DMA + PCIe copy overhead for the timestamp readback resolve (milliseconds).
    ///
    /// Measured by a `write_timestamp` probe placed after `resolve_query_set` +
    /// `copy_buffer_to_buffer` on the command encoder.  A spike here indicates
    /// PCIe contention, IOMMU remapping pressure, or a TTM buffer eviction
    /// mid-frame, none of which are visible from shader time alone.
    ///
    /// Healthy range: < 0.1 ms.  Persistent spikes > 0.3 ms indicate memory
    /// subsystem pressure.
    ///
    /// `None` when `TIMESTAMP_QUERY_INSIDE_ENCODERS` is unavailable.
    pub micro_resolve_ms: Option<f32>,

    /// Total GPU pipeline span: driver dispatch + shader + DMA resolve (milliseconds).
    ///
    /// The full elapsed time between `MICRO_SLOT_SUBMIT_PRE` and
    /// `MICRO_SLOT_RESOLVE_END` on the GPU clock.  Equals
    /// `micro_driver_ms + gpu_time_ms + micro_resolve_ms` when all three are
    /// available.  Use as a single "how long did the GPU spend on this
    /// frame end-to-end" number for quick triage.
    ///
    /// `None` when `TIMESTAMP_QUERY_INSIDE_ENCODERS` is unavailable.
    pub micro_total_ms: Option<f32>,

    // ── Late-Stage Presentation Tracking ─────────────────────────────────────
    /// Time from GPU render-pass end (`gpu_time_ms` stop) to actual KMS scanout
    /// start, as reported by the kernel's `DRM_EVENT_FLIP_COMPLETE` interrupt
    /// (milliseconds, CLOCK_MONOTONIC domain).
    ///
    /// This is the **compositor scheduling tax**: the wall-clock time the finished
    /// frame spent waiting in the compositor's buffer queue before the kernel
    /// actually drove it to the display.
    ///
    /// Interpretation:
    /// - `< 1 ms`           → compositor scanned out immediately, no hold.
    /// - `1–8 ms`           → normal compositor compositing overhead (one pass).
    /// - `> (ideal_ms / 2)` → compositor is holding a completed frame across a
    ///                         vblank boundary, the "lie" this tool detects.
    ///                         Cross-reference `slack_ms`: if `slack_ms` is large
    ///                         but `gpu_time_ms < ideal_ms`, the GPU is not the
    ///                         bottleneck, the compositor is.
    ///
    /// Requires an active [`crate::flip_tracker::FlipTracker`] and `TIMESTAMP_QUERY`
    /// support.  `None` when either is unavailable or when the flip event for this
    /// frame has not yet been delivered by the kernel.
    pub flip_latency_ms: Option<f32>,
}

/// Stateful per-frame pacing analyzer driven by hardware presentation timestamps.
///
/// Maintains a phase accumulator so drift is always measured against the *nearest*
/// ideal vblank boundary rather than an absolute origin, which remains meaningful
/// even after monitor blanking, VT switches, or DPMS wakeups.
pub struct PacingAnalyzer {
    /// Vblank period in nanoseconds, derived from the monitor's refresh rate.
    ideal_period_ns: u64,
    /// The presentation timestamp of the first valid frame, used as the phase origin.
    phase_origin_ns: Option<u64>,
    /// Previous valid presentation timestamp for delta computation.
    prev_ts_ns: Option<u64>,
    /// `delta_ms` from the previous frame, for instantaneous jitter (`ipc_delta_ms`).
    prev_delta_ms: Option<f32>,
    /// CPU-domain nanoseconds of the previous frame's `queue.submit()` return.
    ///
    /// Stored one frame behind because the presentation timestamp for a given submit
    /// arrives on the *next* `push()` call, not the same one.
    prev_cpu_submit_ns: Option<u64>,
    /// Monotonically increasing frame counter.
    frame_index: u64,
}

impl PacingAnalyzer {
    /// Creates a new analyzer calibrated to the monitor's refresh rate.
    ///
    /// `frame_budget_ms` is the reciprocal of the refresh rate in milliseconds
    /// (e.g. `16.666` for 60 Hz, `8.333` for 120 Hz). This is used to derive
    /// the ideal vblank period in nanoseconds for phase drift calculations.
    pub fn new(frame_budget_ms: f32) -> Self {
        Self {
            ideal_period_ns: (frame_budget_ms * 1_000_000.0).round() as u64,
            phase_origin_ns: None,
            prev_ts_ns: None,
            prev_delta_ms: None,
            prev_cpu_submit_ns: None,
            frame_index: 0,
        }
    }

    /// Ingests one frame's timing data and returns a fully-populated pacing record.
    ///
    /// Returns `None` for the very first frame (no delta available yet) or when
    /// `delta_ms` is outside the plausible range `(0, 1000 ms)`, which indicates a
    /// clock discontinuity (suspend/resume, VT switch, DPMS wakeup).
    ///
    /// The phase origin is recalibrated whenever `delta_ms > 3 × ideal_ms` so that
    /// a DPMS wakeup or VT switch does not leave the grid anchored to a stale origin
    /// and produce phantom drift on every subsequent frame.
    ///
    /// # Arguments
    /// * `ts_ns` Raw KMS/WSI nanosecond timestamp from
    ///   `adapter.get_presentation_timestamp()`.  Must be CLOCK_MONOTONIC domain.
    /// * `cpu_frame_ms`, CPU-observed total frame time (`RedrawRequested` →
    ///   `present()` return), in milliseconds.  `None` if not measured.
    /// * `cpu_submit_ns`, `std::time::Instant` converted to nanoseconds, sampled
    ///   immediately after `queue.submit()` returns for **this** frame.  Stored and
    ///   matched against the **next** frame's `ts_ns` to derive `slack_ms`, because
    ///   the hardware presentation timestamp for a given submit arrives one frame later.
    ///   `None` if not measured.
    /// * `gpu_time_ms` True GPU execution time from a resolved `TIMESTAMP_QUERY`
    ///   `QuerySet`, in milliseconds.  `None` when the feature is unavailable or the
    ///   readback buffer has not yet been mapped.
    /// * `micro` Decomposed GPU pipeline timings from the micro-stutter diagnostic
    ///   query set (`TIMESTAMP_QUERY_INSIDE_ENCODERS`).  `None` when the feature is
    ///   unavailable.  When present, `driver_overhead_ms`, `resolve_ms`, and
    ///   `total_ms` are logged to the frame log alongside `gpu_time_ms`.
    /// * `flip_latency_ms` Compositor scheduling tax derived from the KMS
    ///   `DRM_EVENT_FLIP_COMPLETE` interrupt timestamp minus the GPU render-pass end
    ///   timestamp.  `None` when [`crate::flip_tracker::FlipTracker`] is inactive or
    ///   no flip event has been drained for this frame yet.
    pub fn push(
        &mut self,
        ts_ns: u64,
        cpu_frame_ms: Option<f32>,
        cpu_submit_ns: Option<u64>,
        gpu_time_ms: Option<f32>,
        micro: Option<crate::gpu_timer::MicroTimings>,
        flip_latency_ms: Option<f32>,
    ) -> Option<FramePacingRecord> {
        let idx = self.frame_index;
        self.frame_index += 1;

        let prev = self.prev_ts_ns.replace(ts_ns)?;

        let delta_ns = ts_ns.saturating_sub(prev);
        let delta_ms = delta_ns as f32 / 1_000_000.0;

        if delta_ms <= 0.0 || delta_ms >= 1000.0 {
            self.phase_origin_ns = None;
            self.prev_delta_ms = None;
            self.prev_cpu_submit_ns = cpu_submit_ns;
            return None;
        }

        let ideal_ms = self.ideal_period_ns as f32 / 1_000_000.0;

        // Recalibrate the phase grid on large gaps (DPMS wakeup, VT switch, TTM
        // eviction stall). Without this, every frame after the discontinuity shows
        // phantom drift because `nearest_vblank_count` jumps by hundreds of periods
        // yet the origin stays pinned to the pre-gap timestamp.
        if delta_ms > ideal_ms * 3.0 {
            self.phase_origin_ns = None;
        }

        let origin = *self.phase_origin_ns.get_or_insert(ts_ns);

        // Distance of this timestamp from the phase origin in vblank periods.
        let elapsed_ns = ts_ns.saturating_sub(origin);
        let nearest_vblank_count = (elapsed_ns as f64 / self.ideal_period_ns as f64).round() as u64;
        let ideal_ts_ns = origin + nearest_vblank_count * self.ideal_period_ns;

        // Signed drift: positive = late, negative = early.
        let drift_ns = ts_ns as i64 - ideal_ts_ns as i64;
        let drift_ms = drift_ns as f32 / 1_000_000.0;

        let half_period_ms = ideal_ms / 2.0;
        let sync_score = (100.0 * (1.0 - drift_ms.abs() / half_period_ms)).clamp(0.0, 100.0);

        // Hysteresis band: require delta_ms > 1.25 × ideal_ms before promoting to
        // vblank_mul = 2. A bare round() trips at 0.5 × ideal_ms, which is only
        // ~4 ms at 120 Hz, within CLOCK_MONOTONIC jitter on tickless kernels under
        // load, producing false-positive yellow signals on clean high-refresh panels.
        let vblank_mul = {
            let ratio = delta_ms / ideal_ms;
            if ratio < 1.25 {
                1u32
            } else {
                (ratio.round() as u32).max(1)
            }
        };

        let ipc_delta_ms = self.prev_delta_ms.map(|prev_d| delta_ms - prev_d);

        // `prev_cpu_submit_ns` is the submit timestamp from the *previous* frame,
        // which is what produced this frame's scanout.  `ts_ns` and `cpu_submit_ns`
        // are both CLOCK_MONOTONIC on Linux, making the subtraction meaningful despite
        // the mixed capture method (WSI vs. std::time::Instant).
        let slack_ms = self.prev_cpu_submit_ns.and_then(|submit_ns| {
            let gap_ns = ts_ns.saturating_sub(submit_ns);
            let gap_ms = gap_ns as f32 / 1_000_000.0;
            // Discard values outside [0, 5 × ideal]: negative = clock skew,
            // extreme positive = submit timestamp predates WSI origin or stale value.
            let ceiling = ideal_ms * 5.0;
            (gap_ms > 0.0 && gap_ms < ceiling).then_some(gap_ms)
        });

        self.prev_delta_ms = Some(delta_ms);
        self.prev_cpu_submit_ns = cpu_submit_ns;

        Some(FramePacingRecord {
            frame_index: idx,
            timestamp_ns: ts_ns,
            delta_ms,
            ideal_ms,
            phase_drift_ms: drift_ms,
            phase_drift_ns: drift_ns,
            vblank_mul,
            sync_score,
            ipc_delta_ms,
            cpu_frame_ms,
            slack_ms,
            gpu_time_ms,
            micro_driver_ms: micro.map(|m| m.driver_overhead_ms),
            micro_resolve_ms: micro.map(|m| m.resolve_ms),
            micro_total_ms: micro.map(|m| m.total_ms),
            flip_latency_ms,
        })
    }
}

impl FrameMetrics {
    /// Creates a new `FrameMetrics` instance seeded with the monitor's frame budget.
    pub fn new(frame_budget_ms: f32) -> Self {
        let now = std::time::Instant::now();
        // Cap the ring to 30 seconds of frames at the reported refresh rate.
        // A fixed count of 3600 is only ~15 s at 240 Hz and ~2 min at 30 Hz;
        // tying the cap to the frame budget gives a consistent analysis window
        // regardless of refresh rate.
        let ring_cap = ((30_000.0 / frame_budget_ms).ceil() as usize).max(256);
        Self {
            frame_times: VecDeque::with_capacity(ring_cap),
            sync_scores: VecDeque::with_capacity(ring_cap),
            frame_budget_ms,
            frame_count: 0,
            dropped_frames: 0,
            current_fps: 0.0,
            min_fps: 0.0,
            max_fps: 0.0,
            last_fps_update: now,
            presentation_timestamps: VecDeque::with_capacity(ring_cap),
            hw_timestamps_available: false,
        }
    }

    /// Records one frame delta and its hardware presentation timestamp.
    ///
    /// `presentation_ts` should come directly from
    /// `wgpu::Queue::get_timestamp_period`-gated `SurfaceTexture::presentation_timestamp()`.
    /// When the backend returns [`wgpu::PresentationTimestamp::INVALID`], pass `None`
    /// and the metric pipeline falls back to CPU-side deltas with `hw_verified: false`.
    ///
    /// Returns `Some(TickStats)` every 500ms update interval; `None` otherwise.
    pub fn push(
        &mut self,
        delta_ms: f32,
        threshold_ms: f32,
        now: std::time::Instant,
        presentation_ts: Option<u64>,
    ) -> Option<TickStats> {
        self.frame_count += 1;

        if let Some(ts) = presentation_ts {
            if ts != u64::MAX {
                self.hw_timestamps_available = true;
            }
        }

        if delta_ms > threshold_ms {
            self.dropped_frames += (delta_ms / self.frame_budget_ms).floor() as u32;
        }

        // Evict oldest entry once the ring reaches its time-window capacity.
        let cap = self.frame_times.capacity();
        self.frame_times.push_back(delta_ms);
        if self.frame_times.len() > cap {
            self.frame_times.pop_front();
        }

        self.presentation_timestamps.push_back(presentation_ts);
        if self.presentation_timestamps.len() > cap {
            self.presentation_timestamps.pop_front();
        }

        let diff = now.duration_since(self.last_fps_update);
        if diff.as_secs_f32() < 0.5 {
            return None;
        }

        let stats = self.compute_tick(diff.as_secs_f32());
        self.frame_count = 0;
        self.dropped_frames = 0;
        self.last_fps_update = now;

        Some(stats)
    }

    pub fn calculate_sync_var(&self) -> f32 {
        let n = self.sync_scores.len();
        if n < 2 {
            return 0.0;
        }

        let mean = self.sync_scores.iter().sum::<f32>() / n as f32;
        let variance = self
            .sync_scores
            .iter()
            .map(|&s| {
                let diff = s - mean;
                diff * diff
            })
            .sum::<f32>()
            / n as f32;

        variance.sqrt()
    }

    fn compute_tick(&mut self, elapsed_secs: f32) -> TickStats {
        self.current_fps = self.frame_count as f32 / elapsed_secs;

        if self.min_fps == 0.0 || self.current_fps < self.min_fps {
            self.min_fps = self.current_fps;
        }
        if self.current_fps > self.max_fps {
            self.max_fps = self.current_fps;
        }

        let (jitter, ftv, hw_verified) = if self.hw_timestamps_available {
            self.compute_hw_jitter_ftv()
        } else {
            (self.compute_cpu_jitter(), self.compute_cpu_ftv(), false)
        };

        // Calculate 1% Lows
        let source_times = self.hw_frame_times_ms();
        let times_for_lows = if source_times.is_empty() {
            self.frame_times.iter().copied().collect::<Vec<_>>()
        } else {
            source_times
        };

        let mut sorted_times = times_for_lows;
        sorted_times.sort_by(|a, b| b.partial_cmp(a).unwrap());
        let one_percent_index = ((sorted_times.len() as f32 * 0.01).ceil() as usize)
            .max(1)
            .min(sorted_times.len());
        let avg_1pct_time: f32 =
            sorted_times[..one_percent_index].iter().sum::<f32>() / one_percent_index as f32;
        let low_1_fps = if avg_1pct_time > 0.0 {
            1000.0 / avg_1pct_time
        } else {
            0.0
        };

        TickStats {
            current_fps: self.current_fps,
            min_fps: self.min_fps,
            max_fps: self.max_fps,
            low_1_fps,
            jitter,
            dropped_frames: self.dropped_frames,
            ftv,
            sync_var: self.calculate_sync_var(),
            hw_verified,
        }
    }

    /// Derives inter-frame deltas in milliseconds from consecutive KMS timestamps.
    ///
    /// Skips pairs where either slot is `None` or `u64::MAX` (unsupported).
    fn hw_frame_times_ms(&self) -> Vec<f32> {
        let ts: Vec<u64> = self
            .presentation_timestamps
            .iter()
            .filter_map(|&t| t.filter(|&v| v != u64::MAX))
            .collect();

        ts.windows(2)
            .map(|w| {
                // nanoseconds → milliseconds; saturating to avoid wrapping on
                // timestamp resets (e.g. CLOCK_MONOTONIC discontinuities).
                w[1].saturating_sub(w[0]) as f32 / 1_000_000.0
            })
            .filter(|&ms| ms > 0.0 && ms < 1000.0) // discard absurd outliers
            .collect()
    }

    /// Computes jitter and FTV from hardware KMS flip timestamps.
    ///
    /// Returns `(jitter_ms, ftv_percent, hw_verified: true)`.
    fn compute_hw_jitter_ftv(&self) -> (f32, f32, bool) {
        let hw_times = self.hw_frame_times_ms();
        if hw_times.len() < 2 {
            return (0.0, 0.0, false);
        }

        let jitter = {
            let sum: f32 = hw_times.windows(2).map(|w| (w[1] - w[0]).abs()).sum();
            sum / (hw_times.len() - 1) as f32
        };

        let mean = hw_times.iter().sum::<f32>() / hw_times.len() as f32;
        let ftv = if mean > 0.0 {
            let variance = hw_times.iter().map(|&t| (t - mean).powi(2)).sum::<f32>()
                / (hw_times.len() - 1) as f32;
            (variance.sqrt() / mean * 100.0).min(999.0)
        } else {
            0.0
        };

        (jitter, ftv, true)
    }

    fn compute_cpu_jitter(&self) -> f32 {
        let mut sum = 0.0;
        for i in 1..self.frame_times.len() {
            sum += (self.frame_times[i] - self.frame_times[i - 1]).abs();
        }
        if self.frame_times.len() > 1 {
            sum / (self.frame_times.len() - 1) as f32
        } else {
            0.0
        }
    }

    // FTV (Frame Time Variance %): coefficient of variation of frame times within
    // the rolling window, expressed as a percentage. Measures how evenly frames
    // are spaced across the 1000ms budget, 0% is perfectly uniform delivery,
    // high values mean frames are bunching (some very fast, some very slow),
    // which the eye perceives as judder even when mean FPS looks acceptable.
    // e.g. frames of [5ms, 48ms, 6ms, 47ms] at "~20fps" will look skippy
    // because visually two frames arrive nearly simultaneously then a long gap.
    fn compute_cpu_ftv(&self) -> f32 {
        let mean = if !self.frame_times.is_empty() {
            self.frame_times.iter().sum::<f32>() / self.frame_times.len() as f32
        } else {
            0.0
        };
        if mean > 0.0 && self.frame_times.len() > 1 {
            let variance = self
                .frame_times
                .iter()
                .map(|&t| (t - mean).powi(2))
                .sum::<f32>()
                / (self.frame_times.len() - 1) as f32;
            (variance.sqrt() / mean * 100.0).min(999.0)
        } else {
            0.0
        }
    }
}

/// Writes a CSV row for a completed tick, if a file handle is present.
///
/// The header (`FPS,MIN,MAX,LOW_1,JITTER,DROPPED,FTV`) is written once at
/// file creation time in [`State::new`]; this function only appends rows.
pub fn write_csv_row(file: &mut Option<File>, stats: &TickStats) {
    if let Some(f) = file {
        let _ = writeln!(
            f,
            "{:.2},{:.2},{:.2},{:.2},{:.4},{},{:.2},{}",
            stats.current_fps,
            stats.min_fps,
            stats.max_fps,
            stats.low_1_fps,
            stats.jitter,
            stats.dropped_frames,
            stats.ftv,
            if stats.hw_verified { "hw" } else { "cpu" },
        );
    }
}

/// Writes one NDJSON line for a completed tick, if a file handle is present.
///
/// Each line is a self-contained JSON object so the file is streamable and
/// appendable without wrapping in an array. Field names mirror the CSV header.
pub fn write_json_row(file: &mut Option<File>, stats: &TickStats) {
    if let Some(f) = file {
        let _ = writeln!(
            f,
            r#"{{"fps":{:.2},"min":{:.2},"max":{:.2},"low_1":{:.2},"jitter":{:.4},"dropped":{},"ftv":{:.2},"hw_verified":{}}}"#,
            stats.current_fps,
            stats.min_fps,
            stats.max_fps,
            stats.low_1_fps,
            stats.jitter,
            stats.dropped_frames,
            stats.ftv,
            stats.hw_verified,
        );
    }
}

/// Writes one NDJSON line for a single frame's pacing record, if a file handle is present.
///
/// Emitted once per frame (not per tick), providing nanosecond-resolution insight
/// into how each individual frame landed relative to the ideal vblank grid.
///
/// # Fields emitted
/// - `schema`           — version tag, increment when the field set changes
/// - `frame`            — monotonic frame index since session start
/// - `cube_count`       — cube count active during this frame
/// - `ts_ns`            — raw KMS/WSI presentation timestamp (ns, CLOCK_MONOTONIC)
/// - `delta_ms`         — measured inter-frame interval on the presentation clock
/// - `ideal_ms`         — target vblank period from monitor refresh rate
/// - `drift_ms`         — signed deviation from nearest vblank boundary (+ = late, − = early)
/// - `drift_ns`         — same drift at nanosecond precision (for PLL / repaint-timer use)
/// - `vblank_mul`       — vblank periods consumed (1 = on-time, 2 = one dropped, ≥3 = stall)
/// - `sync`             — 0–100 quality score (100 = perfectly on-vblank)
/// - `ipc_delta_ms`     — instantaneous Δ between this and the previous `delta_ms`
/// - `cpu_frame_ms`     — CPU-observed total frame time
/// - `slack_ms`         — `present_ts − submit_cpu`: GPU execution + flip pipeline depth
/// - `gpu_time_ms`      — true GPU render-pass execution time from hardware timestamp queries
/// - `micro_driver_ms`  — Vulkan driver command-buffer submission latency (encoder bracket)
/// - `micro_resolve_ms` — GPU DMA + PCIe copy overhead for timestamp readback resolve
/// - `micro_total_ms`   — full GPU pipeline span: driver + shader + resolve combined
/// - `flip_latency_ms`  — compositor scheduling tax: GPU end → KMS scanout start
///
/// Optional fields are omitted from the JSON object entirely when `None`.
pub fn write_frame_log_row(file: &mut Option<File>, record: &FramePacingRecord, cube_count: u32) {
    let Some(f) = file else { return };

    // schema:4 adds flip_latency_ms field.
    let _ = write!(
        f,
        r#"{{"schema":4,"frame":{},"cube_count":{},"ts_ns":{},"delta_ms":{:.4},"ideal_ms":{:.4},"drift_ms":{:.4},"drift_ns":{},"vblank_mul":{},"sync":{:.2}"#,
        record.frame_index,
        cube_count,
        record.timestamp_ns,
        record.delta_ms,
        record.ideal_ms,
        record.phase_drift_ms,
        record.phase_drift_ns,
        record.vblank_mul,
        record.sync_score,
    );

    // Optional fields, emitted only when the caller supplied the source data.
    if let Some(v) = record.ipc_delta_ms {
        let _ = write!(f, r#","ipc_delta_ms":{:.4}"#, v);
    }
    if let Some(v) = record.cpu_frame_ms {
        let _ = write!(f, r#","cpu_frame_ms":{:.4}"#, v);
    }
    if let Some(v) = record.slack_ms {
        let _ = write!(f, r#","slack_ms":{:.4}"#, v);
    }
    if let Some(v) = record.gpu_time_ms {
        let _ = write!(f, r#","gpu_time_ms":{:.4}"#, v);
    }
    if let Some(v) = record.micro_driver_ms {
        let _ = write!(f, r#","micro_driver_ms":{:.4}"#, v);
    }
    if let Some(v) = record.micro_resolve_ms {
        let _ = write!(f, r#","micro_resolve_ms":{:.4}"#, v);
    }
    if let Some(v) = record.micro_total_ms {
        let _ = write!(f, r#","micro_total_ms":{:.4}"#, v);
    }
    if let Some(v) = record.flip_latency_ms {
        let _ = write!(f, r#","flip_latency_ms":{:.4}"#, v);
    }

    let _ = writeln!(f, "}}");
}
