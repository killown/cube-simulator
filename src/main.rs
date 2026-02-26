use clap::Parser;
use std::sync::Arc;
use wgpu::util::DeviceExt;
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    window::{Window, WindowAttributes},
};

#[derive(Parser, Debug, Clone, Copy)]
#[command(author, version, about = "WGPU Cube Simulator")]
struct Args {
    #[arg(short, long, default_value_t = 6)]
    cubes: u32,
    #[arg(short, long, default_value_t = 0.5)]
    size: f32,
    #[arg(short, long, default_value_t = 1.0)]
    speed: f32,
    #[arg(long, default_value_t = 0.5)]
    red: f32,
    #[arg(long, default_value_t = 0.8)]
    green: f32,
    #[arg(long, default_value_t = 0.2)]
    blue: f32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct ShaderUniforms {
    color: [f32; 4],
    cube_count: u32,
    size: f32,
    speed: f32,
    _padding: f32,
    fps_data: [f32; 4],
}

struct State<'a> {
    surface: wgpu::Surface<'a>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    window: Arc<Window>,
    render_pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
    start_time: std::time::Instant,
    last_fps_update: std::time::Instant,
    frame_count: u32,
    current_fps: f32,
    min_fps: f32,
    max_fps: f32,
    args: Args,
}

