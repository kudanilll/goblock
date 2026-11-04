//! In-game settings data model, scalable tabs, and pixelated UI renderer.

use std::fs;
use std::io::Write;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::screens::Transition;
use crate::ui::{self, Rect, Rgba, Widget};
use goblock_render::{draw_text, measure_text, push_rect, FontKind, HudVertex};

pub const SETTINGS_FILE: &str = "settings.json";

const WHITE: Rgba = [255, 255, 255, 255];
const GREEN: Rgba = [80, 230, 160, 255];
const CONTROL_FILL: Rgba = [0, 0, 0, 120];
const CONTROL_BORDER: Rgba = [55, 65, 80, 160];
const PILL_BORDER: Rgba = [65, 75, 95, 160];

const TOP_BAR_HEIGHT: f32 = 60.0;
const TABS_X: f32 = 240.0;
const TABS_Y: f32 = 16.0;
const LIST_TOP: f32 = 85.0;
const LIST_X: f32 = 50.0;
const ROW_HEIGHT: f32 = 36.0;
const ROW_STEP: f32 = 40.0;
const CATEGORY_HEIGHT: f32 = 28.0;
const BOTTOM_BAR_HEIGHT: f32 = 48.0;
/// A slider starts dragging this far either side of its track.
const TRACK_GRAB_MARGIN: f32 = 10.0;

/// Core game settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    // Video
    pub fov: f32,
    pub view_distance: i32,
    pub reflections: bool,
    pub shadows: bool,
    pub ambient_occlusion: bool,

    // Controls
    pub sensitivity: f32,
    pub invert_y: bool,

    // Audio
    pub master_volume: f32,
    pub music_volume: f32,
    pub sfx_volume: f32,
}

impl Settings {
    /// Load settings from the given path, falling back to default if missing or unparseable.
    pub fn load_from(path: &Path) -> Self {
        if let Ok(content) = fs::read_to_string(path) {
            match serde_json::from_str::<Self>(&content) {
                Ok(settings) => return settings,
                Err(e) => eprintln!("goblock: failed to parse settings {}: {e}", path.display()),
            }
        }
        Self::default()
    }

    /// Load settings from the default settings.json file.
    pub fn load() -> Self {
        Self::load_from(Path::new(SETTINGS_FILE))
    }

    /// Save settings atomically to the given path.
    pub fn save_to(&self, path: &Path) -> std::io::Result<()> {
        let tmp_string = format!("{}.tmp", path.display());
        let tmp_path = Path::new(&tmp_string);
        let json = serde_json::to_string_pretty(self)?;
        {
            let mut file = fs::File::create(tmp_path)?;
            file.write_all(json.as_bytes())?;
            file.sync_all()?;
        }
        fs::rename(tmp_path, path)
    }

    /// Save settings atomically to the default settings.json file.
    pub fn save(&self) -> std::io::Result<()> {
        self.save_to(Path::new(SETTINGS_FILE))
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            fov: 70.0,
            view_distance: 16,
            reflections: true,
            shadows: true,
            ambient_occlusion: true,
            sensitivity: 2.5,
            invert_y: false,
            master_volume: 1.0,
            music_volume: 0.8,
            sfx_volume: 1.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Video,
    Controls,
    Audio,
}

impl Tab {
    pub const ALL: [Tab; 3] = [Tab::Video, Tab::Controls, Tab::Audio];

    pub fn name(self) -> &'static str {
        match self {
            Tab::Video => "Video",
            Tab::Controls => "Controls",
            Tab::Audio => "Audio",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Tab::Video => Tab::Controls,
            Tab::Controls => Tab::Audio,
            Tab::Audio => Tab::Video,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Tab::Video => Tab::Audio,
            Tab::Controls => Tab::Video,
            Tab::Audio => Tab::Controls,
        }
    }
}

#[derive(Debug, Clone)]
pub enum SettingKind {
    Slider { min: f32, max: f32, step: f32 },
    Toggle,
    Keybind { key: &'static str },
}

#[derive(Debug, Clone)]
pub struct SettingDef {
    pub id: &'static str,
    pub category: &'static str,
    pub label: &'static str,
    pub description: &'static str,
    pub kind: SettingKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SettingsItem {
    Back,
    TitleMenu,
    Quit,
    Tab(Tab),
    Left(usize),
    Right(usize),
    Row(usize),
}

/// One setting row, and where its category heading goes if it starts one.
struct RowSlot {
    heading_y: Option<f32>,
    rect: Rect,
}

/// The controls inside a row. `middle` is a slider track or a value box.
struct RowParts {
    left: Rect,
    middle: Rect,
    right: Rect,
    value: Rect,
}

impl RowParts {
    fn of(row: Rect) -> Self {
        let x = row.x + row.w - 280.0;
        let y = row.y + 6.0;
        Self {
            left: Rect::new(x, y, 24.0, 24.0),
            middle: Rect::new(x + 34.0, y, 140.0, 24.0),
            right: Rect::new(x + 184.0, y, 24.0, 24.0),
            value: Rect::new(x + 216.0, y, 48.0, 24.0),
        }
    }
}

pub struct SettingsMenu {
    pub active_tab: Tab,
    pub selected_row: usize,
    hovered: Option<SettingsItem>,
    dragging_slider: Option<usize>,
}

impl SettingsMenu {
    pub fn new() -> Self {
        Self {
            active_tab: Tab::Video,
            selected_row: 0,
            hovered: None,
            dragging_slider: None,
        }
    }

