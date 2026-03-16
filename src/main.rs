mod app;
mod args;
mod benchmark;
mod drm;
mod flip_tracker;
mod gpu_tier;
mod gpu_timer;
mod metrics;
mod renderer;
mod uniforms;

use clap::Parser;
use winit::event_loop::EventLoop;

use app::App;
use args::Args;

fn main() {
    let args = Args::parse();

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
        // No --connector supplied: print what's available and require the user to choose.
        (None, Some(info)) => {
            let active: Vec<_> = info
                .connectors
                .iter()
                .filter(|c| c.active_mode.is_some())
                .collect();

            if active.len() == 1 {
                let mut auto_args = args;
                auto_args.connector = Some(active[0].name.clone());
                eprintln!("Auto-selecting only active connector: {}", active[0].name);
                run(auto_args, drm_info);
                return;
            }

            if !active.is_empty() {
                eprintln!("Active display outputs detected:");
                for c in &active {
                    if let Some(m) = &c.active_mode {
                        let vrr = match c.vrr_enabled {
                            Some(true) => " (VRR: On)",
                            Some(false) => " (VRR: Off)",
                            None => "",
                        };
                        eprintln!(
                            "  --connector {}  ({}x{} @ {}Hz{})",
                            c.name, m.width, m.height, m.refresh_hz, vrr
                        );
                    }
                }
                eprintln!("\nRe-run with --connector <name> to select the output under test.");
                std::process::exit(1);
            }

            // DRM available but no active connectors, fall through with winit fallback.
            run(args, drm_info);
        }

        // --connector supplied but DRM is unavailable or the name isn't found.
        (Some(name), info) => {
            let found = info
                .as_ref()
                .and_then(|i| i.find_refresh_hz(name))
                .is_some();

            if !found {
                eprintln!("Error: connector '{}' not found in DRM topology.", name);
                if let Some(i) = info {
                    eprintln!("Available connectors:");
                    i.print();
                } else {
                    eprintln!("(DRM unavailable on this system)");
                }
                std::process::exit(1);
            }

            run(args, drm_info);
        }

        // No --connector and no DRM, proceed with winit fallback, warn once.
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
