//! Which screen is open, and how input moves between screens.
//!
//! Screens answer each event with a [`Transition`]. [`Screens::apply`] does the
//! part that stays inside the menus, so screen flow can be tested without a
//! window. `ActiveState` in `main.rs` does the rest: files, the cursor and the
//! world.

use crate::menu::{CreateWorldMenu, MainMenu, SelectWorldMenu};
use crate::saves::WorldMeta;
use crate::settings::{Settings, SettingsMenu};
use goblock_render::HudVertex;
use winit::keyboard::KeyCode;

const SLIDE_SECONDS: f32 = 8.0;
const CROSSFADE_SECONDS: f32 = 1.5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMode {
    Title,
    SelectWorld,
    CreateWorld,
    Playing,
    Settings { from_title: bool },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Transition {
    None,
    OpenTitle,
    OpenWorldList,
    OpenCreateWorld,
    OpenSettings { from_title: bool },
    ResumeWorld,
    StartWorld(WorldMeta),
    CreateWorld { name: String, seed: String },
    DeleteWorld(String),
    LeaveWorld,
    Quit,
}

/// The menu state shared by every screen.
pub struct Screens {
    pub mode: AppMode,
    pub settings: Settings,
    worlds: Vec<WorldMeta>,
    title: MainMenu,
    select: SelectWorldMenu,
    create: CreateWorldMenu,
    settings_menu: SettingsMenu,
}

impl Screens {
    pub fn new(worlds: Vec<WorldMeta>) -> Self {
        Self {
            mode: AppMode::Title,
            settings: Settings::default(),
            worlds,
            title: MainMenu::new(),
            select: SelectWorldMenu::new(),
            create: CreateWorldMenu::new(),
            settings_menu: SettingsMenu::new(),
        }
    }

    /// Replaces the world list and keeps the selection inside it.
    pub fn set_worlds(&mut self, worlds: Vec<WorldMeta>) {
        self.worlds = worlds;
        let last = self.worlds.len().saturating_sub(1);
        self.select.selected = self.select.selected.min(last);
    }

    pub fn mouse_moved(&mut self, x: f32, y: f32, width: f32, height: f32) {
        match self.mode {
            AppMode::Title => self.title.handle_mouse_move(x, y, width, height),
            AppMode::SelectWorld => {
                self.select
                    .handle_mouse_move(&self.worlds, x, y, width, height)
            }
            AppMode::CreateWorld => self.create.handle_mouse_move(x, y, width, height),
            AppMode::Settings { from_title } => self.settings_menu.handle_mouse_move(
                &mut self.settings,
                from_title,
                x,
                y,
                width,
                height,
            ),
            AppMode::Playing => {}
        }
    }

    pub fn mouse_button(
        &mut self,
        pressed: bool,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    ) -> Transition {
        if !pressed {
            self.settings_menu.handle_mouse_up();
            return Transition::None;
        }
        match self.mode {
            AppMode::Title => self.title.handle_mouse_down(x, y, width, height),
            AppMode::SelectWorld => {
                self.select
                    .handle_mouse_down(&self.worlds, x, y, width, height)
            }
            AppMode::CreateWorld => self.create.handle_mouse_down(x, y, width, height),
            AppMode::Settings { from_title } => self.settings_menu.handle_mouse_down(
                &mut self.settings,
                from_title,
                x,
                y,
                width,
                height,
            ),
            AppMode::Playing => Transition::None,
        }
    }

    pub fn wheel(&mut self, lines: f32, height: f32) {
        if self.mode == AppMode::SelectWorld {
            self.select.handle_wheel(&self.worlds, lines, height);
        }
    }

    pub fn key_down(&mut self, key: KeyCode, height: f32) -> Transition {
        match self.mode {
            AppMode::Title => self.title.handle_key_down(key),
            AppMode::SelectWorld => self.select.handle_key_down(&self.worlds, key, height),
            AppMode::CreateWorld => self.create.handle_key_down(key),
            AppMode::Settings { from_title } => {
                self.settings_menu
                    .handle_key_down(&mut self.settings, from_title, key)
            }
            AppMode::Playing if key == KeyCode::Escape => {
                Transition::OpenSettings { from_title: false }
            }
            AppMode::Playing => Transition::None,
        }
    }

    pub fn typed(&mut self, text: &str) {
        if self.mode == AppMode::CreateWorld {
            for ch in text.chars() {
                self.create.handle_char_input(ch);
            }
        }
    }

    /// Moves between screens. Transitions that need files, the cursor or the
    /// world change nothing here.
    pub fn apply(&mut self, transition: &Transition) {
        match transition {
            Transition::OpenTitle | Transition::LeaveWorld => self.mode = AppMode::Title,
            Transition::OpenWorldList => {
                if self.mode == AppMode::Title {
                    self.select = SelectWorldMenu::new();
                }
                self.mode = AppMode::SelectWorld;
            }
            Transition::OpenCreateWorld => {
                self.create = CreateWorldMenu::new();
                self.mode = AppMode::CreateWorld;
            }
            Transition::OpenSettings { from_title } => {
                self.mode = AppMode::Settings {
                    from_title: *from_title,
                };
            }
            Transition::ResumeWorld => self.mode = AppMode::Playing,
            Transition::None
            | Transition::StartWorld(_)
            | Transition::CreateWorld { .. }
            | Transition::DeleteWorld(_)
            | Transition::Quit => {}
        }
    }

    /// The open menu at UI scale. Empty while playing.
    pub fn vertices(&self, fps: f32, time: f32, width: f32, height: f32) -> Vec<HudVertex> {
        match self.mode {
            AppMode::Title => self.title.build_vertices(fps, width, height),
            AppMode::SelectWorld => self.select.build_vertices(&self.worlds, width, height),
            AppMode::CreateWorld => self.create.build_vertices(width, height, time),
            AppMode::Settings { from_title } => {
                self.settings_menu
                    .build_vertices(&self.settings, from_title, width, height)
            }
            AppMode::Playing => Vec::new(),
        }
    }
}

/// Layers and blend for the menu slideshow: each image holds, then the end of
/// its slot crossfades into the next one.
pub fn slideshow(time: f32, layers: u32) -> (u32, u32, f32) {
    let layers = layers.max(1);
    let slide_time = time / SLIDE_SECONDS;
    let current = (slide_time.floor() as u32) % layers;
    let next = (current + 1) % layers;
    let frac = slide_time.fract();
    let crossfade = CROSSFADE_SECONDS / SLIDE_SECONDS;
    let blend = if frac > 1.0 - crossfade {
        (frac - (1.0 - crossfade)) / crossfade
    } else {
        0.0
    };
    (current, next, blend)
}

#[cfg(test)]
mod tests {
    use super::*;

    const W: f32 = 800.0;
    const H: f32 = 600.0;

    fn screens_with_worlds(count: usize) -> Screens {
        Screens::new(
            (0..count)
                .map(|i| WorldMeta::new(format!("w{i}"), format!("World {i}"), i as u64))
                .collect(),
        )
    }

    /// Presses a key and applies what the screen answered.
    fn press(screens: &mut Screens, key: KeyCode) -> Transition {
        let transition = screens.key_down(key, H);
        screens.apply(&transition);
        transition
    }

    #[test]
    fn title_opens_the_world_list_and_escape_comes_back() {
        let mut screens = screens_with_worlds(2);
        press(&mut screens, KeyCode::Enter);
        assert_eq!(screens.mode, AppMode::SelectWorld);
        press(&mut screens, KeyCode::Escape);
        assert_eq!(screens.mode, AppMode::Title);
    }

    #[test]
    fn world_list_resets_when_entered_from_the_title() {
        let mut screens = screens_with_worlds(3);
        press(&mut screens, KeyCode::Enter);
        press(&mut screens, KeyCode::ArrowDown);
        assert_eq!(screens.select.selected, 1);
        press(&mut screens, KeyCode::Escape);
        press(&mut screens, KeyCode::Enter);
        assert_eq!(screens.select.selected, 0);
    }

    #[test]
    fn cancelling_a_new_world_keeps_the_selection() {
        let mut screens = screens_with_worlds(3);
        press(&mut screens, KeyCode::Enter);
        press(&mut screens, KeyCode::ArrowDown);
        screens.apply(&Transition::OpenCreateWorld);
        assert_eq!(screens.mode, AppMode::CreateWorld);

        assert_eq!(
            press(&mut screens, KeyCode::Escape),
            Transition::OpenWorldList
        );
        assert_eq!(screens.mode, AppMode::SelectWorld);
        assert_eq!(screens.select.selected, 1);
    }

    #[test]
    fn settings_from_the_title_return_to_the_title() {
        let mut screens = screens_with_worlds(0);
        press(&mut screens, KeyCode::KeyS);
        assert_eq!(screens.mode, AppMode::Settings { from_title: true });
        assert_eq!(press(&mut screens, KeyCode::Escape), Transition::OpenTitle);
        assert_eq!(screens.mode, AppMode::Title);
    }

    #[test]
    fn escape_in_a_world_toggles_settings() {
        let mut screens = screens_with_worlds(1);
        screens.mode = AppMode::Playing;
        press(&mut screens, KeyCode::Escape);
        assert_eq!(screens.mode, AppMode::Settings { from_title: false });
        assert_eq!(
            press(&mut screens, KeyCode::Escape),
            Transition::ResumeWorld
        );
        assert_eq!(screens.mode, AppMode::Playing);
    }

    #[test]
    fn other_keys_do_nothing_in_a_world() {
        let mut screens = screens_with_worlds(1);
        screens.mode = AppMode::Playing;
        assert_eq!(press(&mut screens, KeyCode::KeyS), Transition::None);
        assert_eq!(screens.mode, AppMode::Playing);
    }

    #[test]
    fn enter_starts_the_selected_world_without_leaving_the_menu_yet() {
        let mut screens = screens_with_worlds(2);
        press(&mut screens, KeyCode::Enter);
        press(&mut screens, KeyCode::ArrowDown);
        let transition = press(&mut screens, KeyCode::Enter);
        assert!(matches!(&transition, Transition::StartWorld(w) if w.id == "w1"));
        assert_eq!(screens.mode, AppMode::SelectWorld);
    }

    #[test]
    fn typing_only_reaches_the_new_world_screen() {
        let mut screens = screens_with_worlds(1);
        screens.typed("x");
        assert_eq!(screens.create.name_input, "New World");
        screens.apply(&Transition::OpenCreateWorld);
        screens.typed("!");
        assert_eq!(screens.create.name_input, "New World!");
    }

    #[test]
    fn shrinking_the_world_list_clamps_the_selection() {
        let mut screens = screens_with_worlds(3);
        press(&mut screens, KeyCode::Enter);
        press(&mut screens, KeyCode::ArrowDown);
        press(&mut screens, KeyCode::ArrowDown);
        let keep = screens.worlds[..1].to_vec();
        screens.set_worlds(keep);
        assert_eq!(screens.select.selected, 0);
    }

    #[test]
    fn mouse_release_ends_a_slider_drag_on_any_screen() {
        let mut screens = screens_with_worlds(1);
        assert_eq!(
            screens.mouse_button(false, 0.0, 0.0, W, H),
            Transition::None
        );
    }

    #[test]
    fn slideshow_holds_then_crossfades() {
        assert_eq!(slideshow(0.0, 3), (0, 1, 0.0));
        assert_eq!(slideshow(6.0, 3), (0, 1, 0.0));
        let (current, next, blend) = slideshow(7.25, 3);
        assert_eq!((current, next), (0, 1));
        assert!((blend - 0.5).abs() < 1e-3, "blend {blend}");
    }

    #[test]
    fn slideshow_wraps_and_survives_no_layers() {
        assert_eq!(slideshow(16.0, 3).0, 2);
        assert_eq!(slideshow(16.0, 3).1, 0);
        assert_eq!(slideshow(24.0, 3).0, 0);
        assert_eq!(slideshow(5.0, 0), (0, 0, 0.0));
    }
}
