use std::collections::VecDeque;
use std::fs::File;
use std::io::Write;

/// Rolling-window frame timing statistics.
///
/// All computation is pure (no GPU interaction). The caller is responsible
/// for feeding raw frame deltas and reading back the derived metrics.
pub struct FrameMetrics {
    /// Rolling window capped at 3600 samples (~1s at 3600fps, ~60s at 60fps).
    pub frame_times: VecDeque<f32>,
    /// Frame budget in ms derived from the monitor's actual refresh rate.
    pub frame_budget_ms: f32,
    pub frame_count: u32,
    pub dropped_frames: u32,
    pub current_fps: f32,
    pub min_fps: f32,
    pub max_fps: f32,
    pub last_fps_update: std::time::Instant,
}

/// Derived statistics emitted every half-second update tick.
#[derive(Debug, Clone, Copy)]
pub struct TickStats {
    pub current_fps: f32,
    pub min_fps: f32,
    pub max_fps: f32,
    pub low_1_fps: f32,
    pub jitter: f32,
    pub dropped_frames: u32,
    pub ftv: f32,
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
        }
    }

    /// Records one frame delta and increments drop counters when over budget.
    ///
    /// Returns `Some(TickStats)` every 500ms update interval; `None` otherwise.
    pub fn push(
        &mut self,
        delta_ms: f32,
        threshold_ms: f32,
        now: std::time::Instant,
    ) -> Option<TickStats> {
        //FIXME: To get true, microsecond-accurate frame pacing, we need hardware-level presentation timestamps
        // In Fifo the driver absorbs the vsync wait internally before returning from get_current_texture(), so our CPU timer is ~0ms.
        // hardware timestamps would also improve Immediate/Mailbox precision.
        // https://docs.rs/wgpu/latest/wgpu/struct.PresentationTimestamp.html
        self.frame_count += 1;

        if delta_ms > threshold_ms {
            self.dropped_frames += (delta_ms / self.frame_budget_ms).floor() as u32;
        }

        self.frame_times.push_back(delta_ms);
        if self.frame_times.len() > 3600 {
            self.frame_times.pop_front();
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

        // Calculate Jitter (Frame Time Variance)
        let mut jitter_sum = 0.0;
        for i in 1..self.frame_times.len() {
            jitter_sum += (self.frame_times[i] - self.frame_times[i - 1]).abs();
        }
        let jitter = if self.frame_times.len() > 1 {
            jitter_sum / (self.frame_times.len() - 1) as f32
        } else {
            0.0
        };

        // FTV (Frame Time Variance %): coefficient of variation of frame times within
        // the rolling window, expressed as a percentage. Measures how evenly frames
        // are spaced across the 1000ms budget — 0% is perfectly uniform delivery,
        // high values mean frames are bunching (some very fast, some very slow),
        // which the eye perceives as judder even when mean FPS looks acceptable.
        // e.g. frames of [5ms, 48ms, 6ms, 47ms] at "~20fps" will look skippy
        // because visually two frames arrive nearly simultaneously then a long gap.
        let mean = if !self.frame_times.is_empty() {
            self.frame_times.iter().sum::<f32>() / self.frame_times.len() as f32
        } else {
            0.0
        };
        let ftv = if mean > 0.0 && self.frame_times.len() > 1 {
            let variance = self
                .frame_times
                .iter()
                .map(|&t| (t - mean).powi(2))
                .sum::<f32>()
                / (self.frame_times.len() - 1) as f32;
            (variance.sqrt() / mean * 100.0).min(999.0)
        } else {
            0.0
        };

        // Calculate 1% Lows
        let mut sorted_times: Vec<f32> = self.frame_times.iter().copied().collect();
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
            "{:.2},{:.2},{:.2},{:.2},{:.4},{},{:.2}",
            stats.current_fps,
            stats.min_fps,
            stats.max_fps,
            stats.low_1_fps,
            stats.jitter,
            stats.dropped_frames,
            stats.ftv,
        );
    }
}
