//! In-game settings data model, scalable tabs, and pixelated UI renderer.

use goblock_render::{draw_text, measure_text, push_rect, FontKind, HudVertex};

/// Core persistent game settings.
#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    // Video
    pub fov: f32,
    pub view_distance: i32,
    pub reflections: bool,
    pub shadows: bool,

    // Controls
    pub sensitivity: f32,
    pub invert_y: bool,

    // Audio
    pub master_volume: f32,
    pub music_volume: f32,
    pub sfx_volume: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            fov: 70.0,
            view_distance: 16,
            reflections: true,
            shadows: true,
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

pub enum Action {
    None,
    Close,
    ReturnToTitle,
    Quit,
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

pub struct SettingsMenu {
    pub active_tab: Tab,
    pub selected_row: usize,
    pub hovered_row: Option<usize>,
    pub hovered_tab: Option<Tab>,
    pub back_hovered: bool,
    pub title_menu_hovered: bool,
    pub quit_hovered: bool,
    dragging_slider: Option<usize>,
}

impl SettingsMenu {
    pub fn new() -> Self {
        Self {
            active_tab: Tab::Video,
            selected_row: 0,
            hovered_row: None,
            hovered_tab: None,
            back_hovered: false,
            title_menu_hovered: false,
            quit_hovered: false,
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
        self.hovered_row
            .unwrap_or(self.selected_row)
            .min(self.current_items().len().saturating_sub(1))
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
                "invert_y" => settings.invert_y = !settings.invert_y,
                _ => {}
            },
            SettingKind::Keybind { .. } => {}
        }
    }

