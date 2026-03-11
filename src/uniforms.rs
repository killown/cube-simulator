// uniforms.rs
use wgpu::util::DeviceExt;

use crate::args::Args;

/// GPU-side uniform block mirroring the WGSL `Uniforms` struct.
///
/// Must remain `#[repr(C)]` and implement `Pod`/`Zeroable` so it can be
/// cast directly to bytes via `bytemuck` without any intermediate copy.
///
/// Layout (80 bytes, 16-byte aligned):
/// ```text
/// offset  0: color      [f32; 4]
/// offset 16: cube_count u32
/// offset 20: size       f32
/// offset 24: speed      f32
/// offset 28: steps      u32
/// offset 32: fps_data   [f32; 4]
/// offset 48: adv_data   [f32; 4]
/// offset 64: time       f32
/// offset 68: _pad       [f32; 3]
/// ```
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ShaderUniforms {
    pub color: [f32; 4],
    pub cube_count: u32,
    pub size: f32,
    pub speed: f32,
    pub steps: u32,
    pub fps_data: [f32; 4],
    /// [jitter, dropped, ftv, _pad]
    pub adv_data: [f32; 4],
    pub time: f32,
    pub _pad: [f32; 3],
}

impl ShaderUniforms {
    /// Constructs the initial uniform value from CLI args with zeroed metric fields.
    pub fn from_args(args: &Args, cube_count: u32) -> Self {
        Self {
            color: [args.red, args.green, args.blue, 1.0],
            cube_count,
            size: args.size,
            speed: args.speed,
            steps: args.steps,
            fps_data: [0.0; 4],
            adv_data: [0.0; 4],
            time: 0.0,
            _pad: [0.0; 3],
        }
    }

    /// Constructs a uniform with live per-frame metric data.
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
            _pad: [0.0; 3],
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
