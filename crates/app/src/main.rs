//! Desktop entry point: opens the window, sets up Vulkan, and runs the frame
//! loop.
//!
//! Platform code stays in this crate. An Android build would add its own
//! module here and reuse the camera, world and render crates unchanged.

mod assets;
mod game;
mod input;
mod menu;
mod saves;
mod screens;
mod settings;
mod ui;

use std::sync::Arc;

use goblock_input::InputSource;
use goblock_render::{FrameView, Renderer};
use goblock_world::WorldEvent;
use input::DesktopInput;
use screens::{AppMode, Screens, Transition};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use winit::window::{Fullscreen, Window, WindowId};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let seed = world_seed(args.first().map(String::as_str));

    let event_loop = EventLoop::new().expect("create event loop");
    let mut app = App {
        seed,
        state: None,
        error: None,
    };
    if let Err(e) = event_loop.run_app(&mut app) {
        eprintln!("goblock: {e}");
    }
    if let Some(e) = app.error {
        eprintln!("goblock: {e}");
        std::process::exit(1);
    }
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
    game: Option<game::Game>,
    input: DesktopInput,
    screens: Screens,
    current_world: Option<saves::WorldMeta>,
    mouse_pos: (f32, f32),
    start: std::time::Instant,
    last_frame: std::time::Instant,
    last_save: std::time::Instant,
    fps: ui::FpsCounter,
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
            WindowEvent::CloseRequested => state.apply(Transition::Quit, event_loop),
            WindowEvent::Focused(false) => state.save_current_world(),
            WindowEvent::Resized(size) => {
                if let Err(e) = state.renderer.resize(size.width, size.height) {
                    state.fail(event_loop, e);
                    return;
                }
                if let Some(game) = state.game.as_mut() {
                    game.set_screen_size(size);
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                state.mouse_pos = (position.x as f32, position.y as f32);
                let (x, y, w, h) = state.ui_cursor();
                state.screens.mouse_moved(x, y, w, h);
            }
            WindowEvent::MouseInput {
                state: press,
                button: MouseButton::Left,
                ..
            } => {
                let (x, y, w, h) = state.ui_cursor();
                let pressed = press == ElementState::Pressed;
                let transition = state.screens.mouse_button(pressed, x, y, w, h);
                state.apply(transition, event_loop);
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let lines = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(position) => position.y as f32,
                };
                let (_, _, _, h) = state.ui_cursor();
                state.screens.wheel(lines, h);
            }
            WindowEvent::RedrawRequested => match state.frame() {
                Ok(()) => state.window.request_redraw(),
                Err(e) => state.fail(event_loop, e),
            },
            WindowEvent::KeyboardInput { event: key, .. }
                if key.state == ElementState::Pressed && !key.repeat =>
            {
                if let Some(text) = &key.text {
                    state.screens.typed(text);
                }
                if let PhysicalKey::Code(code) = key.physical_key {
                    if code == KeyCode::F11 {
                        state.toggle_fullscreen();
                        return;
                    }
                    let (_, _, _, h) = state.ui_cursor();
                    let transition = state.screens.key_down(code, h);
                    state.apply(transition, event_loop);
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
            if state.screens.mode == AppMode::Playing {
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
        let backgrounds = assets::load_background_textures();

        let renderer = unsafe {
            Renderer::new(
                device,
                size.width.max(1),
                size.height.max(1),
                &loaded.textures,
                &backgrounds,
            )
        }
        .map_err(|e| e.to_string())?;

        let mut input = DesktopInput::new();
        input.release_cursor_grab(&window);

        let saved_worlds = saves::list_worlds();

        // If saved worlds is empty, prefill a default world so player has something to test right away
        let saved_worlds = if saved_worlds.is_empty() {
            if let Err(e) = saves::create_world("Survival World", &seed.to_string()) {
                eprintln!("goblock: create world error: {e}");
            }
            saves::list_worlds()
        } else {
            saved_worlds
        };

        let now = std::time::Instant::now();
        Ok(Self {
            renderer,
            window,
            game: None,
            input,
            screens: Screens::new(saved_worlds),
            current_world: None,
            mouse_pos: (0.0, 0.0),
            start: now,
            last_frame: now,
            last_save: now,
            fps: ui::FpsCounter::default(),
        })
    }

    /// The cursor and window size in UI units.
    fn ui_cursor(&self) -> (f32, f32, f32, f32) {
        let size = self.window.inner_size();
        let (w, h) = (size.width as f32, size.height as f32);
        let scale = ui::scale_for(h);
        (
            self.mouse_pos.0 / scale,
            self.mouse_pos.1 / scale,
            w / scale,
            h / scale,
        )
    }

    fn toggle_fullscreen(&self) {
        let fullscreen = match self.window.fullscreen() {
            Some(_) => None,
            None => Some(Fullscreen::Borderless(None)),
        };
        self.window.set_fullscreen(fullscreen);
    }

    /// Carries out what a screen asked for. [`Screens::apply`] has already
    /// moved the menus by the time the match runs.
    fn apply(&mut self, transition: Transition, event_loop: &ActiveEventLoop) {
        self.screens.apply(&transition);
        match transition {
            Transition::OpenWorldList => self.screens.set_worlds(saves::list_worlds()),
            Transition::OpenSettings { from_title: false } => {
                self.input.release_cursor_grab(&self.window);
            }
            Transition::ResumeWorld => self.input.capture_cursor_grab(&self.window),
            Transition::StartWorld(meta) => self.start_world(meta, event_loop),
            Transition::CreateWorld { name, seed } => match saves::create_world(&name, &seed) {
                Ok(meta) => self.start_world(meta, event_loop),
                Err(e) => eprintln!("goblock: create world error: {e}"),
            },
            Transition::DeleteWorld(id) => self.delete_world(&id),
            Transition::LeaveWorld => self.save_and_quit_to_title(),
            Transition::Quit => {
                self.save_current_world();
                event_loop.exit();
            }
            _ => {}
        }
    }

    fn start_world(&mut self, mut meta: saves::WorldMeta, event_loop: &ActiveEventLoop) {
        let size = self.window.inner_size();
        match game::Game::new(meta.seed) {
            Ok(mut game) => {
                game.set_screen_size(size);
                if meta.player_pos != [0.0, 0.0, 0.0] {
                    game.player.feet = glam::DVec3::from_array(meta.player_pos);
                    game.camera.position = game.player.eye();
                    game.player.flying = meta.player_flying;
                    game.camera
                        .set_orientation(meta.player_yaw, meta.player_pitch);
                    game.spawn_settled = true;
                }
                meta.format_version = saves::SAVE_FORMAT_VERSION;
                meta.last_played = std::time::SystemTime::now()
                    .duration_since(std::time::SystemTime::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                if let Err(e) = saves::save_world(&meta) {
                    eprintln!("goblock: save world error: {e}");
                }

                self.last_save = std::time::Instant::now();
                self.renderer.clear_chunks();
                self.current_world = Some(meta);
                self.game = Some(game);
                self.screens.mode = AppMode::Playing;
                self.input.capture_cursor_grab(&self.window);
            }
            Err(e) => {
                eprintln!("goblock: failed to start world: {e}");
                self.fail(event_loop, e);
            }
        }
    }

    fn save_current_world(&mut self) {
        self.last_save = std::time::Instant::now();
        if let (Some(game), Some(meta)) = (self.game.as_ref(), self.current_world.as_mut()) {
            if game.spawn_settled {
                meta.player_pos = game.player.feet.to_array();
                meta.player_yaw = game.camera.yaw();
                meta.player_pitch = game.camera.pitch();
                meta.player_flying = game.player.flying;
            }
            meta.last_played = std::time::SystemTime::now()
                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            if let Err(e) = saves::save_world(meta) {
                eprintln!("goblock: save world error: {e}");
            }
        }
    }

    fn delete_world(&mut self, id: &str) {
        if let Err(e) = saves::delete_world(id) {
            eprintln!("goblock: delete world error: {e}");
        }
        self.screens.set_worlds(saves::list_worlds());
    }

    fn save_and_quit_to_title(&mut self) {
        self.save_current_world();
        self.game = None;
        self.renderer.clear_chunks();
        self.current_world = None;
        self.screens.set_worlds(saves::list_worlds());
        self.input.release_cursor_grab(&self.window);
    }

    /// Advance the world and draw one frame.
    fn frame(&mut self) -> Result<(), goblock_render::VkError> {
        let now = std::time::Instant::now();
        let delta = now - self.last_frame;
        self.last_frame = now;
        let dt = delta.as_secs_f32();

        self.fps.tick(dt);

        if self.screens.mode == AppMode::Playing {
            if let Some(game) = self.game.as_mut() {
                let settings = &self.screens.settings;
                let mut input = self.input.poll();
                input.look_delta.x *= settings.sensitivity / 2.5;
                input.look_delta.y *=
                    (settings.sensitivity / 2.5) * (if settings.invert_y { -1.0 } else { 1.0 });

                game.update(dt, &input);

                for event in game.stream() {
                    match event {
                        WorldEvent::Meshed(pos, mesh) => self.renderer.upload_chunk(pos, &mesh)?,
                        WorldEvent::Unloaded(pos) => self.renderer.remove_chunk(pos),
                    }
                }
            }
            if saves::autosave_due(self.last_save, now) {
                self.save_current_world();
            }
        }

        let mode = self.screens.mode;
        let is_in_game_settings = matches!(mode, AppMode::Settings { from_title: false });
        let is_menu = mode != AppMode::Playing && !is_in_game_settings;
        let time = self.start.elapsed().as_secs_f32();

        let menu_background =
            is_menu.then(|| screens::slideshow(time, self.renderer.background_layers));

        let (camera_pos, player_feet, view_proj, fog_end, elapsed, underwater) =
            if let Some(game) = self.game.as_ref() {
                (
                    game.camera.position,
                    game.player.feet.as_vec3(),
                    game.camera.projection_matrix(game.aspect()) * game.camera.view_matrix(),
                    game.fog_end(),
                    game.elapsed(),
                    game.underwater(),
                )
            } else {
                (
                    glam::Vec3::new(0.0, 70.0, 0.0),
                    glam::Vec3::new(0.0, 70.0, 0.0),
                    glam::Mat4::IDENTITY,
                    100.0,
                    time,
                    false,
                )
            };

        let view = FrameView {
            camera: camera_pos,
            player_feet,
            player_height: goblock_input::player::HEIGHT as f32,
            view_projection: view_proj,
            fog_end,
            time: elapsed,
            underwater,
            blur_background: is_in_game_settings,
            menu_background,
        };

        let size = self.window.inner_size();
        let scale = ui::scale_for(size.height as f32);
        let fps = self.fps.fps();
        let mut ui_vertices = if mode == AppMode::Playing {
            let chunk_count = self.renderer.chunk_count();
            let lines = self
                .game
                .as_ref()
                .map(|g| g.hud_lines(chunk_count, fps))
                .unwrap_or_default();
            goblock_render::build_hud_vertices(&lines)
        } else {
            self.screens.vertices(
                fps,
                time,
                size.width as f32 / scale,
                size.height as f32 / scale,
            )
        };
        ui::scale_vertices(&mut ui_vertices, scale);

        self.renderer.draw_frame(&view, &ui_vertices)
    }

    fn fail(&mut self, event_loop: &ActiveEventLoop, e: impl std::fmt::Display) {
        eprintln!("goblock: {e}");
        event_loop.exit();
    }
}

/// Seed for the starter world: from `arg` when one is given, otherwise a fresh
/// random one.
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

fn random_seed() -> u64 {
    use std::time::SystemTime;
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    now as u64 ^ 0x517c_c1b7_2722_0a95
}
