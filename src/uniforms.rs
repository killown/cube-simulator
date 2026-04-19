use wgpu::util::DeviceExt;

use crate::args::Args;

/// GPU-side uniform block mirroring the WGSL `Uniforms` struct.
///
///
/// This struct uses `#[repr(C)]` and 16-byte alignment to ensure compatibility with
/// WGSL `std140` layout rules. The total size is exactly 112 bytes.
///
/// | Offset | Field         | Type       | Description                                     |
/// |--------|---------------|------------|-------------------------------------------------|
/// | 0      | color         | [f32; 4]   | Base RGBA color                                 |
/// | 16     | cube_count    | u32        | Active instance count                           |
/// | 20     | size          | f32        | Cube edge length                                |
/// | 24     | speed         | f32        | Animation speed multiplier                      |
/// | 28     | steps         | u32        | Raymarching step limit                          |
/// | 32     | fps_data      | [f32; 4]   | [avg, min, max, low_1%]                         |
/// | 48     | adv_data      | [f32; 4]   | [jitter, dropped, ftv, _unused]                 |
/// | 64     | time          | f32        | Total elapsed seconds                           |
/// | 68     | stutter_decay | f32        | Instantaneous stutter marker (fades out)        |
/// | 72     | pacing_decay  | f32        | Sustained delivery pressure (regime indicator)  |
/// | 76     | gpu_time_ms   | f32        | Measured GPU execution time                     |
/// | 80     | sync_score    | f32        | Alignment with vblank edge (0-100)              |
/// | 84     | cpu_time_ms   | f32        | Measured CPU frame processing time              |
/// | 88     | slack_ms      | f32        | Margin before vblank deadline                   |
/// | 92     | sync_var      | f32        | Variance in frame delivery timing               |
/// | 96     | _pad          | [f32; 4]   | Padding to 112 bytes (multiple of 16)           |
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ShaderUniforms {
    /// Base RGBA color for the cubes.
    pub color: [f32; 4],
    /// Number of active cubes to render.
    pub cube_count: u32,
    /// Edge length of each cube.
    pub size: f32,
    /// Multiplier for the rotation/animation speed.
    pub speed: f32,
    /// Raymarching step count or iteration limit.
    pub steps: u32,
    /// Standard FPS metrics: `[avg_fps, min_fps, max_fps, low_1_fps]`.
    pub fps_data: [f32; 4],
    /// Extended metrics: `[jitter_ms, dropped_count, frame_time_variance, _unused]`.
    pub adv_data: [f32; 4],
    /// Total elapsed time in seconds.
    pub time: f32,
    /// Decaying stutter indicator: set to `1.0` when a frame is dropped or
    /// jitter exceeds the configured threshold; decays by `STUTTER_DECAY_RATE`
    /// each frame so the shader can render a fading visual marker.
    pub stutter_decay: f32,
    /// Sustained delivery-pressure indicator driven by an exponential moving
    /// average of `vblank_mul`. Rises toward `1.0` while `EMA > 1.15` (the
    /// compositor is consistently delivering frames late) and decays at
    /// `PACING_DECAY_RATE` per frame when delivery recovers. Completely
    /// orthogonal to `stutter_decay`: it measures a bad *regime*, not a
    /// single bad frame.
    pub pacing_decay: f32,
    /// GPU execution time for the previous frame in milliseconds.
    pub gpu_time_ms: f32,
    /// Normalized score (0-100) representing vblank alignment.
    pub sync_score: f32,
    /// CPU processing time for the previous frame in milliseconds.
    pub cpu_time_ms: f32,
    /// Margin between frame completion and the vblank deadline.
    pub slack_ms: f32,
    /// Variance in frame delivery timing.
    pub sync_var: f32,
    /// Explicit padding to ensure the struct size is a multiple of 16 bytes.
    /// Reaching 112 bytes satisfies the WGSL layout contract.
    pub _pad: [f32; 4],
}

impl ShaderUniforms {
    /// Creates a zero-initialized instance.
    pub fn zeroed() -> Self {
        Self {
            color: [0.0; 4],
            cube_count: 0,
            size: 0.0,
            speed: 0.0,
            steps: 0,
            fps_data: [0.0; 4],
            adv_data: [0.0; 4],
            time: 0.0,
            stutter_decay: 0.0,
            pacing_decay: 0.0,
            gpu_time_ms: 0.0,
            sync_score: 0.0,
            cpu_time_ms: 0.0,
            slack_ms: 0.0,
            sync_var: 0.0,
            _pad: [0.0; 4],
        }
    }

    /// Constructs the initial uniform value from CLI args with zeroed metric fields.
    pub fn from_args(args: &Args, cube_count: u32) -> Self {
        let mut u = Self::zeroed();
        u.color = [args.red, args.green, args.blue, 1.0];
        u.cube_count = cube_count;
        u.size = args.size;
        u.speed = args.speed;
        u.steps = args.steps;
        u
    }

    /// Constructs a uniform with live per-frame metric data.
    #[allow(clippy::too_many_arguments)]
    pub fn with_metrics(
        args: &Args,
        cube_count: u32,
        current_fps: f32,
        min_fps: f32,
        max_fps: f32,
        low_1_fps: f32,
        jitter: f32,
        dropped_frames: u32,
        ftv: f32,
        time: f32,
        stutter_decay: f32,
        pacing_decay: f32,
        gpu_time_ms: f32,
        sync_score: f32,
        cpu_time_ms: f32,
        slack_ms: f32,
        sync_var: f32,
    ) -> Self {
        Self {
            color: [args.red, args.green, args.blue, 1.0],
            cube_count,
            size: args.size,
            speed: args.speed,
            steps: args.steps,
            fps_data: [current_fps, min_fps, max_fps, low_1_fps],
            adv_data: [jitter, dropped_frames as f32, ftv, 0.0],
            time,
            stutter_decay,
            pacing_decay,
            gpu_time_ms,
            sync_score,
            cpu_time_ms,
            slack_ms,
            sync_var,
            _pad: [0.0; 4],
        }
    }
}

/// Owns the GPU uniform buffer and its bind group.
pub struct UniformBinding {
    pub buffer: wgpu::Buffer,
    pub bind_group: wgpu::BindGroup,
    pub layout: wgpu::BindGroupLayout,
}

impl UniformBinding {
    /// Allocates the uniform buffer, layout, and bind group in one shot.
    pub fn new(device: &wgpu::Device, initial: &ShaderUniforms) -> Self {
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&[*initial]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
            label: None,
        });

        Self {
            buffer,
            bind_group,
            layout,
        }
    }

    /// Uploads a new uniform value to the GPU.
    #[inline]
    pub fn write(&self, queue: &wgpu::Queue, uniforms: &ShaderUniforms) {
        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&[*uniforms]));
    }
}
