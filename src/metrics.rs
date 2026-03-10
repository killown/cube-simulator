// metrics.rs
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
