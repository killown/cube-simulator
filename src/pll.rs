//! Software Phase-Locked Loop for vblank-synchronised frame submission.
//!
//! # Goal
//!
//! Lock the render loop to exactly one frame per vblank period (matching the
//! monitor refresh rate) and present each frame as close to the leading edge
//! of its vblank slot as possible, "just-in-time" delivery.
//!
//! # How it works
//!
//! Every vblank occurs at a predictable absolute time:
//!
//! ```text
//! vblank[n] = phase_origin + n × ideal_period_ns
//! ```
//!
//! The GPU needs `render_budget_ns` to finish a frame before the compositor
//! can scan it out.  The submit deadline for frame N is therefore:
//!
//! ```text
//! deadline[n] = vblank[n] − render_budget_ns
//! ```
//!
//! Each frame the controller:
//!
//! 1. **Advances the vblank grid** - increments `n` by one from the previous
//!    frame's anchor.  If the previous frame missed its vblank (`vblank_mul > 1`),
//!    `n` is advanced by the actual multiplier so the grid stays in phase with
//!    the hardware rather than drifting forward one period per miss.
//!
//! 2. **Trims with PI** - the measured `phase_drift_ns` from the previous frame
//!    is a signed residual between the ideal grid and the actual presentation
//!    timestamp.  A PI controller folds this residual into the deadline:
//!
//!    ```text
//!    deadline_trimmed = deadline[n] − (Kp × e[n−1] + Ki × Σe)
//!    ```
//!
//!    Subtracting a positive drift (frame arrived late) pulls the deadline
//!    earlier, causing the next submit to happen sooner relative to the vblank
//!    edge, which advances the presentation timestamp back toward the ideal grid.
//!
//! 3. **Sleeps until `deadline_trimmed`** using `clock_nanosleep(TIMER_ABSTIME)`
//!    on `CLOCK_MONOTONIC` - the same epoch as WSI presentation timestamps.
//!    An absolute-time sleep prevents wakeup-jitter compounding across frames.
//!
//! # FPS cap
//!
//! Because the controller advances the grid by exactly `ideal_period_ns` each
//! frame, the render loop is rate-limited to the monitor refresh rate regardless
//! of how fast the GPU renders.  A frame that finishes early waits at the sleep;
//! a frame that overruns causes the next deadline to be skipped forward by the
//! appropriate number of missed periods (no spiralling back-pressure).
//!
//! # Render budget estimation
//!
//! `render_budget_ns` is initialised to 70% of `ideal_period_ns` and updated
//! each frame via an exponential moving average of the actual GPU execution time
//! (`gpu_time_ms` from `TIMESTAMP_QUERY`).  When GPU timestamps are unavailable
//! it falls back to `cpu_frame_ms`.  A 20% safety margin is added on top of
//! the EMA so the deadline is set early enough for the driver flip pipeline to
//! complete before the vblank edge.
//!
//! # Present mode compatibility
//!
//! * `Mailbox` / `Immediate` - full control.  The sleep determines the submit
//!   instant; the driver does not add a blocking wait.  This is the recommended
//!   mode for `--pll`.
//!
//! * `Fifo` - the driver blocks inside `get_current_texture()` until the next
//!   vblank, consuming most or all of the sleep budget.  The FPS cap is still
//!   enforced (you cannot submit faster than one-per-vblank in Fifo regardless),
//!   but the just-in-time phase alignment is less precise because the wakeup
//!   point is controlled by the driver, not by this controller.  A startup
//!   warning is printed in this case.

// ── Tuning constants ──────────────────────────────────────────────────────────

/// Proportional gain applied to `phase_drift_ns`.
///
/// A drift of 1 ms -> 0.5 ms of deadline advance/retard.  Converges within
/// ~4 frames at 120 Hz without overshoot.
const KP: f64 = 0.5;

/// Integral gain.  Eliminates steady-state offset caused by fixed GPU latency
/// that is not a round multiple of `ideal_period_ns`.
///
/// At 120 Hz, Ki = 0.02 integrates out a 1 ms fixed offset in ~50 frames.
const KI: f64 = 0.02;

