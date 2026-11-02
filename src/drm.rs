use drm::Device;
use drm::control::{Device as ControlDevice, crtc};
use std::fs::{File, OpenOptions};
use std::os::unix::io::{AsFd, BorrowedFd};

struct Card(File);

impl AsFd for Card {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.0.as_fd()
    }
}

/// SAFETY: `Card` wraps an open `/dev/dri` node and is never cloned or sent
/// across threads. The fd remains valid for the lifetime of the struct.
impl Device for Card {}
impl ControlDevice for Card {}

/// The single mode the CRTC is currently programmed to.
#[derive(Clone)]
pub struct ActiveMode {
    pub width: u16,
    pub height: u16,
    /// Whole-Hz refresh as reported by the kernel (`drm_mode_vrefresh`, rounded).
    /// For display only, use `refresh_mhz` for any timing arithmetic.
    pub refresh_hz: u32,
    /// Exact refresh in millihertz computed from the mode timings
    /// (`clock / (htotal * vtotal)`), so 59.94 Hz stays 59940 instead of becoming 60.
    pub refresh_mhz: u32,
}

/// Exact refresh rate in millihertz, mirroring the kernel's `drm_mode_vrefresh()`
/// (interlace x2, doublescan /2, vscan divisor) without rounding to whole Hz.
/// Falls back to `vrefresh * 1000` when the timings are unusable.
fn mode_refresh_mhz(m: &drm::control::Mode) -> u32 {
    use drm::control::ModeFlags;

    let clock_khz = m.clock() as u64;
    let htotal = m.hsync().2 as u64;
    let vtotal = m.vsync().2 as u64;
    if clock_khz == 0 || htotal == 0 || vtotal == 0 {
        return m.vrefresh() * 1000;
    }

    let mut num = clock_khz * 1_000_000;
    let mut den = htotal * vtotal;
    if m.flags().contains(ModeFlags::INTERLACE) {
        num *= 2;
    }
    if m.flags().contains(ModeFlags::DBLSCAN) {
        den *= 2;
    }
    if m.vscan() > 1 {
        den *= m.vscan() as u64;
    }
    ((num + den / 2) / den) as u32
}

/// Discovered properties of one display output.
#[derive(Clone)]
pub struct ConnectorInfo {
    pub name: String,
    pub active_mode: Option<ActiveMode>,
    pub vrr_enabled: Option<bool>,
}

/// Full snapshot of DRM topology at the moment of the call.
#[derive(Clone)]
pub struct DrmInfo {
    pub connectors: Vec<ConnectorInfo>,
    /// Raw CRTC handle for the first active connector found during `query()`.
    ///
    /// Passed into [`crate::flip_tracker::FlipTracker::new`] so the page-flip
    /// epoll loop can register for `DRM_EVENT_FLIP_COMPLETE` on the correct CRTC
    /// without re-querying DRM topology at tracker-start time.
    ///
    /// `None` when no active CRTC was found (e.g. headless, all connectors inactive).
    pub primary_crtc: Option<u32>,
    /// Device node path used to open the fd that produced this snapshot.
    ///
    /// Stored so `FlipTracker::new` can open a fresh, independent fd for the
    /// epoll thread without any fd-lifetime dependency on the query-time `Card`.
    pub dev_path: String,
}

impl DrmInfo {
    pub fn print(&self) {
        for c in &self.connectors {
            if let Some(m) = &c.active_mode {
                let vrr = match c.vrr_enabled {
                    Some(true) => " (VRR: On)",
                    Some(false) => " (VRR: Off)",
                    None => "",
                };
                println!(
                    "{}: {}x{} @ {:.3}Hz{}",
                    c.name,
                    m.width,
                    m.height,
                    m.refresh_mhz as f64 / 1000.0,
                    vrr
                );
            }
        }
    }

