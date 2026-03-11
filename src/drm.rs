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
    pub refresh_hz: u32,
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
                    "{}: {}x{} @ {}Hz{}",
                    c.name, m.width, m.height, m.refresh_hz, vrr
                );
            }
        }
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
/// node can be opened or the DRM ioctls fail. Non-fatal — the caller should
/// degrade gracefully and continue without DRM info.
pub fn query() -> Option<DrmInfo> {
    let card = (0..10).find_map(|i| {
        OpenOptions::new()
            .read(true)
            .write(true)
            .open(format!("/dev/dri/card{}", i))
            .ok()
            .map(Card)
    })?;

    let res = card.resource_handles().ok()?;

    let connectors = res
        .connectors()
        .iter()
        .filter_map(|&handle| {
            let info = card.get_connector(handle, true).ok()?;
            let name = format!("{}-{}", info.interface().as_str(), info.interface_id());

            let crtc_id = info.current_encoder().and_then(|enc_handle| {
                card.get_encoder(enc_handle)
                    .ok()
                    .and_then(|enc| enc.crtc().map(|h| u32::from(h)))
            });

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
                        })
                    })
            });

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

    Some(DrmInfo { connectors })
}
