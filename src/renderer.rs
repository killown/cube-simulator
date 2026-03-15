use std::fs::File;
use std::io::Write;
use std::sync::Arc;

use winit::window::Window;

use crate::args::Args;
use crate::benchmark::BenchmarkState;
use crate::gpu_tier::GpuTier;
use crate::gpu_timer::GpuTimer;
use crate::metrics::{
    FrameMetrics, PacingAnalyzer, write_csv_row, write_frame_log_row, write_json_row,
};
use crate::uniforms::{ShaderUniforms, UniformBinding};

/// Full GPU rendering state for one window.
///
/// Owns the wgpu surface, device, queue, pipeline, uniform binding, and all
/// per-frame timing state. Created once in [`App::resumed`] and driven by
/// `winit`'s `RedrawRequested` event.
pub struct State<'a> {
    pub surface: wgpu::Surface<'a>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    pub window: Arc<Window>,
    /// Retained to call `get_presentation_timestamp()` each frame.
    adapter: wgpu::Adapter,
    render_pipeline: wgpu::RenderPipeline,
    uniform_binding: UniformBinding,
    start_time: std::time::Instant,
    last_frame_time: std::time::Instant,
    /// WSI-domain timestamp from the previous frame's post-present sample.
    /// Used to compute inter-frame deltas in the presentation engine's own
    /// nanosecond clock, which `max_render_time` cannot shift.
    last_pres_ts: Option<u64>,
    metrics: FrameMetrics,
    /// Phase-locked pacing analyzer fed every frame before the 500ms tick gate,
    /// so every individual frame's drift and sync score are captured.
    pacing: PacingAnalyzer,
    csv_file: Option<File>,
    json_file: Option<File>,
    /// Per-frame NDJSON pacing log; `None` when `--frame-log` is not passed or
    /// when the backend returns invalid presentation timestamps.
    frame_log_file: Option<File>,
    args: Args,
    current_uniforms: ShaderUniforms,
    /// Per-frame decaying stutter intensity. Set to `1.0` when a frame is
    /// detected as dropped or jitter exceeds the configured threshold;
    /// decremented by `STUTTER_DECAY_RATE` each frame toward `0.0`.
    stutter_decay: f32,
    /// Exponential moving average of `vblank_mul` sampled each frame from
    /// `FramePacingRecord`.  Tracks sustained delivery pressure: `1.0` = perfect
    /// on-time delivery, `2.0` = every frame costs two vblanks.  The EMA
    /// smooths over isolated spikes so only regime-level badness triggers yellow.
    vblank_mul_ema: f32,
    /// Decaying compositor-pressure indicator.  Set to `1.0` when
    /// `vblank_mul_ema` exceeds the `PACING_EMA_THRESHOLD`; decays at
    /// `PACING_DECAY_RATE` per frame when delivery recovers.
    pacing_decay: f32,
    /// Active benchmark state machine; `None` when not in `--bench-secs` mode.
    pub benchmark: Option<BenchmarkState>,
    /// Set to `true` on the frame where the benchmark terminates, signalling the
    /// app layer to call `el.exit()` after this `RedrawRequested` completes.
    pub benchmark_done: bool,
    /// Whether the most-recently-processed pacing record had `vblank_mul > 1`.
    /// Reset each frame; fed into the benchmark tick.
    any_vblank_miss_this_frame: bool,
    /// Hardware GPU execution timer. Wraps a `TIMESTAMP_QUERY` `QuerySet`; is a
    /// zero-cost no-op when the adapter does not support the feature.
    gpu_timer: GpuTimer,
    /// Last printed `raw_vblank_mul`; suppresses duplicate `[MICRO]` lines when
    /// two consecutive WSI deltas both reflect the same overrun event.
    last_micro_vblank_mul: u32,
}