    pub fn items_for_tab(&self, tab: Tab) -> &'static [SettingDef] {
        match tab {
            Tab::Video => &[
                SettingDef {
                    id: "fov",
                    category: "DISPLAY",
                    label: "Field of View",
                    description: "Adjust camera FOV angle for broader peripheral vision.",
                    kind: SettingKind::Slider {
                        min: 60.0,
                        max: 110.0,
                        step: 5.0,
                    },
                },
                SettingDef {
                    id: "view_distance",
                    category: "DISPLAY",
                    label: "View Distance",
                    description:
                        "Chunk streaming radius: Distance in chunks rendered around player.",
                    kind: SettingKind::Slider {
                        min: 8.0,
                        max: 24.0,
                        step: 1.0,
                    },
                },
                SettingDef {
                    id: "reflections",
                    category: "GRAPHICS",
                    label: "Water Reflections",
                    description: "Screen-space raymarched reflections over lakes and oceans.",
                    kind: SettingKind::Toggle,
                },
                SettingDef {
                    id: "shadows",
                    category: "GRAPHICS",
                    label: "Sunlight Shadows",
                    description: "Direct sunlight shadow map for terrain, trees, and grass.",
                    kind: SettingKind::Toggle,
                },
                SettingDef {
                    id: "ambient_occlusion",
                    category: "GRAPHICS",
                    label: "Ambient Occlusion",
                    description: "Screen-space contact shadows and crevice darkening.",
                    kind: SettingKind::Toggle,
                },
            ],
            Tab::Controls => &[
                SettingDef {
                    id: "sens",
                    category: "MOUSE SETTINGS",
                    label: "Horizontal Sensitivity",
                    description: "Controls rotation speed and responsiveness of camera look.",
                    kind: SettingKind::Slider {
                        min: 0.5,
                        max: 5.0,
                        step: 0.1,
                    },
                },
                SettingDef {
                    id: "invert_y",
                    category: "MOUSE SETTINGS",
                    label: "Invert Vertical Look",
                    description: "Inverts up/down pitch axis for flight or inverted preferences.",
                    kind: SettingKind::Toggle,
                },
                SettingDef {
                    id: "k_forward",
                    category: "KEYBOARD",
                    label: "Move Forward",
                    description: "Walk or fly forward in the facing direction.",
                    kind: SettingKind::Keybind { key: "W" },
                },
                SettingDef {
                    id: "k_backward",
                    category: "KEYBOARD",
                    label: "Move Backward",
                    description: "Walk or fly backwards.",
                    kind: SettingKind::Keybind { key: "S" },
                },
                SettingDef {
                    id: "k_jump",
                    category: "KEYBOARD",
                    label: "Jump / Fly Up",
                    description: "Jump on ground, or fly upward. Double tap space toggles fly.",
                    kind: SettingKind::Keybind { key: "Space Bar" },
                },
                SettingDef {
                    id: "k_sprint",
                    category: "KEYBOARD",
                    label: "Sprint",
                    description: "Sprint faster across terrain.",
                    kind: SettingKind::Keybind { key: "Left Shift" },
                },
                SettingDef {
                    id: "k_crouch",
                    category: "KEYBOARD",
                    label: "Crouch / Fly Down",
                    description: "Crouch or descend while flying.",
                    kind: SettingKind::Keybind { key: "C / Ctrl" },
                },
            ],
            Tab::Audio => &[
                SettingDef {
                    id: "master_vol",
                    category: "VOLUME",
                    label: "Master Volume",
                    description: "Overall master volume level.",
                    kind: SettingKind::Slider {
                        min: 0.0,
                        max: 1.0,
                        step: 0.05,
                    },
                },
                SettingDef {
                    id: "music_vol",
                    category: "VOLUME",
                    label: "Music Volume",
                    description: "Background music and ambient atmospheric tracks volume.",
                    kind: SettingKind::Slider {
                        min: 0.0,
                        max: 1.0,
                        step: 0.05,
                    },
                },
                SettingDef {
                    id: "sfx_vol",
                    category: "VOLUME",
                    label: "Sound Effects",
                    description: "Player footsteps, blocks, and environmental effects volume.",
                    kind: SettingKind::Slider {
                        min: 0.0,
                        max: 1.0,
                        step: 0.05,
                    },
                },
            ],
        }
    }

