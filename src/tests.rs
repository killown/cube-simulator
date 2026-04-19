#[cfg(test)]
mod frame_metrics {
    use crate::metrics::FrameMetrics;
    use std::time::{Duration, Instant};

    const HZ_60_MS: f32 = 1000.0 / 60.0;

    fn instant_plus(base: Instant, ms: u64) -> Instant {
        base + Duration::from_millis(ms)
    }

    // ── compute_cpu_jitter ───────────────────────────────────────────────────

    #[test]
    fn cpu_jitter_zero_for_uniform_series() {
        let mut m = FrameMetrics::new(HZ_60_MS);
        let base = Instant::now();
        for i in 0..10 {
            m.push(HZ_60_MS, HZ_60_MS * 1.5, instant_plus(base, i * 17), None);
        }
        if let Some(stats) = m.push(HZ_60_MS, HZ_60_MS * 1.5, instant_plus(base, 600), None) {
            assert!(
                stats.jitter < 0.1,
                "jitter={} expected≈0 for uniform series",
                stats.jitter
            );
        }
    }

    // ── compute_cpu_ftv ──────────────────────────────────────────────────────

    #[test]
    fn cpu_ftv_zero_for_uniform_series() {
        let mut m = FrameMetrics::new(HZ_60_MS);
        let base = Instant::now();
        for i in 0..30 {
            m.push(HZ_60_MS, HZ_60_MS * 1.5, instant_plus(base, i * 17), None);
        }
        if let Some(stats) = m.push(HZ_60_MS, HZ_60_MS * 1.5, instant_plus(base, 600), None) {
            assert!(
                stats.ftv < 0.1,
                "ftv={} expected≈0 for uniform series",
                stats.ftv
            );
        }
    }

    #[test]
    fn cpu_ftv_nonzero_for_alternating_series() {
        let mut m = FrameMetrics::new(HZ_60_MS);
        let base = Instant::now();
        let mut t_ms = 0u64;
        for i in 0..30 {
            let delta = if i % 2 == 0 { 5.0f32 } else { 48.0f32 };
            t_ms += delta as u64;
            m.push(delta, HZ_60_MS * 1.5, instant_plus(base, t_ms), None);
        }
        if let Some(stats) = m.push(5.0, HZ_60_MS * 1.5, instant_plus(base, t_ms + 600), None) {
            assert!(
                stats.ftv > 50.0,
                "ftv={} expected >50 for alternating series",
                stats.ftv
            );
        }
    }

    // ── low_1_fps is bottom percentile, not minimum ──────────────────────────

    #[test]
    fn low_1_fps_less_than_or_equal_current_fps() {
        let mut m = FrameMetrics::new(HZ_60_MS);
        let base = Instant::now();
        for i in 0..60u64 {
            let delta = if i == 29 { 100.0f32 } else { HZ_60_MS };
            m.push(delta, HZ_60_MS * 1.5, instant_plus(base, i * 17), None);
        }
        if let Some(stats) = m.push(HZ_60_MS, HZ_60_MS * 1.5, instant_plus(base, 700), None) {
            assert!(
                stats.low_1_fps <= stats.current_fps,
                "low_1_fps={} should be ≤ current_fps={}",
                stats.low_1_fps,
                stats.current_fps
            );
        }
    }

    // ── dropped_frames counting ──────────────────────────────────────────────

    #[test]
    fn dropped_frames_counted_on_large_delta() {
        let mut m = FrameMetrics::new(HZ_60_MS);
        let base = Instant::now();
        let threshold = HZ_60_MS * 1.5;
        let two_vblanks = HZ_60_MS * 2.0;
        m.push(two_vblanks, threshold, base, None);
        if let Some(stats) = m.push(HZ_60_MS, threshold, instant_plus(base, 600), None) {
            assert!(
                stats.dropped_frames >= 1,
                "expected ≥1 dropped frame for 2-vblank delta, got {}",
                stats.dropped_frames
            );
        }
    }

    // ── calculate_sync_var ───────────────────────────────────────────────────

