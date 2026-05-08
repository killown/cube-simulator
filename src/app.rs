use std::sync::Arc;

use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::ActiveEventLoop,
    window::{Fullscreen, WindowAttributes},
};

use crate::args::Args;
use crate::drm::DrmInfo;
use crate::renderer::State;

/// Top-level winit application container.
///
/// `state` is `None` until [`ApplicationHandler::resumed`] fires, at which
/// point the window and GPU state are created. This matches the documented
/// winit lifecycle for cross-platform suspend/resume support.
pub struct App<'a> {
    pub state: Option<State<'a>>,
    pub args: Args,
    pub drm_info: Option<DrmInfo>,
}

impl<'a> ApplicationHandler for App<'a> {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        let attributes =
            WindowAttributes::default().with_fullscreen(Some(Fullscreen::Borderless(None)));
        let window = Arc::new(el.create_window(attributes).unwrap());
        self.state = Some(pollster::block_on(State::new(
            window,
            self.args.clone(),
            self.drm_info.take(),
        )));

        println!(
            "\nMETRIC LEGEND:\n\
            ==============\n\
            FPS:  Average Frames Per Second\n\
            MIN:  Minimum FPS observed\n\
            MAX:  Maximum FPS observed\n\
            LOW:  1% Low FPS (stutter indicator)\n\
            JIT:  Frame-to-frame jitter (ms)\n\
            MSD:  Missed/dropped frame count\n\
            FTV:  Frame time variance (%)\n\
            CPU:  CPU frame time (ms)\n\
            GPU:  GPU render time (ms)\n\
            SYN:  Vblank alignment sync score (0-100)\n\
            SLA:  Slack (ms)\n\
            SVA:  Sync score standard deviation\n\
            ==============\n"
        );
    }

    fn window_event(
        &mut self,
        el: &ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        let Some(state) = &mut self.state else { return };
        if window_id == state.window.id() {
            match event {
                WindowEvent::CloseRequested => el.exit(),
                WindowEvent::KeyboardInput {
                    event:
                        winit::event::KeyEvent {
                            logical_key:
                                winit::keyboard::Key::Named(winit::keyboard::NamedKey::Escape),
                            state: winit::event::ElementState::Pressed,
                            ..
                        },
                    ..
                } => el.exit(),
                WindowEvent::Resized(s) => {
                    state.config.width = s.width.max(1);
                    state.config.height = s.height.max(1);
                    state.surface.configure(&state.device, &state.config);
                }
                WindowEvent::RedrawRequested => {
                    let rendered = state.render();

                    // A completed benchmark terminates the event loop and prints
                    // the report. The render() call above still completes so the
                    // final frame (with the triggering cube count) is presented.
                    if rendered && state.benchmark_done {
                        if let Some(bench) = &state.benchmark {
                            bench.print_report();
                        }
                        el.exit();
                        return;
                    }

                    state.window.request_redraw();
                }
                _ => (),
            }
        }
    }
}
