use std::sync::{
    Arc,
    atomic::{AtomicU8, Ordering},
};

/// Number of timestamp slots: render pass begin + end.
const QUERY_COUNT: u32 = 2;

/// Slot written at the start of the render pass.
pub const SLOT_BEGIN: u32 = 0;
/// Slot written at the end of the render pass.
pub const SLOT_END: u32 = 1;

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

/// Hardware GPU execution timer backed by a `wgpu::QuerySet`.
///
/// Measures true GPU render-pass duration using two `TIMESTAMP` queries, one
/// written at render pass begin, one at render pass end, resolved into a
/// `QUERY_RESOLVE | COPY_SRC` buffer and async-mapped back to the CPU.
///
/// Because `map_async` is non-blocking the resolved value lags one frame:
/// timestamps written in frame N are available when [`GpuTimer::poll`] is called
/// at the top of frame N+1.  This one-frame lag is unavoidable without a GPU
/// stall; the caller should treat `last_gpu_time_ms` as "previous frame's GPU
/// cost", sufficient to drive all diagnostic decisions.
///
/// When `TIMESTAMP_QUERY` is not available on the adapter every method is a
/// zero-cost no-op and [`GpuTimer::last_gpu_time_ms`] always returns `None`.
///
/// # Frame lifecycle
/// ```text
/// frame N:
///   gpu_timer.poll()                // if Mapped: read data + unmap → Idle
///   encoder = device.create_command_encoder(...)
///   gpu_timer.resolve(&mut encoder) // copy frame N-1 queries → readback buf
///   { render pass with timestamp_writes: gpu_timer.timestamp_writes() }
///   queue.submit([encoder.finish()])
///   gpu_timer.arm_readback()        // Idle → Pending; callback sets Mapped when ready
///
/// frame N+1:
///   gpu_timer.poll()                // Mapped → read → Idle; last_gpu_time_ms updated
/// ```
pub struct GpuTimer {
    /// `None` when `TIMESTAMP_QUERY` is not supported by the device.
    inner: Option<GpuTimerInner>,
}

struct GpuTimerInner {
    query_set: wgpu::QuerySet,
    /// GPU-side resolve destination (`QUERY_RESOLVE | COPY_SRC`).
    resolve_buf: wgpu::Buffer,
    /// CPU-mappable staging buffer (`MAP_READ | COPY_DST`).
    readback_buf: wgpu::Buffer,
    /// Nanoseconds per GPU timestamp tick from [`wgpu::Queue::get_timestamp_period`].
    timestamp_period_ns: f32,
    /// Atomic map-state machine: `STATE_IDLE / STATE_PENDING / STATE_MAPPED`.
    /// Written by the `map_async` callback; read by [`GpuTimer::poll`] and
    /// [`GpuTimer::arm_readback`].
    state: Arc<AtomicU8>,
    /// Cached result exposed to callers; updated once per [`GpuTimer::poll`] call.
    pub last_gpu_time_ms: Option<f32>,
}

impl GpuTimer {
    /// Creates a `GpuTimer` for `device`/`queue`.
    ///
    /// Returns a fully functional timer when `TIMESTAMP_QUERY` is present, or a
    /// disabled no-op instance when it is not. Check [`GpuTimer::is_available`]
    /// once at startup and log accordingly.
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        if !device.features().contains(wgpu::Features::TIMESTAMP_QUERY) {
            return Self { inner: None };
        }

        let timestamp_period_ns = queue.get_timestamp_period();
        let buf_size = (QUERY_COUNT as u64) * std::mem::size_of::<u64>() as u64;