    #[test]
    fn sync_var_zero_for_single_score() {
        let m = FrameMetrics::new(HZ_60_MS);
        assert_eq!(m.calculate_sync_var(), 0.0);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod bench_score {
    use crate::benchmark::{BenchScore, BenchStepResult, BenchTrigger};

    fn clean(cube_count: u32) -> BenchStepResult {
        BenchStepResult {
            cube_count,
            measured_secs: 3.0,
            trigger: None,
        }
    }

    fn yellow(cube_count: u32) -> BenchStepResult {
        BenchStepResult {
            cube_count,
            measured_secs: 1.2,
            trigger: Some(BenchTrigger::Yellow),
        }
    }

    fn red(cube_count: u32) -> BenchStepResult {
        BenchStepResult {
            cube_count,
            measured_secs: 0.5,
            trigger: Some(BenchTrigger::Red),
        }
    }

    #[test]
    fn perfect_sweep_3_cubes() {
        let results = vec![clean(1), clean(2), clean(3)];
        let s = BenchScore::compute(&results);
        assert_eq!(s.clean_cubes, 3);
        assert_eq!(s.clean_points, 300);
        assert_eq!(s.perfect_bonus, 500);
        assert_eq!(s.trigger_points, 0);
        assert_eq!(s.total, 800);
        assert!(s.trigger.is_none());
    }

    #[test]
    fn yellow_trigger_at_cube_5_after_4_clean() {
        let results = vec![clean(1), clean(2), clean(3), clean(4), yellow(5)];
        let s = BenchScore::compute(&results);
        assert_eq!(s.clean_cubes, 4);
        assert_eq!(s.clean_points, 400);
        assert_eq!(s.trigger_points, 5 * 40); // 200
        assert_eq!(s.perfect_bonus, 0);
        assert_eq!(s.total, 600);
        assert!(matches!(s.trigger, Some((5, BenchTrigger::Yellow))));
    }

    #[test]
    fn red_trigger_at_cube_3_after_2_clean() {
        let results = vec![clean(1), clean(2), red(3)];
        let s = BenchScore::compute(&results);
        assert_eq!(s.clean_cubes, 2);
        assert_eq!(s.clean_points, 200);
        assert_eq!(s.trigger_points, 3 * 10); // 30
        assert_eq!(s.perfect_bonus, 0);
        assert_eq!(s.total, 230);
        assert!(matches!(s.trigger, Some((3, BenchTrigger::Red))));
    }

    #[test]
    fn yellow_trigger_on_first_step() {
        let results = vec![yellow(1)];
        let s = BenchScore::compute(&results);
        assert_eq!(s.clean_cubes, 0);
        assert_eq!(s.clean_points, 0);
        assert_eq!(s.trigger_points, 1 * 40);
        assert_eq!(s.total, 40);
    }

    #[test]
    fn red_trigger_on_first_step() {
        let results = vec![red(1)];
        let s = BenchScore::compute(&results);
        assert_eq!(s.trigger_points, 1 * 10);
        assert_eq!(s.total, 10);
    }

    #[test]
    fn empty_results_gives_perfect_bonus_only() {
        let s = BenchScore::compute(&[]);
        assert_eq!(s.perfect_bonus, 500);
        assert_eq!(s.total, 500);
        assert_eq!(s.clean_cubes, 0);
    }

    // ── Grade thresholds ─────────────────────────────────────────────────────

    #[test]
    fn grade_s_at_4500() {
        let results: Vec<_> = (1..=44).map(clean).collect();
        let s = BenchScore::compute(&results);
        assert!(s.total >= 4500, "total={} should be ≥4500 for S", s.total);
    }

    #[test]
    fn grade_d_below_500() {
        let s = BenchScore::compute(&[red(1)]);
        assert!(s.total < 500, "total={} should be <500 for D", s.total);
    }
}

// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod uniforms {
    use crate::args::Args;
    use crate::uniforms::ShaderUniforms;

    fn default_args() -> Args {
        Args {
            cubes: 6,
            size: 0.5,
            speed: 1.0,
            red: 0.18,
            green: 0.18,
            blue: 0.18,
            threshold: None,
            format: None,
            mode: None,
            steps: 80,
            csv: None,
            json: None,
            frame_log: None,
            connector: None,
            latency: 2,
            bench_secs: None,
            bench_warmup: 2,
            bench_max: 64,
            shader: None,
            pll: false,
        }
    }

    // ── Size and alignment ───────────────────────────────────────────────────

    #[test]
    fn struct_size_is_112_bytes() {
        assert_eq!(
            std::mem::size_of::<ShaderUniforms>(),
            112,
            "ShaderUniforms must be exactly 112 bytes (WGSL layout contract)"
        );
    }

    #[test]
    fn struct_alignment_is_4_bytes() {
        assert_eq!(
            std::mem::align_of::<ShaderUniforms>(),
            4,
            "ShaderUniforms must be 4-byte aligned for bytemuck casting"
        );
    }

    // ── from_args field mapping ──────────────────────────────────────────────

    #[test]
    fn from_args_maps_color_channels() {
        let args = default_args();
        let u = ShaderUniforms::from_args(&args, 6);
        assert!((u.color[0] - 0.18).abs() < 1e-6, "color[0] (red) mismatch");
        assert!(
            (u.color[1] - 0.18).abs() < 1e-6,
            "color[1] (green) mismatch"
        );
        assert!((u.color[2] - 0.18).abs() < 1e-6, "color[2] (blue) mismatch");
        assert!(
            (u.color[3] - 1.0).abs() < 1e-6,
            "color[3] (alpha) must be 1.0"
        );
    }

    #[test]
    fn from_args_zeroes_all_metric_fields() {
        let u = ShaderUniforms::from_args(&default_args(), 6);
        assert_eq!(u.fps_data, [0.0f32; 4]);
        assert_eq!(u.adv_data, [0.0f32; 4]);
        assert_eq!(u.time, 0.0);
        assert_eq!(u.stutter_decay, 0.0);
        assert_eq!(u.pacing_decay, 0.0);
        assert_eq!(u.gpu_time_ms, 0.0);
        assert_eq!(u.sync_score, 0.0);
        assert_eq!(u.cpu_time_ms, 0.0);
        assert_eq!(u.slack_ms, 0.0);
    }

    // ── with_metrics round-trip ──────────────────────────────────────────────

    #[test]
    fn with_metrics_fps_data_correct() {
        let args = default_args();
        let u = ShaderUniforms::with_metrics(
            &args, 6, 120.0, 60.0, 144.0, 55.0, 0.5, 2, 3.0, 1.0, 0.0, 0.0, 4.5, 95.0, 8.0, 12.0,
            1.5,
        );
        assert_eq!(u.fps_data, [120.0, 60.0, 144.0, 55.0]);
    }

    #[test]
    fn with_metrics_adv_data_dropped_frames() {
        let args = default_args();
        let u = ShaderUniforms::with_metrics(
            &args, 6, 60.0, 60.0, 60.0, 60.0, 0.5, 7, 3.0, 1.0, 0.0, 0.0, 4.5, 95.0, 8.0, 12.0, 1.5,
        );
        assert_eq!(
            u.adv_data[1], 7.0,
            "adv_data[1] must equal dropped_frames as f32"
        );
    }

    // ── Pod/Zeroable: bytemuck cast must not panic ───────────────────────────

    #[test]
    fn bytemuck_cast_does_not_panic() {
        let u = ShaderUniforms::zeroed();
        let _bytes: &[u8] = bytemuck::cast_slice(&[u]);
    }

    #[test]
    fn bytemuck_roundtrip_preserves_values() {
        let args = default_args();
        let original = ShaderUniforms::from_args(&args, 4);
        let arr = [original];
        let bytes: &[u8] = bytemuck::cast_slice(&arr);
        let back: &[ShaderUniforms] = bytemuck::cast_slice(bytes);
        assert_eq!(back[0].cube_count, 4);
        assert!((back[0].color[0] - 0.18).abs() < 1e-6);
    }
}

// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod drm_info {
    use crate::drm::{ActiveMode, ConnectorInfo, DrmInfo};

