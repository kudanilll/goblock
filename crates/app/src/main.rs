//! Desktop entry point: opens the window, sets up Vulkan, and runs the frame
//! loop.
//!
//! Platform code stays in this crate. An Android build would add its own
//! module here and reuse the camera, world and render crates unchanged.

mod assets;
mod game;
mod input;
mod settings;

use std::sync::Arc;

use goblock_input::InputSource;
use goblock_render::{FrameView, Renderer};
use goblock_world::WorldEvent;
use input::DesktopInput;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use winit::window::{Fullscreen, Window, WindowId};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let seed = world_seed(std::env::args().nth(1).as_deref());
    // Printed as signed, which is also how the HUD shows it, so it can be
    // passed straight back on the command line.
    println!("goblock: world seed {}", seed as i64);

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
    settings: settings::Settings,
    settings_menu: settings::SettingsMenu,
    settings_open: bool,
    mouse_pos: (f32, f32),
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
            WindowEvent::CursorMoved { position, .. } => {
                state.mouse_pos = (position.x as f32, position.y as f32);
                if state.settings_open {
                    let size = state.window.inner_size();
                    state.settings_menu.handle_mouse_move(
                        &mut state.settings,
                        position.x as f32,
                        position.y as f32,
                        size.width as f32,
                        size.height as f32,
                    );
                }
            }
            WindowEvent::MouseInput {
                state: button_state,
                button: MouseButton::Left,
                ..
            } => {
                if state.settings_open {
                    let size = state.window.inner_size();
                    if button_state == ElementState::Pressed {
                        let action = state.settings_menu.handle_mouse_down(
                            &mut state.settings,
                            state.mouse_pos.0,
                            state.mouse_pos.1,
                            size.width as f32,
                            size.height as f32,
                        );
                        match action {
                            settings::Action::Close => {
                                state.settings_open = false;
                                state.input.capture_cursor_grab(&state.window);
                            }
                            settings::Action::Quit => {
                                event_loop.exit();
                            }
                            settings::Action::None => {}
                        }
                    } else {
                        state.settings_menu.handle_mouse_up();
                    }
                }
            }
            WindowEvent::RedrawRequested => match state.frame() {
                Ok(true) => state.window.request_redraw(),
                Ok(false) => event_loop.exit(),
                Err(e) => state.fail(event_loop, e),
            },
            WindowEvent::KeyboardInput { event: key, .. }
                if key.state == ElementState::Pressed && !key.repeat =>
            {
                if let PhysicalKey::Code(code) = key.physical_key {
                    if code == KeyCode::Escape {
                        state.settings_open = !state.settings_open;
                        if state.settings_open {
                            state.input.release_cursor_grab(&state.window);
                        } else {
                            state.input.capture_cursor_grab(&state.window);
                        }
                    } else if state.settings_open {
                        let action = state
                            .settings_menu
                            .handle_key_down(&mut state.settings, code);
                        match action {
                            settings::Action::Close => {
                                state.settings_open = false;
                                state.input.capture_cursor_grab(&state.window);
                            }
                            settings::Action::Quit => {
                                event_loop.exit();
                            }
                            settings::Action::None => {}
                        }
                    } else if code == KeyCode::F11 {
                        // Borderless rather than exclusive fullscreen: it switches
                        // instantly and works on every platform winit supports.
                        let fullscreen = match state.window.fullscreen() {
                            Some(_) => None,
                            None => Some(Fullscreen::Borderless(None)),
                        };
                        state.window.set_fullscreen(fullscreen);
                    }
                }
            }
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
            if !state.settings_open {
                state.input.handle_device_event(&event);
            }
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

        let mut input = DesktopInput::new();
        input.capture_cursor_grab(&window);

        let settings = settings::Settings::default();
        let settings_menu = settings::SettingsMenu::new();

        Ok(Self {
            renderer,
            window,
            game,
            input,
            settings,
            settings_menu,
            settings_open: false,
            mouse_pos: (0.0, 0.0),
        })
    }

    /// Run one frame. Returns `Ok(false)` once the player asks to quit.
    fn frame(&mut self) -> Result<bool, goblock_render::VkError> {
        let now = std::time::Instant::now();
        let delta = now - self.game.last_frame();
        self.game.set_last_frame(now);

        let size = self.window.inner_size();
        let screen_w = size.width as f32;
        let screen_h = size.height as f32;

        if !self.settings_open {
            let mut input = self.input.poll();
            input.look_delta.x *= self.settings.sensitivity / 2.5;
            input.look_delta.y *= (self.settings.sensitivity / 2.5)
                * (if self.settings.invert_y { -1.0 } else { 1.0 });

            self.game.update(delta.as_secs_f32(), &input);

            for event in self.game.stream() {
                match event {
                    WorldEvent::Meshed(pos, mesh) => self.renderer.upload_chunk(pos, &mesh)?,
                    WorldEvent::Unloaded(pos) => self.renderer.remove_chunk(pos),
                }
            }
        }

        let camera = &self.game.camera;
        let view = FrameView {
            camera: camera.position,
            player_feet: self.game.player.feet.as_vec3(),
            player_height: goblock_input::player::HEIGHT as f32,
            view_projection: camera.projection_matrix(self.game.aspect()) * camera.view_matrix(),
            fog_end: self.game.fog_end(),
            time: self.game.elapsed(),
            underwater: self.game.underwater(),
            blur_background: self.settings_open,
        };

        let ui_vertices = if self.settings_open {
            self.settings_menu
                .build_vertices(&self.settings, screen_w, screen_h)
        } else {
            let lines = self.game.hud_lines(self.renderer.chunk_count());
            goblock_render::build_hud_vertices(&lines)
        };

        self.renderer.draw_frame(&view, &ui_vertices)?;
        Ok(true)
    }

    fn fail(&mut self, event_loop: &ActiveEventLoop, e: impl std::fmt::Display) {
        eprintln!("goblock: {e}");
        event_loop.exit();
    }
}