/// Anti-windup clamp on the integrator (nanoseconds).
///
/// Capped to one 60 Hz period so a burst of missed vblanks (thermal throttle,
/// TTM eviction) cannot wind the integrator beyond one frame of correction.
const INTEGRATOR_CLAMP_NS: f64 = 16_666_667.0;

/// Safety margin added on top of the EMA render budget (fraction of budget).
///
/// The GPU finishes rendering at `gpu_time_ms`, but the driver flip pipeline,
/// DMA, and compositor compositing pass take additional time before the pixel
/// reaches the display.  0.20 = 20% headroom leaves room for that overhead.
const BUDGET_SAFETY_MARGIN: f64 = 0.20;

/// EMA smoothing factor for the render budget estimate.
///
/// alpha = 0.15 gives a half-life of ~4 frames, tracking gradual load changes
/// without overreacting to single-frame GPU spikes.
const BUDGET_EMA_ALPHA: f64 = 0.15;

/// Minimum sleep duration issued via `clock_nanosleep`.
///
/// Sleeps shorter than 100 us are skipped entirely: the kernel timer wheel
/// on a tickless kernel wakes up ~50-100 us late, so a 10 us sleep would
/// overshoot by 5-10x, injecting more jitter than it removes.
const MIN_SLEEP_NS: u64 = 100_000; // 100 us

/// Frames with |drift| below this threshold count toward the lock counter.
const CONVERGENCE_WINDOW_NS: i64 = 500_000; // 0.5 ms

/// Consecutive on-time frames required before entering Locked (tracking) mode.
const LOCK_THRESHOLD_FRAMES: u32 = 8;

// ── Public types ──────────────────────────────────────────────────────────────

/// Convergence state of the PLL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PllLockState {
    /// Actively correcting; drift exceeds the convergence window.
    Acquiring,
    /// Drift has been within the convergence window for [`LOCK_THRESHOLD_FRAMES`]
    /// consecutive frames.  Gains are halved to minimise unnecessary corrections.
    Locked,
}

/// Per-frame diagnostic snapshot from [`PllController::compute_deadline`].
///
/// Written to `--frame-log` when both `--pll` and `--frame-log` are active.
/// Fields tagged `#[allow(dead_code)]` are not consumed by the render loop but
/// are essential for offline PLL convergence analysis.
#[derive(Debug, Clone, Copy)]
pub struct PllDiagnostics {
    /// Phase error fed into this iteration (nanoseconds, signed).
    ///
    /// Positive = previous frame presented late; negative = presented early.
    pub phase_error_ns: i64,

    /// Proportional correction term (nanoseconds).
    #[allow(dead_code)]
    pub p_term_ns: i64,

    /// Integral correction term (nanoseconds).
    #[allow(dead_code)]
    pub i_term_ns: i64,

    /// Raw PI sum before the deadline is clamped (nanoseconds, signed).
    #[allow(dead_code)]
    pub raw_correction_ns: i64,

    /// Absolute `CLOCK_MONOTONIC` deadline the controller slept until (nanoseconds).
    ///
    /// `0` when no sleep was issued (deadline already passed or first frame).
    pub deadline_ns: u64,

    /// Actual sleep duration issued (nanoseconds).
    ///
    /// `0` when the deadline had already passed by the time `compute_deadline`
    /// was called (frame is running late; skip the sleep and submit immediately).
    pub sleep_ns: u64,

    /// Render budget used to set the deadline (nanoseconds).
    ///
    /// Tracks an EMA of GPU execution time plus the safety margin.
    pub render_budget_ns: u64,

    /// Current PLL convergence state.
    pub lock_state: PllLockState,
}

/// Deadline-based software PLL that rate-limits and phase-aligns frame submission.
///
/// # Usage
///
/// ```rust,ignore
/// // In State::new:
/// let pll = PllController::new(frame_budget_ms);
///
/// // At the top of render(), after draining pacing records:
/// if let Some(ctrl) = &mut self.pll {
///     let diag = ctrl.compute_deadline(
///         self.pacing.last_phase_drift_ns(),
///         self.pacing.last_vblank_mul(),
///         self.gpu_timer.last_gpu_time_ms(),
///         cpu_frame_ms_previous,
///     );
///     pll::sleep_until(diag.deadline_ns);
///     self.pll_diag = Some(diag);
/// }
/// // then call get_current_texture()
/// ```
pub struct PllController {
    /// Ideal vblank period in nanoseconds.
    ideal_period_ns: u64,