    pub fn current_items(&self) -> &'static [SettingDef] {
        self.items_for_tab(self.active_tab)
    }

    pub fn active_index(&self) -> usize {
        self.hovered_row()
            .unwrap_or(self.selected_row)
            .min(self.current_items().len().saturating_sub(1))
    }

    fn hovered_row(&self) -> Option<usize> {
        match self.hovered {
            Some(SettingsItem::Left(row) | SettingsItem::Right(row) | SettingsItem::Row(row)) => {
                Some(row)
            }
            _ => None,
        }
    }

    pub fn adjust_item(&self, settings: &mut Settings, index: usize, direction: f32) {
        let items = self.current_items();
        if index >= items.len() {
            return;
        }
        let item = &items[index];
        match &item.kind {
            SettingKind::Slider { min, max, step } => match item.id {
                "fov" => settings.fov = (settings.fov + direction * step).clamp(*min, *max),
                "view_distance" => {
                    settings.view_distance = (settings.view_distance + (direction * step) as i32)
                        .clamp(*min as i32, *max as i32)
                }
                "sens" => {
                    settings.sensitivity =
                        (settings.sensitivity + direction * step).clamp(*min, *max)
                }
                "master_vol" => {
                    settings.master_volume =
                        (settings.master_volume + direction * step).clamp(*min, *max)
                }
                "music_vol" => {
                    settings.music_volume =
                        (settings.music_volume + direction * step).clamp(*min, *max)
                }
                "sfx_vol" => {
                    settings.sfx_volume = (settings.sfx_volume + direction * step).clamp(*min, *max)
                }
                _ => {}
            },
            SettingKind::Toggle => match item.id {
                "reflections" => settings.reflections = !settings.reflections,
                "shadows" => settings.shadows = !settings.shadows,
                "ambient_occlusion" => settings.ambient_occlusion = !settings.ambient_occlusion,
                "invert_y" => settings.invert_y = !settings.invert_y,
                _ => {}
            },
            SettingKind::Keybind { .. } => {}
        }
    }

    fn close(from_title: bool) -> Transition {
        if from_title {
            Transition::OpenTitle
        } else {
            Transition::ResumeWorld
        }
    }

    pub fn handle_key_down(
        &mut self,
        settings: &mut Settings,
        from_title: bool,
        key: winit::keyboard::KeyCode,
    ) -> Transition {
        use winit::keyboard::KeyCode;
        let count = self.current_items().len();
        match key {
            KeyCode::Escape => return Self::close(from_title),
            KeyCode::ArrowUp if count > 0 => {
                self.selected_row = (self.selected_row + count - 1) % count;
            }
            KeyCode::ArrowDown if count > 0 => {
                self.selected_row = (self.selected_row + 1) % count;
            }
            KeyCode::ArrowLeft => self.adjust_item(settings, self.active_index(), -1.0),
            KeyCode::ArrowRight | KeyCode::Enter => {
                self.adjust_item(settings, self.active_index(), 1.0)
            }
            KeyCode::Tab | KeyCode::KeyE => {
                self.active_tab = self.active_tab.next();
                self.selected_row = 0;
            }
            KeyCode::KeyQ => {
                self.active_tab = self.active_tab.prev();
                self.selected_row = 0;
            }
            _ => {}
        }
        Transition::None
    }

    fn row_slots(&self, width: f32) -> Vec<RowSlot> {
        let row_width = (width - 100.0).min(760.0);
        let mut y = LIST_TOP;
        let mut category = "";
        self.current_items()
            .iter()
            .map(|item| {
                let mut heading_y = None;
                if item.category != category {
                    category = item.category;
                    heading_y = Some(y);
                    y += CATEGORY_HEIGHT;
                }
                let rect = Rect::new(LIST_X, y, row_width, ROW_HEIGHT);
                y += ROW_STEP;
                RowSlot { heading_y, rect }
            })
            .collect()
    }

    fn tab_strip() -> Vec<Widget<SettingsItem>> {
        let mut x = TABS_X;
        Tab::ALL
            .iter()
            .map(|&tab| {
                let name = tab.name().to_uppercase();
                let w = measure_text(FontKind::Headline, &name, 0.75) + 32.0;
                let rect = Rect::new(x, TABS_Y, w, 30.0);
                x += w + 12.0;
                Widget {
                    rect,
                    action: SettingsItem::Tab(tab),
                }
            })
            .collect()
    }

