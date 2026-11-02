use std::collections::VecDeque;
use std::fs::OpenOptions;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

/// A single hardware vblank event as delivered by the kernel
/// (`DRM_EVENT_CRTC_SEQUENCE`, requested with `DRM_IOCTL_CRTC_QUEUE_SEQUENCE`).
///
/// Both fields are in `CLOCK_MONOTONIC` nanoseconds (the kernel default,
/// `DRM_CAP_TIMESTAMP_MONOTONIC`), matching the domain used by
/// `clock_monotonic_ns()` in `renderer.rs` and by `wp_presentation` timestamps,
/// so cross-layer subtractions are valid without anchor arithmetic.
#[derive(Debug, Clone, Copy)]
pub struct FlipRecord {
    /// Hardware vblank sequence number reported by the kernel.
    ///
    /// Not consumed by the render loop today but retained in the record so callers
    /// can detect missed vblanks by checking for non-consecutive sequence numbers.
    #[allow(dead_code)]
    pub sequence: u64,
    /// `CLOCK_MONOTONIC` nanoseconds of the vblank edge (scanout start).
    pub flip_ns: u64,
}

/// Shared queue type, render loop holds `Arc<Mutex<FlipQueue>>`.
pub type FlipQueue = std::sync::Mutex<VecDeque<FlipRecord>>;

/// Upper bound on queued records so a render loop that stops draining cannot
/// grow the queue without limit.
const MAX_QUEUED: usize = 256;

/// Owns the epoll loop, the live DRM fd, and the producer end of the flip queue.
///
/// Spawns exactly one background thread on construction. The thread runs until
/// `FlipTracker` is dropped or until the DRM fd becomes invalid (compositor restart,
/// TTY switch), at which point it clears `healthy` and exits.
pub struct FlipTracker {
    /// Consumer-side handle for the render loop to drain `FlipRecord`s from.
    pub queue: Arc<FlipQueue>,
    /// `false` once the background thread detects the DRM fd is gone or epoll fails.
    /// The render loop should check this each frame and log a warning the first time
    /// it transitions to `false`, then fall back to `cpu_time_ms`-only metrics.
    pub healthy: Arc<AtomicBool>,
    /// Asks the background thread to exit, set by `Drop`.
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl FlipTracker {
    /// Opens a fresh DRM fd from `dev_path`, arms a vblank-sequence event on
    /// `crtc_handle` with `DRM_IOCTL_CRTC_QUEUE_SEQUENCE`, and starts the epoll
    /// event loop on a dedicated background thread.
    ///
    /// DRM events are delivered per open file description, so a freshly opened fd
    /// receives nothing until it asks for events itself. That is why this arms
    /// the CRTC explicitly and re-arms after every event. The fd is independent
    /// of the one used by `drm::query()`.
    ///
    /// # Arguments
    /// * `dev_path`    - DRM device node, e.g. `"/dev/dri/card0"`.
    /// * `crtc_handle` - Raw CRTC *object id* surfaced in [`crate::drm::DrmInfo`].
    ///
    /// # Errors
    /// Returns `None` when the device node cannot be opened, epoll cannot be
    /// created, or the kernel rejects the sequence request. The caller should
    /// degrade gracefully rather than treating this as fatal.
    pub fn new(dev_path: &str, crtc_handle: u32) -> Option<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(dev_path)
            .ok()?;
        let owned = OwnedFd::from(file);
        let raw = owned.as_raw_fd();

        set_nonblocking(raw)?;
        let epoll_fd = create_epoll(raw)?;

        // Prove the event source works before spawning anything: a rejected
        // request means this fd will never see an event.
        if let Err(e) = queue_sequence(raw, crtc_handle, 1, DRM_CRTC_SEQUENCE_RELATIVE) {
            eprintln!(
                "WARN: DRM_IOCTL_CRTC_QUEUE_SEQUENCE failed on {dev_path} (crtc {crtc_handle}): \
                 {e}; hardware vblank anchor disabled"
            );
            return None;
        }

        let queue: Arc<FlipQueue> = Arc::new(std::sync::Mutex::new(VecDeque::with_capacity(8)));
        let healthy = Arc::new(AtomicBool::new(true));
        let stop = Arc::new(AtomicBool::new(false));

        let queue_tx = Arc::clone(&queue);
        let healthy_tx = Arc::clone(&healthy);
        let stop_rx = Arc::clone(&stop);

        let thread = std::thread::Builder::new()
            .name("drm-flip-tracker".into())
            .spawn(move || {
                flip_loop(owned, epoll_fd, crtc_handle, queue_tx, healthy_tx, stop_rx);
            })
            .ok()?;

        Some(Self {
            queue,
            healthy,
            stop,
            thread: Some(thread),
        })
    }
}

