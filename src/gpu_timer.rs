use std::sync::{
    Arc,
    atomic::{AtomicU8, Ordering},
};

/// Number of timestamp slots for the outer render-pass timer (begin + end).
const QUERY_COUNT: u32 = 2;

/// Slot written at the start of the render pass.
pub const SLOT_BEGIN: u32 = 0;
/// Slot written at the end of the render pass.
pub const SLOT_END: u32 = 1;

/// Number of timestamp slots for the micro-stutter diagnostic query set.
///
/// Layout:
/// ```text
/// 0  SUBMIT_PRE   — written by encoder.write_timestamp() before the render pass.
///                   Requires TIMESTAMP_QUERY_INSIDE_ENCODERS.
///                   Gap to PASS_BEGIN = driver command-buffer submission latency.
/// 1  PASS_BEGIN   — first GPU instruction in the render pass (via timestamp_writes).
///                   Requires TIMESTAMP_QUERY_INSIDE_PASSES.
/// 2  PASS_END     — last GPU instruction in the render pass (via timestamp_writes).
///                   Requires TIMESTAMP_QUERY_INSIDE_PASSES.
/// 3  RESOLVE_END  — written by encoder.write_timestamp() after the resolve copy.
///                   Requires TIMESTAMP_QUERY_INSIDE_ENCODERS.
///                   Gap to PASS_END = GPU DMA + PCIe copy overhead.
/// ```
///
/// The gaps between slots expose three distinct latency sources:
///
/// | Gap                          | Meaning                                       |
/// |------------------------------|-----------------------------------------------|
/// | `PASS_BEGIN − SUBMIT_PRE`    | Driver stall: command-buffer submission lag.  |
/// | `PASS_END   − PASS_BEGIN`    | True shader execution time.                   |
/// | `RESOLVE_END − PASS_END`     | DMA/PCIe resolve overhead; TTM eviction spike.|
const MICRO_QUERY_COUNT: u32 = 4;

pub const MICRO_SLOT_SUBMIT_PRE: u32 = 0;
pub const MICRO_SLOT_PASS_BEGIN: u32 = 1;
pub const MICRO_SLOT_PASS_END: u32 = 2;
pub const MICRO_SLOT_RESOLVE_END: u32 = 3;

/// Decomposed GPU timing from one frame's micro-stutter diagnostic query set.
#[derive(Debug, Clone, Copy, Default)]
pub struct MicroTimings {
    /// Driver command-buffer submission latency (ms).
    /// Healthy: < 0.05 ms.  Spike = driver stall, not shader overrun.
    pub driver_overhead_ms: f32,
    /// True GPU shader execution time for the render pass (ms).
    /// Should closely match `last_gpu_time_ms` from the outer timer.
    pub shader_ms: f32,
    /// GPU DMA + PCIe copy overhead for the timestamp resolve (ms).
    /// Healthy: < 0.1 ms.  Spike = PCIe contention or TTM eviction.
    pub resolve_ms: f32,
    /// Sum of all three components (ms).
    pub total_ms: f32,
}

/// Which wgpu timestamp features are present on this device.
///
/// Tracked separately because the three features gate different call-sites:
/// - `TIMESTAMP_QUERY`                — outer render-pass timer, always needed.
/// - `TIMESTAMP_QUERY_INSIDE_PASSES`  — `PASS_BEGIN`/`PASS_END` via `timestamp_writes`.
/// - `TIMESTAMP_QUERY_INSIDE_ENCODERS`— `write_timestamp()` outside a render pass
///                                      (`SUBMIT_PRE` and `RESOLVE_END` slots).
#[derive(Debug, Clone, Copy)]
struct TimestampCaps {
    outer: bool,
    inside_passes: bool,
    inside_encoders: bool,
}

impl TimestampCaps {
    fn from_device(device: &wgpu::Device) -> Self {
        let f = device.features();
        Self {
            outer: f.contains(wgpu::Features::TIMESTAMP_QUERY),
            inside_passes: f.contains(wgpu::Features::TIMESTAMP_QUERY_INSIDE_PASSES),
            inside_encoders: f.contains(wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS),
        }
    }

    /// Full micro-stutter breakdown requires all three features.
    fn micro_full(&self) -> bool {
        self.outer && self.inside_passes && self.inside_encoders
    }
}