    /// Bottom buttons and tabs come first, so they win over a row in a window
    /// too short for the list.
    fn widgets(&self, from_title: bool, width: f32, height: f32) -> Vec<Widget<SettingsItem>> {
        let y = height - BOTTOM_BAR_HEIGHT + 8.0;
        let back_w = 50.0 + measure_text(FontKind::Regular, "BACK", 0.95);
        let mut widgets = vec![Widget {
            rect: Rect::new(40.0, y, back_w, 28.0),
            action: SettingsItem::Back,
        }];
        if !from_title {
            widgets.push(Widget {
                rect: Rect::new(160.0, y, 140.0, 28.0),
                action: SettingsItem::TitleMenu,
            });
        }
        let quit_w = measure_text(FontKind::Headline, "QUIT GAME", 0.75) + 36.0;
        widgets.push(Widget {
            rect: Rect::new(width - quit_w - 30.0, y, quit_w, 32.0),
            action: SettingsItem::Quit,
        });
        widgets.extend(Self::tab_strip());

        for (index, slot) in self.row_slots(width).iter().enumerate() {
            let parts = RowParts::of(slot.rect);
            widgets.push(Widget {
                rect: parts.left,
                action: SettingsItem::Left(index),
            });
            widgets.push(Widget {
                rect: parts.right,
                action: SettingsItem::Right(index),
            });
            widgets.push(Widget {
                rect: slot.rect,
                action: SettingsItem::Row(index),
            });
        }
        widgets
    }

    pub fn handle_mouse_move(
        &mut self,
        settings: &mut Settings,
        from_title: bool,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    ) {
        self.hovered = ui::hit(&self.widgets(from_title, width, height), x, y);
        if let Some(row) = self.hovered_row() {
            self.selected_row = row;
        }

        let Some(row) = self.dragging_slider else {
            return;
        };
        let slots = self.row_slots(width);
        if let (Some(slot), Some(item)) = (slots.get(row), self.current_items().get(row)) {
            if let SettingKind::Slider { min, max, .. } = item.kind {
                let track = RowParts::of(slot.rect).middle;
                let frac = ((x - track.x) / track.w).clamp(0.0, 1.0);
                set_slider(settings, item.id, min + frac * (max - min));
            }
        }
    }

    pub fn handle_mouse_down(
        &mut self,
        settings: &mut Settings,
        from_title: bool,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    ) -> Transition {
        self.handle_mouse_move(settings, from_title, x, y, width, height);

        match self.hovered {
            Some(SettingsItem::Back) => return Self::close(from_title),
            Some(SettingsItem::TitleMenu) => return Transition::LeaveWorld,
            Some(SettingsItem::Quit) => return Transition::Quit,
            Some(SettingsItem::Tab(tab)) => {
                self.active_tab = tab;
                self.selected_row = 0;
            }
            Some(SettingsItem::Left(row)) => self.adjust_item(settings, row, -1.0),
            Some(SettingsItem::Right(row)) => self.adjust_item(settings, row, 1.0),
            Some(SettingsItem::Row(row)) => match self.current_items()[row].kind {
                SettingKind::Slider { .. } => {
                    let track = RowParts::of(self.row_slots(width)[row].rect).middle;
                    let reach = track.x - TRACK_GRAB_MARGIN..=track.x + track.w + TRACK_GRAB_MARGIN;
                    if reach.contains(&x) {
                        self.dragging_slider = Some(row);
                        self.handle_mouse_move(settings, from_title, x, y, width, height);
                    }
                }
                SettingKind::Toggle => self.adjust_item(settings, row, 1.0),
                SettingKind::Keybind { .. } => {}
            },
            None => {}
        }
        Transition::None
    }

    pub fn handle_mouse_up(&mut self) {
        self.dragging_slider = None;
    }

    pub fn build_vertices(
        &self,
        settings: &Settings,
        from_title: bool,
        width: f32,
        height: f32,
    ) -> Vec<HudVertex> {
        let mut out = Vec::with_capacity(8192);

        // A light backdrop, so the world stays visible behind the menu.
        ui::fill(&mut out, Rect::new(0.0, 0.0, width, height), [0, 0, 0, 75]);

        self.draw_top_bar(&mut out, width);

        let items = self.current_items();
        for (index, (item, slot)) in items.iter().zip(self.row_slots(width)).enumerate() {
            self.draw_row(&mut out, settings, item, index, &slot);
        }

        if let Some(item) = items.get(self.active_index()) {
            draw_text(
                &mut out,
                FontKind::Regular,
                &format!("* {}", item.description),
                LIST_X,
                height - 80.0,
                0.95,
                [180, 225, 240, 255],
            );
        }

        ui::action_bar(
            &mut out,
            width,
            height,
            BOTTOM_BAR_HEIGHT,
            [0, 0, 0, 130],
            [45, 50, 65, 140],
        );
        for widget in self.widgets(from_title, width, height) {
            self.draw_bottom_button(&mut out, &widget);
        }

        out
    }

    fn draw_top_bar(&self, out: &mut Vec<HudVertex>, width: f32) {
        ui::header_bar(
            out,
            width,
            TOP_BAR_HEIGHT,
            [0, 0, 0, 115],
            [45, 50, 65, 140],
        );
        draw_text(
            out,
            FontKind::Headline,
            "SETTINGS",
            40.0,
            TABS_Y,
            1.0,
            WHITE,
        );

        key_pill(out, TABS_X - 38.0, "Q");
        let tabs = Self::tab_strip();
        for widget in &tabs {
            if let SettingsItem::Tab(tab) = widget.action {
                self.draw_tab(out, tab, widget.rect);
            }
        }
        if let Some(last) = tabs.last() {
            key_pill(out, last.rect.x + last.rect.w + 12.0, "E");
        }
    }