    /// Returns the exact refresh rate (millihertz) of the named connector, or
    /// `None` if not found or the connector has no active mode.
    pub fn find_refresh_mhz(&self, name: &str) -> Option<u32> {
        self.connectors
            .iter()
            .find(|c| c.name.eq_ignore_ascii_case(name))
            .and_then(|c| c.active_mode.as_ref())
            .map(|m| m.refresh_mhz)
    }

    /// Returns the CRTC `VRR_ENABLED` state of the named connector, or `None` if
    /// the connector is unknown or the property is not exposed.
    pub fn find_vrr_enabled(&self, name: &str) -> Option<bool> {
        self.connectors
            .iter()
            .find(|c| c.name.eq_ignore_ascii_case(name))
            .and_then(|c| c.vrr_enabled)
    }

    /// Returns the refresh rate of the named connector, or `None` if not found
    /// or the connector has no active mode.
    pub fn find_refresh_hz(&self, name: &str) -> Option<u32> {
        self.connectors
            .iter()
            .find(|c| c.name.eq_ignore_ascii_case(name))
            .and_then(|c| c.active_mode.as_ref())
            .map(|m| m.refresh_hz)
    }
}

/// Opens the first available DRM device node and queries its full topology.
///
/// Tries `/dev/dri/card0` through `card9` in order, returning `None` if no
/// node can be opened or the DRM ioctls fail. Non-fatal - the caller should
/// degrade gracefully and continue without DRM info.
///
/// The `primary_crtc` field in the returned [`DrmInfo`] is populated with the
/// raw CRTC handle of the first active connector found, enabling the caller to
/// construct a [`crate::flip_tracker::FlipTracker`] without a second DRM query.
pub fn query() -> Option<DrmInfo> {
    let (card, dev_path) = (0..10).find_map(|i| {
        let path = format!("/dev/dri/card{}", i);
        OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .ok()
            .map(|f| (Card(f), path))
    })?;

    let res = card.resource_handles().ok()?;

    let mut primary_crtc: Option<u32> = None;

    let connectors = res
        .connectors()
        .iter()
        .filter_map(|&handle| {
            let info = card.get_connector(handle, true).ok()?;
            let name = format!("{}-{}", info.interface().as_str(), info.interface_id());

            let crtc_id = info
                .current_encoder()
                .and_then(|enc_handle| card.get_encoder(enc_handle).ok()?.crtc().map(u32::from));

            let active_mode: Option<ActiveMode> = crtc_id.and_then(|id| {
                res.crtcs()
                    .iter()
                    .find(|&&h| u32::from(h) == id)
                    .and_then(|&h| card.get_crtc(h).ok())
                    .and_then(|crtc_info: crtc::Info| {
                        crtc_info.mode().map(|m: drm::control::Mode| ActiveMode {
                            width: m.size().0,
                            height: m.size().1,
                            refresh_hz: m.vrefresh(),
                            refresh_mhz: mode_refresh_mhz(&m),
                        })
                    })
            });

            // Record the first active CRTC handle for `FlipTracker`.
            if active_mode.is_some() && primary_crtc.is_none() {
                primary_crtc = crtc_id;
            }
            let vrr_enabled: Option<bool> = crtc_id.and_then(|id| {
                res.crtcs()
                    .iter()
                    .find(|&&h| u32::from(h) == id)
                    .and_then(|&h| card.get_properties(h).ok())
                    .and_then(|crtc_props| {
                        let (ids, vals) = crtc_props.as_props_and_values();
                        ids.iter().zip(vals.iter()).find_map(|(&prop_id, &val)| {
                            card.get_property(prop_id).ok().and_then(|p| {
                                if p.name().to_string_lossy() == "VRR_ENABLED" {
                                    Some(val != 0)
                                } else {
                                    None
                                }
                            })
                        })
                    })
            });

            Some(ConnectorInfo {
                name,
                active_mode,
                vrr_enabled,
            })
        })
        .collect();

    Some(DrmInfo {
        connectors,
        primary_crtc,
        dev_path,
    })
}