        let query_set = device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("gpu_timer_queries"),
            ty: wgpu::QueryType::Timestamp,
            count: QUERY_COUNT,
        });

        let resolve_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gpu_timer_resolve"),
            size: buf_size,
            usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let readback_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gpu_timer_readback"),
            size: buf_size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            inner: Some(GpuTimerInner {
                query_set,
                resolve_buf,
                readback_buf,
                timestamp_period_ns,
                state: Arc::new(AtomicU8::new(STATE_IDLE)),
                last_gpu_time_ms: None,
            }),
        }
    }

    /// Returns `true` when the underlying device supports `TIMESTAMP_QUERY`.
    #[inline]
    pub fn is_available(&self) -> bool {
        self.inner.is_some()
    }

    /// Returns the [`wgpu::RenderPassTimestampWrites`] descriptor to attach to
    /// the render pass, or `None` when timestamps are unavailable.
    ///
    /// Pass the returned value directly into
    /// [`wgpu::RenderPassDescriptor::timestamp_writes`].
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

    /// Encodes the resolve of the *previous* frame's query results into the
    /// readback staging buffer.
    ///
    /// Must be called before the render pass that will overwrite the query set,
    /// using the same `encoder` that is submitted in this frame's `queue.submit`.
    /// The GPU then sees `[resolve N-1] → [render pass N]` in order.
    ///
    /// Skips the copy when the readback buffer is `Pending` or `Mapped` (i.e. the
    /// previous frame's `map_async` has not been consumed by `poll()` yet). This
    /// means at most one frame of GPU timing data is silently dropped during a slow
    /// frame, preferable to submitting a `COPY_DST` command against a mapped buffer,
    /// which is a hard Vulkan validation error.
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

    /// Arms the async map for the resolve that was just submitted.
    ///
    /// Must be called *after* `queue.submit` and only when the buffer is idle
    /// (i.e. after [`GpuTimer::poll`] has already consumed the previous mapping).
    /// The `map_async` callback stores only an `AtomicU8` state transition,
    /// no buffer access happens inside it, satisfying the `'static` bound.
    pub fn arm_readback(&mut self) {
        let Some(inner) = self.inner.as_mut() else {
            return;
        };

        // Only arm when idle; if still Pending or Mapped from a slow frame,
        // skip this arm, poll() will drain it next frame.
        if inner.state.load(Ordering::Relaxed) != STATE_IDLE {
            return;
        }

        inner.state.store(STATE_PENDING, Ordering::Relaxed);
        let state = Arc::clone(&inner.state);
        inner
            .readback_buf
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                let next = if result.is_ok() {
                    STATE_MAPPED
                } else {
                    STATE_IDLE
                };
                state.store(next, Ordering::Relaxed);
            });
    }

    /// Reads and caches the GPU time when the readback buffer is mapped, then
    /// unmaps it and returns the state to `Idle`.
    ///
    /// Call once at the top of each frame, before [`GpuTimer::resolve`]. Only
    /// touches the buffer when the state machine is in the `Mapped` state; a
    /// `Pending` or `Idle` buffer is left untouched.
    pub fn poll(&mut self) {
        let Some(inner) = self.inner.as_mut() else {
            return;
        };

        if inner.state.load(Ordering::Relaxed) != STATE_MAPPED {
            return;
        }

        let gpu_ms = {
            let view = inner.readback_buf.slice(..).get_mapped_range();
            // SAFETY: buffer was created as exactly 2 × u64; layout is guaranteed.
            let ts: &[u64] = bytemuck::cast_slice(&view);
            (ts.len() >= 2 && ts[1] >= ts[0])
                .then(|| (ts[1] - ts[0]) as f32 * inner.timestamp_period_ns / 1_000_000.0)
        };
        // `view` dropped here; unmap() must not be called while any view is alive.
        inner.readback_buf.unmap();
        inner.state.store(STATE_IDLE, Ordering::Relaxed);

        if let Some(t) = gpu_ms {
            inner.last_gpu_time_ms = Some(t);
        }
    }

    /// GPU frame time from the previous frame's resolved timestamp pair (milliseconds).
    ///
    /// Returns `None` until at least one frame has been resolved, or permanently
    /// when `TIMESTAMP_QUERY` is unavailable.
    #[inline]
    pub fn last_gpu_time_ms(&self) -> Option<f32> {
        self.inner.as_ref().and_then(|i| i.last_gpu_time_ms)
    }
}