/// Returns the current `CLOCK_MONOTONIC` time in nanoseconds via a direct libc
/// syscall, placing it in the same epoch as WSI/KMS presentation timestamps so
/// that `submit_ns, present_ts` produces an accurate `slack_ms` without any
/// anchor arithmetic.
///
/// Falls back to `0` only on platforms where the syscall is unavailable; the
/// caller treats `0` as `None` in the pacing filter (`gap_ms > 0.0`).
#[cfg(target_os = "linux")]
#[inline]
fn clock_monotonic_ns() -> u64 {
    // SAFETY: `timespec` is zero-initialised before the syscall writes it;
    // `CLOCK_MONOTONIC` (1) is a valid, always-present clock id on Linux.
    let mut ts = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut ts) };
    ts.tv_sec as u64 * 1_000_000_000 + ts.tv_nsec as u64
}

#[cfg(not(target_os = "linux"))]
#[inline]
fn clock_monotonic_ns() -> u64 {
    0
}

impl<'a> State<'a> {
    /// Initialises the full wgpu stack, selects surface format and present mode,
    /// compiles the shader, and builds the render pipeline.
    ///
    /// Exits the process with a diagnostic message if the requested `--format`
    /// or `--mode` is not supported by the adapter.
    pub async fn new(
        window: Arc<Window>,
        args: Args,
        drm_info: Option<crate::drm::DrmInfo>,
    ) -> State<'a> {
        let size = window.inner_size();
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        // Connector-pinned refresh rate is the only reliable source on multi-monitor
        // setups. winit asks the compositor, which may report a virtual 60Hz rate
        // even on a 165Hz panel. DRM talks directly to KMS and is always correct.
        let frame_budget_ms = args
            .connector
            .as_deref()
            .and_then(|name| drm_info.as_ref()?.find_refresh_hz(name))
            .map(|hz| 1000.0 / hz as f32)
            .or_else(|| {
                window
                    .current_monitor()
                    .and_then(|m| m.refresh_rate_millihertz())
                    .map(|mhz| 1_000_000.0 / mhz as f32)
            })
            .unwrap_or(1000.0 / 60.0);

        println!(
            "Frame Budget: {:.4}ms ({:.2}Hz)  ",
            frame_budget_ms,
            1000.0 / frame_budget_ms,
        );

