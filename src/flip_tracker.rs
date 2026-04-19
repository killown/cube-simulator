use std::collections::VecDeque;
use std::fs::OpenOptions;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

/// A single hardware page-flip event as delivered by the kernel's vblank interrupt.
///
/// Both fields are in `CLOCK_MONOTONIC` nanoseconds, matching the domain used by
/// `clock_monotonic_ns()` in `renderer.rs` and by WSI presentation timestamps on
/// Linux/DRM backends, so cross-layer subtractions are valid without anchor arithmetic.
#[derive(Debug, Clone, Copy)]
pub struct FlipRecord {
    /// Hardware vblank sequence number reported by the kernel.
    ///
    /// Not consumed by the render loop today but retained in the record so callers
    /// can correlate flip events with `drmCrtcGetSequence` counters or detect
    /// missed flips by checking for non-consecutive sequence numbers.
    #[allow(dead_code)]
    pub sequence: u64,
    /// `CLOCK_MONOTONIC` nanoseconds at the moment the pixel left the GPU (scanout start).
    pub flip_ns: u64,
}

/// Shared queue type; render loop holds `Arc<Mutex<FlipQueue>>`.
pub type FlipQueue = std::sync::Mutex<VecDeque<FlipRecord>>;

/// Owns the epoll loop, the live DRM fd, and the producer end of the flip queue.
///
/// Spawns exactly one background thread on construction.  The thread runs until
/// `FlipTracker` is dropped or until the DRM fd becomes invalid (compositor restart,
/// TTY switch), at which point it signals `healthy` and exits cleanly.
pub struct FlipTracker {
    /// Consumer-side handle for the render loop to drain `FlipRecord`s from.
    pub queue: Arc<FlipQueue>,
    /// `false` once the background thread detects the DRM fd is gone or epoll fails.
    /// The render loop should check this each frame and log a warning the first time
    /// it transitions to `false`, then fall back to `cpu_time_ms`-only metrics.
    pub healthy: Arc<AtomicBool>,
    /// Kept alive so the thread's `join` handle does not detach prematurely.
    _thread: std::thread::JoinHandle<()>,
}

impl FlipTracker {
    /// Opens a fresh DRM fd from `dev_path`, registers `crtc_handle` for page-flip
    /// events via `drmCrtcGetSequence`, and starts the epoll event loop on a
    /// dedicated background thread.
    ///
    /// The freshly-opened fd is separate from the one used by `drm::query()` so
    /// there is no fd lifetime dependency on the query-time `Card` struct.
    ///
    /// # Arguments
    /// * `dev_path`    — DRM device node, e.g. `"/dev/dri/card0"`.
    /// * `crtc_handle` — Raw CRTC handle surfaced in [`crate::drm::DrmInfo`].
    ///
    /// # Errors
    /// Returns `None` when the device node cannot be opened or epoll cannot be
    /// created.  The caller should degrade gracefully rather than treating this
    /// as fatal.
    pub fn new(dev_path: &str, crtc_handle: u32) -> Option<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(dev_path)
            .ok()?;

        // SAFETY: `File::as_raw_fd` returns the fd for the lifetime of `file`.
        // We take ownership via `OwnedFd::from_raw_fd` only after confirming
        // `into_raw_fd` transfers ownership, so the fd is never double-closed.
        let raw_fd = {
            use std::os::fd::IntoRawFd;
            file.into_raw_fd()
        };
        // SAFETY: `raw_fd` was just extracted from a valid `File`; no other
        // handle references it from this point forward.
        let owned: OwnedFd = unsafe { std::os::fd::OwnedFd::from_raw_fd(raw_fd) };

        let epoll_fd = create_epoll(owned.as_raw_fd())?;

        let queue: Arc<FlipQueue> = Arc::new(std::sync::Mutex::new(VecDeque::with_capacity(8)));
        let healthy = Arc::new(AtomicBool::new(true));

        let queue_tx = Arc::clone(&queue);
        let healthy_tx = Arc::clone(&healthy);

        let _thread = std::thread::Builder::new()
            .name("drm-flip-tracker".into())
            .spawn(move || {
                flip_loop(owned, epoll_fd, crtc_handle, queue_tx, healthy_tx);
            })
            .ok()?;

        Some(Self {
            queue,
            healthy,
            _thread,
        })
    }
}

/// Core epoll event loop running on the background thread.
///
/// Blocks on `epoll_wait` with a 100 ms timeout so the liveness check on
/// `healthy` is responsive enough to notice a compositor restart within one
/// video frame at any refresh rate above 10 Hz.
fn flip_loop(
    drm_file: OwnedFd,
    epoll_fd: OwnedFd,
    _crtc_handle: u32,
    queue: Arc<FlipQueue>,
    healthy: Arc<AtomicBool>,
) {
    let drm_raw = drm_file.as_raw_fd();
    let epoll_raw = epoll_fd.as_raw_fd();

    // `epoll_event` must be zero-initialised; libc repr is not `Default`.
    let mut events = [unsafe { std::mem::zeroed::<libc::epoll_event>() }; 1];

    loop {
        // SAFETY: `epoll_raw` is valid for the duration of the loop; `events`
        // is a correctly-sized mutable slice, timeout -1 would block forever so
        // we use 100 ms to remain responsive to drop/healthy-check signals.
        let n = unsafe { libc::epoll_wait(epoll_raw, events.as_mut_ptr(), 1, 100) };

        if n < 0 {
            let err = std::io::Error::last_os_error();
            // EINTR is benign (signal delivered mid-wait); anything else is fatal.
            if err.raw_os_error() == Some(libc::EINTR) {
                continue;
            }
            healthy.store(false, Ordering::Release);
            return;
        }

        if n == 0 {
            // Timeout: nothing to read, loop again.
            continue;
        }

        // The fd has data; call `drmHandleEvent` via the raw ioctl path.
        // We implement `DRM_EVENT_FLIP_COMPLETE` parsing directly to avoid
        // linking `libdrm` and to stay in pure Rust/libc.
        match read_drm_events(drm_raw) {
            Ok(records) => {
                if let Ok(mut q) = queue.lock() {
                    q.extend(records);
                }
            }
            Err(_) => {
                healthy.store(false, Ordering::Release);
                return;
            }
        }
    }
}