    fn draw_tab(&self, out: &mut Vec<HudVertex>, tab: Tab, r: Rect) {
        let name = tab.name().to_uppercase();
        if self.active_tab == tab {
            ui::banded(
                out,
                r,
                [80, 230, 160, 240],
                [150, 255, 200, 255],
                [40, 160, 100, 255],
            );
            ui::centered_text(
                out,
                FontKind::Headline,
                &name,
                r,
                5.0,
                0.75,
                [15, 25, 20, 255],
            );
        } else {
            let bg = if self.hovered == Some(SettingsItem::Tab(tab)) {
                [35, 45, 60, 180]
            } else {
                [0, 0, 0, 90]
            };
            ui::outlined(out, r, bg, [50, 60, 75, 140]);
            ui::centered_text(
                out,
                FontKind::Headline,
                &name,
                r,
                5.0,
                0.75,
                [170, 185, 205, 230],
            );
        }
    }

    fn draw_row(
        &self,
        out: &mut Vec<HudVertex>,
        settings: &Settings,
        item: &SettingDef,
        index: usize,
        slot: &RowSlot,
    ) {
        let r = slot.rect;
        if let Some(y) = slot.heading_y {
            draw_text(
                out,
                FontKind::Headline,
                item.category,
                r.x + 12.0,
                y + 4.0,
                0.75,
                [60, 215, 240, 255],
            );
        }

        let lit = self.selected_row == index || self.hovered_row() == Some(index);
        if lit {
            ui::fill(out, r, [30, 48, 70, 140]);
            ui::fill(out, Rect::new(r.x, r.y, 3.0, r.h), GREEN);
            ui::fill(out, Rect::new(r.x, r.y, r.w, 1.0), [50, 75, 105, 140]);
            ui::fill(
                out,
                Rect::new(r.x, r.y + r.h - 1.0, r.w, 1.0),
                [50, 75, 105, 140],
            );
        }

        let label_color = if lit { WHITE } else { [185, 195, 210, 230] };
        draw_text(
            out,
            FontKind::Regular,
            item.label,
            r.x + 16.0,
            r.y + 8.0,
            1.0,
            label_color,
        );

        let arrow_color = if lit {
            [240, 245, 255, 255]
        } else {
            [140, 150, 170, 180]
        };
        let parts = RowParts::of(r);
        arrow_button(out, parts.left, "<", arrow_color);

        match item.kind {
            SettingKind::Slider { min, max, .. } => {
                let value = slider_value(settings, item.id).unwrap_or(min);
                draw_slider(out, &parts, value, min, max, arrow_color);
            }
            SettingKind::Toggle => {
                draw_toggle(out, &parts, toggle_value(settings, item.id));
                arrow_button(out, parts.right, ">", arrow_color);
            }
            SettingKind::Keybind { key } => {
                ui::outlined(out, parts.middle, CONTROL_FILL, CONTROL_BORDER);
                ui::centered_text(out, FontKind::Regular, key, parts.middle, 3.0, 0.95, WHITE);
                arrow_button(out, parts.right, ">", arrow_color);
            }
        }
    }

    fn draw_bottom_button(&self, out: &mut Vec<HudVertex>, widget: &Widget<SettingsItem>) {
        let r = widget.rect;
        let hovered = self.hovered == Some(widget.action);
        match widget.action {
            SettingsItem::Back => {
                let bg = if hovered {
                    [60, 75, 95, 240]
                } else {
                    [0, 0, 0, 150]
                };
                ui::outlined(out, Rect::new(r.x, r.y, 40.0, r.h), bg, [80, 95, 120, 200]);
                draw_text(
                    out,
                    FontKind::Regular,
                    "ESC",
                    r.x + 7.0,
                    r.y + 5.0,
                    0.85,
                    WHITE,
                );
                draw_text(
                    out,
                    FontKind::Regular,
                    "BACK",
                    r.x + 50.0,
                    r.y + 5.0,
                    0.95,
                    [210, 220, 235, 255],
                );
            }
            SettingsItem::TitleMenu => {
                let (bg, border) = if hovered {
                    ([40, 55, 80, 230], [80, 160, 240, 255])
                } else {
                    ([20, 28, 42, 170], [50, 65, 90, 180])
                };
                ui::outlined(out, r, bg, border);
                draw_text(
                    out,
                    FontKind::Headline,
                    "TITLE MENU",
                    r.x + 18.0,
                    r.y + 5.0,
                    0.65,
                    WHITE,
                );
            }
            SettingsItem::Quit => {
                let bg = if hovered {
                    [220, 45, 45, 240]
                } else {
                    [150, 35, 35, 200]
                };
                ui::panel(out, r, bg, [255, 120, 120, 255], [100, 20, 20, 255], 1.0);
                draw_text(
                    out,
                    FontKind::Headline,
                    "QUIT GAME",
                    r.x + 18.0,
                    r.y + 6.0,
                    0.75,
                    WHITE,
                );
            }
            _ => {}
        }
    }
}

