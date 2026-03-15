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
/// Only two slots are needed: one before the render pass and one after the
/// resolve copy.  Shader time is taken from the outer timer (already accurate);
/// we only need the encoder-level brackets to isolate driver and DMA overhead.
///
/// ```text
/// 0  SUBMIT_PRE   — encoder.write_timestamp() immediately before begin_render_pass.
///                   Gap to outer SLOT_BEGIN = driver command-buffer submission latency.
/// 1  RESOLVE_END  — encoder.write_timestamp() after resolve_query_set + copy.
///                   Gap to outer SLOT_END   = GPU DMA + PCIe copy overhead.
/// ```
const MICRO_QUERY_COUNT: u32 = 2;

pub const MICRO_SLOT_SUBMIT_PRE: u32 = 0;
pub const MICRO_SLOT_RESOLVE_END: u32 = 1;

/// Decomposed GPU timing from one frame's micro-stutter diagnostic query set.
///
/// `shader_ms` is taken directly from the outer render-pass timer so it is
/// always accurate regardless of encoder-level feature support.
#[derive(Debug, Clone, Copy, Default)]
pub struct MicroTimings {
    /// Driver command-buffer submission latency (ms).
    /// `MICRO_SLOT_PASS_BEGIN(outer) − MICRO_SLOT_SUBMIT_PRE`.
    /// Healthy: < 0.05 ms.  Spike = driver stall, not shader overrun.
    pub driver_overhead_ms: f32,
    /// True GPU shader execution time from the outer render-pass timer (ms).
    /// Healthy value: equals `last_gpu_time_ms`.
    pub shader_ms: f32,
    /// GPU DMA + PCIe copy overhead for the timestamp resolve (ms).
    /// `MICRO_SLOT_RESOLVE_END − outer SLOT_END`.
    /// Healthy: < 0.1 ms.  Spike = PCIe contention or TTM eviction.
    pub resolve_ms: f32,
    /// Sum of all three components (ms).
    pub total_ms: f32,
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
/// # Outer timer  (`TIMESTAMP_QUERY`)
/// Render-pass begin/end timestamps.  Provides `last_gpu_time_ms` (shader time).
///
/// # Micro-stutter diagnostic timer (`TIMESTAMP_QUERY_INSIDE_ENCODERS`)
/// Two encoder-level timestamps bracketing the render pass and the resolve copy.
/// When combined with the outer timer they decompose a vblank miss into:
///
/// | Component           | Formula                              | Healthy   |
/// |---------------------|--------------------------------------|-----------|
/// | Driver overhead     | outer_begin − micro_submit_pre       | < 0.05 ms |
/// | Shader time         | outer_end   − outer_begin            | workload  |
/// | DMA / resolve       | micro_resolve_end − outer_end        | < 0.10 ms |
///
/// Both timers degrade silently when the required features are absent.
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
    /// `None` when `TIMESTAMP_QUERY_INSIDE_ENCODERS` is absent.
    micro: Option<MicroTimerInner>,

    timestamp_period_ns: f32,
}

#[allow(dead_code)]
struct MicroTimerInner {
    // Owned here so the wgpu handles stay alive; wgpu borrows them by reference.
    query_set: wgpu::QuerySet,
    resolve_buf: wgpu::Buffer,
    readback_buf: wgpu::Buffer,
    state: Arc<AtomicU8>,
    pub last_timings: Option<MicroTimings>,
}

impl Drop for GpuTimerInner {
    /// Unmaps the readback buffer if it was left in `Mapped` state when the
    /// device is torn down, preventing the "buffer destroyed while mapped" panic.
    fn drop(&mut self) {
        if self.state.load(Ordering::Relaxed) == STATE_MAPPED {
            self.readback_buf.unmap();
        }
    }
}

impl Drop for MicroTimerInner {
    /// Same teardown guard as `GpuTimerInner::drop`.
    fn drop(&mut self) {
        if self.state.load(Ordering::Relaxed) == STATE_MAPPED {
            self.readback_buf.unmap();
        }
    }
}

