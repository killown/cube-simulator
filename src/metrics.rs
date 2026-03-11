use std::collections::VecDeque;
use std::fs::File;
use std::io::Write;

pub struct FrameMetrics {
    pub frame_times: VecDeque<f32>,
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
    /// `true` when jitter/ftv are derived from KMS flip timestamps rather than
    /// CPU-side `Instant` deltas. When `false`, `max_render_time` on a Mailbox
    /// compositor can artificially deflate these values.
    pub hw_verified: bool,
}

/// A single frame's pacing record derived from hardware presentation timestamps.
///
/// Captures the exact moment a frame was scanned out by the display engine,
/// how far it drifted from its ideal vblank slot, and a normalised sync score.
#[derive(Debug, Clone, Copy)]
pub struct FramePacingRecord {
    /// Absolute KMS/WSI presentation timestamp of this frame (nanoseconds).
    pub timestamp_ns: u64,
    /// Measured inter-frame interval for this frame (milliseconds).
    pub delta_ms: f32,
    /// Target vblank period derived from the monitor's refresh rate (milliseconds).
    pub ideal_ms: f32,
    /// Signed deviation from the nearest ideal vblank boundary (milliseconds).
    ///
    /// Positive = frame arrived late (missed its slot, presented on next vblank).
    /// Negative = frame arrived early (unlikely on vsync'd paths; possible on Immediate).
    /// The magnitude is clamped to `ideal_ms / 2` so it always represents the
    /// closest boundary, not an accumulated offset.
    pub phase_drift_ms: f32,
    /// Normalised frame sync quality: 100 = perfectly on-vblank, 0 = half-period drift.
    ///
    /// Computed as `100 × (1 − |phase_drift| / (ideal_ms / 2))`, clamped to `[0, 100]`.
    /// A score ≥ 95 is considered perceptually indistinguishable from perfect pacing.
    pub sync_score: f32,
    /// Frame sequence number since the analyzer was created (0-based).
    pub frame_index: u64,
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
            frame_index: 0,
        }
    }

    /// Ingests one hardware presentation timestamp and returns a pacing record.
    ///
    /// Returns `None` for the very first frame (no delta available yet) or when
    /// the delta is outside the plausible range `(0, 1000ms)` — which indicates
    /// a clock discontinuity (suspend/resume, VT switch).
    ///
    /// # Arguments
    /// * `ts_ns` — Raw KMS/WSI nanosecond timestamp from the presentation engine.
    pub fn push(&mut self, ts_ns: u64) -> Option<FramePacingRecord> {
        let idx = self.frame_index;
        self.frame_index += 1;

        let prev = self.prev_ts_ns.replace(ts_ns)?;

        let delta_ns = ts_ns.saturating_sub(prev);
        let delta_ms = delta_ns as f32 / 1_000_000.0;

        if delta_ms <= 0.0 || delta_ms >= 1000.0 {
            self.phase_origin_ns = None;
            return None;
        }

        let origin = *self.phase_origin_ns.get_or_insert(ts_ns);

        // Distance of this timestamp from the phase origin in vblank periods.
        let elapsed_ns = ts_ns.saturating_sub(origin);
        let nearest_vblank_count = (elapsed_ns as f64 / self.ideal_period_ns as f64).round() as u64;
        let ideal_ts_ns = origin + nearest_vblank_count * self.ideal_period_ns;

        // Signed drift: positive = late, negative = early.
        let drift_ns = ts_ns as i64 - ideal_ts_ns as i64;
        let drift_ms = drift_ns as f32 / 1_000_000.0;

        let half_period_ms = self.ideal_period_ns as f32 / 2_000_000.0;
        let sync_score = (100.0 * (1.0 - drift_ms.abs() / half_period_ms)).clamp(0.0, 100.0);

        Some(FramePacingRecord {
            timestamp_ns: ts_ns,
            delta_ms,
            ideal_ms: self.ideal_period_ns as f32 / 1_000_000.0,
            phase_drift_ms: drift_ms,
            sync_score,
            frame_index: idx,
        })
    }
}

impl FrameMetrics {
    /// Creates a new `FrameMetrics` instance seeded with the monitor's frame budget.
    pub fn new(frame_budget_ms: f32) -> Self {
        let now = std::time::Instant::now();
        Self {
            frame_times: VecDeque::with_capacity(3600),
            frame_budget_ms,
            frame_count: 0,
            dropped_frames: 0,
            current_fps: 0.0,
            min_fps: 0.0,
            max_fps: 0.0,
            last_fps_update: now,
            presentation_timestamps: VecDeque::with_capacity(3600),
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

        self.frame_times.push_back(delta_ms);
        if self.frame_times.len() > 3600 {
            self.frame_times.pop_front();
        }

        self.presentation_timestamps.push_back(presentation_ts);
        if self.presentation_timestamps.len() > 3600 {
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
    // are spaced across the 1000ms budget — 0% is perfectly uniform delivery,
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
/// Emitted once per frame (not per tick), providing microsecond-resolution insight
/// into how each individual frame landed relative to the ideal vblank grid.
/// Fields:
/// - `frame`: monotonic frame index since session start
/// - `ts_ns`: raw hardware presentation timestamp (nanoseconds, CLOCK_MONOTONIC domain)
/// - `delta_ms`: measured inter-frame interval
/// - `ideal_ms`: target vblank period from monitor refresh rate
/// - `drift_ms`: signed deviation from nearest vblank boundary (+ = late, − = early)
/// - `sync`: 0–100 quality score (100 = perfectly on-vblank)
pub fn write_frame_log_row(file: &mut Option<File>, record: &FramePacingRecord) {
    if let Some(f) = file {
        let _ = writeln!(
            f,
            r#"{{"frame":{},"ts_ns":{},"delta_ms":{:.4},"ideal_ms":{:.4},"drift_ms":{:.4},"sync":{:.2}}}"#,
            record.frame_index,
            record.timestamp_ns,
            record.delta_ms,
            record.ideal_ms,
            record.phase_drift_ms,
            record.sync_score,
        );
    }
}