fn slider_value(settings: &Settings, id: &str) -> Option<f32> {
    match id {
        "fov" => Some(settings.fov),
        "view_distance" => Some(settings.view_distance as f32),
        "sens" => Some(settings.sensitivity),
        "master_vol" => Some(settings.master_volume),
        "music_vol" => Some(settings.music_volume),
        "sfx_vol" => Some(settings.sfx_volume),
        _ => None,
    }
}

/// Stores a dragged value, rounded to the steps each slider moves in.
fn set_slider(settings: &mut Settings, id: &str, value: f32) {
    match id {
        "fov" => settings.fov = (value / 5.0).round() * 5.0,
        "view_distance" => settings.view_distance = value.round() as i32,
        "sens" => settings.sensitivity = (value * 10.0).round() / 10.0,
        "master_vol" => settings.master_volume = (value * 20.0).round() / 20.0,
        "music_vol" => settings.music_volume = (value * 20.0).round() / 20.0,
        "sfx_vol" => settings.sfx_volume = (value * 20.0).round() / 20.0,
        _ => {}
    }
}

fn toggle_value(settings: &Settings, id: &str) -> bool {
    match id {
        "reflections" => settings.reflections,
        "shadows" => settings.shadows,
        "ambient_occlusion" => settings.ambient_occlusion,
        "invert_y" => settings.invert_y,
        _ => false,
    }
}

fn value_text(value: f32, min: f32, max: f32) -> String {
    if max <= 1.0 {
        format!("{:.0}%", value * 100.0)
    } else if (max - min) > 20.0 {
        format!("{:.0}", value)
    } else {
        format!("{:.1}", value)
    }
}

/// The small key hint beside the tabs.
fn key_pill(out: &mut Vec<HudVertex>, x: f32, key: &str) {
    let y = TABS_Y + 3.0;
    ui::outlined(out, Rect::new(x, y, 26.0, 24.0), CONTROL_FILL, PILL_BORDER);
    draw_text(
        out,
        FontKind::Regular,
        key,
        x + 8.0,
        y + 3.0,
        0.9,
        [170, 185, 205, 255],
    );
}

fn arrow_button(out: &mut Vec<HudVertex>, r: Rect, glyph: &str, color: Rgba) {
    ui::outlined(out, r, CONTROL_FILL, CONTROL_BORDER);
    draw_text(
        out,
        FontKind::Regular,
        glyph,
        r.x + 8.0,
        r.y + 3.0,
        1.0,
        color,
    );
}

fn draw_slider(
    out: &mut Vec<HudVertex>,
    parts: &RowParts,
    value: f32,
    min: f32,
    max: f32,
    arrow_color: Rgba,
) {
    let track = parts.middle;
    let y = track.y + 10.0;
    let frac = ((value - min) / (max - min)).clamp(0.0, 1.0);
    push_rect(out, track.x, y, track.w, 4.0, [45, 55, 70, 220]);
    push_rect(out, track.x, y, track.w * frac, 4.0, GREEN);
    push_rect(
        out,
        track.x + track.w * frac - 4.0,
        y - 5.0,
        8.0,
        14.0,
        WHITE,
    );

    arrow_button(out, parts.right, ">", arrow_color);

    ui::outlined(out, parts.value, [0, 0, 0, 140], CONTROL_BORDER);
    ui::centered_text(
        out,
        FontKind::Regular,
        &value_text(value, min, max),
        parts.value,
        3.0,
        0.95,
        WHITE,
    );
}

fn draw_toggle(out: &mut Vec<HudVertex>, parts: &RowParts, on: bool) {
    let r = parts.middle;
    let (text, text_color, dash_color) = if on {
        ("ON", GREEN, GREEN)
    } else {
        ("OFF", [210, 215, 225, 200], [70, 75, 85, 200])
    };
    ui::outlined(out, r, CONTROL_FILL, CONTROL_BORDER);
    ui::centered_text(out, FontKind::Regular, text, r, 3.0, 0.95, text_color);
    push_rect(out, r.x + 55.0, r.y + 20.0, 14.0, 2.0, dash_color);
    push_rect(out, r.x + 73.0, r.y + 20.0, 14.0, 2.0, dash_color);
}

#[cfg(test)]
mod tests {
    use super::*;

    const W: f32 = 800.0;
    const H: f32 = 600.0;
    const SIZES: [(f32, f32); 3] = [(800.0, 600.0), (1920.0, 1080.0), (3840.0, 2160.0)];