/// Buffer map state machine, stored atomically so the `map_async` callback
/// (potentially on another thread) can transition `Pending → Mapped`.
///
/// Transitions (all others are invalid and never occur):
/// ```text
/// Idle ──arm_readback()──► Pending
/// Pending ──callback ok──► Mapped
/// Pending ──callback err─► Idle
/// Mapped ──poll() read──► Idle   (after unmap)
/// ```
const STATE_IDLE: u8 = 0;
const STATE_PENDING: u8 = 1;
const STATE_MAPPED: u8 = 2;

/// Hardware GPU execution timer backed by two `wgpu::QuerySet` instances.
///
/// # Outer timer (render-pass begin → end)
///
/// Measures true GPU render-pass duration using two `TIMESTAMP` queries attached
/// to `RenderPassDescriptor::timestamp_writes`. Requires `TIMESTAMP_QUERY`.
///
/// # Micro-stutter diagnostic timer
///
/// A second four-slot query set that isolates *where inside the GPU pipeline*
/// time is lost when a vblank miss fires. Requires all three timestamp features:
/// `TIMESTAMP_QUERY`, `TIMESTAMP_QUERY_INSIDE_PASSES`, and
/// `TIMESTAMP_QUERY_INSIDE_ENCODERS`. Degrades silently when any are absent.
///
/// # Feature degradation matrix
///
/// | Features available                          | Behaviour                        |
/// |---------------------------------------------|----------------------------------|
/// | None                                        | All methods are no-ops.          |
/// | `TIMESTAMP_QUERY` only                      | Outer timer active; no micro.    |
/// | All three                                   | Outer + micro timers active.     |
pub struct GpuTimer {
    inner: Option<GpuTimerInner>,
}

struct GpuTimerInner {
    // ── Outer render-pass timer ──────────────────────────────────────────────
    query_set: wgpu::QuerySet,
    resolve_buf: wgpu::Buffer,
    readback_buf: wgpu::Buffer,
    state: Arc<AtomicU8>,
    pub last_gpu_time_ms: Option<f32>,

    // ── Micro-stutter diagnostic timer (optional) ────────────────────────────
    /// `None` when any of the three required timestamp features is absent.
    micro: Option<MicroTimerInner>,

    timestamp_period_ns: f32,
    caps: TimestampCaps,
}

struct MicroTimerInner {
    query_set: wgpu::QuerySet,
    resolve_buf: wgpu::Buffer,
    readback_buf: wgpu::Buffer,
    state: Arc<AtomicU8>,
    pub last_timings: Option<MicroTimings>,
}

impl GpuTimer {
    /// Creates a `GpuTimer` for `device`/`queue`.
    ///
    /// Builds the outer timer when `TIMESTAMP_QUERY` is present, and additionally
    /// builds the micro-stutter diagnostic set when all three timestamp features
    /// are available. Each path degrades independently with no effect on the other.
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let caps = TimestampCaps::from_device(device);
        if !caps.outer {
            return Self { inner: None };
        }

        let timestamp_period_ns = queue.get_timestamp_period();
        let outer_buf_size = (QUERY_COUNT as u64) * std::mem::size_of::<u64>() as u64;

