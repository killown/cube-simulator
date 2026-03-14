use std::time::Instant;

/// Trigger that ended a benchmark step early.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BenchTrigger {
    /// `vblank_mul > 1` for any single frame: the amber-yellow hollow ring.
    Yellow,
    /// `EMA(vblank_mul) > 1.15`: the red filled diamond, sustained pressure.
    Red,
}

/// Outcome of one completed benchmark step.
#[derive(Debug, Clone)]
pub struct BenchStepResult {
    pub cube_count: u32,
    /// How many seconds the measurement window ran (excluding warmup).
    pub measured_secs: f32,
    pub trigger: Option<BenchTrigger>,
}

/// Phase of a single benchmark step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StepPhase {
    /// Still in the warmup window, signals are ignored.
    Warmup,
    /// Warmup has elapsed, signals now count.
    Measuring,
}

/// Drives the benchmark state machine across a cube-count sweep.
///
/// Each step tests a single cube count for `step_secs` seconds total, of which
/// the first `warmup_secs` are discarded.  When a yellow or red signal fires
/// during the measurement window the step is immediately terminated and the
/// result is recorded.  Advancing past `max_cubes` or receiving a terminal
/// trigger both cause [`BenchmarkState::is_done`] to return `true`, at which
/// point the caller should close the window and call [`BenchmarkState::print_report`].
pub struct BenchmarkState {
    /// Seconds allotted to each step (including warmup).
    step_secs: u64,
    /// Seconds to discard at the start of each step.
    warmup_secs: u64,
    /// Highest cube count to probe before declaring a clean pass.
    max_cubes: u32,

    /// Current cube count under test.
    pub current_cubes: u32,
    /// Wall-clock anchor for the current step (set when the step starts).
    step_start: Instant,
    /// Wall-clock anchor for the start of the measurement window within a step.
    measure_start: Option<Instant>,
    phase: StepPhase,

    /// Accumulated results, one per completed step.
    results: Vec<BenchStepResult>,
    /// Set once we have a terminal result (trigger fired or sweep complete).
    done: bool,
}

impl BenchmarkState {
    /// Creates a new benchmark starting at 1 cube.
    ///
    /// # Panics
    /// Panics if `warmup_secs >= step_secs`.
    pub fn new(step_secs: u64, warmup_secs: u64, max_cubes: u32) -> Self {
        assert!(
            warmup_secs < step_secs,
            "--bench-warmup ({warmup_secs}s) must be less than --bench-secs ({step_secs}s)"
        );
        let now = Instant::now();
        eprintln!(
            "\n[BENCH] Starting sweep: 1..={max_cubes} cubes, {step_secs}s/step, {warmup_secs}s warmup"
        );
        Self {
            step_secs,
            warmup_secs,
            max_cubes,
            current_cubes: 1,
            step_start: now,
            measure_start: None,
            phase: StepPhase::Warmup,
            results: Vec::new(),
            done: false,
        }
    }

    /// Called every frame.  `pacing_decay` carries the yellow (momentary vblank
    /// miss) signal and `stutter_decay` carries the red (sustained EMA pressure)
    /// signal, mirroring the field semantics in `State`.
    ///
    /// `vblank_mul_ema` is informational only and is no longer used for trigger
    /// logic here: `stutter_decay` is already set to `1.0` in the renderer
    /// whenever `vblank_mul_ema > PACING_EMA_THRESHOLD`, so testing the EMA
    /// again here would double-count the same condition.
    ///
    /// Returns `true` when the benchmark has just transitioned to done, so the
    /// caller knows to request an event-loop exit after this frame.
    pub fn tick(
        &mut self,
        pacing_decay: f32,
        stutter_decay: f32,
        _vblank_mul_ema: f32,
        any_vblank_miss: bool,
    ) -> bool {
        if self.done {
            return false;
        }

        let now = Instant::now();
        let step_elapsed = now.duration_since(self.step_start).as_secs_f64();

        // ── Warmup → Measure transition ────────────────────────────────────
        if self.phase == StepPhase::Warmup && step_elapsed >= self.warmup_secs as f64 {
            self.phase = StepPhase::Measuring;
            self.measure_start = Some(now);
            eprintln!(
                "[BENCH] {} cubes - warmup done, measuring…",
                self.current_cubes
            );
        }

        // ── Signal detection (measurement window only) ─────────────────────
        if self.phase == StepPhase::Measuring {
            let measured_secs = now
                .duration_since(self.measure_start.unwrap())
                .as_secs_f32();

            // Red: sustained EMA pressure, stutter_decay is set to 1.0 in the
            // renderer whenever vblank_mul_ema > PACING_EMA_THRESHOLD, so a
            // value ≥ 1.0 here means the EMA threshold was crossed this frame.
            // Yellow: any single vblank miss, pacing_decay or the per-frame flag.
            let trigger = if stutter_decay >= 1.0 {
                Some(BenchTrigger::Red)
            } else if pacing_decay >= 1.0 || any_vblank_miss {
                Some(BenchTrigger::Yellow)
            } else {
                None
            };

            if let Some(t) = trigger {
                self.record_step(measured_secs, Some(t));
                self.done = true;
                return true;
            }

            // Step timer expired, clean pass for this cube count.
            if step_elapsed >= self.step_secs as f64 {
                self.record_step(measured_secs, None);

                if self.current_cubes >= self.max_cubes {
                    // Entire sweep completed without a trigger.
                    self.done = true;
                    return true;
                }

                self.advance_step(now);
            }
        }

        false
    }