    /// Absolute `CLOCK_MONOTONIC` nanoseconds of the most recently targeted vblank.
    ///
    /// Advanced by `ideal_period_ns x vblank_mul` each frame so the grid tracks
    /// the hardware vblank sequence even across dropped frames.
    next_vblank_ns: Option<u64>,

    /// EMA of the GPU render time plus safety margin (nanoseconds).
    ///
    /// Used to set the submit deadline early enough for the flip pipeline to
    /// complete before the vblank edge.  Initialised to 70% of `ideal_period_ns`
    /// and updated each frame from `gpu_time_ms` or `cpu_frame_ms`.
    render_budget_ns: u64,

    /// Accumulated integral error for the PI controller (nanoseconds, f64).
    integrator_ns: f64,

    /// Consecutive frames with |drift| < [`CONVERGENCE_WINDOW_NS`].
    lock_count: u32,

    /// Current convergence state.
    lock_state: PllLockState,
}

impl PllController {
    /// Creates a new controller calibrated to `frame_budget_ms`.
    ///
    /// The render budget is initialised to 70% of the vblank period, which is
    /// a conservative starting point that prevents the first few frames from
    /// overshooting the deadline before the EMA converges.
    ///
    /// # Arguments
    /// * `frame_budget_ms` - Reciprocal of the monitor refresh rate in ms
    ///   (e.g. `8.333` for 120 Hz).  Must match the value used by
    ///   [`crate::metrics::PacingAnalyzer`].
    pub fn new(frame_budget_ms: f32) -> Self {
        let ideal_period_ns = (frame_budget_ms as f64 * 1_000_000.0).round() as u64;
        Self {
            ideal_period_ns,
            next_vblank_ns: None,
            render_budget_ns: (ideal_period_ns as f64 * 0.70) as u64,
            integrator_ns: 0.0,
            lock_count: 0,
            lock_state: PllLockState::Acquiring,
        }
    }

    /// Resets the phase grid and integrator without reallocating.
    ///
    /// Call after a long GPU stall, VT switch, or DPMS wakeup, any event that
    /// causes [`crate::metrics::PacingAnalyzer`] to recalibrate its phase origin,
    /// so wound-up integral and a stale grid anchor do not produce a large
    /// overcorrection on the first frames after recovery.
    pub fn reset(&mut self) {
        self.next_vblank_ns = None;
        self.integrator_ns = 0.0;
        self.lock_count = 0;
        self.lock_state = PllLockState::Acquiring;
    }

