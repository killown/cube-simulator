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
            JIT:  Frame-to-frame variance (ms)\n\
            MSD:  Missed frames (>{:.1}ms threshold)\n\
            FTV:  Frame Time Variance %%, stddev/mean of frame times in the rolling window.\n\
                  0%% = perfectly uniform delivery. High %% = frames bunching (some near-instant,\n\
                  some very slow), which looks skippy even when mean FPS appears acceptable.\n",
            self.args.threshold
        );
    }

    fn window_event(
        &mut self,
        el: &ActiveEventLoop,
        _id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        if let Some(state) = self.state.as_mut() {
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
                    let _ = state.render();

                    // A completed benchmark terminates the event loop and prints
                    // the report. The render() call above still completes so the
                    // final frame (with the triggering cube count) is presented.
                    if state.benchmark_done {
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