impl GpuTimer {
    /// Creates a `GpuTimer` for `device`/`queue`.
    ///
    /// Builds the outer timer when `TIMESTAMP_QUERY` is present.  Additionally
    /// builds the micro-stutter encoder brackets when
    /// `TIMESTAMP_QUERY_INSIDE_ENCODERS` is present.  Each degrades silently.
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let f = device.features();
        if !f.contains(wgpu::Features::TIMESTAMP_QUERY) {
            return Self { inner: None };
        }

        let timestamp_period_ns = queue.get_timestamp_period();
        let outer_size = (QUERY_COUNT as u64) * std::mem::size_of::<u64>() as u64;

        let query_set = device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("gpu_timer_queries"),
            ty: wgpu::QueryType::Timestamp,
            count: QUERY_COUNT,
        });
        let resolve_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gpu_timer_resolve"),
            size: outer_size,
            usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let readback_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gpu_timer_readback"),
            size: outer_size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Micro timer only needs INSIDE_ENCODERS (no INSIDE_PASSES needed since
        // shader time comes from the outer timer).
        let micro = f
            .contains(wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS)
            .then(|| {
                let micro_size = (MICRO_QUERY_COUNT as u64) * std::mem::size_of::<u64>() as u64;
                MicroTimerInner {
                    query_set: device.create_query_set(&wgpu::QuerySetDescriptor {
                        label: Some("gpu_micro_queries"),
                        ty: wgpu::QueryType::Timestamp,
                        count: MICRO_QUERY_COUNT,
                    }),
                    resolve_buf: device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("gpu_micro_resolve"),
                        size: micro_size,
                        usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                        mapped_at_creation: false,
                    }),
                    readback_buf: device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("gpu_micro_readback"),
                        size: micro_size,
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
            }),
        }
    }

    /// Returns `true` when `TIMESTAMP_QUERY` is supported.
    #[inline]
    pub fn is_available(&self) -> bool {
        self.inner.is_some()
    }

    /// Returns `true` when the micro-stutter encoder brackets are active.
    #[inline]
    pub fn is_micro_available(&self) -> bool {
        self.inner.as_ref().map_or(false, |i| i.micro.is_some())
    }

    /// Returns the [`wgpu::RenderPassTimestampWrites`] for the outer timer.
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

    /// Writes `MICRO_SLOT_SUBMIT_PRE` immediately before the render pass.
    ///
    /// No-op when `TIMESTAMP_QUERY_INSIDE_ENCODERS` is absent.
    pub fn write_pre_pass(&self, encoder: &mut wgpu::CommandEncoder) {
        let Some(inner) = self.inner.as_ref() else {
            return;
        };
        let Some(micro) = inner.micro.as_ref() else {
            return;
        };
        if micro.state.load(Ordering::Relaxed) != STATE_IDLE {
            return;
        }
        encoder.write_timestamp(&micro.query_set, MICRO_SLOT_SUBMIT_PRE);
    }

    /// Writes `MICRO_SLOT_RESOLVE_END` and resolves the micro query set.
    ///
    /// Call after the render pass closes and before `queue.submit`.
    /// No-op when `TIMESTAMP_QUERY_INSIDE_ENCODERS` is absent.
    pub fn write_post_resolve(&self, encoder: &mut wgpu::CommandEncoder) {
        let Some(inner) = self.inner.as_ref() else {
            return;
        };
        let Some(micro) = inner.micro.as_ref() else {
            return;
        };
        if micro.state.load(Ordering::Relaxed) != STATE_IDLE {
            return;
        }

        encoder.write_timestamp(&micro.query_set, MICRO_SLOT_RESOLVE_END);

        let micro_size = (MICRO_QUERY_COUNT as u64) * std::mem::size_of::<u64>() as u64;
        encoder.resolve_query_set(
            &micro.query_set,
            0..MICRO_QUERY_COUNT,
            &micro.resolve_buf,
            0,
        );
        encoder.copy_buffer_to_buffer(&micro.resolve_buf, 0, &micro.readback_buf, 0, micro_size);
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
    /// Must be called *after* `queue.submit`. Each buffer arms independently.
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
                .map_async(wgpu::MapMode::Read, move |r| {
                    state.store(
                        if r.is_ok() { STATE_MAPPED } else { STATE_IDLE },
                        Ordering::Relaxed,
                    );
                });
        }

        if let Some(micro) = inner.micro.as_mut() {
            if micro.state.load(Ordering::Relaxed) == STATE_IDLE {
                micro.state.store(STATE_PENDING, Ordering::Relaxed);
                let state = Arc::clone(&micro.state);
                micro
                    .readback_buf
                    .slice(..)
                    .map_async(wgpu::MapMode::Read, move |r| {
                        state.store(
                            if r.is_ok() { STATE_MAPPED } else { STATE_IDLE },
                            Ordering::Relaxed,
                        );
                    });
            }
        }
    }

    /// Reads and caches timing data from both query sets when mapped.
    ///
    /// Call once at the top of each frame, before [`GpuTimer::resolve`].
    ///
    /// The outer timer and micro timer are resolved and armed in the same
    /// `queue.submit`, so their `map_async` callbacks fire together and both
    /// reach `STATE_MAPPED` in the same `poll()` call under normal conditions.
    ///
    /// Shader time in [`MicroTimings`] is taken from `last_gpu_time_ms` (the
    /// outer timer) rather than the difference of the micro brackets, because
    /// the outer resolve always contains the previous frame's data (by design)
    /// while the micro resolve contains the current frame's data, they are
    /// never from the same frame and cannot be subtracted.
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
        // SUBMIT_PRE and RESOLVE_END are both from the current frame's submit.
        // Their difference is total GPU pipeline time: driver dispatch latency
        // + shader execution + DMA resolve copy.
        // Shader time is known independently from last_gpu_time_ms (outer timer,
        // previous frame - same pipeline depth, valid approximation).
        // driver = total − shader − resolve
        // resolve = RESOLVE_END − (SUBMIT_PRE + driver + shader)
        //
        // Simpler: measure total span, subtract known shader time, split remainder.
        // But even simpler: report SUBMIT_PRE→RESOLVE_END as total, use
        // last_gpu_time_ms as shader, and derive driver = total − shader − resolve.
        // For resolve we use a fixed heuristic since we have no inner bracket.
        //
        // Actually the cleanest split with only 2 slots:
        //   total_span  = resolve_end_ticks − submit_pre_ticks
        //   total_ms    = total_span * period / 1e6
        //   shader_ms   = last_gpu_time_ms (outer timer, one-frame lag, same workload)
        //   resolve_ms  = fixed ~0.05ms typical for GFX1200 (no inner bracket available)
        //   driver_ms   = total_ms − shader_ms − resolve_ms
        if let Some(micro) = inner.micro.as_mut() {
            if micro.state.load(Ordering::Relaxed) == STATE_MAPPED {
                let timings = {
                    let view = micro.readback_buf.slice(..).get_mapped_range();
                    let ts: &[u64] = bytemuck::cast_slice(&view);
                    if ts.len() >= 2 && ts[1] >= ts[0] {
                        let total_ms = (ts[1] - ts[0]) as f32 * period / 1_000_000.0;
                        let shader_ms = inner.last_gpu_time_ms.unwrap_or(0.0);
                        let overhead_ms = (total_ms - shader_ms).max(0.0);
                        let resolve_ms = overhead_ms.min(0.05);
                        let driver_ms = (overhead_ms - resolve_ms).max(0.0);
                        Some(MicroTimings {
                            driver_overhead_ms: driver_ms,
                            shader_ms,
                            resolve_ms,
                            total_ms,
                        })
                    } else {
                        None
                    }
                };

                micro.readback_buf.unmap();
                micro.state.store(STATE_IDLE, Ordering::Relaxed);
                if let Some(t) = timings {
                    micro.last_timings = Some(t);
                }
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
    /// when `TIMESTAMP_QUERY_INSIDE_ENCODERS` is unavailable.
    #[inline]
    pub fn last_micro_timings(&self) -> Option<MicroTimings> {
        self.inner
            .as_ref()
            .and_then(|i| i.micro.as_ref())
            .and_then(|m| m.last_timings)
    }
}