        let surface = instance.create_surface(Arc::clone(&window)).unwrap();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .await
            .unwrap();

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                // TIMESTAMP_QUERY: outer render-pass timer (shader time).
                // TIMESTAMP_QUERY_INSIDE_ENCODERS: encoder brackets for driver
                // overhead and DMA resolve time. Both degrade silently if absent.
                required_features: adapter.features()
                    & (wgpu::Features::TIMESTAMP_QUERY
                        | wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS),
                ..Default::default()
            })
            .await
            .unwrap();

        let caps = surface.get_capabilities(&adapter);
        let surface_format = resolve_surface_format(&caps, args.format.as_deref());
        let present_mode = resolve_present_mode(&caps, args.mode.as_deref());

        println!("Surface Format: {:?}", surface_format);
        println!("Present Mode: {:?}", present_mode);
        println!(
            "Frame Latency: {} (desired_maximum_frame_latency)",
            args.latency
        );

        if present_mode == wgpu::PresentMode::Fifo {
            println!("NOTE: In Fifo mode, the driver and compositor handle synchronization");
            println!(
                "      internally. Frame pacing is controlled via the display refresh cycle.\n"
            );
        }

        println!("MODE EXPLANATIONS:");
        println!("  - Fifo: Standard VSync. Blocks CPU until the next monitor refresh.");
        println!("  - Mailbox: Triple Buffering. Never blocks, replaces the last waiting frame.");
        println!("  - Immediate: Uncapped. Renders as fast as possible, may cause tearing.\n");

        // Warn once if the backend does not support WSI-domain timestamps.
        // In that case jitter/FTV fall back to CPU Instant deltas, which are
        // schedulable by compositor tricks such as max_render_time.
        let hw_timestamps_available = {
            let ts = adapter.get_presentation_timestamp();
            if ts.is_invalid() {
                eprintln!(
                    "WARN: Backend does not support presentation timestamps, \
                     jitter/FTV will use CPU timers (compositor-schedulable)"
                );
                false
            } else {
                println!("Presentation timestamps: available (WSI-domain, compositor-resistant)");
                true
            }
        };

        let gpu_timer = GpuTimer::new(&device, &queue);
        if gpu_timer.is_available() {
            println!("GPU timestamps:          available (TIMESTAMP_QUERY, hardware-accurate)");
        } else {
            eprintln!(
                "WARN: TIMESTAMP_QUERY not supported; gpu_time_ms absent from frame logs. \
                 Cannot distinguish GPU budget overrun from compositor buffer-hold."
            );
        }
        if gpu_timer.is_micro_available() {
            println!(
                "GPU micro-timings:       available (driver/shader/resolve breakdown on vblank miss)"
            );
        }

        let cmd_line = std::env::args().collect::<Vec<_>>().join(" ");
        let mut raw_header = format!("Command: {}\n", cmd_line);

        if let Some(drm) = crate::drm::query() {
            for c in &drm.connectors {
                if let Some(m) = &c.active_mode {
                    let vrr = match c.vrr_enabled {
                        Some(true) => " (VRR: On)",
                        Some(false) => " (VRR: Off)",
                        None => "",
                    };
                    raw_header.push_str(&format!(
                        "{}: {}x{} @ {}Hz{}\n",
                        c.name, m.width, m.height, m.refresh_hz, vrr
                    ));
                }
            }
        }
        raw_header.push_str(&format!("Surface Format: {:?}\n", surface_format));
        raw_header.push_str(&format!("Present Mode: {:?}\n", present_mode));
        raw_header.push_str(&format!(
            "GPU Timestamps: {}\n",
            if gpu_timer.is_available() {
                "available"
            } else {
                "unavailable"
            }
        ));

        let write_info = |base_path: &str, content: &str| {
            let path = std::path::Path::new(base_path);
            let stem = path.file_stem().unwrap_or_default().to_string_lossy();
            let parent = path.parent().unwrap_or_else(|| std::path::Path::new(""));
            let info_path = parent.join(format!("{}-info.txt", stem));
            if let Ok(mut f) = std::fs::File::create(info_path) {
                let _ = write!(f, "{}", content);
            }
        };

        let csv_file = args.csv.as_ref().map(|path| {
            write_info(path, &raw_header);
            let mut f = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .unwrap();
            let _ = writeln!(f, "FPS,MIN,MAX,LOW_1,JITTER,DROPPED,FTV,TS_SOURCE");
            f
        });

        let json_file = args.json.as_ref().map(|path| {
            write_info(path, &raw_header);
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .unwrap()
        });

        // Only open the frame log when the backend can supply hardware timestamps;
        // silently skip it otherwise to avoid writing a file full of CPU-derived
        // data that would be misleading under the per-frame pacing contract.
        let frame_log_file = args.frame_log.as_ref().and_then(|path| {
            if !hw_timestamps_available {
                eprintln!(
                    "WARN: --frame-log requested but backend has no HW timestamps, skipping"
                );
                return None;
            }
            write_info(path, &raw_header);
            let mut f = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .unwrap();
            let _ = writeln!(f, "# schema:2 frame,cube_count,ts_ns,delta_ms,ideal_ms,drift_ms,drift_ns,vblank_mul,sync[,ipc_delta_ms][,cpu_frame_ms][,slack_ms][,gpu_time_ms]");
            Some(f)
        });

        // In benchmark mode the initial cube count is always 1; the machine
        // drives `current_cubes` forward from there.
        let initial_cube_count = if args.bench_secs.is_some() {
            1
        } else {
            args.cubes
        };
        let initial_uniforms = ShaderUniforms::from_args(&args, initial_cube_count);
        let uniform_binding = UniformBinding::new(&device, &initial_uniforms);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: args.latency,
        };
        surface.configure(&device, &config);

        let gpu_tier = GpuTier::resolve(&adapter, args.shader.as_deref());
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(gpu_tier.shader_source())),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&uniform_binding.layout],
            immediate_size: 0,
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let benchmark = args
            .bench_secs
            .map(|secs| BenchmarkState::new(secs, args.bench_warmup, args.bench_max));

        let now = std::time::Instant::now();
        Self {
            surface,
            device,
            queue,
            config,
            window,
            adapter,
            render_pipeline,
            uniform_binding,
            start_time: now,
            last_frame_time: now,
            last_pres_ts: None,
            metrics: FrameMetrics::new(frame_budget_ms),
            pacing: PacingAnalyzer::new(frame_budget_ms),
            csv_file,
            json_file,
            frame_log_file,
            args,
            current_uniforms: initial_uniforms,
            stutter_decay: 0.0,
            vblank_mul_ema: 1.0,
            pacing_decay: 0.0,
            benchmark,
            benchmark_done: false,
            any_vblank_miss_this_frame: false,
            gpu_timer,
            last_micro_vblank_mul: 0,
        }
    }

    /// Encodes and submits one frame, then updates per-frame timing metrics.
    pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        // Snapshot before acquire so delta excludes swapchain stall
        let frame_start = std::time::Instant::now();
        let total_frame_delta = frame_start
            .duration_since(self.last_frame_time)
            .as_secs_f32()
            * 1000.0;
        self.last_frame_time = frame_start;

        // RED  (stutter_decay):  severe regime, EMA(vblank_mul) > 1.15.
        //   Lingers ~45 frames so a bad run leaves a long-lasting red ghost.
        const STUTTER_DECAY_RATE: f32 = 1.0 / 45.0;
        // YELLOW (pacing_decay): per-frame warning, any single vblank_mul > 1.
        //   Fades quickly (~30 frames) so it reads as a momentary ping, not an alarm.
        const PACING_DECAY_RATE: f32 = 1.0 / 30.0;
        // EMA smoothing: α=0.05 gives a ~19-frame half-life, long enough to
        // ignore a single dropped frame but short enough to react to a bad run
        // within ~60ms at 165 Hz.
        const PACING_EMA_ALPHA: f32 = 0.05;
        // Fire red once the rolling mean vblank cost exceeds 1.15×.
        // 1.15 corresponds to roughly 1 dropped frame every 7 frames, a
        // clearly degraded regime without penalising isolated one-off drops.
        const PACING_EMA_THRESHOLD: f32 = 1.15;

        // Decay both markers unconditionally; trigger paths only set them to 1.0.
        // stutter_decay (red) is set in the HW pacing block below on EMA breach,
        // or in the CPU fallback on threshold breach.
        // pacing_decay (yellow) is set on any single vblank_mul > 1.
        self.stutter_decay = (self.stutter_decay - STUTTER_DECAY_RATE).max(0.0);
        self.pacing_decay = (self.pacing_decay - PACING_DECAY_RATE).max(0.0);
        self.any_vblank_miss_this_frame = false;

        // Flush pending wgpu callbacks (including the map_async completion for the
        // previous frame's timestamp readback) before draining the gpu_timer state
        // machine. With latency=1 the swapchain does not block long enough for the
        // Vulkan driver to fire the callback on its own; a non-blocking Poll here
        // guarantees the Mapped state is visible to gpu_timer.poll() below without
        // stalling the CPU.
        let _ = self.device.poll(wgpu::PollType::Poll);

        // Drain the async readback from the previous frame. Must happen before
        // resolve() overwrites the readback buffer with frame N-1's queries.
        self.gpu_timer.poll();
        let gpu_time_ms = self.gpu_timer.last_gpu_time_ms();
        let micro = self.gpu_timer.last_micro_timings();

        // Write time in one upload before acquire.
        self.current_uniforms.time = self.start_time.elapsed().as_secs_f32();
        self.current_uniforms.stutter_decay = self.stutter_decay;
        self.current_uniforms.pacing_decay = self.pacing_decay;
        self.uniform_binding
            .write(&self.queue, &self.current_uniforms);

        // Measure JIT/Back-pressure: How long does the swapchain block us?
        let output = self.surface.get_current_texture()?;

        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());

        // Resolve frame N-1's timestamp queries before the render pass overwrites
        // the query set with frame N's timestamps. Both ops are in the same submit
        // so the GPU sees them in order: [resolve N-1] → [render pass N].
        self.gpu_timer.resolve(&mut encoder);

        // MICRO_SLOT_SUBMIT_PRE: marks when the GPU starts processing this
        // command buffer. Gap to PASS_BEGIN = driver submission latency.
        self.gpu_timer.write_pre_pass(&mut encoder);

        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                timestamp_writes: self.gpu_timer.timestamp_writes(),
                ..Default::default()
            });
            rpass.set_pipeline(&self.render_pipeline);
            rpass.set_bind_group(0, &self.uniform_binding.bind_group, &[]);
            rpass.draw(0..4, 0..1);
        }

        // MICRO_SLOT_RESOLVE_END: written after the render pass and the micro
        // query-set resolve. Gap to PASS_END = GPU DMA + PCIe copy overhead.
        self.gpu_timer.write_post_resolve(&mut encoder);

        self.queue.submit(std::iter::once(encoder.finish()));
        // Arm the async map for the resolve just submitted. The result becomes
        // available the next time gpu_timer.poll() runs (top of next frame).
        self.gpu_timer.arm_readback();
        // Capture CPU-domain submit timestamp immediately after the driver has
        // accepted the command buffer. Uses CLOCK_MONOTONIC directly via libc so
        // the value is in the same nanosecond epoch as WSI presentation timestamps
        // without any anchor arithmetic or accumulated error.
        let cpu_submit_ns = clock_monotonic_ns();
        output.present();
        let cpu_frame_ms = frame_start.elapsed().as_secs_f32() * 1000.0;

        let mut pacing_record = None;

        // Sample the WSI clock after present(). On DRM/KMS+Vulkan backends this
        // is CLOCK_MONOTONIC nanoseconds from the presentation engine, the same
        // domain as vblank timestamps. Computing deltas between consecutive samples
        // gives inter-frame intervals that compositor scheduling (max_render_time,
        // frame callbacks) cannot fabricate, because the clock ticks independently
        // of any Wayland protocol messages.
        //
        // On backends that return is_invalid() (OpenGL, some Vulkan ICDs without
        // VK_EXT_calibrated_timestamps) we fall back to CPU Instant deltas, which
        // ARE schedulable. The `hw_verified` flag in TickStats exposes which path ran.
        //
        //FIXME: To get true, microsecond-accurate frame pacing, we need hardware-level presentation timestamps
        // In Fifo the driver absorbs the vsync wait internally before returning from get_current_texture(), so our CPU timer is ~0ms.
        // hardware timestamps would also improve Immediate/Mailbox precision.
        // https://docs.rs/wgpu/latest/wgpu/struct.PresentationTimestamp.html
        let wsi_delta_ms: Option<f32> = {
            let ts = self.adapter.get_presentation_timestamp();
            if ts.is_invalid() {
                self.last_pres_ts = None;
                // No HW timestamps: threshold breach is the only coarse proxy available.
                // Maps to red (severe) since a 25ms+ stall is always a hard hitch.
                if total_frame_delta > self.args.threshold {
                    self.stutter_decay = 1.0;
                }
                None
            } else {
                let now_ns = ts.0 as u64;
                let delta = self.last_pres_ts.map(|prev| {
                    // nanoseconds → milliseconds; saturating_sub guards against
                    // monotonic clock resets on suspend/resume cycles.
                    now_ns.saturating_sub(prev) as f32 / 1_000_000.0
                });
                self.last_pres_ts = Some(now_ns);

                // Compute a raw vblank multiplier directly from the WSI delta
                // before feeding it into the pacing analyzer. The analyzer may
                // discard this frame (first frame, clock discontinuity, or
                // delta > 3×ideal_ms on a severe GPU overrun), but the micro-
                // stutter diagnosis must fire unconditionally on any missed vblank.
                let ideal_ms = self.metrics.frame_budget_ms;
                let raw_vblank_mul = if let Some(d) = delta {
                    if d > ideal_ms * 1.25 {
                        (d / ideal_ms).round() as u32
                    } else {
                        1
                    }
                } else {
                    1
                };

                // Suppress [MICRO] output for the first 3 seconds after startup.
                // The outer GPU timer lags one frame on cold start so shader_ms
                // reads 0.00ms, producing misleading DRIVER STALL labels that are
                // purely an artefact of the readback pipeline warming up.
                const MICRO_WARMUP_SECS: f32 = 3.0;

                if raw_vblank_mul > 1 {
                    if let Some(m) = micro {
                        let elapsed = self.start_time.elapsed().as_secs_f32();
                        // Print only when vblank_mul changes value: suppresses the
                        // duplicate that appears when two consecutive WSI deltas both
                        // reflect the same overrun (overrun frame + recovery frame).
                        if elapsed >= MICRO_WARMUP_SECS
                            && raw_vblank_mul != self.last_micro_vblank_mul
                        {
                            let cause = if m.driver_overhead_ms > 2.0 {
                                "DRIVER STALL"
                            } else if m.resolve_ms > 0.3 {
                                "RESOLVE/DMA SPIKE"
                            } else {
                                "SHADER OVERRUN"
                            };
                            eprintln!(
                                "[MICRO] vblank×{} — {} \
                                 (driver {:.2}ms  shader {:.2}ms  resolve {:.2}ms  total {:.2}ms)",
                                raw_vblank_mul,
                                cause,
                                m.driver_overhead_ms,
                                m.shader_ms,
                                m.resolve_ms,
                                m.total_ms,
                            );
                        }
                        self.last_micro_vblank_mul = raw_vblank_mul;
                    }
                } else {
                    self.last_micro_vblank_mul = 0;
                }

                // Feed the raw timestamp into the pacing analyzer before the
                // 500ms tick gate so every individual frame's phase drift and
                // sync score are captured and logged at full frame resolution.
                pacing_record = self.pacing.push(
                    now_ns,
                    Some(cpu_frame_ms),
                    Some(cpu_submit_ns),
                    gpu_time_ms,
                    micro,
                );

                if let Some(record) = &pacing_record {
                    let live_cube_count = self
                        .benchmark
                        .as_ref()
                        .map_or(self.args.cubes, |b| b.current_cubes);
                    write_frame_log_row(&mut self.frame_log_file, record, live_cube_count);

                    if record.vblank_mul > 1 {
                        self.pacing_decay = 1.0;
                        self.any_vblank_miss_this_frame = true;
                    }

                    // EMA breach is the severe regime, fire red and let it linger.
                    self.vblank_mul_ema = PACING_EMA_ALPHA * record.vblank_mul as f32
                        + (1.0 - PACING_EMA_ALPHA) * self.vblank_mul_ema;
                    if self.vblank_mul_ema > PACING_EMA_THRESHOLD {
                        self.stutter_decay = 1.0;
                    }
                }

                delta.filter(|&ms| ms > 0.0 && ms < 1000.0)
            }
        };

        let presentation_ts: Option<u64> = self.last_pres_ts.filter(|_| wsi_delta_ms.is_some());

        if let Some(stats) = self.metrics.push(
            total_frame_delta,
            self.args.threshold,
            frame_start,
            presentation_ts,
        ) {
            write_csv_row(&mut self.csv_file, &stats);
            write_json_row(&mut self.json_file, &stats);

            // In benchmark mode the cube count may have been updated mid-step;
            // always read from the state machine so the uniform stays in sync.
            let live_cube_count = self
                .benchmark
                .as_ref()
                .map_or(self.args.cubes, |b| b.current_cubes);

            self.current_uniforms = ShaderUniforms::with_metrics(
                &self.args,
                live_cube_count,
                stats.current_fps,
                stats.min_fps,
                stats.max_fps,
                stats.low_1_fps,
                stats.jitter,
                stats.dropped_frames,
                stats.ftv,
                self.start_time.elapsed().as_secs_f32(),
                self.stutter_decay,
                self.pacing_decay,
                self.gpu_timer.last_gpu_time_ms().unwrap_or(0.0),
                pacing_record
                    .as_ref()
                    .map(|r| r.sync_score)
                    .unwrap_or(self.current_uniforms.sync_score),
                cpu_frame_ms,
            );

            self.uniform_binding
                .write(&self.queue, &self.current_uniforms);
        }
        // ── Benchmark tick ────────────────────────────────────────────────────
        // Runs after the uniforms are updated so the last frame of a step still
        // renders the correct cube count before the machine advances.
        if let Some(bench) = self.benchmark.as_mut() {
            let just_done = bench.tick(
                self.pacing_decay,
                self.stutter_decay,
                self.vblank_mul_ema,
                self.any_vblank_miss_this_frame,
            );

            if just_done {
                self.benchmark_done = true;
            } else {
                // Hot-patch the cube count in the live uniform without waiting
                // for the next 500ms metrics tick.
                let new_count = bench.current_cubes;
                if self.current_uniforms.cube_count != new_count {
                    self.current_uniforms.cube_count = new_count;
                    self.uniform_binding
                        .write(&self.queue, &self.current_uniforms);
                }
            }
        }

        Ok(())
    }
}