        let query_set = device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("gpu_timer_queries"),
            ty: wgpu::QueryType::Timestamp,
            count: QUERY_COUNT,
        });
        let resolve_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gpu_timer_resolve"),
            size: outer_buf_size,
            usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let readback_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gpu_timer_readback"),
            size: outer_buf_size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Micro timer requires all three features, skip if any is absent.
        let micro = caps.micro_full().then(|| {
            let micro_buf_size = (MICRO_QUERY_COUNT as u64) * std::mem::size_of::<u64>() as u64;
            MicroTimerInner {
                query_set: device.create_query_set(&wgpu::QuerySetDescriptor {
                    label: Some("gpu_micro_timer_queries"),
                    ty: wgpu::QueryType::Timestamp,
                    count: MICRO_QUERY_COUNT,
                }),
                resolve_buf: device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("gpu_micro_timer_resolve"),
                    size: micro_buf_size,
                    usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                    mapped_at_creation: false,
                }),
                readback_buf: device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("gpu_micro_timer_readback"),
                    size: micro_buf_size,
                    usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }),
                state: Arc::new(AtomicU8::new(STATE_IDLE)),
                last_timings: None,
            }
        });

        Self {
            inner: Some(GpuTimerInner {
                query_set,
                resolve_buf,
                readback_buf,
                state: Arc::new(AtomicU8::new(STATE_IDLE)),
                last_gpu_time_ms: None,
                micro,
                timestamp_period_ns,
                caps,
            }),
        }
    }

    /// Returns `true` when the underlying device supports `TIMESTAMP_QUERY`.
    #[inline]
    pub fn is_available(&self) -> bool {
        self.inner.is_some()
    }

    /// Returns `true` when all three timestamp features are present and the
    /// micro-stutter diagnostic timer is active.
    #[inline]
    pub fn is_micro_available(&self) -> bool {
        self.inner.as_ref().map_or(false, |i| i.micro.is_some())
    }

    /// Returns the outer [`wgpu::RenderPassTimestampWrites`] descriptor.
    #[inline]
    pub fn timestamp_writes(&self) -> Option<wgpu::RenderPassTimestampWrites<'_>> {
        self.inner
            .as_ref()
            .map(|i| wgpu::RenderPassTimestampWrites {
                query_set: &i.query_set,
                beginning_of_pass_write_index: Some(SLOT_BEGIN),
                end_of_pass_write_index: Some(SLOT_END),
            })
    }

    /// Writes `MICRO_SLOT_SUBMIT_PRE` via `encoder.write_timestamp()`.
    ///
    /// Call after [`GpuTimer::resolve`] and before `begin_render_pass`.
    /// Requires `TIMESTAMP_QUERY_INSIDE_ENCODERS`, silently skipped otherwise.
    pub fn write_pre_pass(&self, encoder: &mut wgpu::CommandEncoder) {
        let Some(inner) = self.inner.as_ref() else {
            return;
        };
        if !inner.caps.inside_encoders {
            return;
        }
        let Some(micro) = inner.micro.as_ref() else {
            return;
        };
        if micro.state.load(Ordering::Relaxed) != STATE_IDLE {
            return;
        }
        encoder.write_timestamp(&micro.query_set, MICRO_SLOT_SUBMIT_PRE);
    }

    /// Writes `MICRO_SLOT_RESOLVE_END` and resolves all micro-timer slots.
    ///
    /// Call after the render pass closes and before `queue.submit`.
    /// Requires `TIMESTAMP_QUERY_INSIDE_ENCODERS`; silently skipped otherwise.
    pub fn write_post_resolve(&self, encoder: &mut wgpu::CommandEncoder) {
        let Some(inner) = self.inner.as_ref() else {
            return;
        };
        if !inner.caps.inside_encoders {
            return;
        }
        let Some(micro) = inner.micro.as_ref() else {
            return;
        };
        if micro.state.load(Ordering::Relaxed) != STATE_IDLE {
            return;
        }

        encoder.write_timestamp(&micro.query_set, MICRO_SLOT_RESOLVE_END);

        let micro_buf_size = (MICRO_QUERY_COUNT as u64) * std::mem::size_of::<u64>() as u64;
        encoder.resolve_query_set(
            &micro.query_set,
            0..MICRO_QUERY_COUNT,
            &micro.resolve_buf,
            0,
        );
        encoder.copy_buffer_to_buffer(
            &micro.resolve_buf,
            0,
            &micro.readback_buf,
            0,
            micro_buf_size,
        );
    }

    /// Encodes the resolve of the *previous* frame's outer query results.
    ///
    /// Must be called before the render pass that will overwrite the query set.
    /// Skips when the readback buffer is still `Pending` or `Mapped`.
    pub fn resolve(&self, encoder: &mut wgpu::CommandEncoder) {
        let Some(inner) = self.inner.as_ref() else {
            return;
        };
        if inner.state.load(Ordering::Relaxed) != STATE_IDLE {
            return;
        }
        let buf_size = (QUERY_COUNT as u64) * std::mem::size_of::<u64>() as u64;
        encoder.resolve_query_set(&inner.query_set, 0..QUERY_COUNT, &inner.resolve_buf, 0);
        encoder.copy_buffer_to_buffer(&inner.resolve_buf, 0, &inner.readback_buf, 0, buf_size);
    }

    /// Arms the async map for both the outer and micro readback buffers.
    ///
    /// Must be called *after* `queue.submit`. Each buffer arms independently;
    /// a buffer already `Pending` or `Mapped` from a slow frame is skipped.
    pub fn arm_readback(&mut self) {
        let Some(inner) = self.inner.as_mut() else {
            return;
        };

        if inner.state.load(Ordering::Relaxed) == STATE_IDLE {
            inner.state.store(STATE_PENDING, Ordering::Relaxed);
            let state = Arc::clone(&inner.state);
            inner
                .readback_buf
                .slice(..)
                .map_async(wgpu::MapMode::Read, move |result| {
                    state.store(
                        if result.is_ok() {
                            STATE_MAPPED
                        } else {
                            STATE_IDLE
                        },
                        Ordering::Relaxed,
                    );
                });
        }

        if let Some(micro) = inner
            .micro
            .as_mut()
            .filter(|m| m.state.load(Ordering::Relaxed) == STATE_IDLE)
        {
            micro.state.store(STATE_PENDING, Ordering::Relaxed);
            let state = Arc::clone(&micro.state);
            micro
                .readback_buf
                .slice(..)
                .map_async(wgpu::MapMode::Read, move |res| {
                    state.store(
                        if res.is_ok() {
                            STATE_MAPPED
                        } else {
                            STATE_IDLE
                        },
                        Ordering::Relaxed,
                    );
                });
        }
    }

    /// Reads and caches timing data from both query sets when mapped.
    ///
    /// Call once at the top of each frame, before [`GpuTimer::resolve`].
    pub fn poll(&mut self) {
        let Some(inner) = self.inner.as_mut() else {
            return;
        };
        let period = inner.timestamp_period_ns;

        // ── Outer timer ──────────────────────────────────────────────────────
        if inner.state.load(Ordering::Relaxed) == STATE_MAPPED {
            let gpu_ms = {
                let view = inner.readback_buf.slice(..).get_mapped_range();
                // SAFETY: buffer was created as exactly 2 × u64; layout is guaranteed.
                let ts: &[u64] = bytemuck::cast_slice(&view);
                (ts.len() >= 2 && ts[1] >= ts[0])
                    .then(|| (ts[1] - ts[0]) as f32 * period / 1_000_000.0)
            };
            inner.readback_buf.unmap();
            inner.state.store(STATE_IDLE, Ordering::Relaxed);
            if let Some(t) = gpu_ms {
                inner.last_gpu_time_ms = Some(t);
            }
        }

        // ── Micro timer ──────────────────────────────────────────────────────
        if let Some(micro) = inner
            .micro
            .as_mut()
            .filter(|m| m.state.load(Ordering::Relaxed) == STATE_MAPPED)
        {
            let timings = {
                let view = micro.readback_buf.slice(..).get_mapped_range();
                // SAFETY: buffer was created as exactly 4 × u64.
                let ts: &[u64] = bytemuck::cast_slice(&view);
                let valid = ts.len() >= 4
                    && ts[MICRO_SLOT_PASS_BEGIN as usize] >= ts[MICRO_SLOT_SUBMIT_PRE as usize]
                    && ts[MICRO_SLOT_PASS_END as usize] >= ts[MICRO_SLOT_PASS_BEGIN as usize]
                    && ts[MICRO_SLOT_RESOLVE_END as usize] >= ts[MICRO_SLOT_PASS_END as usize];

                valid.then(|| {
                    let to_ms = |t0: u64, t1: u64| (t1 - t0) as f32 * period / 1_000_000.0;
                    let driver = to_ms(
                        ts[MICRO_SLOT_SUBMIT_PRE as usize],
                        ts[MICRO_SLOT_PASS_BEGIN as usize],
                    );
                    let shader = to_ms(
                        ts[MICRO_SLOT_PASS_BEGIN as usize],
                        ts[MICRO_SLOT_PASS_END as usize],
                    );
                    let resolve = to_ms(
                        ts[MICRO_SLOT_PASS_END as usize],
                        ts[MICRO_SLOT_RESOLVE_END as usize],
                    );
                    MicroTimings {
                        driver_overhead_ms: driver,
                        shader_ms: shader,
                        resolve_ms: resolve,
                        total_ms: driver + shader + resolve,
                    }
                })
            };

            micro.readback_buf.unmap();
            micro.state.store(STATE_IDLE, Ordering::Relaxed);
            if let Some(t) = timings {
                micro.last_timings = Some(t);
            }
        }
    }

    /// GPU render-pass time from the previous frame (milliseconds).
    ///
    /// Returns `None` until at least one frame has been resolved, or permanently
    /// when `TIMESTAMP_QUERY` is unavailable.
    #[inline]
    pub fn last_gpu_time_ms(&self) -> Option<f32> {
        self.inner.as_ref().and_then(|i| i.last_gpu_time_ms)
    }

    /// Decomposed micro-stutter timings from the previous frame.
    ///
    /// Returns `None` until at least one frame has been resolved, or permanently
    /// when any of the three required timestamp features is unavailable.
    #[inline]
    pub fn last_micro_timings(&self) -> Option<MicroTimings> {
        self.inner
            .as_ref()
            .and_then(|i| i.micro.as_ref())
            .and_then(|m| m.last_timings)
    }
}