impl<'a> State<'a> {
    async fn new(window: Arc<Window>, args: Args) -> State<'a> {
        let size = window.inner_size();
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::GL,
            ..Default::default()
        });

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

        let present_mode = if caps.present_modes.contains(&wgpu::PresentMode::Mailbox) {
            wgpu::PresentMode::Mailbox
        } else if caps.present_modes.contains(&wgpu::PresentMode::Immediate) {
            wgpu::PresentMode::Immediate
        } else {
            wgpu::PresentMode::Fifo
        };

        let uniforms = ShaderUniforms {
            color: [args.red, args.green, args.blue, 1.0],
            cube_count: args.cubes.min(128),
            size: args.size,
            speed: args.speed,
            _padding: 0.0,
            fps_data: [0.0, 0.0, 0.0, 0.0],
        };

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&[uniforms]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
                label: None,
            });

        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
            label: None,
        });

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: caps.formats[0],
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed("
                struct Uniforms {
                    color: vec4<f32>,
                    cube_count: u32,
                    size: f32,
                    speed: f32,
                    padding: f32,
                    fps_data: vec4<f32>,
                };
                @group(0) @binding(0) var<uniform> u: Uniforms;

                struct VertexOutput {
                    @builtin(position) clip_position: vec4<f32>,
                    @location(0) uv: vec2<f32>,
                    @location(1) time: f32,
                };

                @vertex
                fn vs_main(@builtin(vertex_index) v_idx: u32, @builtin(instance_index) i_idx: u32) -> VertexOutput {
                    var out: VertexOutput;
                    let pos = array<vec2<f32>, 4>(vec2(-1.0, -1.0), vec2(1.0, -1.0), vec2(-1.0, 1.0), vec2(1.0, 1.0));
                    out.clip_position = vec4<f32>(pos[v_idx], 0.0, 1.0);
                    out.uv = pos[v_idx];
                    out.time = f32(i_idx) * 0.001;
                    return out;
                }

                fn rot(a: f32) -> mat2x2<f32> {
                    let s = sin(a); let c = cos(a);
                    return mat2x2<f32>(c, s, -s, c);
                }

                fn hash(p: vec2<f32>) -> f32 {
                    return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453123);
                }

                fn sd_digit(uv: vec2<f32>, n: i32) -> f32 {
                    let p = uv * vec2(1.0, -1.0) + vec2(0.5);
                    if p.x < 0.0 || p.x > 1.0 || p.y < 0.0 || p.y > 1.0 { return 0.0; }
                    let segs = array<i32, 10>(252, 96, 218, 242, 102, 182, 190, 224, 254, 246)[n];
                    let w = 0.15;
                    var d = 0.0;
                    if (segs & 128) != 0 && p.y < w { d = 1.0; }
                    if (segs & 64) != 0 && p.x > 1.0-w && p.y < 0.5 { d = 1.0; }
                    if (segs & 32) != 0 && p.x > 1.0-w && p.y > 0.5 { d = 1.0; }
                    if (segs & 16) != 0 && p.y > 1.0-w { d = 1.0; }
                    if (segs & 8) != 0 && p.x < w && p.y > 0.5 { d = 1.0; }
                    if (segs & 4) != 0 && p.x < w && p.y < 0.5 { d = 1.0; }
                    if (segs & 2) != 0 && p.y > 0.5-w/2.0 && p.y < 0.5+w/2.0 { d = 1.0; }
                    return d;
                }

                fn draw_num(uv: vec2<f32>, val: i32) -> f32 {
                    return max(sd_digit(uv, (val/100)%10), 
                           max(sd_digit(uv-vec2(1.2,0.0), (val/10)%10), 
                               sd_digit(uv-vec2(2.4,0.0), val%10)));
                }

                fn map(p: vec3<f32>, t: f32) -> f32 {
                    var d = 1e10;
                    let speed = u.speed;
                    for(var i = 0u; i < u.cube_count; i++) {
                        let fi = f32(i);
                        let offset = vec3(
                            sin(t * 0.5 * speed + fi * 1.047) * 3.5,
                            cos(t * 0.7 * speed + fi * 0.8) * 2.0,
                            sin(t * 0.3 * speed + fi * 2.1) * 1.5
                        );
                        var q = p - offset;
                        let r1 = rot(t * speed * (0.2 + fi * 0.1));
                        let r2 = rot(t * speed * (0.15 + fi * 0.05));
                        let q_xz = r1 * q.xz; q.x = q_xz.x; q.z = q_xz.y;
                        let q_yz = r2 * q.yz; q.y = q_yz.x; q.z = q_yz.y;
                        let a = abs(q);
                        let cube = max(a.x, max(a.y, a.z)) - u.size;
                        let sphere = length(q) - (u.size * 1.4);
                        d = min(d, max(-sphere, cube));
                    }
                    return d;
                }

                @fragment
                fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
                    let t = in.time;
                    let uv = in.uv * vec2(1.77, 1.0);
                    var ro = vec3(0.0, 0.0, 10.0);
                    var rd = normalize(vec3(uv, -1.8));

                    var total = 0.0; var hit = false; var p: vec3<f32>;
                    for(var i=0; i<80; i++) {
                        p = ro + rd * total;
                        let d = map(p, t);
                        if d < 0.002 { hit = true; break; }
                        total += d; if total > 30.0 { break; }
                    }

                    var color: vec3<f32>;
                    let grain = hash(in.uv + fract(t));
                    if !hit {
                        color = mix(vec3(0.01, 0.02, 0.05), vec3(0.05, 0.08, 0.15), in.uv.y * 0.5 + 0.5) + grain * 0.04;
                    } else {
                        let eps = vec2(0.005, 0.0);
                        let n = normalize(vec3(
                            map(p+eps.xyy, t)-map(p-eps.xyy, t), 
                            map(p+eps.yxy, t)-map(p-eps.yxy, t), 
                            map(p+eps.yyx, t)-map(p-eps.yyx, t)
                        ));
                        let light = max(dot(n, normalize(vec3(1.0, 2.0, 1.0))), 0.2);
                        color = u.color.rgb * light + grain * 0.03;
                    }

                    let d_c = draw_num((in.uv - vec2(-0.88, 0.88)) * 12.0, i32(u.fps_data.x));
                    let d_ma = draw_num((in.uv - vec2(-0.88, 0.76)) * 12.0, i32(u.fps_data.z));
                    let d_mi = draw_num((in.uv - vec2(-0.88, 0.64)) * 12.0, i32(u.fps_data.y));
                    let d = max(d_c, max(d_ma, d_mi));

                    return vec4(mix(color, vec3(0.0, 1.0, 0.0), d), 1.0);
                }
            ")),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&uniform_bind_group_layout],
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

        Self {
            surface,
            device,
            queue,
            config,
            window,
            render_pipeline,
            uniform_buffer,
            uniform_bind_group,
            start_time: std::time::Instant::now(),
            last_fps_update: std::time::Instant::now(),
            frame_count: 0,
            current_fps: 0.0,
            min_fps: 0.0,
            max_fps: 0.0,
            args,
        }
    }

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
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
            rpass.set_bind_group(0, &self.uniform_bind_group, &[]);
            rpass.draw(0..4, packed..(packed + 1));
        }
        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        self.frame_count += 1;
        let now = std::time::Instant::now();
        let diff = now.duration_since(self.last_fps_update);
        if diff.as_secs_f32() >= 0.5 {
            self.current_fps = self.frame_count as f32 / diff.as_secs_f32();

            if self.min_fps == 0.0 || self.current_fps < self.min_fps {
                self.min_fps = self.current_fps;
            }
            if self.current_fps > self.max_fps {
                self.max_fps = self.current_fps;
            }

            let uniforms = ShaderUniforms {
                color: [self.args.red, self.args.green, self.args.blue, 1.0],
                cube_count: self.args.cubes.min(128),
                size: self.args.size,
                speed: self.args.speed,
                _padding: 0.0,
                fps_data: [self.current_fps, self.min_fps, self.max_fps, 0.0],
            };
            self.queue
                .write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));

            self.frame_count = 0;
            self.last_fps_update = now;
        }
        Ok(())
    }
}

struct App<'a> {
    state: Option<State<'a>>,
    args: Args,
}

impl<'a> ApplicationHandler for App<'a> {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        let window = Arc::new(el.create_window(WindowAttributes::default()).unwrap());
        self.state = Some(pollster::block_on(State::new(window, self.args)));
    }
    fn window_event(
        &mut self,
        el: &ActiveEventLoop,
        _id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        if let Some(state) = self.state.as_mut() {
            match event {
                WindowEvent::CloseRequested => el.exit(),
                WindowEvent::Resized(s) => {
                    state.config.width = s.width.max(1);
                    state.config.height = s.height.max(1);
                    state.surface.configure(&state.device, &state.config);
                }
                WindowEvent::RedrawRequested => {
                    let _ = state.render();
                    state.window.request_redraw();
                }
                _ => (),
            }
        }
    }
}

fn main() {
    let args = Args::parse();
    let mut app = App { state: None, args };
    EventLoop::new().unwrap().run_app(&mut app).unwrap();
}
