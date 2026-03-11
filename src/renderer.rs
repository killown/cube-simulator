use std::fs::File;
use std::io::Write;
use std::sync::Arc;

use winit::window::Window;

use crate::args::Args;
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
            .request_device(&wgpu::DeviceDescriptor::default())
            .await
            .unwrap();

        let caps = surface.get_capabilities(&adapter);
        let surface_format = resolve_surface_format(&caps, args.format.as_deref());
        let present_mode = resolve_present_mode(&caps, args.mode.as_deref());

        println!("Surface Format: {:?}", surface_format);
        println!("Present Mode: {:?}", present_mode);

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
                    "WARN: Backend does not support presentation timestamps — \
                     jitter/FTV will use CPU timers (compositor-schedulable)"
                );
                false
            } else {
                println!("Presentation timestamps: available (WSI-domain, compositor-resistant)");
                true
            }
        };

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
                    "WARN: --frame-log requested but backend has no HW timestamps — skipping"
                );
                return None;
            }
            write_info(path, &raw_header);
            let mut f = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .unwrap();
            let _ = writeln!(f, "# frame,ts_ns,delta_ms,ideal_ms,drift_ms,sync");
            Some(f)
        });

        let initial_uniforms = ShaderUniforms::from_args(&args, args.cubes);
        let uniform_binding = UniformBinding::new(&device, &initial_uniforms);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(include_str!(
                "shader.wgsl"
            ))),
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

        // Write time in one upload before acquire.
        self.current_uniforms.time = self.start_time.elapsed().as_secs_f32();
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
                ..Default::default()
            });
            rpass.set_pipeline(&self.render_pipeline);
            rpass.set_bind_group(0, &self.uniform_binding.bind_group, &[]);
            rpass.draw(0..4, 0..1);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        // Sample the WSI clock after present(). On DRM/KMS+Vulkan backends this
        // is CLOCK_MONOTONIC nanoseconds from the presentation engine — the same
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
                None
            } else {
                let now_ns = ts.0 as u64;
                let delta = self.last_pres_ts.map(|prev| {
                    // nanoseconds → milliseconds; saturating_sub guards against
                    // monotonic clock resets on suspend/resume cycles.
                    now_ns.saturating_sub(prev) as f32 / 1_000_000.0
                });
                self.last_pres_ts = Some(now_ns);

                // Feed the raw timestamp into the pacing analyzer before the
                // 500ms tick gate so every individual frame's phase drift and
                // sync score are captured and logged at full frame resolution.
                if let Some(record) = self.pacing.push(now_ns) {
                    write_frame_log_row(&mut self.frame_log_file, &record);
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

            self.current_uniforms = ShaderUniforms::with_metrics(
                &self.args,
                self.args.cubes,
                stats.current_fps,
                stats.min_fps,
                stats.max_fps,
                stats.low_1_fps,
                stats.jitter,
                stats.dropped_frames,
                stats.ftv,
                self.start_time.elapsed().as_secs_f32(),
            );
            self.uniform_binding
                .write(&self.queue, &self.current_uniforms);
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
