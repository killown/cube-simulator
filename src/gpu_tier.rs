// gpu_tier.rs

use wgpu::{Adapter, DeviceType};

/// Hardware performance classification used to select the shader variant.
///
/// `HighEnd` maps to the full raymarched SDF pipeline (`shader.wgsl`).
/// `LowEnd`  maps to the analytic rasterised pipeline (`shader_low.wgsl`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuTier {
    HighEnd,
    LowEnd,
}

/// Source of the final [`GpuTier`] decision, carried into the banner so the
/// user always knows whether the selection was automatic or manually forced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SelectionSource {
    /// Chosen from adapter capabilities with no `--shader` flag.
    AutoDetected,
    /// User passed `--shader high` or `--shader low`.
    ManualOverride,
}

/// Minimum `max_compute_invocations_per_workgroup` a discrete GPU must report
/// before it is promoted to `HighEnd`. Cards below this threshold are treated
/// as low-end even when `DeviceType` is `DiscreteGpu` (e.g. very old dGPUs,
/// entry-level laptop dGPUs, or drivers that report a conservative cap).
const MIN_INVOCATIONS_HIGH_END: u32 = 1024;

impl GpuTier {
    /// Resolves the final shader tier, respecting an optional CLI override.
    ///
    /// When `shader_override` is `Some("high")` or `Some("low")` the adapter
    /// check is skipped entirely and the user's choice is used directly.
    /// When it is `None` the tier is inferred from two adapter signals:
    ///
    /// 1. **Device type** - `IntegratedGpu` and `Cpu` (software renderer) are
    ///    immediately `LowEnd`; `VirtualGpu` and `Other` are treated conservatively
    ///    as `LowEnd` because their compute throughput is unpredictable.
    ///    Only `DiscreteGpu` proceeds to the limit check.
    ///
    /// 2. **Compute invocation limit** - A discrete GPU must advertise at least
    ///    [`MIN_INVOCATIONS_HIGH_END`] invocations per workgroup. Cards that fall
    ///    below this are typically entry-level or first-generation dGPUs whose
    ///    per-pixel raymarching budget is insufficient for the high-end shader.
    ///
    /// Prints a clearly-bordered startup banner to stdout showing the selected
    /// shader file, selection source (auto vs override), and a comparability
    /// warning so benchmark results cannot be silently mixed between variants.
    pub fn resolve(adapter: &Adapter, shader_override: Option<&str>) -> Self {
        let info = adapter.get_info();
        let limits = adapter.limits();

        let (tier, source) = match shader_override {
            Some("high") => (Self::HighEnd, SelectionSource::ManualOverride),
            Some("low") => (Self::LowEnd, SelectionSource::ManualOverride),
            _ => {
                let detected = match info.device_type {
                    DeviceType::IntegratedGpu | DeviceType::Cpu => Self::LowEnd,
                    DeviceType::DiscreteGpu => {
                        if limits.max_compute_invocations_per_workgroup >= MIN_INVOCATIONS_HIGH_END
                        {
                            Self::HighEnd
                        } else {
                            Self::LowEnd
                        }
                    }
                    _ => Self::LowEnd,
                };
                (detected, SelectionSource::AutoDetected)
            }
        };

        tier.print_banner(&info, &limits, source);
        tier
    }

    /// Returns the WGSL source string for this tier's shader.
    ///
    /// Embedded at compile time via `include_str!`; no file I/O at runtime.
    #[inline]
    pub fn shader_source(self) -> &'static str {
        match self {
            Self::HighEnd => include_str!("shader.wgsl"),
            Self::LowEnd => include_str!("shader_low.wgsl"),
        }
    }

    /// Prints a clearly-bordered startup banner to stdout.
    ///
    /// The banner is intentionally wide and decorated so it cannot be missed
    /// in terminal output or log files. It shows the loaded file, whether the
    /// choice was automatic or a manual `--shader` override, and a hard
    /// comparability warning when the low-end variant is active.
    fn print_banner(
        self,
        info: &wgpu::AdapterInfo,
        limits: &wgpu::Limits,
        source: SelectionSource,
    ) {
        let border = "═".repeat(60);
        let source_label = match source {
            SelectionSource::AutoDetected => "auto-detected from adapter capabilities",
            SelectionSource::ManualOverride => "manually forced via --shader flag",
        };

        println!("\n╔{border}╗");
        println!("║  SHADER VARIANT SELECTED                                   ║");
        println!("╠{border}╣");

        match self {
            Self::HighEnd => {
                println!("║  ✔  HIGH-END shader loaded                                 ║");
                println!("║     File : shader.wgsl                                     ║");
                println!("║     Mode : Raymarched SDF + inner shapes + film grain      ║");
            }
            Self::LowEnd => {
                println!("║  ⚠  LOW-END shader loaded                                  ║");
                println!("║     File : shader_low.wgsl                                 ║");
                println!("║     Mode : Analytic raster, reduced visual workload        ║");
            }
        }

        println!("║  Selection : {:<46}║", trim_field(source_label, 46));
        println!("╠{border}╣");
        println!("║  GPU  : {:<51}║", trim_field(&info.name, 51));
        println!(
            "║  Type : {:<51}║",
            trim_field(&format!("{:?}", info.device_type), 51)
        );
        println!(
            "║  Compute invocations/wg : {:<33}║",
            trim_field(
                &format!(
                    "{} (need >= {} for high-end)",
                    limits.max_compute_invocations_per_workgroup, MIN_INVOCATIONS_HIGH_END
                ),
                33
            )
        );
        println!("╠{border}╣");

        match self {
            Self::HighEnd => {
                println!("║  Results quality : FULL                                    ║");
                println!("║  Comparable with : other HIGH-END runs only                ║");
            }
            Self::LowEnd => {
                println!("║  !! Results quality : REDUCED                            !!║");
                println!("║  !! NOT comparable with high-end shader results.         !!║");
                println!("║  !! Do not mix these numbers in benchmarks.              !!║");
            }
        }

        println!("╚{border}╝\n");

        //FIXME: Consider removing the low shader option.
        // Instead, inform users that the tool may not function correctly on non-discrete GPUs,
        // as the FPS data for low-end shaders is inherently unreliable.
        if matches!(self, Self::LowEnd) {
            eprintln!(
                "WHY FPS IS UNRELIABLE WITH THE LOW-END SHADER\n\
                 ──────────────────────────────────────────────\n\
                 The low-end shader exits early when a ray misses a cube's\n\
                 bounding box. This makes GPU cost depend on the animation\n\
                 state, not just cube count. When cubes are spread apart,\n\
                 rays exit early and the frame is cheap. When cubes cluster\n\
                 together, more intersection math runs and the frame is\n\
                 expensive. This causes FPS to swing 20-40% between frames\n\
                 for the same -c value.\n\
                 \n\
                 As a result, -c 5 can be slower than -c 6 at any given\n\
                 moment, and the LOW 1% FPS reflects animation luck as much\n\
                 as hardware capability.\n\
                 \n\
                 The high-end shader does not have this problem: its SDF\n\
                 loop always runs steps x cube_count x pixels, constant\n\
                 regardless of where the cubes are. Use it for any real\n\
                 compositor or performance measurement.\n"
            );
        }
    }
}

/// Trims or pads `s` to exactly `width` bytes for fixed-width banner columns.
fn trim_field(s: &str, width: usize) -> String {
    if s.len() >= width {
        format!("{}..", &s[..width.saturating_sub(2)])
    } else {
        format!("{:<width$}", s, width = width)
    }
}