impl Drop for FlipTracker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(t) = self.thread.take() {
            // The loop wakes at least every 100 ms, so this is bounded.
            let _ = t.join();
        }
    }
}

/// Core epoll event loop running on the background thread.
///
/// Blocks on `epoll_wait` with a 100 ms timeout so the `stop` flag is
/// checked promptly. After every delivered event it re-arms the next vblank,
/// because `DRM_IOCTL_CRTC_QUEUE_SEQUENCE` requests are one-shot.
fn flip_loop(
    drm_file: OwnedFd,
    epoll_fd: OwnedFd,
    crtc_handle: u32,
    queue: Arc<FlipQueue>,
    healthy: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
) {
    let drm_raw = drm_file.as_raw_fd();
    let epoll_raw = epoll_fd.as_raw_fd();

    // `epoll_event` must be zero-initialised, libc repr is not `Default`.
    let mut events = [unsafe { std::mem::zeroed::<libc::epoll_event>() }; 1];

    while !stop.load(Ordering::Acquire) {
        // SAFETY: `epoll_raw` is valid for the duration of the loop, `events`
        // is a correctly-sized mutable slice.
        let n = unsafe { libc::epoll_wait(epoll_raw, events.as_mut_ptr(), 1, 100) };

        if n < 0 {
            let err = std::io::Error::last_os_error();
            // EINTR is benign (signal delivered mid-wait), anything else is fatal.
            if err.raw_os_error() == Some(libc::EINTR) {
                continue;
            }
            healthy.store(false, Ordering::Release);
            return;
        }

        if n == 0 {
            continue;
        }

        match read_drm_events(drm_raw) {
            Ok(records) => {
                let Some(last) = records.last().copied() else {
                    continue;
                };
                // Re-arm for the vblank after the one just delivered. NEXT_ON_MISS
                // makes a late re-arm fire on the next vblank instead of never.
                if queue_sequence(
                    drm_raw,
                    crtc_handle,
                    last.sequence + 1,
                    DRM_CRTC_SEQUENCE_NEXT_ON_MISS,
                )
                .is_err()
                {
                    healthy.store(false, Ordering::Release);
                    return;
                }
                if let Ok(mut q) = queue.lock() {
                    q.extend(records);
                    while q.len() > MAX_QUEUED {
                        q.pop_front();
                    }
                }
            }
            Err(e) => {
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                ) {
                    continue;
                }
                healthy.store(false, Ordering::Release);
                return;
            }
        }
    }
}

// ── Kernel ABI (include/uapi/drm/drm.h) ──────────────────────────────────────

/// `struct drm_crtc_queue_sequence`.
#[repr(C)]
struct DrmCrtcQueueSequence {
    crtc_id: u32,
    flags: u32,
    sequence: u64,
    user_data: u64,
}

/// `_IOWR('d', nr, size)` for the common Linux ioctl encoding (read|write = 3).
const fn drm_iowr(nr: u32, size: usize) -> libc::c_ulong {
    ((3u32 << 30) | ((size as u32) << 16) | ((b'd' as u32) << 8) | nr) as libc::c_ulong
}

const DRM_IOCTL_CRTC_QUEUE_SEQUENCE: libc::c_ulong =
    drm_iowr(0x3c, std::mem::size_of::<DrmCrtcQueueSequence>());

/// `sequence` is relative to the current vblank counter.
const DRM_CRTC_SEQUENCE_RELATIVE: u32 = 0x1;
/// If the requested sequence already passed, fire on the next vblank.
const DRM_CRTC_SEQUENCE_NEXT_ON_MISS: u32 = 0x2;