    fn make_info() -> DrmInfo {
        DrmInfo {
            connectors: vec![
                ConnectorInfo {
                    name: "DP-1".into(),
                    active_mode: Some(ActiveMode {
                        width: 2560,
                        height: 1440,
                        refresh_hz: 144,
                    }),
                    vrr_enabled: Some(true),
                },
                ConnectorInfo {
                    name: "HDMI-A-1".into(),
                    active_mode: None,
                    vrr_enabled: None,
                },
            ],
            primary_crtc: Some(42),
            dev_path: "/dev/dri/card0".into(),
        }
    }

    #[test]
    fn find_refresh_hz_returns_correct_rate() {
        let info = make_info();
        assert_eq!(info.find_refresh_hz("DP-1"), Some(144));
    }

    #[test]
    fn find_refresh_hz_is_case_insensitive() {
        let info = make_info();
        assert_eq!(info.find_refresh_hz("dp-1"), Some(144));
        assert_eq!(info.find_refresh_hz("DP-1"), Some(144));
    }

    #[test]
    fn find_refresh_hz_returns_none_for_unknown_connector() {
        let info = make_info();
        assert!(info.find_refresh_hz("VGA-1").is_none());
    }

    #[test]
    fn find_refresh_hz_returns_none_for_inactive_connector() {
        let info = make_info();
        assert!(
            info.find_refresh_hz("HDMI-A-1").is_none(),
            "inactive connector should return None"
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod gpu_tier_helpers {
    use crate::gpu_tier::trim_field;

    #[test]
    fn trim_field_pads_short_string_to_width() {
        let s = trim_field("hi", 10);
        assert_eq!(s.len(), 10, "expected len=10, got {}", s.len());
        assert!(s.starts_with("hi"), "expected 'hi' prefix");
    }

    #[test]
    fn trim_field_truncates_long_string_with_dotdot() {
        let s = trim_field("abcdefghijklmnop", 8);
        assert_eq!(s.len(), 8, "expected len=8, got {}", s.len());
        assert!(s.ends_with(".."), "expected '..' suffix, got '{s}'");
    }

    #[test]
    fn trim_field_exact_width_no_ellipsis() {
        let s = trim_field("12345678", 8);
        assert_eq!(s.len(), 8);
        assert!(
            !s.ends_with(".."),
            "exact-width string should not get '..' suffix"
        );
    }

    #[test]
    fn trim_field_empty_string_pads_to_width() {
        let s = trim_field("", 5);
        assert_eq!(s.len(), 5);
        assert_eq!(s.trim(), "");
    }
}

// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod flip_tracker_parsing {
    use crate::flip_tracker::read_drm_events_from_buf;

    fn make_flip_event(tv_sec: u32, tv_usec: u32, sequence: u32) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&0x02u32.to_le_bytes()); // DRM_EVENT_FLIP_COMPLETE
        buf.extend_from_slice(&24u32.to_le_bytes()); // length = 24 bytes
        buf.extend_from_slice(&tv_sec.to_le_bytes());
        buf.extend_from_slice(&tv_usec.to_le_bytes());
        buf.extend_from_slice(&sequence.to_le_bytes());
        buf.extend_from_slice(&0u32.to_le_bytes()); // crtc_id
        buf
    }

