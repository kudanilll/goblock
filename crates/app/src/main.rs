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
    settings: settings::Settings,
    settings_menu: settings::SettingsMenu,
    main_menu: menu::MainMenu,
    select_menu: menu::SelectWorldMenu,
    create_menu: menu::CreateWorldMenu,
    mode: menu::AppMode,
    current_world: Option<saves::WorldMeta>,
    saved_worlds: Vec<saves::WorldMeta>,
    mouse_pos: (f32, f32),
    start: std::time::Instant,
    last_frame: std::time::Instant,
    fps_accum: f32,
    fps_frames: u32,
    fps: f32,
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
            WindowEvent::CloseRequested => {
                state.save_current_world();
                event_loop.exit();
            }
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
                let size = state.window.inner_size();
                let (w, h) = (size.width as f32, size.height as f32);

                match state.mode {
                    menu::AppMode::Title => {
                        state.main_menu.handle_mouse_move(
                            state.mouse_pos.0,
                            state.mouse_pos.1,
                            w,
                            h,
                        );
                    }
                    menu::AppMode::SelectWorld => {
                        state.select_menu.handle_mouse_move(
                            &state.saved_worlds,
                            state.mouse_pos.0,
                            state.mouse_pos.1,
                            w,
                            h,
                        );
                    }
                    menu::AppMode::CreateWorld => {
                        state.create_menu.handle_mouse_move(
                            state.mouse_pos.0,
                            state.mouse_pos.1,
                            w,
                            h,
                        );
                    }
                    menu::AppMode::Settings { from_title } => {
                        state.settings_menu.handle_mouse_move(
                            &mut state.settings,
                            from_title,
                            state.mouse_pos.0,
                            state.mouse_pos.1,
                            w,
                            h,
                        );
                    }
                    menu::AppMode::Playing => {}
                }
            }
            WindowEvent::MouseInput {
                state: button_state,
                button: MouseButton::Left,
                ..
            } => {
                let size = state.window.inner_size();
                let (w, h) = (size.width as f32, size.height as f32);

                match state.mode {
                    menu::AppMode::Title => {
                        if button_state == ElementState::Pressed {
                            let action = state.main_menu.handle_mouse_down(
                                state.mouse_pos.0,
                                state.mouse_pos.1,
                                w,
                                h,
                            );
                            match action {
                                menu::MainMenuAction::OpenSelectWorld => {
                                    state.saved_worlds = saves::list_worlds();
                                    state.select_menu.selected = 0;
                                    state.mode = menu::AppMode::SelectWorld;
                                }
                                menu::MainMenuAction::OpenSettings => {
                                    state.mode = menu::AppMode::Settings { from_title: true };
                                }
                                menu::MainMenuAction::Quit => {
                                    event_loop.exit();
                                }
                                menu::MainMenuAction::None => {}
                            }
                        }
                    }
                    menu::AppMode::SelectWorld => {
                        if button_state == ElementState::Pressed {
                            let action = state.select_menu.handle_mouse_down(
                                &state.saved_worlds,
                                state.mouse_pos.0,
                                state.mouse_pos.1,
                                w,
                                h,
                            );
                            match action {
                                menu::SelectWorldAction::Play(meta) => {
                                    state.start_world(meta, event_loop);
                                }
                                menu::SelectWorldAction::OpenCreate => {
                                    state.create_menu = menu::CreateWorldMenu::new();
                                    state.mode = menu::AppMode::CreateWorld;
                                }
                                menu::SelectWorldAction::Delete(id) => {
                                    let _ = saves::delete_world(&id);
                                    state.saved_worlds = saves::list_worlds();
                                    if state.select_menu.selected >= state.saved_worlds.len() {
                                        state.select_menu.selected =
                                            state.saved_worlds.len().saturating_sub(1);
                                    }
                                }
                                menu::SelectWorldAction::Back => {
                                    state.mode = menu::AppMode::Title;
                                }
                                menu::SelectWorldAction::None => {}
                            }
                        }
                    }
                    menu::AppMode::CreateWorld => {
                        if button_state == ElementState::Pressed {
                            let action = state.create_menu.handle_mouse_down(
                                state.mouse_pos.0,
                                state.mouse_pos.1,
                                w,
                                h,
                            );
                            match action {
                                menu::CreateWorldAction::Create { name, seed } => {
                                    match saves::create_world(&name, &seed) {
                                        Ok(meta) => state.start_world(meta, event_loop),
                                        Err(e) => eprintln!("goblock: create world error: {e}"),
                                    }
                                }
                                menu::CreateWorldAction::Cancel => {
                                    state.saved_worlds = saves::list_worlds();
                                    state.mode = menu::AppMode::SelectWorld;
                                }
                                menu::CreateWorldAction::None => {}
                            }
                        }
                    }
                    menu::AppMode::Settings { from_title } => {
                        if button_state == ElementState::Pressed {
                            let action = state.settings_menu.handle_mouse_down(
                                &mut state.settings,
                                from_title,
                                state.mouse_pos.0,
                                state.mouse_pos.1,
                                w,
                                h,
                            );
                            match action {
                                settings::Action::Close => {
                                    if from_title {
                                        state.mode = menu::AppMode::Title;
                                    } else {
                                        state.mode = menu::AppMode::Playing;
                                        state.input.capture_cursor_grab(&state.window);
                                    }
                                }
                                settings::Action::ReturnToTitle => {
                                    state.save_and_quit_to_title();
                                }
                                settings::Action::Quit => {
                                    state.save_current_world();
                                    event_loop.exit();
                                }
                                settings::Action::None => {}
                            }
                        } else {
                            state.settings_menu.handle_mouse_up();
                        }
                    }
                    menu::AppMode::Playing => {}
                }
            }
            WindowEvent::RedrawRequested => match state.frame() {
                Ok(true) => state.window.request_redraw(),
                Ok(false) => {
                    state.save_current_world();
                    event_loop.exit();
                }
                Err(e) => state.fail(event_loop, e),
            },
            WindowEvent::KeyboardInput { event: key, .. }
                if key.state == ElementState::Pressed && !key.repeat =>
            {
                // Typing input into Create World input fields
                if state.mode == menu::AppMode::CreateWorld {
                    if let Some(text) = &key.text {
                        for ch in text.chars() {
                            state.create_menu.handle_char_input(ch);
                        }
                    }
                }

                if let PhysicalKey::Code(code) = key.physical_key {
                    if code == KeyCode::F11 {
                        let fullscreen = match state.window.fullscreen() {
                            Some(_) => None,
                            None => Some(Fullscreen::Borderless(None)),
                        };
                        state.window.set_fullscreen(fullscreen);
                        return;
                    }

                    match state.mode {
                        menu::AppMode::Title => {
                            let action = state.main_menu.handle_key_down(code);
                            match action {
                                menu::MainMenuAction::OpenSelectWorld => {
                                    state.saved_worlds = saves::list_worlds();
                                    state.select_menu.selected = 0;
                                    state.mode = menu::AppMode::SelectWorld;
                                }
                                menu::MainMenuAction::OpenSettings => {
                                    state.mode = menu::AppMode::Settings { from_title: true };
                                }
                                menu::MainMenuAction::Quit => {
                                    event_loop.exit();
                                }
                                menu::MainMenuAction::None => {}
                            }
                        }
                        menu::AppMode::SelectWorld => {
                            let action =
                                state.select_menu.handle_key_down(&state.saved_worlds, code);
                            match action {
                                menu::SelectWorldAction::Play(meta) => {
                                    state.start_world(meta, event_loop);
                                }
                                menu::SelectWorldAction::OpenCreate => {
                                    state.create_menu = menu::CreateWorldMenu::new();
                                    state.mode = menu::AppMode::CreateWorld;
                                }
                                menu::SelectWorldAction::Delete(id) => {
                                    let _ = saves::delete_world(&id);
                                    state.saved_worlds = saves::list_worlds();
                                    if state.select_menu.selected >= state.saved_worlds.len() {
                                        state.select_menu.selected =
                                            state.saved_worlds.len().saturating_sub(1);
                                    }
                                }
                                menu::SelectWorldAction::Back => {
                                    state.mode = menu::AppMode::Title;
                                }
                                menu::SelectWorldAction::None => {}
                            }
                        }
                        menu::AppMode::CreateWorld => {
                            let action = state.create_menu.handle_key_down(code);
                            match action {
                                menu::CreateWorldAction::Create { name, seed } => {
                                    match saves::create_world(&name, &seed) {
                                        Ok(meta) => state.start_world(meta, event_loop),
                                        Err(e) => eprintln!("goblock: create world error: {e}"),
                                    }
                                }
                                menu::CreateWorldAction::Cancel => {
                                    state.saved_worlds = saves::list_worlds();
                                    state.mode = menu::AppMode::SelectWorld;
                                }
                                menu::CreateWorldAction::None => {}
                            }
                        }
                        menu::AppMode::Playing => {
                            if code == KeyCode::Escape {
                                state.mode = menu::AppMode::Settings { from_title: false };
                                state.input.release_cursor_grab(&state.window);
                            }
                        }
                        menu::AppMode::Settings { from_title } => {
                            if code == KeyCode::Escape {
                                if from_title {
                                    state.mode = menu::AppMode::Title;
                                } else {
                                    state.mode = menu::AppMode::Playing;
                                    state.input.capture_cursor_grab(&state.window);
                                }
                            } else {
                                let action = state
                                    .settings_menu
                                    .handle_key_down(&mut state.settings, code);
                                match action {
                                    settings::Action::Close => {
                                        if from_title {
                                            state.mode = menu::AppMode::Title;
                                        } else {
                                            state.mode = menu::AppMode::Playing;
                                            state.input.capture_cursor_grab(&state.window);
                                        }
                                    }
                                    settings::Action::ReturnToTitle => {
                                        state.save_and_quit_to_title();
                                    }
                                    settings::Action::Quit => {
                                        state.save_current_world();
                                        event_loop.exit();
                                    }
                                    settings::Action::None => {}
                                }
                            }
                        }
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
            if state.mode == menu::AppMode::Playing {
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

        let settings = settings::Settings::default();
        let settings_menu = settings::SettingsMenu::new();
        let main_menu = menu::MainMenu::new();
        let select_menu = menu::SelectWorldMenu::new();
        let create_menu = menu::CreateWorldMenu::new();
        let saved_worlds = saves::list_worlds();

        // If saved worlds is empty, prefill a default world so player has something to test right away
        let saved_worlds = if saved_worlds.is_empty() {
            let _ = saves::create_world("Survival World", &seed.to_string());
            saves::list_worlds()
        } else {
            saved_worlds
        };

        Ok(Self {
            renderer,
            window,
            game: None,
            input,
            settings,
            settings_menu,
            main_menu,
            select_menu,
            create_menu,
            mode: menu::AppMode::Title,
            current_world: None,
            saved_worlds,
            mouse_pos: (0.0, 0.0),
            start: std::time::Instant::now(),
            last_frame: std::time::Instant::now(),
            fps_accum: 0.0,
            fps_frames: 0,
            fps: 0.0,
        })
    }

    fn start_world(&mut self, mut meta: saves::WorldMeta, event_loop: &ActiveEventLoop) {
        let size = self.window.inner_size();
        match game::Game::new(meta.seed) {
            Ok(mut game) => {
                game.set_screen_size(size);
                if meta.player_pos != [0.0, 0.0, 0.0] {
                    game.player.feet = glam::DVec3::new(
                        meta.player_pos[0] as f64,
                        meta.player_pos[1] as f64,
                        meta.player_pos[2] as f64,
                    );
                    game.camera.position = game.player.eye();
                    game.player.flying = meta.player_flying;
                    game.camera
                        .set_orientation(meta.player_yaw, meta.player_pitch);
                }
                meta.last_played = std::time::SystemTime::now()
                    .duration_since(std::time::SystemTime::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                let _ = saves::save_world(&meta);

                self.renderer.clear_chunks();
                self.current_world = Some(meta);
                self.game = Some(game);
                self.mode = menu::AppMode::Playing;
                self.input.capture_cursor_grab(&self.window);
            }
            Err(e) => {
                eprintln!("goblock: failed to start world: {e}");
                self.fail(event_loop, e);
            }
        }
    }

    fn save_current_world(&mut self) {
        if let (Some(game), Some(meta)) = (self.game.as_ref(), self.current_world.as_mut()) {
            meta.player_pos = [
                game.player.feet.x as f32,
                game.player.feet.y as f32,
                game.player.feet.z as f32,
            ];
            meta.player_yaw = game.camera.yaw();
            meta.player_pitch = game.camera.pitch();
            meta.player_flying = game.player.flying;
            meta.last_played = std::time::SystemTime::now()
                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let _ = saves::save_world(meta);
        }
    }

    fn save_and_quit_to_title(&mut self) {
        self.save_current_world();
        self.game = None;
        self.renderer.clear_chunks();
        self.current_world = None;
        self.saved_worlds = saves::list_worlds();
        self.mode = menu::AppMode::Title;
        self.input.release_cursor_grab(&self.window);
    }

    /// Run one frame. Returns `Ok(false)` once the player asks to quit.
    fn frame(&mut self) -> Result<bool, goblock_render::VkError> {
        let now = std::time::Instant::now();
        let delta = now - self.last_frame;
        self.last_frame = now;
        let dt = delta.as_secs_f32();

        if dt > 0.0 {
            self.fps_accum += dt;
            self.fps_frames += 1;
            if self.fps_accum >= 1.0 {
                self.fps = self.fps_frames as f32 / self.fps_accum;
                self.fps_accum = 0.0;
                self.fps_frames = 0;
            }
        }

        let size = self.window.inner_size();
        let screen_w = size.width as f32;
        let screen_h = size.height as f32;

        if self.mode == menu::AppMode::Playing {
            if let Some(game) = self.game.as_mut() {
                let mut input = self.input.poll();
                input.look_delta.x *= self.settings.sensitivity / 2.5;
                input.look_delta.y *= (self.settings.sensitivity / 2.5)
                    * (if self.settings.invert_y { -1.0 } else { 1.0 });

                game.update(dt, &input);

                for event in game.stream() {
                    match event {
                        WorldEvent::Meshed(pos, mesh) => self.renderer.upload_chunk(pos, &mesh)?,
                        WorldEvent::Unloaded(pos) => self.renderer.remove_chunk(pos),
                    }
                }
            }
        }

        let is_in_game_settings =
            matches!(self.mode, menu::AppMode::Settings { from_title: false });
        let is_menu = !matches!(self.mode, menu::AppMode::Playing) && !is_in_game_settings;

        let menu_background = if is_menu {
            let layers = self.renderer.background_layers.max(1);
            let time = self.start.elapsed().as_secs_f32();
            let slide_time = time / 8.0;
            let current = (slide_time.floor() as u32) % layers;
            let next = (current + 1) % layers;
            let frac = slide_time.fract();
            let crossfade_dur = 1.5 / 8.0;
            let blend = if frac > (1.0 - crossfade_dur) {
                (frac - (1.0 - crossfade_dur)) / crossfade_dur
            } else {
                0.0
            };
            Some((current, next, blend))
        } else {
            None
        };

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
                    self.start.elapsed().as_secs_f32(),
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

        let ui_vertices = match self.mode {
            menu::AppMode::Playing => {
                let chunk_count = self.renderer.chunk_count();
                let lines = self
                    .game
                    .as_ref()
                    .map(|g| g.hud_lines(chunk_count, self.fps))
                    .unwrap_or_default();
                goblock_render::build_hud_vertices(&lines)
            }
            menu::AppMode::Title => self.main_menu.build_vertices(self.fps, screen_w, screen_h),
            menu::AppMode::SelectWorld => {
                self.select_menu
                    .build_vertices(&self.saved_worlds, screen_w, screen_h)
            }
            menu::AppMode::CreateWorld => self.create_menu.build_vertices(
                screen_w,
                screen_h,
                self.start.elapsed().as_secs_f32(),
            ),
            menu::AppMode::Settings { from_title } => {
                self.settings_menu
                    .build_vertices(&self.settings, from_title, screen_w, screen_h)
            }
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

fn random_seed() -> u64 {
    use std::time::SystemTime;
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    now as u64 ^ 0x517c_c1b7_2722_0a95
}