/// `struct drm_event_crtc_sequence`, delivered for `DRM_EVENT_CRTC_SEQUENCE`:
///
/// ```text
/// offset  0: type      (u32)   = 0x03
/// offset  4: length    (u32)   includes header
/// offset  8: user_data (u64)
/// offset 16: time_ns   (s64)   CLOCK_MONOTONIC by default
/// offset 24: sequence  (u64)
/// ```
const DRM_EVENT_CRTC_SEQUENCE: u32 = 0x03;
const DRM_EVENT_HEADER_SIZE: usize = 8;
/// Size of a well-formed `drm_event_crtc_sequence`.
const DRM_SEQUENCE_EVENT_SIZE: usize = 32;

/// Asks the kernel to deliver one `DRM_EVENT_CRTC_SEQUENCE` on `fd`.
fn queue_sequence(fd: libc::c_int, crtc_id: u32, sequence: u64, flags: u32) -> std::io::Result<()> {
    let mut req = DrmCrtcQueueSequence {
        crtc_id,
        flags,
        sequence,
        user_data: 0,
    };
    loop {
        // SAFETY: `req` is a correctly laid-out, live `drm_crtc_queue_sequence`
        // and `fd` is an open DRM fd.
        let ret = unsafe { libc::ioctl(fd, DRM_IOCTL_CRTC_QUEUE_SEQUENCE as _, &mut req) };
        if ret >= 0 {
            return Ok(());
        }
        let err = std::io::Error::last_os_error();
        if err.raw_os_error() != Some(libc::EINTR) {
            return Err(err);
        }
    }
}

/// Reads all pending DRM events from `drm_fd` in one `read(2)` call.
///
/// Returns one [`FlipRecord`] per `DRM_EVENT_CRTC_SEQUENCE` event in the
/// buffer. Unknown event types are skipped.
fn read_drm_events(drm_fd: libc::c_int) -> std::io::Result<Vec<FlipRecord>> {
    // 4 KiB covers several back-to-back events without a second syscall.
    let mut buf = [0u8; 4096];

    // SAFETY: `buf` is valid writable memory, `drm_fd` is the live DRM fd.
    let n = unsafe { libc::read(drm_fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len()) };

    if n < 0 {
        return Err(std::io::Error::last_os_error());
    }
    if n == 0 {
        return Err(std::io::ErrorKind::UnexpectedEof.into());
    }

    Ok(read_drm_events_from_buf(&buf[..n as usize]))
}

/// Parses all `DRM_EVENT_CRTC_SEQUENCE` records from a raw byte slice.
///
/// Extracted from [`read_drm_events`] so the ABI parsing logic can be exercised
/// in unit tests without a live DRM file descriptor. Fields are native-endian,
/// as written by the kernel.
pub(crate) fn read_drm_events_from_buf(data: &[u8]) -> Vec<FlipRecord> {
    let mut records = Vec::new();
    let mut offset = 0usize;

    while offset + DRM_EVENT_HEADER_SIZE <= data.len() {
        let event_type = u32::from_ne_bytes(data[offset..offset + 4].try_into().unwrap());
        let event_len =
            u32::from_ne_bytes(data[offset + 4..offset + 8].try_into().unwrap()) as usize;

        if event_len < DRM_EVENT_HEADER_SIZE || offset + event_len > data.len() {
            break;
        }

        if event_type == DRM_EVENT_CRTC_SEQUENCE && event_len >= DRM_SEQUENCE_EVENT_SIZE {
            let ev = &data[offset..offset + event_len];
            let time_ns = i64::from_ne_bytes(ev[16..24].try_into().unwrap());
            let sequence = u64::from_ne_bytes(ev[24..32].try_into().unwrap());

            if time_ns > 0 {
                records.push(FlipRecord {
                    sequence,
                    flip_ns: time_ns as u64,
                });
            }
        }

        offset += event_len;
    }

    records
}

/// Puts `fd` in non-blocking mode so a spurious epoll wakeup can never hang `read`.
fn set_nonblocking(fd: libc::c_int) -> Option<()> {
    // SAFETY: `fd` is a valid open descriptor owned by the caller.
    unsafe {
        let flags = libc::fcntl(fd, libc::F_GETFL);
        if flags < 0 || libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) < 0 {
            return None;
        }
    }
    Some(())
}

/// Creates an `epoll` instance and registers `drm_fd` for `EPOLLIN` events.
///
/// Returns `None` on any syscall failure, the caller should degrade gracefully.
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