/// Seed for the world: from `arg` when one is given, otherwise a fresh
/// random one, so every launch starts in a new world.
fn world_seed(arg: Option<&str>) -> u64 {
    match arg {
        Some(text) => parse_seed(text),
        None => random_seed(),
    }
}

/// Numbers are used as they are, negative ones included. Any other text is
/// hashed with FNV-1a, which never changes between builds or platforms, so
/// the same word always gives the same world.
fn parse_seed(text: &str) -> u64 {
    let text = text.trim();
    if let Ok(number) = text.parse::<i64>() {
        return number as u64;
    }
    if let Ok(number) = text.parse::<u64>() {
        return number;
    }
    text.bytes().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ byte as u64).wrapping_mul(0x0100_0000_01b3)
    })
}

/// A random seed. The standard library keys `RandomState` from the
/// operating system's random source, so it differs on every launch.
fn random_seed() -> u64 {
    use std::hash::{BuildHasher, Hasher};
    std::collections::hash_map::RandomState::new()
        .build_hasher()
        .finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_are_used_as_they_are() {
        assert_eq!(parse_seed("42"), 42);
        assert_eq!(parse_seed(" 42 "), 42);
        assert_eq!(parse_seed("-1") as i64, -1);
        assert_eq!(parse_seed(&u64::MAX.to_string()), u64::MAX);
    }

    #[test]
    fn printed_seeds_parse_back_to_the_same_world() {
        for seed in [0u64, 7, u64::MAX, 1 << 63] {
            assert_eq!(parse_seed(&(seed as i64).to_string()), seed);
        }
    }

    #[test]
    fn text_seeds_are_stable_and_distinct() {
        assert_eq!(parse_seed("goblock"), parse_seed("goblock"));
        assert_ne!(parse_seed("goblock"), parse_seed("GoBlock"));
        // FNV-1a of "a", fixed for good.
        assert_eq!(parse_seed("a"), 0xaf63_dc4c_8601_ec8c);
    }

    #[test]
    fn launches_without_a_seed_get_different_worlds() {
        assert_ne!(world_seed(None), world_seed(None));
        assert_eq!(world_seed(Some("5")), 5);
    }
}