    #[test]
    fn parses_single_flip_event() {
        let buf = make_flip_event(1_000, 500_000, 42);
        let records = read_drm_events_from_buf(&buf);
        assert_eq!(records.len(), 1);
        let r = records[0];
        let expected_ns = 1_000u64 * 1_000_000_000 + 500_000u64 * 1_000;
        assert_eq!(r.flip_ns, expected_ns);
        assert_eq!(r.sequence, 42);
    }

    #[test]
    fn parses_two_consecutive_flip_events() {
        let mut buf = make_flip_event(1, 0, 1);
        buf.extend(make_flip_event(1, 16_667, 2));
        let records = read_drm_events_from_buf(&buf);
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].sequence, 1);
        assert_eq!(records[1].sequence, 2);
    }

    #[test]
    fn ignores_unknown_event_type() {
        let mut buf = Vec::new();
        buf.extend_from_slice(&0x99u32.to_le_bytes()); // unknown type
        buf.extend_from_slice(&24u32.to_le_bytes());
        buf.extend_from_slice(&[0u8; 16]);
        let records = read_drm_events_from_buf(&buf);
        assert!(records.is_empty(), "unknown event type should be skipped");
    }

    #[test]
    fn empty_buffer_returns_empty_vec() {
        let records = read_drm_events_from_buf(&[]);
        assert!(records.is_empty());
    }

    #[test]
    fn truncated_buffer_does_not_panic() {
        // Buffer large enough for header but not for payload
        let buf = &[0x02u8, 0, 0, 0, 24, 0, 0, 0, 0, 0];
        let records = read_drm_events_from_buf(buf);
        assert!(
            records.is_empty(),
            "truncated buffer should yield no records"
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod cross_module_pll_convergence {
    use crate::metrics::PacingAnalyzer;
    use crate::pll::PllController;

    #[test]
    fn pll_sleep_ns_trend_decreases_toward_vblank() {
        const HZ_60_MS: f32 = 1000.0 / 60.0;
        let period_ns: u64 = (HZ_60_MS as f64 * 1_000_000.0).round() as u64;

        let mut analyzer = PacingAnalyzer::new(HZ_60_MS);
        let mut ctrl = PllController::new(HZ_60_MS);

        let mut ts = 2_000_000_000u64;
        let late_offset = 4_000_000u64; // 4 ms in ns
        let mut sleep_samples = Vec::new();

        for _ in 0..24 {
            ts += period_ns + late_offset / 24; // gradually drift in
            analyzer.push(ts, None, None, None, None, None, None);

            let drift = analyzer.last_phase_drift_ns();
            let mul = analyzer.last_vblank_mul();
            let d = ctrl.compute_deadline(drift, mul, None, Some(5.0), HZ_60_MS);
            sleep_samples.push(d.sleep_ns);
        }

        let _mid = sleep_samples.len() / 2;
        let max_sleep = *sleep_samples.iter().max().unwrap();
        assert!(
            max_sleep < period_ns * 2,
            "max sleep_ns={max_sleep} exceeded 2× period, controller diverged"
        );
    }

    #[test]
    fn pll_locks_on_perfect_pacing() {
        use crate::pll::PllLockState;

        const HZ_60_MS: f32 = 1000.0 / 60.0;
        let period_ns: u64 = (HZ_60_MS as f64 * 1_000_000.0).round() as u64;

        let mut analyzer = PacingAnalyzer::new(HZ_60_MS);
        let mut ctrl = PllController::new(HZ_60_MS);
        let mut ts = 2_000_000_000u64;

        let mut last_state = PllLockState::Acquiring;
        for _ in 0..16 {
            ts += period_ns;
            analyzer.push(ts, None, None, None, None, None, None);
            let d = ctrl.compute_deadline(
                analyzer.last_phase_drift_ns(),
                analyzer.last_vblank_mul(),
                None,
                Some(5.0),
                HZ_60_MS,
            );
            last_state = d.lock_state;
        }

        assert_eq!(
            last_state,
            PllLockState::Locked,
            "PLL should be Locked after 16 perfect frames"
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod frame_log_json_schema {
    use crate::pll::{PllDiagnostics, PllLockState};

    fn make_pll() -> PllDiagnostics {
        PllDiagnostics {
            phase_error_ns: 100_000,
            p_term_ns: 50_000,
            i_term_ns: 2_000,
            raw_correction_ns: 52_000,
            deadline_ns: 2_008_000_000,
            sleep_ns: 3_000_000,
            render_budget_ns: 8_000_000,
            lock_state: PllLockState::Locked,
        }
    }

    #[test]
    fn emitted_line_is_valid_ndjson_with_schema_5() {
        let mut buf: Vec<u8> = Vec::new();
        {
            use std::io::Write;
            write!(
                &mut buf,
                r#"{{"schema":5,"frame":{},"cube_count":{},"ts_ns":{},"delta_ms":{:.4},"ideal_ms":{:.4},"drift_ms":{:.4},"drift_ns":{},"vblank_mul":{},"sync":{:.2}}}"#,
                0u64, 6u32, 2_000_000_000u64, 16.667f32, 16.667f32, 0.1f32, 100_000i64, 1u32, 98.8f32,
            ).unwrap();
            writeln!(&mut buf).unwrap();
        }

        let line = std::str::from_utf8(&buf).unwrap();
        let v: serde_json::Value = serde_json::from_str(line.trim()).expect("must be valid JSON");
        assert_eq!(v["schema"], 5, "schema field must be 5");
        assert_eq!(v["frame"], 0);
        assert_eq!(v["vblank_mul"], 1);
    }

    #[test]
    fn emitted_line_contains_pll_fields_when_pll_active() {
        let mut buf: Vec<u8> = Vec::new();
        {
            use std::io::Write;
            let pll = make_pll();
            write!(
                &mut buf,
                r#"{{"schema":5,"frame":0,"cube_count":6,"ts_ns":0,"delta_ms":0.0000,"ideal_ms":0.0000,"drift_ms":0.0000,"drift_ns":0,"vblank_mul":1,"sync":0.00,"pll_error_ns":{},"pll_sleep_ns":{},"pll_deadline_ns":{},"pll_budget_ns":{},"pll_lock":{}}}"#,
                pll.phase_error_ns,
                pll.sleep_ns,
                pll.deadline_ns,
                pll.render_budget_ns,
                if pll.lock_state == PllLockState::Locked { 1 } else { 0 },
            ).unwrap();
            writeln!(&mut buf).unwrap();
        }

        let line = std::str::from_utf8(&buf).unwrap();
        let v: serde_json::Value = serde_json::from_str(line.trim()).expect("must be valid JSON");
        assert_eq!(v["pll_lock"], 1, "pll_lock must be 1 when Locked");
        assert_eq!(v["pll_error_ns"], 100_000i64);
    }
}
