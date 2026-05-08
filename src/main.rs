mod app;
mod args;
mod benchmark;
mod drm;
mod flip_tracker;
mod gpu_tier;
mod gpu_timer;
mod metrics;
mod pll;
mod renderer;
mod uniforms;

use clap::Parser;
use winit::event_loop::EventLoop;

use app::App;
use args::Args;

#[cfg(test)]
mod tests;

fn main() {
    let mut args = Args::parse();

    if args
        .bench_secs
        .is_some_and(|secs| args.bench_warmup >= secs)
    {
        eprintln!(
            "Error: --bench-warmup ({}) must be less than --bench-secs ({})",
            args.bench_warmup,
            args.bench_secs.unwrap()
        );
        std::process::exit(1);
    }

    let drm_info = drm::query();

    match (&args.connector, &drm_info) {
        // No --connector supplied: try to auto-pick the only active one or warn about multi-output.
        (None, Some(info)) => {
            let active: Vec<_> = info
                .connectors
                .iter()
                .filter(|c| c.active_mode.is_some())
                .collect();

            if !active.is_empty() {
                let selected = active[0].name.clone();
                if active.len() > 1 {
                    eprintln!(
                        "WARNING: Multi-output setup detected ({} monitors active).",
                        active.len()
                    );
                    eprintln!(
                        "Auto-selecting primary: {}. Benchmark results may be inconsistent.",
                        selected
                    );
                    eprintln!("Recommendation: Disable secondary monitors for maximum accuracy.");
                } else {
                    eprintln!("Auto-selecting connector: {}", selected);
                }
                args.connector = Some(selected);
            }
            run(args, drm_info);
        }

        // --connector supplied: verify it exists, otherwise fallback to the first active one.
        (Some(name), info) => {
            let found = info
                .as_ref()
                .and_then(|i| i.find_refresh_hz(name))
                .is_some();

            if !found {
                eprintln!("Error: connector '{}' not found in DRM topology.", name);
                if let Some(i) = info {
                    let active: Vec<_> = i
                        .connectors
                        .iter()
                        .filter(|c| c.active_mode.is_some())
                        .collect();
                    if !active.is_empty() {
                        let fallback = active[0].name.clone();
                        eprintln!(
                            "FALLBACK: Using available connector '{}' instead.",
                            fallback
                        );
                        if active.len() > 1 {
                            eprintln!(
                                "CRITICAL: Multi-output active. Test environment is UNRELIABLE."
                            );
                            eprintln!(
                                "Please disable secondary outputs to ensure accurate frame pacing logs."
                            );
                        }
                        args.connector = Some(fallback);
                    }
                }
            } else {
                eprintln!("Targeting output: {}", name);
                if let Some(i) = info {
                    let active_count = i
                        .connectors
                        .iter()
                        .filter(|c| c.active_mode.is_some())
                        .count();
                    if active_count > 1 {
                        eprintln!(
                            "Warning: Multi-output detected. Performance may be degraded by compositor overhead."
                        );
                    }
                }
            }

            run(args, drm_info);
        }

        // No --connector and no DRM, proceed with winit fallback.
        (None, None) => {
            eprintln!(
                "WARN: DRM unavailable, refresh rate from winit may be wrong. \
                       Pass --connector if pacing metrics look incorrect."
            );
            run(args, None);
        }
    }
}

fn run(args: Args, drm_info: Option<drm::DrmInfo>) {
    if let Some(ref info) = drm_info {
        info.print();
    }
    let mut app = App {
        state: None,
        args,
        drm_info,
    };
    EventLoop::new().unwrap().run_app(&mut app).unwrap();
}