    fn rect_of(menu: &SettingsMenu, item: SettingsItem) -> Rect {
        menu.widgets(true, W, H)
            .iter()
            .find(|widget| widget.action == item)
            .expect("widget is listed")
            .rect
    }

    fn click_center(menu: &mut SettingsMenu, settings: &mut Settings, item: SettingsItem) {
        let (x, y) = rect_of(menu, item).center();
        menu.handle_mouse_down(settings, true, x, y, W, H);
        menu.handle_mouse_up();
    }

    #[test]
    fn default_settings_values_are_valid() {
        let s = Settings::default();
        assert_eq!(s.fov, 70.0);
        assert_eq!(s.view_distance, 16);
        assert!(s.reflections);
        assert!(s.shadows);
        assert!(s.ambient_occlusion);
        assert_eq!(s.sensitivity, 2.5);
        assert!(!s.invert_y);
        assert_eq!(s.master_volume, 1.0);
    }

    #[test]
    fn tab_cycling_wraps_around() {
        let t = Tab::Video;
        assert_eq!(t.next(), Tab::Controls);
        assert_eq!(t.next().next(), Tab::Audio);
        assert_eq!(t.next().next().next(), Tab::Video);
        assert_eq!(t.prev(), Tab::Audio);
    }

    #[test]
    fn slider_clamps_between_min_and_max() {
        let mut menu = SettingsMenu::new();
        menu.active_tab = Tab::Controls;
        let mut settings = Settings::default();

        // Sensitivity index 0
        menu.adjust_item(&mut settings, 0, 100.0);
        assert_eq!(settings.sensitivity, 5.0);

        menu.adjust_item(&mut settings, 0, -100.0);
        assert_eq!(settings.sensitivity, 0.5);
    }

    #[test]
    fn toggle_flips_boolean_value() {
        let mut menu = SettingsMenu::new();
        menu.active_tab = Tab::Controls;
        let mut settings = Settings::default();

        assert!(!settings.invert_y);
        menu.adjust_item(&mut settings, 1, 1.0);
        assert!(settings.invert_y);
        menu.adjust_item(&mut settings, 1, 1.0);
        assert!(!settings.invert_y);
    }