    /// Advances to the next cube count, resetting all per-step state.
    fn advance_step(&mut self, now: Instant) {
        self.current_cubes += 1;
        self.step_start = now;
        self.measure_start = None;
        self.phase = StepPhase::Warmup;
        eprintln!(
            "[BENCH] Advancing to {} cubes (warmup {}s)…",
            self.current_cubes, self.warmup_secs
        );
    }

    fn record_step(&mut self, measured_secs: f32, trigger: Option<BenchTrigger>) {
        let label = match trigger {
            None => "PASS".to_owned(),
            Some(BenchTrigger::Yellow) => "YELLOW (vblank miss)".to_owned(),
            Some(BenchTrigger::Red) => "RED    (sustained pressure)".to_owned(),
        };
        eprintln!(
            "[BENCH] {} cubes → {label}  ({measured_secs:.1}s measured)",
            self.current_cubes
        );
        self.results.push(BenchStepResult {
            cube_count: self.current_cubes,
            measured_secs,
            trigger,
        });
    }

    /// Prints the final human-readable benchmark report to stdout.
    ///
    /// Should be called after [`BenchmarkState::is_done`] returns `true`,
    /// typically just before the event loop exits.
    pub fn print_report(&self) {
        println!("\n══════════════════════════════════════════");
        println!("  COMPOSITOR BENCHMARK RESULTS");
        println!("══════════════════════════════════════════");

        let clean: Vec<_> = self
            .results
            .iter()
            .filter(|r| r.trigger.is_none())
            .collect();

        let trigger_step = self.results.iter().find(|r| r.trigger.is_some());

        if clean.is_empty() && trigger_step.is_none() {
            println!("  No steps completed.");
            return;
        }

        if !clean.is_empty() {
            let max_clean = clean.last().unwrap().cube_count;
            println!("  ✓  Clean up to   : {max_clean} cubes");
            println!("  Steps passed     :");
            for r in &clean {
                println!(
                    "       {} cubes - PASS ({:.1}s measured)",
                    r.cube_count, r.measured_secs
                );
            }
        }

        match trigger_step {
            None => {
                println!("\n  ✓  Entire sweep completed with NO compositor pressure detected.");
                println!(
                    "     The compositor handled all {} cube counts cleanly.",
                    self.results.last().map_or(0, |r| r.cube_count)
                );
            }
            Some(r) => {
                let signal = match r.trigger.unwrap() {
                    BenchTrigger::Yellow => "YELLOW - isolated vblank miss (amber ring)",
                    BenchTrigger::Red => "RED    - sustained EMA pressure (red diamond)",
                };
                println!("\n  ✗  Trigger at     : {} cubes", r.cube_count);
                println!("     Signal         : {signal}");
                println!(
                    "     Measured for   : {:.1}s before trigger",
                    r.measured_secs
                );

                let safe_count = r.cube_count.saturating_sub(1);
                if safe_count == 0 {
                    println!("\n  ⚠  Compositor could not sustain even 1 cube without pressure.");
                } else {
                    println!("\n  ➜  Maximum safe cube count: {safe_count}");
                }
            }
        }

        println!("══════════════════════════════════════════\n");
    }
}