/// Resolves the surface format from caps, exiting with diagnostics on mismatch.
fn resolve_surface_format(
    caps: &wgpu::SurfaceCapabilities,
    requested: Option<&str>,
) -> wgpu::TextureFormat {
    match requested {
        Some(req) => {
            match caps
                .formats
                .iter()
                .find(|f| format!("{:?}", f).eq_ignore_ascii_case(req))
            {
                Some(&f) => f,
                None => {
                    println!("Error: Invalid or unsupported format '{}'", req);
                    println!("Available formats for this surface:");
                    for f in &caps.formats {
                        println!("  {:?}", f);
                    }
                    std::process::exit(1);
                }
            }
        }
        None => caps
            .formats
            .iter()
            .find(|&&f| f == wgpu::TextureFormat::Bgra8UnormSrgb)
            .copied()
            .unwrap_or(caps.formats[0]),
    }
}

/// Resolves the present mode from caps, exiting with diagnostics on mismatch.
fn resolve_present_mode(
    caps: &wgpu::SurfaceCapabilities,
    requested: Option<&str>,
) -> wgpu::PresentMode {
    if let Some(req) = requested {
        let selected = match req.to_lowercase().as_str() {
            "mailbox" => Some(wgpu::PresentMode::Mailbox),
            "immediate" => Some(wgpu::PresentMode::Immediate),
            "fifo" => Some(wgpu::PresentMode::Fifo),
            _ => None,
        };

        return match selected {
            Some(m) if caps.present_modes.contains(&m) => m,
            _ => {
                println!("Error: Invalid or unsupported present mode '{}'", req);
                println!("Available present modes for this surface:");
                for m in &caps.present_modes {
                    println!("  {:?}", m);
                }
                std::process::exit(1);
            }
        };
    }

    if caps.present_modes.contains(&wgpu::PresentMode::Mailbox) {
        wgpu::PresentMode::Mailbox
    } else if caps.present_modes.contains(&wgpu::PresentMode::Immediate) {
        wgpu::PresentMode::Immediate
    } else {
        wgpu::PresentMode::Fifo
    }
}