    /// Computes the absolute submit deadline for the upcoming frame.
    ///
    /// Must be called once per frame, before `get_current_texture()`.  The
    /// caller should then invoke [`sleep_until`] with `diag.deadline_ns` to
    /// block until the deadline, then proceed with the GPU submit.
    ///
    /// # Arguments
    ///
    /// * `phase_drift_ns` - Signed deviation of the previous frame's presentation
    ///   timestamp from the nearest ideal vblank boundary, from
    ///   [`crate::metrics::PacingAnalyzer::last_phase_drift_ns`].  `None` on the
    ///   first frame or after a clock discontinuity.
    ///
    /// * `last_vblank_mul` - Vblank periods consumed by the previous frame, from
    ///   [`crate::metrics::PacingAnalyzer::last_vblank_mul`].  Used to advance the
    ///   grid by the correct number of periods when a frame was dropped.
    ///
    /// * `hw_vblank_ns` - Hardware vblank timestamp from `DRM_EVENT_FLIP_COMPLETE`
    ///   for this frame, if one arrived.  When `Some` and the grid has not yet been
    ///   anchored, `next_vblank_ns` is seeded directly from this timestamp rather
    ///   than from `now % ideal_period`, aligning the PLL grid with the actual
    ///   hardware scanout clock.  This is the same anchor used by `PacingAnalyzer`
    ///   so both grids are always in phase with each other.
    ///
    /// * `gpu_time_ms` - True GPU execution time from `TIMESTAMP_QUERY`.
    ///
    /// * `cpu_frame_ms` - CPU-observed total frame time, used as a budget fallback.
    pub fn compute_deadline(
        &mut self,
        phase_drift_ns: Option<i64>,
        last_vblank_mul: u32,
        hw_vblank_ns: Option<u64>,
        gpu_time_ms: Option<f32>,
        cpu_frame_ms: f32,
    ) -> PllDiagnostics {
        let now_ns = clock_monotonic_ns();

        // ── Update render budget EMA ──────────────────────────────────────────
        let measured_ns = gpu_time_ms
            .map(|ms| (ms * 1_000_000.0) as u64)
            .unwrap_or_else(|| (cpu_frame_ms * 1_000_000.0) as u64);

        let with_margin = (measured_ns as f64 * (1.0 + BUDGET_SAFETY_MARGIN)) as u64;
        self.render_budget_ns = (BUDGET_EMA_ALPHA * with_margin as f64
            + (1.0 - BUDGET_EMA_ALPHA) * self.render_budget_ns as f64)
            as u64;

        let budget_min = self.ideal_period_ns / 10;
        let budget_max = self.ideal_period_ns * 8 / 10;
        self.render_budget_ns = self.render_budget_ns.clamp(budget_min, budget_max);

        // ── Advance the vblank grid ───────────────────────────────────────────
        // Seed from the hardware flip timestamp when available, so the PLL grid
        // is anchored to the same CLOCK_MONOTONIC epoch as PacingAnalyzer.
        // Without this, `now % ideal_period` produces a different phase offset
        // than the hardware vblank grid, causing the deadline to target the wrong
        // vblank boundary and the PLL to fight the compositor rather than align.
        let mul = last_vblank_mul.max(1) as u64;
        let mut next_vblank = match self.next_vblank_ns {
            None => {
                if let Some(flip_ns) = hw_vblank_ns {
                    // Snap forward from the hardware vblank to find the next
                    // one after `now`.
                    let periods_elapsed = now_ns.saturating_sub(flip_ns) / self.ideal_period_ns;
                    flip_ns + (periods_elapsed + 1) * self.ideal_period_ns
                } else {
                    // No hardware anchor yet; snap to the next multiple of the
                    // period from `now`.  Will be corrected once a flip event
                    // arrives and the PI term converges.
                    let period = self.ideal_period_ns;
                    now_ns + period - (now_ns % period)
                }
            }
            Some(prev) => prev + self.ideal_period_ns * mul,
        };

        // If the calculated vblank is in the past, snap it forward until it is
        // in the future. This prevents massive sleep_ns values when frames stall.
        while next_vblank <= now_ns {
            next_vblank += self.ideal_period_ns;
        }
        self.next_vblank_ns = Some(next_vblank);

        // ── PI correction ─────────────────────────────────────────────────────
        let (p_term, i_term, raw_correction) = match phase_drift_ns {
            None => (0i64, 0i64, 0i64),
            Some(error_ns) => {
                if error_ns.abs() < CONVERGENCE_WINDOW_NS {
                    self.lock_count = self.lock_count.saturating_add(1);
                } else {
                    self.lock_count = 0;
                    if self.lock_state == PllLockState::Locked {
                        // Phase escaped the window; reset integrator to avoid
                        // fighting the now-stale accumulated correction.
                        self.integrator_ns = 0.0;
                    }
                    self.lock_state = PllLockState::Acquiring;
                }
                if self.lock_count >= LOCK_THRESHOLD_FRAMES {
                    self.lock_state = PllLockState::Locked;
                }

                // Halve gains once locked to avoid injecting jitter into a
                // stable loop.
                let (kp, ki) = match self.lock_state {
                    PllLockState::Acquiring => (KP, KI),
                    PllLockState::Locked => (KP * 0.5, KI * 0.5),
                };

                let e = error_ns as f64;
                self.integrator_ns =
                    (self.integrator_ns + e).clamp(-INTEGRATOR_CLAMP_NS, INTEGRATOR_CLAMP_NS);

                let p = kp * e;
                let i = ki * self.integrator_ns;
                (p as i64, i as i64, (p + i) as i64)
            }
        };

        // ── Compute absolute deadline ─────────────────────────────────────────
        //
        // deadline = next_vblank - render_budget - PI_correction
        //
        // A positive PI correction (frame was late) subtracts from the deadline,
        // pulling it earlier so the next submit happens sooner relative to the
        // vblank edge, advancing the presentation timestamp toward the ideal grid.
        //
        // A negative PI correction (frame was early) adds to the deadline,
        // pushing the submit slightly later to avoid the opposite overshoot.
        let deadline_ns = {
            let base = next_vblank.saturating_sub(self.render_budget_ns);
            if raw_correction >= 0 {
                base.saturating_sub(raw_correction as u64)
            } else {
                base.saturating_add(raw_correction.unsigned_abs())
            }
        };

        // If the deadline has already passed (frame is running late), skip the
        // sleep and submit immediately.  PacingAnalyzer will record the vblank
        // miss and advance the grid via `mul` on the next call.
        let sleep_ns = if deadline_ns > now_ns {
            let delta = deadline_ns - now_ns;
            // Prevent divergence: if the delta is physically impossible (> 2 periods),
            // clamp it to zero to force immediate execution and resync.
            if delta >= MIN_SLEEP_NS && delta < self.ideal_period_ns * 2 {
                delta
            } else {
                0
            }
        } else {
            0
        };

        PllDiagnostics {
            phase_error_ns: phase_drift_ns.unwrap_or(0),
            p_term_ns: p_term,
            i_term_ns: i_term,
            raw_correction_ns: raw_correction,
            deadline_ns,
            sleep_ns,
            render_budget_ns: self.render_budget_ns,
            lock_state: self.lock_state,
        }
    }

