//! Compositor-reported presentation feedback via the Wayland `wp_presentation`
//! protocol.
//!
//! `wp_presentation_feedback.presented` reports the time the frame actually hit
//! the screen (`CLOCK_MONOTONIC`, see `wp_presentation.clock_id`), the msc
//! sequence, the output refresh period and flags (`VSYNC`, `HW_CLOCK`,
//! `HW_COMPLETION`, `ZERO_COPY`). This is what the compositor really delivered,
//! unlike `adapter.get_presentation_timestamp()` which is just "now".
//!
//! The module attaches to winit's existing `wl_display` / `wl_surface` through
//! raw-window-handle and uses a private event queue, so winit's own dispatching
//! is untouched. A feedback object is requested immediately before every
//! `SurfaceTexture::present()`, so it rides on the `wl_surface.commit` that
//! `present()` issues.

use winit::raw_window_handle::{
    HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle,
};
use winit::window::Window;

use wayland_backend::sys::client::{Backend, ObjectId};
use wayland_client::{
    Connection, Dispatch, EventQueue, Proxy, QueueHandle, WEnum,
    globals::{GlobalListContents, registry_queue_init},
    protocol::{wl_registry::WlRegistry, wl_surface::WlSurface},
};
use wayland_protocols::wp::presentation_time::client::{
    wp_presentation::{self, WpPresentation as WpPresentationProxy},
    wp_presentation_feedback::{self, WpPresentationFeedback},
};

/// `wp_presentation_feedback.kind` bit: presentation was synchronised to vblank.
pub const WP_FLAG_VSYNC: u32 = 0x1;
/// `wp_presentation_feedback.kind` bit: the timestamp came from a hardware clock
/// (kernel page-flip/vblank time), not a clock the compositor read in software.
/// Only such timestamps are scanout times, without it the value carries the
/// compositor's own CPU scheduling jitter.
pub const WP_FLAG_HW_CLOCK: u32 = 0x2;

const CLOCK_MONOTONIC: u32 = 1;

/// One resolved feedback object.
#[derive(Debug, Clone, Copy)]
pub enum PresentEvent {
    /// Frame reached the screen.
    Presented {
        token: u64,
        /// `CLOCK_MONOTONIC` nanoseconds of actual presentation.
        present_ns: u64,
        /// Output refresh period in ns for this presentation (0 = unknown/VRR).
        refresh_ns: u32,
        #[allow(dead_code)]
        seq: u64,
        flags: u32,
    },
    /// Frame was superseded (e.g. Mailbox replace) and never shown.
    Discarded { token: u64 },
}

#[derive(Default)]
struct Inner {
    events: Vec<PresentEvent>,
    clock_id: Option<u32>,
}

pub struct WpPresentation {
    conn: Connection,
    queue: EventQueue<Inner>,
    qh: QueueHandle<Inner>,
    presentation: WpPresentationProxy,
    surface: WlSurface,
    inner: Inner,
    next_token: u64,
    /// `refresh` from the most recent `presented` event (0 = unknown/VRR).
    last_refresh_ns: Option<u32>,
}

impl WpPresentation {
    /// Returns `None` when not on Wayland, the compositor lacks `wp_presentation`,
    /// or its clock is not `CLOCK_MONOTONIC`.
    pub fn new(window: &Window) -> Option<Self> {
        let RawDisplayHandle::Wayland(d) = window.display_handle().ok()?.as_raw() else {
            return None;
        };
        let RawWindowHandle::Wayland(w) = window.window_handle().ok()?.as_raw() else {
            return None;
        };

        // SAFETY: the pointers come from winit and outlive this struct (the
        // window is held by `State`). A foreign backend never closes the display.
        let backend = unsafe { Backend::from_foreign_display(d.display.as_ptr().cast()) };
        let conn = Connection::from_backend(backend);

        let (globals, mut queue) = registry_queue_init::<Inner>(&conn).ok()?;
        let qh = queue.handle();
        let presentation: WpPresentationProxy = globals.bind(&qh, 1..=1, ()).ok()?;

        let mut inner = Inner::default();
        queue.roundtrip(&mut inner).ok()?; // receives `clock_id`
        if inner.clock_id != Some(CLOCK_MONOTONIC) {
            eprintln!(
                "WARN: wp_presentation clock_id {:?} is not CLOCK_MONOTONIC, ignoring it",
                inner.clock_id
            );
            return None;
        }

        // SAFETY: `w.surface` is winit's live `wl_surface*`.
        let id = unsafe { ObjectId::from_ptr(WlSurface::interface(), w.surface.as_ptr().cast()) }
            .ok()?;
        let surface = WlSurface::from_id(&conn, id).ok()?;

        Some(Self {
            conn,
            queue,
            qh,
            presentation,
            surface,
            inner,
            next_token: 0,
            last_refresh_ns: None,
        })
    }

    /// Must be called immediately before `present()`, returns the token that
    /// identifies this submit in the matching [`PresentEvent`].
    pub fn request_feedback(&mut self) -> u64 {
        self.next_token += 1;
        let token = self.next_token;
        let _ = self.presentation.feedback(&self.surface, &self.qh, token);
        let _ = self.conn.flush();
        token
    }

    /// Drains feedback that has arrived since the last call (non-blocking).
    pub fn poll(&mut self) -> Vec<PresentEvent> {
        let _ = self.queue.dispatch_pending(&mut self.inner);
        let events = std::mem::take(&mut self.inner.events);
        for ev in &events {
            if let PresentEvent::Presented { refresh_ns, .. } = ev {
                self.last_refresh_ns = Some(*refresh_ns);
            }
        }
        events
    }

    /// Output refresh period (ns) from the latest `presented` event.
    /// `None` until the first feedback arrives, `Some(0)` means the compositor
    /// reports an unknown or variable period (VRR).
    pub fn last_refresh_ns(&self) -> Option<u32> {
        self.last_refresh_ns
    }
}

impl Dispatch<WlRegistry, GlobalListContents> for Inner {
    fn event(
        _: &mut Self,
        _: &WlRegistry,
        _: <WlRegistry as Proxy>::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WpPresentationProxy, ()> for Inner {
    fn event(
        state: &mut Self,
        _: &WpPresentationProxy,
        event: wp_presentation::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wp_presentation::Event::ClockId { clk_id } = event {
            state.clock_id = Some(clk_id);
        }
    }
}

impl Dispatch<WpPresentationFeedback, u64> for Inner {
    fn event(
        state: &mut Self,
        _: &WpPresentationFeedback,
        event: wp_presentation_feedback::Event,
        token: &u64,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wp_presentation_feedback::Event::Presented {
                tv_sec_hi,
                tv_sec_lo,
                tv_nsec,
                refresh,
                seq_hi,
                seq_lo,
                flags,
            } => {
                let secs = ((tv_sec_hi as u64) << 32) | tv_sec_lo as u64;
                let flags = match flags {
                    WEnum::Value(k) => k.bits(),
                    WEnum::Unknown(v) => v,
                };
                state.events.push(PresentEvent::Presented {
                    token: *token,
                    present_ns: secs * 1_000_000_000 + tv_nsec as u64,
                    refresh_ns: refresh,
                    seq: ((seq_hi as u64) << 32) | seq_lo as u64,
                    flags,
                });
            }
            wp_presentation_feedback::Event::Discarded => {
                state.events.push(PresentEvent::Discarded { token: *token });
            }
            _ => {}
        }
    }
}