    #[test]
    fn every_widget_fires_at_its_center() {
        for (w, h) in SIZES {
            for from_title in [true, false] {
                for tab in Tab::ALL {
                    let mut listing = SettingsMenu::new();
                    listing.active_tab = tab;
                    for widget in listing.widgets(from_title, w, h) {
                        let (x, y) = widget.rect.center();
                        let mut menu = SettingsMenu::new();
                        menu.active_tab = tab;
                        let mut settings = Settings::default();
                        let action = menu.handle_mouse_down(&mut settings, from_title, x, y, w, h);
                        match widget.action {
                            SettingsItem::Back => {
                                assert_eq!(action, SettingsMenu::close(from_title))
                            }
                            SettingsItem::TitleMenu => assert_eq!(action, Transition::LeaveWorld),
                            SettingsItem::Quit => assert_eq!(action, Transition::Quit),
                            SettingsItem::Tab(clicked) => assert_eq!(menu.active_tab, clicked),
                            SettingsItem::Left(row)
                            | SettingsItem::Right(row)
                            | SettingsItem::Row(row) => {
                                assert_eq!(action, Transition::None);
                                assert_eq!(menu.selected_row, row, "at {w}x{h}");
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn title_menu_button_only_exists_in_a_world() {
        let menu = SettingsMenu::new();
        let has_button = |from_title: bool| {
            menu.widgets(from_title, W, H)
                .iter()
                .any(|widget| widget.action == SettingsItem::TitleMenu)
        };
        assert!(has_button(false));
        assert!(!has_button(true));
    }

    #[test]
    fn drawn_arrows_step_the_value() {
        let mut menu = SettingsMenu::new();
        let mut settings = Settings::default();

        click_center(&mut menu, &mut settings, SettingsItem::Right(0));
        assert_eq!(settings.fov, 75.0);
        click_center(&mut menu, &mut settings, SettingsItem::Left(0));
        assert_eq!(settings.fov, 70.0);

        // The left edge of the right arrow is part of the arrow, not the track.
        let right = rect_of(&menu, SettingsItem::Right(0));
        menu.handle_mouse_down(&mut settings, true, right.x + 1.0, right.y + 5.0, W, H);
        assert_eq!(settings.fov, 75.0);
        assert_eq!(menu.dragging_slider, None);
    }

    #[test]
    fn value_box_does_not_step_the_value() {
        let mut menu = SettingsMenu::new();
        let mut settings = Settings::default();
        let right = rect_of(&menu, SettingsItem::Right(0));
        let (x, y) = (right.x + right.w + 20.0, right.y + 12.0);
        menu.handle_mouse_down(&mut settings, true, x, y, W, H);
        assert_eq!(settings.fov, 70.0);
    }

    #[test]
    fn hover_starts_at_the_top_of_the_drawn_row() {
        let mut menu = SettingsMenu::new();
        let mut settings = Settings::default();
        let row = rect_of(&menu, SettingsItem::Row(0));
        menu.handle_mouse_move(&mut settings, true, row.x + 5.0, row.y + 1.0, W, H);
        assert_eq!(menu.hovered_row(), Some(0));
        menu.handle_mouse_move(&mut settings, true, row.x + 5.0, row.y - 1.0, W, H);
        assert_eq!(menu.hovered_row(), None);
    }

    #[test]
    fn slider_drag_follows_the_drawn_track() {
        let mut menu = SettingsMenu::new();
        let mut settings = Settings::default();
        let track = RowParts::of(rect_of(&menu, SettingsItem::Row(0))).middle;
        let (x, y) = track.center();

        menu.handle_mouse_down(&mut settings, true, x, y, W, H);
        assert_eq!(settings.fov, 85.0);

        menu.handle_mouse_move(&mut settings, true, track.x, y, W, H);
        assert_eq!(settings.fov, 60.0);
        menu.handle_mouse_move(&mut settings, true, track.x + track.w, y, W, H);
        assert_eq!(settings.fov, 110.0);

        menu.handle_mouse_up();
        menu.handle_mouse_move(&mut settings, true, track.x, y, W, H);
        assert_eq!(settings.fov, 110.0);
    }

    #[test]
    fn clicking_a_toggle_row_flips_it() {
        let mut menu = SettingsMenu::new();
        let mut settings = Settings::default();
        click_center(&mut menu, &mut settings, SettingsItem::Row(2));
        assert!(!settings.reflections);
    }

    #[test]
    fn clicking_ambient_occlusion_toggle_flips_it() {
        let mut menu = SettingsMenu::new();
        let mut settings = Settings::default();
        click_center(&mut menu, &mut settings, SettingsItem::Row(4));
        assert!(!settings.ambient_occlusion);
    }

    #[test]
    fn keys_step_tabs_and_rows() {
        use winit::keyboard::KeyCode;
        let mut menu = SettingsMenu::new();
        let mut settings = Settings::default();

        menu.handle_key_down(&mut settings, true, KeyCode::ArrowDown);
        assert_eq!(menu.selected_row, 1);
        menu.handle_key_down(&mut settings, true, KeyCode::ArrowUp);
        menu.handle_key_down(&mut settings, true, KeyCode::ArrowUp);
        assert_eq!(menu.selected_row, 4);

        menu.handle_key_down(&mut settings, true, KeyCode::KeyE);
        assert_eq!((menu.active_tab, menu.selected_row), (Tab::Controls, 0));
        menu.handle_key_down(&mut settings, true, KeyCode::KeyQ);
        assert_eq!(menu.active_tab, Tab::Video);

        menu.handle_key_down(&mut settings, true, KeyCode::ArrowRight);
        assert_eq!(settings.fov, 75.0);
        menu.handle_key_down(&mut settings, true, KeyCode::Enter);
        assert_eq!(settings.fov, 80.0);
        menu.handle_key_down(&mut settings, true, KeyCode::ArrowLeft);
        assert_eq!(settings.fov, 75.0);
    }

    #[test]
    fn escape_closes_to_where_settings_came_from() {
        use winit::keyboard::KeyCode;
        let mut menu = SettingsMenu::new();
        let mut settings = Settings::default();
        assert_eq!(
            menu.handle_key_down(&mut settings, true, KeyCode::Escape),
            Transition::OpenTitle
        );
        assert_eq!(
            menu.handle_key_down(&mut settings, false, KeyCode::Escape),
            Transition::ResumeWorld
        );
    }

    #[test]
    fn settings_save_and_load_round_trip() {
        let temp =
            std::env::temp_dir().join(format!("goblock_settings_test_{}.json", std::process::id()));
        let original = Settings {
            fov: 95.0,
            view_distance: 20,
            reflections: false,
            shadows: false,
            ambient_occlusion: false,
            sensitivity: 3.5,
            invert_y: true,
            ..Default::default()
        };

        original.save_to(&temp).unwrap();
        let loaded = Settings::load_from(&temp);
        let _ = std::fs::remove_file(&temp);
        assert_eq!(original, loaded);
    }

    #[test]
    fn settings_load_defaults_when_missing_or_corrupt() {
        let temp_missing =
            std::env::temp_dir().join(format!("goblock_missing_{}.json", std::process::id()));
        let _ = std::fs::remove_file(&temp_missing);
        assert_eq!(Settings::load_from(&temp_missing), Settings::default());

        let temp_corrupt =
            std::env::temp_dir().join(format!("goblock_corrupt_{}.json", std::process::id()));
        std::fs::write(&temp_corrupt, "{ broken json").unwrap();
        let loaded = Settings::load_from(&temp_corrupt);
        let _ = std::fs::remove_file(&temp_corrupt);
        assert_eq!(loaded, Settings::default());
    }
}