    pub fn handle_key_down(
        &mut self,
        settings: &mut Settings,
        key: winit::keyboard::KeyCode,
    ) -> Action {
        use winit::keyboard::KeyCode;
        match key {
            KeyCode::Escape => Action::Close,
            KeyCode::ArrowUp => {
                let count = self.current_items().len();
                if count > 0 {
                    self.selected_row = (self.selected_row + count - 1) % count;
                }
                Action::None
            }
            KeyCode::ArrowDown => {
                let count = self.current_items().len();
                if count > 0 {
                    self.selected_row = (self.selected_row + 1) % count;
                }
                Action::None
            }
            KeyCode::ArrowLeft => {
                self.adjust_item(settings, self.active_index(), -1.0);
                Action::None
            }
            KeyCode::ArrowRight => {
                self.adjust_item(settings, self.active_index(), 1.0);
                Action::None
            }
            KeyCode::Tab | KeyCode::KeyE => {
                self.active_tab = self.active_tab.next();
                self.selected_row = 0;
                Action::None
            }
            KeyCode::KeyQ => {
                self.active_tab = self.active_tab.prev();
                self.selected_row = 0;
                Action::None
            }
            KeyCode::Enter => {
                self.adjust_item(settings, self.active_index(), 1.0);
                Action::None
            }
            _ => Action::None,
        }
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
        // Back button hitbox (bottom-left [ESC] BACK)
        let bot_y = height - 44.0;
        self.back_hovered =
            (24.0..=140.0).contains(&x) && (bot_y + 4.0..=bot_y + 40.0).contains(&y);

        // Title Menu button hitbox (only when in-game)
        let title_btn_x = 160.0;
        let title_btn_w = 140.0;
        self.title_menu_hovered = !from_title
            && (title_btn_x..=title_btn_x + title_btn_w).contains(&x)
            && (bot_y + 4.0..=bot_y + 40.0).contains(&y);

        // Quit button hitbox (bottom-right [QUIT GAME])
        let quit_w = 140.0;
        let quit_x = width - quit_w - 24.0;
        self.quit_hovered =
            (quit_x..=quit_x + quit_w).contains(&x) && (bot_y + 4.0..=bot_y + 40.0).contains(&y);

        // Tab buttons hitboxes across header
        let mut tab_x = 240.0;
        self.hovered_tab = None;
        for &tab in Tab::ALL.iter() {
            let name = tab.name().to_uppercase();
            let text_w = measure_text(FontKind::Headline, &name, 0.7);
            let tab_w = text_w + 32.0;
            if (tab_x..=tab_x + tab_w).contains(&x) && (16.0..=52.0).contains(&y) {
                self.hovered_tab = Some(tab);
            }
            tab_x += tab_w + 12.0;
        }

        // Setting rows hitbox
        let content_x = 50.0;
        let content_w = (width - 100.0).min(760.0);
        let list_y = 90.0;
        let row_h = 36.0;
        let row_spacing = 4.0;
        let items = self.current_items();
        self.hovered_row = None;

        let mut curr_y = list_y;
        let mut last_category = "";
        for (i, item) in items.iter().enumerate() {
            if item.category != last_category {
                last_category = item.category;
                curr_y += 28.0;
            }
            if (content_x..=content_x + content_w).contains(&x)
                && (curr_y..=curr_y + row_h).contains(&y)
            {
                self.hovered_row = Some(i);
                self.selected_row = i;
            }
            curr_y += row_h + row_spacing;
        }

        if let Some(dragging) = self.dragging_slider {
            if dragging < items.len() {
                let track_x = content_x + content_w - 240.0;
                let track_w = 150.0;
                let frac = ((x - track_x) / track_w).clamp(0.0, 1.0);
                let item = &items[dragging];
                if let SettingKind::Slider { min, max, .. } = item.kind {
                    let val = min + frac * (max - min);
                    match item.id {
                        "fov" => settings.fov = (val / 5.0).round() * 5.0,
                        "view_distance" => settings.view_distance = val.round() as i32,
                        "sens" => settings.sensitivity = (val * 10.0).round() / 10.0,
                        "master_vol" => settings.master_volume = (val * 20.0).round() / 20.0,
                        "music_vol" => settings.music_volume = (val * 20.0).round() / 20.0,
                        "sfx_vol" => settings.sfx_volume = (val * 20.0).round() / 20.0,
                        _ => {}
                    }
                }
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
    ) -> Action {
        self.handle_mouse_move(settings, from_title, x, y, width, height);

        if self.back_hovered {
            return Action::Close;
        }

        if self.title_menu_hovered {
            return Action::ReturnToTitle;
        }

        if self.quit_hovered {
            return Action::Quit;
        }

        if let Some(tab) = self.hovered_tab {
            self.active_tab = tab;
            self.selected_row = 0;
            return Action::None;
        }

        if let Some(row) = self.hovered_row {
            let items = self.current_items();
            let item = &items[row];
            let content_x = 50.0;
            let content_w = (width - 100.0).min(760.0);
            let ctrl_x = content_x + content_w - 280.0;

            // Arrow left clicked
            if (ctrl_x..=ctrl_x + 30.0).contains(&x) {
                self.adjust_item(settings, row, -1.0);
                return Action::None;
            }

            // Arrow right clicked
            if (ctrl_x + 200.0..=ctrl_x + 235.0).contains(&x) {
                self.adjust_item(settings, row, 1.0);
                return Action::None;
            }

            match item.kind {
                SettingKind::Slider { .. } => {
                    let track_x = content_x + content_w - 240.0;
                    let track_w = 150.0;
                    if (track_x - 10.0..=track_x + track_w + 10.0).contains(&x) {
                        self.dragging_slider = Some(row);
                        self.handle_mouse_move(settings, from_title, x, y, width, height);
                    }
                }
                SettingKind::Toggle => {
                    self.adjust_item(settings, row, 1.0);
                }
                SettingKind::Keybind { .. } => {}
            }
        }

        Action::None
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

        // 1. Gentle dark transparent backdrop so game scene is clearly visible
        push_rect(&mut out, 0.0, 0.0, width, height, [0, 0, 0, 75]);

        // 2. Top Header Bar (semi-transparent black with fine bottom border)
        push_rect(&mut out, 0.0, 0.0, width, 60.0, [0, 0, 0, 115]);
        push_rect(&mut out, 0.0, 59.0, width, 1.0, [45, 50, 65, 140]);

        // Headline Title: SETTINGS (upheavtt.ttf)
        draw_text(
            &mut out,
            FontKind::Headline,
            "SETTINGS",
            40.0,
            16.0,
            1.0,
            [255, 255, 255, 255],
        );

        // Tab Navigation Header (Q, Tabs, E) like Image 1
        let mut tab_x = 240.0;
        let tab_y = 16.0;

        // [ Q ] Pill
        push_rect(
            &mut out,
            tab_x - 38.0,
            tab_y + 3.0,
            26.0,
            24.0,
            [0, 0, 0, 120],
        );
        push_rect(
            &mut out,
            tab_x - 38.0,
            tab_y + 3.0,
            26.0,
            1.0,
            [65, 75, 95, 160],
        );
        push_rect(
            &mut out,
            tab_x - 38.0,
            tab_y + 26.0,
            26.0,
            1.0,
            [65, 75, 95, 160],
        );
        push_rect(
            &mut out,
            tab_x - 38.0,
            tab_y + 3.0,
            1.0,
            24.0,
            [65, 75, 95, 160],
        );
        push_rect(
            &mut out,
            tab_x - 13.0,
            tab_y + 3.0,
            1.0,
            24.0,
            [65, 75, 95, 160],
        );
        draw_text(
            &mut out,
            FontKind::Regular,
            "Q",
            tab_x - 30.0,
            tab_y + 6.0,
            0.9,
            [170, 185, 205, 255],
        );

        for &tab in Tab::ALL.iter() {
            let name = tab.name().to_uppercase();
            let text_w = measure_text(FontKind::Headline, &name, 0.75);
            let tab_w = text_w + 32.0; // Proper dynamic width with 16px padding on each side!
            let tab_h = 30.0;
            let is_active = self.active_tab == tab;
            let is_hover = self.hovered_tab == Some(tab);

            if is_active {
                // Vibrant highlighted tab pill like Image 1 (lime/cyan glow)
                push_rect(&mut out, tab_x, tab_y, tab_w, tab_h, [80, 230, 160, 240]);
                push_rect(&mut out, tab_x, tab_y, tab_w, 1.0, [150, 255, 200, 255]);
                push_rect(
                    &mut out,
                    tab_x,
                    tab_y + tab_h - 1.0,
                    tab_w,
                    1.0,
                    [40, 160, 100, 255],
                );
                let center_x = tab_x + (tab_w - text_w) * 0.5;
                draw_text(
                    &mut out,
                    FontKind::Headline,
                    &name,
                    center_x,
                    tab_y + 5.0,
                    0.75,
                    [15, 25, 20, 255], // crisp dark text on bright tab
                );
            } else {
                let bg_col = if is_hover {
                    [35, 45, 60, 180]
                } else {
                    [0, 0, 0, 90]
                };
                push_rect(&mut out, tab_x, tab_y, tab_w, tab_h, bg_col);
                push_rect(&mut out, tab_x, tab_y, tab_w, 1.0, [50, 60, 75, 140]);
                push_rect(
                    &mut out,
                    tab_x,
                    tab_y + tab_h - 1.0,
                    tab_w,
                    1.0,
                    [50, 60, 75, 140],
                );
                push_rect(&mut out, tab_x, tab_y, 1.0, tab_h, [50, 60, 75, 140]);
                push_rect(
                    &mut out,
                    tab_x + tab_w - 1.0,
                    tab_y,
                    1.0,
                    tab_h,
                    [50, 60, 75, 140],
                );
                let center_x = tab_x + (tab_w - text_w) * 0.5;
                draw_text(
                    &mut out,
                    FontKind::Headline,
                    &name,
                    center_x,
                    tab_y + 5.0,
                    0.75,
                    [170, 185, 205, 230],
                );
            }

            tab_x += tab_w + 12.0;
        }

        // [ E ] Pill
        push_rect(&mut out, tab_x, tab_y + 3.0, 26.0, 24.0, [0, 0, 0, 120]);
        push_rect(&mut out, tab_x, tab_y + 3.0, 26.0, 1.0, [65, 75, 95, 160]);
        push_rect(&mut out, tab_x, tab_y + 26.0, 26.0, 1.0, [65, 75, 95, 160]);
        push_rect(&mut out, tab_x, tab_y + 3.0, 1.0, 24.0, [65, 75, 95, 160]);
        push_rect(
            &mut out,
            tab_x + 25.0,
            tab_y + 3.0,
            1.0,
            24.0,
            [65, 75, 95, 160],
        );
        draw_text(
            &mut out,
            FontKind::Regular,
            "E",
            tab_x + 8.0,
            tab_y + 6.0,
            0.9,
            [170, 185, 205, 255],
        );

        // 3. Main Settings List (Clean, single-pane flow matching Image 1)
        let content_x = 50.0;
        let content_w = (width - 100.0).min(760.0);
        let list_y = 85.0;
        let row_h = 36.0;
        let row_spacing = 4.0;
        let items = self.current_items();

        let mut curr_y = list_y;
        let mut last_category = "";

        for (i, item) in items.iter().enumerate() {
            // Category header in cyan (upheavtt.ttf)
            if item.category != last_category {
                last_category = item.category;
                draw_text(
                    &mut out,
                    FontKind::Headline,
                    last_category,
                    content_x + 12.0,
                    curr_y + 4.0,
                    0.75,
                    [60, 215, 240, 255],
                );
                curr_y += 28.0;
            }

            let is_selected = self.selected_row == i;
            let is_hovered = self.hovered_row == Some(i);

            // Subtle row highlight bar across entire row width
            if is_selected || is_hovered {
                push_rect(
                    &mut out,
                    content_x,
                    curr_y,
                    content_w,
                    row_h,
                    [30, 48, 70, 140],
                );
                push_rect(&mut out, content_x, curr_y, 3.0, row_h, [80, 230, 160, 255]);
                push_rect(
                    &mut out,
                    content_x,
                    curr_y,
                    content_w,
                    1.0,
                    [50, 75, 105, 140],
                );
                push_rect(
                    &mut out,
                    content_x,
                    curr_y + row_h - 1.0,
                    content_w,
                    1.0,
                    [50, 75, 105, 140],
                );
            }

            // Setting Label on left side (eas-vhs.ttf)
            let label_col = if is_selected || is_hovered {
                [255, 255, 255, 255]
            } else {
                [185, 195, 210, 230]
            };
            draw_text(
                &mut out,
                FontKind::Regular,
                item.label,
                content_x + 16.0,
                curr_y + 8.0,
                1.0,
                label_col,
            );

            // Control element on right side
            let ctrl_x = content_x + content_w - 280.0;
            let arrow_w = 24.0;
            let arrow_h = 24.0;
            let arrow_y = curr_y + 6.0;

            let arrow_col = if is_selected || is_hovered {
                [240, 245, 255, 255]
            } else {
                [140, 150, 170, 180]
            };

            // Left arrow button [ < ]
            push_rect(&mut out, ctrl_x, arrow_y, arrow_w, arrow_h, [0, 0, 0, 120]);
            push_rect(&mut out, ctrl_x, arrow_y, arrow_w, 1.0, [55, 65, 80, 160]);
            push_rect(
                &mut out,
                ctrl_x,
                arrow_y + arrow_h - 1.0,
                arrow_w,
                1.0,
                [55, 65, 80, 160],
            );
            push_rect(&mut out, ctrl_x, arrow_y, 1.0, arrow_h, [55, 65, 80, 160]);
            push_rect(
                &mut out,
                ctrl_x + arrow_w - 1.0,
                arrow_y,
                1.0,
                arrow_h,
                [55, 65, 80, 160],
            );
            draw_text(
                &mut out,
                FontKind::Regular,
                "<",
                ctrl_x + 8.0,
                arrow_y + 3.0,
                1.0,
                arrow_col,
            );

            match item.kind {
                SettingKind::Slider { min, max, .. } => {
                    let val = match item.id {
                        "fov" => settings.fov,
                        "view_distance" => settings.view_distance as f32,
                        "sens" => settings.sensitivity,
                        "master_vol" => settings.master_volume,
                        "music_vol" => settings.music_volume,
                        "sfx_vol" => settings.sfx_volume,
                        _ => min,
                    };
                    let val_text = if max <= 1.0 {
                        format!("{:.0}%", val * 100.0)
                    } else if (max - min) > 20.0 {
                        format!("{:.0}", val)
                    } else {
                        format!("{:.1}", val)
                    };

                    let track_x = ctrl_x + arrow_w + 10.0;
                    let track_w = 140.0;
                    let track_y = curr_y + 16.0;

                    // Slider track
                    push_rect(&mut out, track_x, track_y, track_w, 4.0, [45, 55, 70, 220]);

                    // Filled slider bar (cyan/lime)
                    let frac = ((val - min) / (max - min)).clamp(0.0, 1.0);
                    push_rect(
                        &mut out,
                        track_x,
                        track_y,
                        track_w * frac,
                        4.0,
                        [80, 230, 160, 255],
                    );

                    // Slider knob
                    let knob_x = track_x + track_w * frac - 4.0;
                    push_rect(
                        &mut out,
                        knob_x,
                        track_y - 5.0,
                        8.0,
                        14.0,
                        [255, 255, 255, 255],
                    );

                    // Right arrow button [ > ]
                    let r_arrow_x = track_x + track_w + 10.0;
                    push_rect(
                        &mut out,
                        r_arrow_x,
                        arrow_y,
                        arrow_w,
                        arrow_h,
                        [0, 0, 0, 120],
                    );
                    push_rect(
                        &mut out,
                        r_arrow_x,
                        arrow_y,
                        arrow_w,
                        1.0,
                        [55, 65, 80, 160],
                    );
                    push_rect(
                        &mut out,
                        r_arrow_x,
                        arrow_y + arrow_h - 1.0,
                        arrow_w,
                        1.0,
                        [55, 65, 80, 160],
                    );
                    push_rect(
                        &mut out,
                        r_arrow_x,
                        arrow_y,
                        1.0,
                        arrow_h,
                        [55, 65, 80, 160],
                    );
                    push_rect(
                        &mut out,
                        r_arrow_x + arrow_w - 1.0,
                        arrow_y,
                        1.0,
                        arrow_h,
                        [55, 65, 80, 160],
                    );
                    draw_text(
                        &mut out,
                        FontKind::Regular,
                        ">",
                        r_arrow_x + 8.0,
                        arrow_y + 3.0,
                        1.0,
                        arrow_col,
                    );

                    // Value box on far right
                    let val_box_x = r_arrow_x + arrow_w + 8.0;
                    let val_box_w = 48.0;
                    push_rect(
                        &mut out,
                        val_box_x,
                        arrow_y,
                        val_box_w,
                        arrow_h,
                        [0, 0, 0, 140],
                    );
                    push_rect(
                        &mut out,
                        val_box_x,
                        arrow_y,
                        val_box_w,
                        1.0,
                        [55, 65, 80, 160],
                    );
                    push_rect(
                        &mut out,
                        val_box_x,
                        arrow_y + arrow_h - 1.0,
                        val_box_w,
                        1.0,
                        [55, 65, 80, 160],
                    );
                    push_rect(
                        &mut out,
                        val_box_x,
                        arrow_y,
                        1.0,
                        arrow_h,
                        [55, 65, 80, 160],
                    );
                    push_rect(
                        &mut out,
                        val_box_x + val_box_w - 1.0,
                        arrow_y,
                        1.0,
                        arrow_h,
                        [55, 65, 80, 160],
                    );
                    let vt_w = measure_text(FontKind::Regular, &val_text, 0.95);
                    let vt_x = val_box_x + (val_box_w - vt_w) * 0.5;
                    draw_text(
                        &mut out,
                        FontKind::Regular,
                        &val_text,
                        vt_x,
                        arrow_y + 3.0,
                        0.95,
                        [255, 255, 255, 255],
                    );
                }
                SettingKind::Toggle => {
                    let state = match item.id {
                        "reflections" => settings.reflections,
                        "shadows" => settings.shadows,
                        "invert_y" => settings.invert_y,
                        _ => false,
                    };
                    let text = if state { "ON" } else { "OFF" };
                    let text_col = if state {
                        [80, 230, 160, 255]
                    } else {
                        [210, 215, 225, 200]
                    };

                    let box_x = ctrl_x + arrow_w + 10.0;
                    let box_w = 140.0;
                    push_rect(&mut out, box_x, arrow_y, box_w, arrow_h, [0, 0, 0, 120]);
                    push_rect(&mut out, box_x, arrow_y, box_w, 1.0, [55, 65, 80, 160]);
                    push_rect(
                        &mut out,
                        box_x,
                        arrow_y + arrow_h - 1.0,
                        box_w,
                        1.0,
                        [55, 65, 80, 160],
                    );
                    push_rect(&mut out, box_x, arrow_y, 1.0, arrow_h, [55, 65, 80, 160]);
                    push_rect(
                        &mut out,
                        box_x + box_w - 1.0,
                        arrow_y,
                        1.0,
                        arrow_h,
                        [55, 65, 80, 160],
                    );

                    let tw = measure_text(FontKind::Regular, text, 0.95);
                    let tx = box_x + (box_w - tw) * 0.5;
                    draw_text(
                        &mut out,
                        FontKind::Regular,
                        text,
                        tx,
                        arrow_y + 3.0,
                        0.95,
                        text_col,
                    );

                    // Decorative dashes below like in Image 1
                    let dash_col = if state {
                        [80, 230, 160, 255]
                    } else {
                        [70, 75, 85, 200]
                    };
                    push_rect(&mut out, box_x + 55.0, arrow_y + 20.0, 14.0, 2.0, dash_col);
                    push_rect(&mut out, box_x + 73.0, arrow_y + 20.0, 14.0, 2.0, dash_col);

                    // Right arrow button [ > ]
                    let r_arrow_x = box_x + box_w + 10.0;
                    push_rect(
                        &mut out,
                        r_arrow_x,
                        arrow_y,
                        arrow_w,
                        arrow_h,
                        [0, 0, 0, 120],
                    );
                    push_rect(
                        &mut out,
                        r_arrow_x,
                        arrow_y,
                        arrow_w,
                        1.0,
                        [55, 65, 80, 160],
                    );
                    push_rect(
                        &mut out,
                        r_arrow_x,
                        arrow_y + arrow_h - 1.0,
                        arrow_w,
                        1.0,
                        [55, 65, 80, 160],
                    );
                    push_rect(
                        &mut out,
                        r_arrow_x,
                        arrow_y,
                        1.0,
                        arrow_h,
                        [55, 65, 80, 160],
                    );
                    push_rect(
                        &mut out,
                        r_arrow_x + arrow_w - 1.0,
                        arrow_y,
                        1.0,
                        arrow_h,
                        [55, 65, 80, 160],
                    );
                    draw_text(
                        &mut out,
                        FontKind::Regular,
                        ">",
                        r_arrow_x + 8.0,
                        arrow_y + 3.0,
                        1.0,
                        arrow_col,
                    );
                }
                SettingKind::Keybind { key } => {
                    let box_x = ctrl_x + arrow_w + 10.0;
                    let box_w = 140.0;
                    push_rect(&mut out, box_x, arrow_y, box_w, arrow_h, [0, 0, 0, 120]);
                    push_rect(&mut out, box_x, arrow_y, box_w, 1.0, [55, 65, 80, 160]);
                    push_rect(
                        &mut out,
                        box_x,
                        arrow_y + arrow_h - 1.0,
                        box_w,
                        1.0,
                        [55, 65, 80, 160],
                    );
                    push_rect(&mut out, box_x, arrow_y, 1.0, arrow_h, [55, 65, 80, 160]);
                    push_rect(
                        &mut out,
                        box_x + box_w - 1.0,
                        arrow_y,
                        1.0,
                        arrow_h,
                        [55, 65, 80, 160],
                    );

                    let kw = measure_text(FontKind::Regular, key, 0.95);
                    let kx = box_x + (box_w - kw) * 0.5;
                    draw_text(
                        &mut out,
                        FontKind::Regular,
                        key,
                        kx,
                        arrow_y + 3.0,
                        0.95,
                        [255, 255, 255, 255],
                    );

                    // Right arrow button [ > ]
                    let r_arrow_x = box_x + box_w + 10.0;
                    push_rect(
                        &mut out,
                        r_arrow_x,
                        arrow_y,
                        arrow_w,
                        arrow_h,
                        [0, 0, 0, 120],
                    );
                    push_rect(
                        &mut out,
                        r_arrow_x,
                        arrow_y,
                        arrow_w,
                        1.0,
                        [55, 65, 80, 160],
                    );
                    push_rect(
                        &mut out,
                        r_arrow_x,
                        arrow_y + arrow_h - 1.0,
                        arrow_w,
                        1.0,
                        [55, 65, 80, 160],
                    );
                    push_rect(
                        &mut out,
                        r_arrow_x,
                        arrow_y,
                        1.0,
                        arrow_h,
                        [55, 65, 80, 160],
                    );
                    push_rect(
                        &mut out,
                        r_arrow_x + arrow_w - 1.0,
                        arrow_y,
                        1.0,
                        arrow_h,
                        [55, 65, 80, 160],
                    );
                    draw_text(
                        &mut out,
                        FontKind::Regular,
                        ">",
                        r_arrow_x + 8.0,
                        arrow_y + 3.0,
                        1.0,
                        arrow_col,
                    );
                }
            }

            curr_y += row_h + row_spacing;
        }

        // 4. Item Description Toast (Above bottom bar)
        let active_idx = self.active_index();
        if let Some(active_item) = items.get(active_idx) {
            let desc_y = height - 80.0;
            draw_text(
                &mut out,
                FontKind::Regular,
                &format!("* {}", active_item.description),
                50.0,
                desc_y,
                0.95,
                [180, 225, 240, 255],
            );
        }

        // 5. Bottom Navigation Bar matching Image 1
        let bot_y = height - 48.0;
        push_rect(&mut out, 0.0, bot_y, width, 48.0, [0, 0, 0, 130]);
        push_rect(&mut out, 0.0, bot_y, width, 1.0, [45, 50, 65, 140]);

        // Left button: [ESC] BACK (properly padded, no overflow!)
        let back_pill_w = 40.0;
        let back_bg = if self.back_hovered {
            [60, 75, 95, 240]
        } else {
            [0, 0, 0, 150]
        };
        push_rect(&mut out, 40.0, bot_y + 8.0, back_pill_w, 28.0, back_bg);
        push_rect(
            &mut out,
            40.0,
            bot_y + 8.0,
            back_pill_w,
            1.0,
            [80, 95, 120, 200],
        );
        push_rect(
            &mut out,
            40.0,
            bot_y + 35.0,
            back_pill_w,
            1.0,
            [80, 95, 120, 200],
        );
        push_rect(&mut out, 40.0, bot_y + 8.0, 1.0, 28.0, [80, 95, 120, 200]);
        push_rect(
            &mut out,
            40.0 + back_pill_w - 1.0,
            bot_y + 8.0,
            1.0,
            28.0,
            [80, 95, 120, 200],
        );
        draw_text(
            &mut out,
            FontKind::Regular,
            "ESC",
            47.0,
            bot_y + 13.0,
            0.85,
            [255, 255, 255, 255],
        );
        draw_text(
            &mut out,
            FontKind::Regular,
            "BACK",
            90.0,
            bot_y + 13.0,
            0.95,
            [210, 220, 235, 255],
        );

        if !from_title {
            let title_btn_x = 160.0;
            let title_btn_w = 140.0;
            let title_bg = if self.title_menu_hovered {
                [40, 55, 80, 230]
            } else {
                [20, 28, 42, 170]
            };
            push_rect(
                &mut out,
                title_btn_x,
                bot_y + 8.0,
                title_btn_w,
                28.0,
                title_bg,
            );
            let border_col = if self.title_menu_hovered {
                [80, 160, 240, 255]
            } else {
                [50, 65, 90, 180]
            };
            push_rect(
                &mut out,
                title_btn_x,
                bot_y + 8.0,
                title_btn_w,
                1.0,
                border_col,
            );
            push_rect(
                &mut out,
                title_btn_x,
                bot_y + 35.0,
                title_btn_w,
                1.0,
                border_col,
            );
            push_rect(&mut out, title_btn_x, bot_y + 8.0, 1.0, 28.0, border_col);
            push_rect(
                &mut out,
                title_btn_x + title_btn_w - 1.0,
                bot_y + 8.0,
                1.0,
                28.0,
                border_col,
            );
            draw_text(
                &mut out,
                FontKind::Headline,
                "TITLE MENU",
                title_btn_x + 18.0,
                bot_y + 13.0,
                0.65,
                [255, 255, 255, 255],
            );
        }

        // Right button: [QUIT GAME] (Calculated width, perfectly fitted, zero overflow!)
        let quit_text = "QUIT GAME";
        let quit_scale = 0.75;
        let quit_text_w = measure_text(FontKind::Headline, quit_text, quit_scale);
        let quit_pad_x = 18.0;
        let quit_w = quit_text_w + quit_pad_x * 2.0;
        let quit_h = 32.0;
        let quit_x = width - quit_w - 30.0;
        let quit_y = bot_y + 8.0;

        let quit_bg = if self.quit_hovered {
            [220, 45, 45, 240]
        } else {
            [150, 35, 35, 200]
        };
        push_rect(&mut out, quit_x, quit_y, quit_w, quit_h, quit_bg);
        push_rect(&mut out, quit_x, quit_y, quit_w, 1.0, [255, 120, 120, 255]);
        push_rect(
            &mut out,
            quit_x,
            quit_y + quit_h - 1.0,
            quit_w,
            1.0,
            [100, 20, 20, 255],
        );
        push_rect(&mut out, quit_x, quit_y, 1.0, quit_h, [255, 120, 120, 255]);
        push_rect(
            &mut out,
            quit_x + quit_w - 1.0,
            quit_y,
            1.0,
            quit_h,
            [100, 20, 20, 255],
        );

        let quit_tx = quit_x + quit_pad_x;
        draw_text(
            &mut out,
            FontKind::Headline,
            quit_text,
            quit_tx,
            quit_y + 6.0,
            quit_scale,
            [255, 255, 255, 255],
        );

        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_settings_values_are_valid() {
        let s = Settings::default();
        assert_eq!(s.fov, 70.0);
        assert_eq!(s.view_distance, 16);
        assert!(s.reflections);
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
    fn mouse_click_on_back_returns_close() {
        let mut menu = SettingsMenu::new();
        let mut settings = Settings::default();
        let action = menu.handle_mouse_down(&mut settings, true, 50.0, 570.0, 800.0, 600.0);
        assert!(matches!(action, Action::Close));
    }

    #[test]
    fn mouse_click_on_title_menu_returns_return_to_title() {
        let mut menu = SettingsMenu::new();
        let mut settings = Settings::default();
        // Title menu button when from_title is false at x = 160..300, y = 556..596
        let action = menu.handle_mouse_down(&mut settings, false, 200.0, 570.0, 800.0, 600.0);
        assert!(matches!(action, Action::ReturnToTitle));
    }

    #[test]
    fn mouse_click_on_quit_returns_quit() {
        let mut menu = SettingsMenu::new();
        let mut settings = Settings::default();
        // Quit button is on bottom right
        let action = menu.handle_mouse_down(&mut settings, true, 730.0, 570.0, 800.0, 600.0);
        assert!(matches!(action, Action::Quit));
    }

    #[test]
    fn mouse_click_on_arrows_adjusts_slider() {
        let mut menu = SettingsMenu::new();
        menu.active_tab = Tab::Video;
        let mut settings = Settings::default();
        let before_fov = settings.fov;

        // Click right arrow on FOV row
        // content_x = 50.0, content_w = 700.0, ctrl_x = 50 + 700 - 280 = 470.0
        // arrow right: ctrl_x + 200..=ctrl_x + 235 = 670..=705
        let action = menu.handle_mouse_down(&mut settings, true, 680.0, 120.0, 800.0, 600.0);
        assert!(matches!(action, Action::None));
        assert!(
            settings.fov > before_fov,
            "fov should increase on right arrow click"
        );
    }
}