    /// Returns the current lock state without advancing the controller.
    #[inline]
    #[allow(dead_code)]
    pub fn lock_state(&self) -> PllLockState {
        self.lock_state
    }

    /// Returns the current render budget EMA (nanoseconds).
    #[inline]
    #[allow(dead_code)]
    pub fn render_budget_ns(&self) -> u64 {
        self.render_budget_ns
    }
}

/// Blocks the calling thread until `target_ns` (`CLOCK_MONOTONIC` nanoseconds).
///
/// Uses `clock_nanosleep(TIMER_ABSTIME)` for an absolute wakeup so accumulated
/// wakeup jitter from previous sleeps does not compound across frames.  A
/// relative sleep (`nanosleep`) would drift by the sum of all previous wakeup
/// errors; an absolute sleep drifts by at most one wakeup error per call.
///
/// No-ops when `target_ns` is zero (first frame) or already in the past
/// (late frame, submit immediately without stalling).
#[inline]
pub fn sleep_until(target_ns: u64) {
    if target_ns == 0 {
        return;
    }

    let now = clock_monotonic_ns();
    if target_ns <= now {
        return;
    }

    let target = libc::timespec {
        tv_sec: (target_ns / 1_000_000_000) as libc::time_t,
        tv_nsec: (target_ns % 1_000_000_000) as libc::c_long,
    };

    let ret = unsafe {
        libc::clock_nanosleep(
            libc::CLOCK_MONOTONIC,
            libc::TIMER_ABSTIME,
            &target,
            std::ptr::null_mut(),
        )
    };
    // EINTR: signal delivered mid-sleep, self-corrects next frame.
    let _ = ret;

    let now_ns = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;
    if target_ns > now_ns {
        std::thread::sleep(std::time::Duration::from_nanos(target_ns - now_ns));
    }
}

/// Returns `CLOCK_MONOTONIC` nanoseconds via direct syscall.
///
/// Duplicated from `renderer` so `pll` has no cross-module dependency for a
/// single timestamp read.
fn clock_monotonic_ns() -> u64 {
    let mut ts = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut ts) };
    ts.tv_sec as u64 * 1_000_000_000 + ts.tv_nsec as u64
}