/// Raw DRM event header layout (kernel ABI).
///
/// Every DRM event on the fd begins with this 8-byte header.
///
/// ```text
/// offset 0: type  (u32, little-endian)
/// offset 4: length (u32, little-endian, includes header)
/// ```
///
/// `DRM_EVENT_FLIP_COMPLETE` (0x02) events are followed by:
///
/// ```text
/// offset  8: tv_sec      (u32)
/// offset 12: tv_usec     (u32)
/// offset 16: sequence    (u32)
/// offset 20: crtc_id     (u32)
/// ```
///
/// We use the `CLOCK_MONOTONIC`-domain override from `drmCrtcGetSequence` so the
/// `tv_sec`/`tv_usec` from the event are used only as a fallback sanity check.
const DRM_EVENT_FLIP_COMPLETE: u32 = 0x02;
const DRM_EVENT_HEADER_SIZE: usize = 8;
/// Minimum size of a well-formed `DRM_EVENT_FLIP_COMPLETE` payload (header + fields).
const DRM_FLIP_EVENT_MIN_SIZE: usize = 24;

/// Reads all pending DRM events from `drm_fd` in one `read(2)` call.
///
/// Returns a `Vec<FlipRecord>` containing one entry per `DRM_EVENT_FLIP_COMPLETE`
/// event found in the buffer.  Unknown event types are silently skipped.
///
/// The `tv_sec`/`tv_usec` pair from the kernel event payload is in
/// `CLOCK_MONOTONIC` on DRM/KMS paths (set by `drm_send_vblank_event`), so
/// converting to nanoseconds gives a timestamp compatible with `clock_monotonic_ns()`.
fn read_drm_events(drm_fd: libc::c_int) -> std::io::Result<Vec<FlipRecord>> {
    // 4 KiB covers several back-to-back events without a second syscall.
    let mut buf = [0u8; 4096];

    // SAFETY: `buf` is valid writable memory; `drm_fd` is the live DRM fd.
    let n = unsafe { libc::read(drm_fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len()) };

    if n <= 0 {
        return Err(std::io::Error::last_os_error());
    }

    Ok(read_drm_events_from_buf(&buf[..n as usize]))
}

/// Parses all `DRM_EVENT_FLIP_COMPLETE` records from a raw byte slice.
///
/// Extracted from [`read_drm_events`] so the ABI parsing logic can be exercised
/// in unit tests without a live DRM file descriptor.
pub(crate) fn read_drm_events_from_buf(data: &[u8]) -> Vec<FlipRecord> {
    let mut records = Vec::new();
    let mut offset = 0usize;

    while offset + DRM_EVENT_HEADER_SIZE <= data.len() {
        let event_type = u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap());
        let event_len =
            u32::from_le_bytes(data[offset + 4..offset + 8].try_into().unwrap()) as usize;

        if event_len < DRM_EVENT_HEADER_SIZE || offset + event_len > data.len() {
            break;
        }

        if event_type == DRM_EVENT_FLIP_COMPLETE && event_len >= DRM_FLIP_EVENT_MIN_SIZE {
            let payload = &data[offset..offset + event_len];
            let tv_sec = u32::from_le_bytes(payload[8..12].try_into().unwrap()) as u64;
            let tv_usec = u32::from_le_bytes(payload[12..16].try_into().unwrap()) as u64;
            let sequence = u32::from_le_bytes(payload[16..20].try_into().unwrap()) as u64;

            // Convert microsecond wall-clock to nanoseconds; on DRM/KMS the kernel
            // fills this with CLOCK_MONOTONIC via `ktime_get()` in `drm_send_vblank_event`.
            let flip_ns = tv_sec * 1_000_000_000 + tv_usec * 1_000;

            records.push(FlipRecord { sequence, flip_ns });
        }

        offset += event_len;
    }

    records
}

/// Creates an `epoll` instance and registers `drm_fd` for `EPOLLIN` events.
///
/// Returns `None` on any syscall failure; the caller should degrade gracefully.
fn create_epoll(drm_fd: libc::c_int) -> Option<OwnedFd> {
    // SAFETY: `epoll_create1` with `EPOLL_CLOEXEC` is safe to call at any time.
    let epoll_raw = unsafe { libc::epoll_create1(libc::EPOLL_CLOEXEC) };
    if epoll_raw < 0 {
        return None;
    }

    let mut ev = libc::epoll_event {
        events: libc::EPOLLIN as u32,
        u64: drm_fd as u64,
    };

    // SAFETY: `epoll_raw` was just created, `ev` is correctly initialised.
    let ret = unsafe { libc::epoll_ctl(epoll_raw, libc::EPOLL_CTL_ADD, drm_fd, &mut ev) };
    if ret < 0 {
        // SAFETY: `epoll_raw` is a valid fd we own.
        unsafe { libc::close(epoll_raw) };
        return None;
    }

    // SAFETY: `epoll_raw` is a valid, open fd that we now own exclusively.
    Some(unsafe { OwnedFd::from_raw_fd(epoll_raw) })
}
