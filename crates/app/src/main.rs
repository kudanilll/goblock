//! Desktop entry point: opens the window, sets up Vulkan, and runs the frame
//! loop.
//!
//! Platform code stays in this crate. An Android build would add its own
//! module here and reuse the camera, world and render crates unchanged.

mod assets;
mod game;
mod input;

use std::sync::Arc;

use goblock_input::InputSource;
use goblock_render::{FrameView, Renderer};
use goblock_world::WorldEvent;
use input::DesktopInput;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use winit::window::{Window, WindowId};

/// World seed when none is given on the command line.
const DEFAULT_SEED: u64 = 1;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // The first argument, if any, is the world seed.
    let seed = match std::env::args().nth(1) {
        Some(arg) => arg.parse().map_err(|e| format!("seed {arg:?}: {e}"))?,
        None => DEFAULT_SEED,
    };

    let event_loop = EventLoop::new()?;
    let mut app = App {
        seed,
        state: None,
        error: None,
    };
    event_loop.run_app(&mut app)?;
    if let Some(err) = app.error.take() {
        return Err(err.into());
    }
    Ok(())
}

struct App {
    seed: u64,
    state: Option<ActiveState>,
    error: Option<String>,
}

struct ActiveState {
    // Fields drop in declaration order. The renderer owns the Vulkan surface,
    // which has to be destroyed before the window it belongs to.
    renderer: Renderer,
    window: Arc<Window>,
    game: game::Game,
    input: DesktopInput,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }
        match ActiveState::new(event_loop, self.seed) {
            Ok(state) => self.state = Some(state),
            Err(e) => {
                self.error = Some(e);
                event_loop.exit();
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(state) = self.state.as_mut() else {
            return;
        };

        state.input.handle_window_event(&event);

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Err(e) = state.renderer.resize(size.width, size.height) {
                    state.fail(event_loop, e);
                    return;
                }
                state.game.set_screen_size(size);
            }
            WindowEvent::RedrawRequested => match state.frame() {
                Ok(true) => state.window.request_redraw(),
                Ok(false) => event_loop.exit(),
                Err(e) => state.fail(event_loop, e),
            },
            _ => {}
        }
    }

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device_id: winit::event::DeviceId,
        event: winit::event::DeviceEvent,
    ) {
        if let Some(state) = self.state.as_mut() {
            state.input.handle_device_event(&event);
        }
    }
}

impl ActiveState {
    fn new(event_loop: &ActiveEventLoop, seed: u64) -> Result<Self, String> {
        let window = Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title("goblock")
                        .with_inner_size(winit::dpi::LogicalSize::new(800.0, 600.0)),
                )
                .map_err(|e| format!("create window: {e}"))?,
        );

        let size = window.inner_size();
        let entry = unsafe { ash::Entry::load() }.map_err(|e| format!("load Vulkan: {e}"))?;

        let raw_window_handle = window
            .window_handle()
            .map_err(|e| format!("window handle: {e}"))?
            .as_raw();
        let raw_display_handle = window
            .display_handle()
            .map_err(|e| format!("display handle: {e}"))?
            .as_raw();

        let enable_validation = cfg!(debug_assertions);
        let device = unsafe {
            goblock_render::vk::device::Device::new(
                entry,
                raw_window_handle,
                raw_display_handle,
                enable_validation,
            )
        }
        .map_err(|e| e.to_string())?;

        let loaded = assets::load_block_textures();
        if !loaded.missing.is_empty() {
            eprintln!(
                "goblock: using placeholder colours for {}",
                loaded.missing.join(", ")
            );
        }

        let renderer = unsafe {
            Renderer::new(
                device,
                size.width.max(1),
                size.height.max(1),
                &loaded.textures,
            )
        }
        .map_err(|e| e.to_string())?;

        let game = game::Game::new(seed).map_err(|e| format!("start world: {e}"))?;

        let input = DesktopInput::new();
        input.set_cursor_grab(&window);

        Ok(Self {
            renderer,
            window,
            game,
            input,
        })
    }

    /// Run one frame. Returns `Ok(false)` once the player asks to quit.
    fn frame(&mut self) -> Result<bool, goblock_render::VkError> {
        let now = std::time::Instant::now();
        let delta = now - self.game.last_frame();
        self.game.set_last_frame(now);

        let input = self.input.poll();
        if input.quit {
            return Ok(false);
        }
        self.game.update(delta.as_secs_f32(), &input);

        for event in self.game.stream() {
            match event {
                WorldEvent::Meshed(pos, mesh) => self.renderer.upload_chunk(pos, &mesh)?,
                WorldEvent::Unloaded(pos) => self.renderer.remove_chunk(pos),
            }
        }

        let camera = &self.game.camera;
        let view = FrameView {
            camera: camera.position,
            view_projection: camera.projection_matrix(self.game.aspect()) * camera.view_matrix(),
            fog_end: self.game.fog_end(),
            time: self.game.elapsed(),
            underwater: self.game.underwater(),
        };
        let lines = self.game.hud_lines(self.renderer.chunk_count());
        self.renderer.draw_frame(&view, &lines)?;
        Ok(true)
    }

    fn fail(&mut self, event_loop: &ActiveEventLoop, e: impl std::fmt::Display) {
        eprintln!("goblock: {e}");
        event_loop.exit();
    }
}
