use std::fs::File;
use std::io::Write;
use std::sync::Arc;

use winit::window::Window;

use crate::args::Args;
use crate::metrics::{FrameMetrics, write_csv_row, write_json_row};
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
    render_pipeline: wgpu::RenderPipeline,
    uniform_binding: UniformBinding,
    start_time: std::time::Instant,
    last_frame_time: std::time::Instant,
    metrics: FrameMetrics,
    csv_file: Option<File>,
    json_file: Option<File>,
    args: Args,
}

impl<'a> State<'a> {
    /// Initialises the full wgpu stack, selects surface format and present mode,
    /// compiles the shader, and builds the render pipeline.
    ///
    /// Exits the process with a diagnostic message if the requested `--format`
    /// or `--mode` is not supported by the adapter.
    pub async fn new(window: Arc<Window>, args: Args) -> State<'a> {
        let size = window.inner_size();
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let frame_budget_ms = window
            .current_monitor()
            .and_then(|m| m.refresh_rate_millihertz())
            .map(|mhz| 1_000_000.0 / mhz as f32) // millihertz → ms per frame
            .unwrap_or(16.666);

        println!(
            "Frame Budget: {:.3}ms ({:.1}Hz)",
            frame_budget_ms,
            1000.0 / frame_budget_ms
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

        let csv_file = args.csv.as_ref().map(|path| {
            let mut f = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .unwrap();
            let _ = writeln!(f, "FPS,MIN,MAX,LOW_1,JITTER,DROPPED,FTV");
            f
        });

        let json_file = args.json.as_ref().map(|path| {
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .unwrap()
        });

        let initial_uniforms = ShaderUniforms::from_args(&args);
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
            render_pipeline,
            uniform_binding,
            start_time: now,
            last_frame_time: now,
            metrics: FrameMetrics::new(frame_budget_ms),
            csv_file,
            json_file,
            args,
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

        // Measure JIT/Back-pressure: How long does the swapchain block us?
        let output = self.surface.get_current_texture()?;

        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        let packed = self.start_time.elapsed().as_millis() as u32;

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
            rpass.draw(0..4, packed..(packed + 1));
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        if let Some(stats) = self
            .metrics
            .push(total_frame_delta, self.args.threshold, frame_start)
        {
            write_csv_row(&mut self.csv_file, &stats);
            write_json_row(&mut self.json_file, &stats);

            let uniforms = ShaderUniforms::with_metrics(
                &self.args,
                stats.current_fps,
                stats.min_fps,
                stats.max_fps,
                stats.low_1_fps,
                stats.jitter,
                stats.dropped_frames,
                stats.ftv,
            );
            self.uniform_binding.write(&self.queue, &uniforms);
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
