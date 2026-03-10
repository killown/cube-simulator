mod app;
mod args;
mod drm;
mod metrics;
mod renderer;
mod uniforms;

use clap::Parser;
use winit::event_loop::EventLoop;

use app::App;
use args::Args;

fn main() {
    let args = Args::parse();

    if let Some(info) = drm::query() {
        info.print();
    }

    let mut app = App { state: None, args };
    EventLoop::new().unwrap().run_app(&mut app).unwrap();
}
