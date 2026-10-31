//! Title screen, World Select, and Create World menus.

use crate::saves::WorldMeta;
use goblock_render::{draw_text, measure_text, push_rect, FontKind, HudVertex};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMode {
    Title,
    SelectWorld,
    CreateWorld,
    Playing,
    Settings { from_title: bool },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MainMenuAction {
    None,
    OpenSelectWorld,
    OpenSettings,
    Quit,
}

pub struct MainMenu {
    pub hovered_play: bool,
    pub hovered_settings: bool,
    pub hovered_quit: bool,
}

impl MainMenu {
    pub fn new() -> Self {
        Self {
            hovered_play: false,
            hovered_settings: false,
            hovered_quit: false,
        }
    }

    pub fn handle_mouse_move(&mut self, x: f32, y: f32, width: f32, height: f32) {
        let bot_y = height - 70.0;
        let right_margin = width - 40.0;

        let quit_w = 110.0;
        let settings_w = 130.0;
        let play_w = 160.0;
        let btn_h = 44.0;
        let btn_y = bot_y + 12.0;

        let quit_x = right_margin - quit_w;
        let settings_x = quit_x - settings_w - 14.0;
        let play_x = settings_x - play_w - 18.0;

        self.hovered_quit =
            (quit_x..=quit_x + quit_w).contains(&x) && (btn_y..=btn_y + btn_h).contains(&y);
        self.hovered_settings = (settings_x..=settings_x + settings_w).contains(&x)
            && (btn_y..=btn_y + btn_h).contains(&y);
        self.hovered_play =
            (play_x..=play_x + play_w).contains(&x) && (btn_y..=btn_y + btn_h).contains(&y);
    }

    pub fn handle_mouse_down(&mut self, x: f32, y: f32, width: f32, height: f32) -> MainMenuAction {
        self.handle_mouse_move(x, y, width, height);
        if self.hovered_play {
            MainMenuAction::OpenSelectWorld
        } else if self.hovered_settings {
            MainMenuAction::OpenSettings
        } else if self.hovered_quit {
            MainMenuAction::Quit
        } else {
            MainMenuAction::None
        }
    }

    pub fn handle_key_down(&mut self, key: winit::keyboard::KeyCode) -> MainMenuAction {
        use winit::keyboard::KeyCode;
        match key {
            KeyCode::Enter | KeyCode::Space => MainMenuAction::OpenSelectWorld,
            KeyCode::KeyS => MainMenuAction::OpenSettings,
            KeyCode::Escape => MainMenuAction::Quit,
            _ => MainMenuAction::None,
        }
    }

    pub fn build_vertices(&self, fps: f32, width: f32, height: f32) -> Vec<HudVertex> {
        let mut out = Vec::with_capacity(4096);

        // 1. Top Bar: Dark transparent gradient banner with title
        push_rect(&mut out, 0.0, 0.0, width, 80.0, [0, 0, 0, 110]);
        push_rect(&mut out, 0.0, 79.0, width, 1.0, [45, 55, 75, 120]);

        // Game Title Logo: "GOBLOCK" (upheavtt.ttf)
        draw_text(
            &mut out,
            FontKind::Headline,
            "GOBLOCK",
            36.0,
            20.0,
            1.5,
            [255, 255, 255, 255],
        );

        // Subtitle / Version tag (eas-vhs.ttf)
        draw_text(
            &mut out,
            FontKind::Regular,
            "ENDLESS VOXEL WORLD",
            210.0,
            27.0,
            0.9,
            [80, 230, 160, 255],
        );
        draw_text(
            &mut out,
            FontKind::Regular,
            "v0.1.0-alpha",
            210.0,
            42.0,
            0.8,
            [150, 165, 185, 200],
        );

        // 2. Bottom Navigation & Action Bar
        let bot_h = 70.0;
        let bot_y = height - bot_h;
        push_rect(&mut out, 0.0, bot_y, width, bot_h, [0, 0, 0, 140]);
        push_rect(&mut out, 0.0, bot_y, width, 1.0, [50, 60, 80, 160]);

        // Left info block
        let dot_x = 36.0;
        let dot_y = bot_y + 24.0;
        push_rect(&mut out, dot_x, dot_y + 4.0, 8.0, 8.0, [80, 230, 160, 255]);
        draw_text(
            &mut out,
            FontKind::Headline,
            "READY",
            dot_x + 16.0,
            dot_y + 2.0,
            0.65,
            [255, 255, 255, 255],
        );

        draw_text(
            &mut out,
            FontKind::Regular,
            "SELECT OR CREATE WORLD TO PLAY",
            dot_x + 16.0,
            dot_y + 20.0,
            0.85,
            [170, 185, 205, 220],
        );

        // Center: FPS
        let fps_text = format!("{:.0} FPS", fps);
        draw_text(
            &mut out,
            FontKind::Regular,
            &fps_text,
            width * 0.46,
            bot_y + 28.0,
            0.9,
            [140, 155, 175, 180],
        );

        // Right Action Buttons
        let right_margin = width - 40.0;
        let quit_w = 110.0;
        let settings_w = 130.0;
        let play_w = 160.0;
        let btn_h = 44.0;
        let btn_y = bot_y + 13.0;

        let quit_x = right_margin - quit_w;
        let settings_x = quit_x - settings_w - 14.0;
        let play_x = settings_x - play_w - 18.0;

        // Button 1: [ PLAY ] (Vibrant glowing cyan/lime accent)
        let play_bg = if self.hovered_play {
            [95, 245, 175, 255]
        } else {
            [80, 230, 160, 230]
        };
        push_rect(&mut out, play_x, btn_y, play_w, btn_h, play_bg);
        push_rect(&mut out, play_x, btn_y, play_w, 2.0, [180, 255, 220, 255]);
        push_rect(
            &mut out,
            play_x,
            btn_y + btn_h - 2.0,
            play_w,
            2.0,
            [40, 160, 100, 255],
        );
        push_rect(&mut out, play_x, btn_y, 2.0, btn_h, [180, 255, 220, 255]);
        push_rect(
            &mut out,
            play_x + play_w - 2.0,
            btn_y,
            2.0,
            btn_h,
            [40, 160, 100, 255],
        );

        let play_text = "PLAY";
        let pt_w = measure_text(FontKind::Headline, play_text, 1.1);
        let pt_x = play_x + (play_w - pt_w) * 0.5;
        draw_text(
            &mut out,
            FontKind::Headline,
            play_text,
            pt_x,
            btn_y + 9.0,
            1.1,
            [15, 30, 20, 255],
        );

        // Button 2: [ SETTINGS ]
        let set_bg = if self.hovered_settings {
            [40, 52, 72, 220]
        } else {
            [20, 26, 38, 160]
        };
        push_rect(&mut out, settings_x, btn_y, settings_w, btn_h, set_bg);
        let set_border = if self.hovered_settings {
            [80, 160, 240, 255]
        } else {
            [55, 65, 85, 180]
        };
        push_rect(&mut out, settings_x, btn_y, settings_w, 1.0, set_border);
        push_rect(
            &mut out,
            settings_x,
            btn_y + btn_h - 1.0,
            settings_w,
            1.0,
            set_border,
        );
        push_rect(&mut out, settings_x, btn_y, 1.0, btn_h, set_border);
        push_rect(
            &mut out,
            settings_x + settings_w - 1.0,
            btn_y,
            1.0,
            btn_h,
            set_border,
        );

        let set_text = "SETTINGS";
        let st_w = measure_text(FontKind::Headline, set_text, 0.75);
        let st_x = settings_x + (settings_w - st_w) * 0.5;
        draw_text(
            &mut out,
            FontKind::Headline,
            set_text,
            st_x,
            btn_y + 12.0,
            0.75,
            [255, 255, 255, 255],
        );

        // Button 3: [ QUIT ]
        let quit_bg = if self.hovered_quit {
            [50, 25, 25, 220]
        } else {
            [25, 20, 22, 160]
        };
        push_rect(&mut out, quit_x, btn_y, quit_w, btn_h, quit_bg);
        let quit_border = if self.hovered_quit {
            [240, 80, 80, 255]
        } else {
            [75, 50, 55, 160]
        };
        push_rect(&mut out, quit_x, btn_y, quit_w, 1.0, quit_border);
        push_rect(
            &mut out,
            quit_x,
            btn_y + btn_h - 1.0,
            quit_w,
            1.0,
            quit_border,
        );
        push_rect(&mut out, quit_x, btn_y, 1.0, btn_h, quit_border);
        push_rect(
            &mut out,
            quit_x + quit_w - 1.0,
            btn_y,
            1.0,
            btn_h,
            quit_border,
        );

        let qt_text = "QUIT";
        let qt_w = measure_text(FontKind::Headline, qt_text, 0.75);
        let qt_x = quit_x + (quit_w - qt_w) * 0.5;
        draw_text(
            &mut out,
            FontKind::Headline,
            qt_text,
            qt_x,
            btn_y + 12.0,
            0.75,
            [220, 225, 235, 240],
        );

        out
    }
}

// ==========================================
// SELECT WORLD MENU
// ==========================================

#[derive(Debug, Clone, PartialEq)]
pub enum SelectWorldAction {
    None,
    Play(WorldMeta),
    OpenCreate,
    Delete(String),
    Back,
}

pub struct SelectWorldMenu {
    pub selected: usize,
    pub hovered_row: Option<usize>,
    pub hovered_play: bool,
    pub hovered_create: bool,
    pub hovered_delete: bool,
    pub hovered_back: bool,
}

impl SelectWorldMenu {
    pub fn new() -> Self {
        Self {
            selected: 0,
            hovered_row: None,
            hovered_play: false,
            hovered_create: false,
            hovered_delete: false,
            hovered_back: false,
        }
    }

    pub fn handle_mouse_move(
        &mut self,
        worlds: &[WorldMeta],
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    ) {
        let list_x = 40.0;
        let list_w = width - 80.0;
        let list_y = 80.0;
        let card_h = 60.0;
        let card_gap = 8.0;

        self.hovered_row = None;
        for (i, _) in worlds.iter().enumerate() {
            let cy = list_y + i as f32 * (card_h + card_gap);
            if (list_x..=list_x + list_w).contains(&x) && (cy..=cy + card_h).contains(&y) {
                self.hovered_row = Some(i);
                self.selected = i;
            }
        }

        let bot_y = height - 56.0;
        let btn_h = 40.0;
        let btn_y = bot_y + 8.0;

        // Bottom buttons: [ PLAY SELECTED WORLD ]  [ CREATE NEW WORLD ]  [ DELETE ]  [ BACK ]
        let b_play_x = 40.0;
        let b_play_w = 200.0;
        let b_create_x = b_play_x + b_play_w + 14.0;
        let b_create_w = 170.0;
        let b_del_x = b_create_x + b_create_w + 14.0;
        let b_del_w = 110.0;
        let b_back_x = width - 140.0;
        let b_back_w = 100.0;

        self.hovered_play =
            (b_play_x..=b_play_x + b_play_w).contains(&x) && (btn_y..=btn_y + btn_h).contains(&y);
        self.hovered_create = (b_create_x..=b_create_x + b_create_w).contains(&x)
            && (btn_y..=btn_y + btn_h).contains(&y);
        self.hovered_delete =
            (b_del_x..=b_del_x + b_del_w).contains(&x) && (btn_y..=btn_y + btn_h).contains(&y);
        self.hovered_back =
            (b_back_x..=b_back_x + b_back_w).contains(&x) && (btn_y..=btn_y + btn_h).contains(&y);
    }

    pub fn handle_mouse_down(
        &mut self,
        worlds: &[WorldMeta],
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    ) -> SelectWorldAction {
        self.handle_mouse_move(worlds, x, y, width, height);

        if self.hovered_back {
            return SelectWorldAction::Back;
        }

        if self.hovered_create {
            return SelectWorldAction::OpenCreate;
        }

        if self.hovered_play {
            if let Some(w) = worlds.get(self.selected) {
                return SelectWorldAction::Play(w.clone());
            }
        }

        if self.hovered_delete {
            if let Some(w) = worlds.get(self.selected) {
                return SelectWorldAction::Delete(w.id.clone());
            }
        }

        if let Some(idx) = self.hovered_row {
            self.selected = idx;
        }

        SelectWorldAction::None
    }

    pub fn handle_key_down(
        &mut self,
        worlds: &[WorldMeta],
        key: winit::keyboard::KeyCode,
    ) -> SelectWorldAction {
        use winit::keyboard::KeyCode;
        match key {
            KeyCode::Escape => SelectWorldAction::Back,
            KeyCode::ArrowUp => {
                if !worlds.is_empty() {
                    self.selected = (self.selected + worlds.len() - 1) % worlds.len();
                }
                SelectWorldAction::None
            }
            KeyCode::ArrowDown => {
                if !worlds.is_empty() {
                    self.selected = (self.selected + 1) % worlds.len();
                }
                SelectWorldAction::None
            }
            KeyCode::Enter => {
                if let Some(w) = worlds.get(self.selected) {
                    SelectWorldAction::Play(w.clone())
                } else {
                    SelectWorldAction::OpenCreate
                }
            }
            _ => SelectWorldAction::None,
        }
    }

    pub fn build_vertices(&self, worlds: &[WorldMeta], width: f32, height: f32) -> Vec<HudVertex> {
        let mut out = Vec::with_capacity(8192);

        // Header
        push_rect(&mut out, 0.0, 0.0, width, 64.0, [0, 0, 0, 130]);
        push_rect(&mut out, 0.0, 63.0, width, 1.0, [50, 60, 80, 160]);

        draw_text(
            &mut out,
            FontKind::Headline,
            "SELECT WORLD",
            40.0,
            18.0,
            1.0,
            [255, 255, 255, 255],
        );

        // World Cards List
        let list_x = 40.0;
        let list_w = width - 80.0;
        let list_y = 80.0;
        let card_h = 60.0;
        let card_gap = 8.0;

        if worlds.is_empty() {
            let msg = "NO WORLDS FOUND. CLICK 'CREATE NEW WORLD' TO START ADVENTURE!";
            let tw = measure_text(FontKind::Regular, msg, 1.0);
            let cx = (width - tw) * 0.5;
            draw_text(
                &mut out,
                FontKind::Regular,
                msg,
                cx,
                220.0,
                1.0,
                [180, 195, 215, 255],
            );
        } else {
            for (i, w) in worlds.iter().enumerate() {
                let cy = list_y + i as f32 * (card_h + card_gap);
                if cy + card_h > height - 70.0 {
                    break;
                }

                let is_sel = self.selected == i;
                let bg_col = if is_sel {
                    [32, 50, 75, 200]
                } else {
                    [0, 0, 0, 110]
                };

                push_rect(&mut out, list_x, cy, list_w, card_h, bg_col);

                let border_col = if is_sel {
                    [80, 230, 160, 255]
                } else {
                    [55, 65, 85, 140]
                };
                push_rect(&mut out, list_x, cy, list_w, 1.0, border_col);
                push_rect(&mut out, list_x, cy + card_h - 1.0, list_w, 1.0, border_col);
                push_rect(&mut out, list_x, cy, 1.0, card_h, border_col);
                push_rect(&mut out, list_x + list_w - 1.0, cy, 1.0, card_h, border_col);

                if is_sel {
                    push_rect(&mut out, list_x, cy, 4.0, card_h, [80, 230, 160, 255]);
                }

                // World Name
                let name_col = if is_sel {
                    [255, 255, 255, 255]
                } else {
                    [210, 220, 235, 240]
                };
                draw_text(
                    &mut out,
                    FontKind::Headline,
                    &w.name,
                    list_x + 20.0,
                    cy + 12.0,
                    0.85,
                    name_col,
                );

                // Subtitle details
                let detail = format!("SEED {}   |   FOLDER saves/{}", w.seed, w.id);
                draw_text(
                    &mut out,
                    FontKind::Regular,
                    &detail,
                    list_x + 20.0,
                    cy + 36.0,
                    0.85,
                    [150, 170, 195, 220],
                );
            }
        }

        // Bottom Action Bar
        let bot_h = 56.0;
        let bot_y = height - bot_h;
        push_rect(&mut out, 0.0, bot_y, width, bot_h, [0, 0, 0, 140]);
        push_rect(&mut out, 0.0, bot_y, width, 1.0, [50, 60, 80, 160]);

        let btn_h = 38.0;
        let btn_y = bot_y + 9.0;
        let b_play_x = 40.0;
        let b_play_w = 210.0;
        let b_create_x = b_play_x + b_play_w + 14.0;
        let b_create_w = 180.0;
        let b_del_x = b_create_x + b_create_w + 14.0;
        let b_del_w = 120.0;
        let b_back_x = width - 140.0;
        let b_back_w = 100.0;

        // Button 1: [ PLAY SELECTED WORLD ]
        let has_world = !worlds.is_empty();
        let play_bg = if !has_world {
            [40, 45, 55, 120]
        } else if self.hovered_play {
            [95, 245, 175, 255]
        } else {
            [80, 230, 160, 240]
        };
        push_rect(&mut out, b_play_x, btn_y, b_play_w, btn_h, play_bg);
        push_rect(
            &mut out,
            b_play_x,
            btn_y,
            b_play_w,
            1.0,
            [180, 255, 220, 255],
        );
        push_rect(
            &mut out,
            b_play_x,
            btn_y + btn_h - 1.0,
            b_play_w,
            1.0,
            [40, 160, 100, 255],
        );
        let pt_text = "PLAY SELECTED WORLD";
        let ptw = measure_text(FontKind::Headline, pt_text, 0.65);
        let ptx = b_play_x + (b_play_w - ptw) * 0.5;
        let text_col = if has_world {
            [15, 30, 20, 255]
        } else {
            [120, 130, 140, 200]
        };
        draw_text(
            &mut out,
            FontKind::Headline,
            pt_text,
            ptx,
            btn_y + 11.0,
            0.65,
            text_col,
        );

        // Button 2: [ CREATE NEW WORLD ]
        let create_bg = if self.hovered_create {
            [45, 60, 85, 220]
        } else {
            [25, 32, 48, 170]
        };
        push_rect(&mut out, b_create_x, btn_y, b_create_w, btn_h, create_bg);
        push_rect(
            &mut out,
            b_create_x,
            btn_y,
            b_create_w,
            1.0,
            [70, 85, 115, 180],
        );
        push_rect(
            &mut out,
            b_create_x,
            btn_y + btn_h - 1.0,
            b_create_w,
            1.0,
            [70, 85, 115, 180],
        );
        push_rect(&mut out, b_create_x, btn_y, 1.0, btn_h, [70, 85, 115, 180]);
        push_rect(
            &mut out,
            b_create_x + b_create_w - 1.0,
            btn_y,
            1.0,
            btn_h,
            [70, 85, 115, 180],
        );
        let ct_text = "CREATE NEW WORLD";
        let ctw = measure_text(FontKind::Headline, ct_text, 0.65);
        let ctx = b_create_x + (b_create_w - ctw) * 0.5;
        draw_text(
            &mut out,
            FontKind::Headline,
            ct_text,
            ctx,
            btn_y + 11.0,
            0.65,
            [255, 255, 255, 255],
        );

        // Button 3: [ DELETE ]
        if has_world {
            let del_bg = if self.hovered_delete {
                [210, 45, 45, 240]
            } else {
                [140, 30, 30, 190]
            };
            push_rect(&mut out, b_del_x, btn_y, b_del_w, btn_h, del_bg);
            push_rect(&mut out, b_del_x, btn_y, b_del_w, 1.0, [255, 120, 120, 255]);
            push_rect(
                &mut out,
                b_del_x,
                btn_y + btn_h - 1.0,
                b_del_w,
                1.0,
                [100, 20, 20, 255],
            );
            let d_text = "DELETE";
            let dtw = measure_text(FontKind::Headline, d_text, 0.65);
            let dtx = b_del_x + (b_del_w - dtw) * 0.5;
            draw_text(
                &mut out,
                FontKind::Headline,
                d_text,
                dtx,
                btn_y + 11.0,
                0.65,
                [255, 255, 255, 255],
            );
        }

        // Button 4: [ BACK ]
        let back_bg = if self.hovered_back {
            [50, 60, 80, 220]
        } else {
            [0, 0, 0, 130]
        };
        push_rect(&mut out, b_back_x, btn_y, b_back_w, btn_h, back_bg);
        push_rect(&mut out, b_back_x, btn_y, b_back_w, 1.0, [65, 75, 95, 160]);
        push_rect(
            &mut out,
            b_back_x,
            btn_y + btn_h - 1.0,
            b_back_w,
            1.0,
            [65, 75, 95, 160],
        );
        push_rect(&mut out, b_back_x, btn_y, 1.0, btn_h, [65, 75, 95, 160]);
        push_rect(
            &mut out,
            b_back_x + b_back_w - 1.0,
            btn_y,
            1.0,
            btn_h,
            [65, 75, 95, 160],
        );
        let b_text = "BACK";
        let btw = measure_text(FontKind::Headline, b_text, 0.65);
        let btx = b_back_x + (b_back_w - btw) * 0.5;
        draw_text(
            &mut out,
            FontKind::Headline,
            b_text,
            btx,
            btn_y + 11.0,
            0.65,
            [210, 220, 235, 255],
        );

        out
    }
}

// ==========================================
// CREATE WORLD MENU
// ==========================================

#[derive(Debug, Clone, PartialEq)]
pub enum CreateWorldAction {
    None,
    Create { name: String, seed: String },
    Cancel,
}

pub struct CreateWorldMenu {
    pub name_input: String,
    pub seed_input: String,
    pub active_field: usize, // 0 = Name, 1 = Seed
    pub hovered_create: bool,
    pub hovered_cancel: bool,
    pub hovered_name_field: bool,
    pub hovered_seed_field: bool,
}

impl CreateWorldMenu {
    pub fn new() -> Self {
        Self {
            name_input: "New World".to_string(),
            seed_input: String::new(),
            active_field: 0,
            hovered_create: false,
            hovered_cancel: false,
            hovered_name_field: false,
            hovered_seed_field: false,
        }
    }

    pub fn handle_mouse_move(&mut self, x: f32, y: f32, width: f32, height: f32) {
        let box_x = (width - 440.0) * 0.5;
        let box_y = 120.0;
        let input_w = 440.0;
        let input_h = 42.0;

        let name_y = box_y + 30.0;
        let seed_y = box_y + 120.0;

        self.hovered_name_field =
            (box_x..=box_x + input_w).contains(&x) && (name_y..=name_y + input_h).contains(&y);
        self.hovered_seed_field =
            (box_x..=box_x + input_w).contains(&x) && (seed_y..=seed_y + input_h).contains(&y);

        let bot_y = height - 60.0;
        let btn_h = 42.0;
        let btn_y = bot_y + 8.0;

        let b_create_x = (width - 340.0) * 0.5;
        let b_create_w = 160.0;
        let b_cancel_x = b_create_x + b_create_w + 20.0;
        let b_cancel_w = 160.0;

        self.hovered_create = (b_create_x..=b_create_x + b_create_w).contains(&x)
            && (btn_y..=btn_y + btn_h).contains(&y);
        self.hovered_cancel = (b_cancel_x..=b_cancel_x + b_cancel_w).contains(&x)
            && (btn_y..=btn_y + btn_h).contains(&y);
    }

    pub fn handle_mouse_down(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    ) -> CreateWorldAction {
        self.handle_mouse_move(x, y, width, height);

        if self.hovered_name_field {
            self.active_field = 0;
            return CreateWorldAction::None;
        }

        if self.hovered_seed_field {
            self.active_field = 1;
            return CreateWorldAction::None;
        }

        if self.hovered_create {
            return CreateWorldAction::Create {
                name: self.name_input.clone(),
                seed: self.seed_input.clone(),
            };
        }

        if self.hovered_cancel {
            return CreateWorldAction::Cancel;
        }

        CreateWorldAction::None
    }

    pub fn handle_key_down(&mut self, key: winit::keyboard::KeyCode) -> CreateWorldAction {
        use winit::keyboard::KeyCode;
        match key {
            KeyCode::Escape => CreateWorldAction::Cancel,
            KeyCode::Enter => CreateWorldAction::Create {
                name: self.name_input.clone(),
                seed: self.seed_input.clone(),
            },
            KeyCode::Tab | KeyCode::ArrowDown | KeyCode::ArrowUp => {
                self.active_field = (self.active_field + 1) % 2;
                CreateWorldAction::None
            }
            KeyCode::Backspace => {
                if self.active_field == 0 {
                    self.name_input.pop();
                } else {
                    self.seed_input.pop();
                }
                CreateWorldAction::None
            }
            _ => CreateWorldAction::None,
        }
    }

    pub fn handle_char_input(&mut self, ch: char) {
        if ch.is_control() {
            return;
        }
        if self.active_field == 0 {
            if self.name_input.len() < 30 {
                self.name_input.push(ch);
            }
        } else if self.seed_input.len() < 30 {
            self.seed_input.push(ch);
        }
    }

    pub fn build_vertices(&self, width: f32, height: f32, time: f32) -> Vec<HudVertex> {
        let mut out = Vec::with_capacity(4096);

        // Header
        push_rect(&mut out, 0.0, 0.0, width, 64.0, [0, 0, 0, 130]);
        push_rect(&mut out, 0.0, 63.0, width, 1.0, [50, 60, 80, 160]);

        draw_text(
            &mut out,
            FontKind::Headline,
            "CREATE NEW WORLD",
            40.0,
            18.0,
            1.0,
            [255, 255, 255, 255],
        );

        let box_x = (width - 440.0) * 0.5;
        let box_y = 120.0;
        let input_w = 440.0;
        let input_h = 42.0;

        // Field 1: World Name
        let name_y = box_y + 30.0;
        draw_text(
            &mut out,
            FontKind::Headline,
            "WORLD NAME",
            box_x,
            box_y,
            0.75,
            [80, 230, 160, 255],
        );

        let name_border = if self.active_field == 0 {
            [80, 230, 160, 255]
        } else {
            [60, 70, 90, 180]
        };
        push_rect(&mut out, box_x, name_y, input_w, input_h, [0, 0, 0, 150]);
        push_rect(&mut out, box_x, name_y, input_w, 1.0, name_border);
        push_rect(
            &mut out,
            box_x,
            name_y + input_h - 1.0,
            input_w,
            1.0,
            name_border,
        );
        push_rect(&mut out, box_x, name_y, 1.0, input_h, name_border);
        push_rect(
            &mut out,
            box_x + input_w - 1.0,
            name_y,
            1.0,
            input_h,
            name_border,
        );

        let name_display = if self.name_input.is_empty() {
            "Enter world name..."
        } else {
            &self.name_input
        };
        let name_col = if self.name_input.is_empty() {
            [120, 130, 150, 180]
        } else {
            [255, 255, 255, 255]
        };
        draw_text(
            &mut out,
            FontKind::Regular,
            name_display,
            box_x + 14.0,
            name_y + 11.0,
            1.0,
            name_col,
        );

        // Blinking caret for Name field
        if self.active_field == 0 && (time * 2.0).fract() < 0.5 {
            let nw = measure_text(FontKind::Regular, &self.name_input, 1.0);
            push_rect(
                &mut out,
                box_x + 15.0 + nw,
                name_y + 10.0,
                2.0,
                20.0,
                [255, 255, 255, 255],
            );
        }

        // Field 2: World Seed
        let seed_lbl_y = box_y + 90.0;
        let seed_y = box_y + 120.0;
        draw_text(
            &mut out,
            FontKind::Headline,
            "SEED (OPTIONAL)",
            box_x,
            seed_lbl_y,
            0.75,
            [80, 230, 160, 255],
        );

        let seed_border = if self.active_field == 1 {
            [80, 230, 160, 255]
        } else {
            [60, 70, 90, 180]
        };
        push_rect(&mut out, box_x, seed_y, input_w, input_h, [0, 0, 0, 150]);
        push_rect(&mut out, box_x, seed_y, input_w, 1.0, seed_border);
        push_rect(
            &mut out,
            box_x,
            seed_y + input_h - 1.0,
            input_w,
            1.0,
            seed_border,
        );
        push_rect(&mut out, box_x, seed_y, 1.0, input_h, seed_border);
        push_rect(
            &mut out,
            box_x + input_w - 1.0,
            seed_y,
            1.0,
            input_h,
            seed_border,
        );

        let seed_display = if self.seed_input.is_empty() {
            "Leave blank for random seed"
        } else {
            &self.seed_input
        };
        let seed_col = if self.seed_input.is_empty() {
            [120, 130, 150, 180]
        } else {
            [255, 255, 255, 255]
        };
        draw_text(
            &mut out,
            FontKind::Regular,
            seed_display,
            box_x + 14.0,
            seed_y + 11.0,
            1.0,
            seed_col,
        );

        // Blinking caret for Seed field
        if self.active_field == 1 && (time * 2.0).fract() < 0.5 {
            let sw = measure_text(FontKind::Regular, &self.seed_input, 1.0);
            push_rect(
                &mut out,
                box_x + 15.0 + sw,
                seed_y + 10.0,
                2.0,
                20.0,
                [255, 255, 255, 255],
            );
        }

        // Bottom Action Bar
        let bot_h = 56.0;
        let bot_y = height - bot_h;
        push_rect(&mut out, 0.0, bot_y, width, bot_h, [0, 0, 0, 140]);
        push_rect(&mut out, 0.0, bot_y, width, 1.0, [50, 60, 80, 160]);

        let btn_h = 42.0;
        let btn_y = bot_y + 8.0;
        let b_create_x = (width - 340.0) * 0.5;
        let b_create_w = 160.0;
        let b_cancel_x = b_create_x + b_create_w + 20.0;
        let b_cancel_w = 160.0;

        // Button 1: [ CREATE WORLD ]
        let cr_bg = if self.hovered_create {
            [95, 245, 175, 255]
        } else {
            [80, 230, 160, 240]
        };
        push_rect(&mut out, b_create_x, btn_y, b_create_w, btn_h, cr_bg);
        push_rect(
            &mut out,
            b_create_x,
            btn_y,
            b_create_w,
            1.0,
            [180, 255, 220, 255],
        );
        push_rect(
            &mut out,
            b_create_x,
            btn_y + btn_h - 1.0,
            b_create_w,
            1.0,
            [40, 160, 100, 255],
        );
        let cr_text = "CREATE WORLD";
        let cr_w = measure_text(FontKind::Headline, cr_text, 0.75);
        let cr_x = b_create_x + (b_create_w - cr_w) * 0.5;
        draw_text(
            &mut out,
            FontKind::Headline,
            cr_text,
            cr_x,
            btn_y + 12.0,
            0.75,
            [15, 30, 20, 255],
        );

        // Button 2: [ CANCEL ]
        let cn_bg = if self.hovered_cancel {
            [50, 60, 80, 220]
        } else {
            [25, 32, 45, 160]
        };
        push_rect(&mut out, b_cancel_x, btn_y, b_cancel_w, btn_h, cn_bg);
        push_rect(
            &mut out,
            b_cancel_x,
            btn_y,
            b_cancel_w,
            1.0,
            [65, 75, 95, 160],
        );
        push_rect(
            &mut out,
            b_cancel_x,
            btn_y + btn_h - 1.0,
            b_cancel_w,
            1.0,
            [65, 75, 95, 160],
        );
        let cn_text = "CANCEL";
        let cn_w = measure_text(FontKind::Headline, cn_text, 0.75);
        let cn_x = b_cancel_x + (b_cancel_w - cn_w) * 0.5;
        draw_text(
            &mut out,
            FontKind::Headline,
            cn_text,
            cn_x,
            btn_y + 12.0,
            0.75,
            [210, 220, 235, 255],
        );

        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mouse_down_on_play_returns_open_select_world() {
        let mut menu = MainMenu::new();
        let width = 800.0;
        let height = 600.0;
        let play_x = 400.0;
        let play_y = height - 70.0 + 20.0;
        let action = menu.handle_mouse_down(play_x, play_y, width, height);
        assert_eq!(action, MainMenuAction::OpenSelectWorld);
    }

    #[test]
    fn enter_or_space_triggers_open_select_world() {
        let mut menu = MainMenu::new();
        assert_eq!(
            menu.handle_key_down(winit::keyboard::KeyCode::Enter),
            MainMenuAction::OpenSelectWorld
        );
        assert_eq!(
            menu.handle_key_down(winit::keyboard::KeyCode::Space),
            MainMenuAction::OpenSelectWorld
        );
    }

    #[test]
    fn select_world_menu_navigation() {
        let mut menu = SelectWorldMenu::new();
        let worlds = vec![
            WorldMeta::new("w1".into(), "World 1".into(), 100),
            WorldMeta::new("w2".into(), "World 2".into(), 200),
        ];

        assert_eq!(menu.selected, 0);
        let action = menu.handle_key_down(&worlds, winit::keyboard::KeyCode::ArrowDown);
        assert_eq!(action, SelectWorldAction::None);
        assert_eq!(menu.selected, 1);

        let play_action = menu.handle_key_down(&worlds, winit::keyboard::KeyCode::Enter);
        assert_eq!(play_action, SelectWorldAction::Play(worlds[1].clone()));
    }

    #[test]
    fn create_world_menu_text_editing() {
        let mut menu = CreateWorldMenu::new();
        assert_eq!(menu.name_input, "New World");

        menu.active_field = 0;
        menu.handle_char_input('!');
        assert_eq!(menu.name_input, "New World!");

        menu.handle_key_down(winit::keyboard::KeyCode::Backspace);
        assert_eq!(menu.name_input, "New World");

        // Tab switches field
        menu.handle_key_down(winit::keyboard::KeyCode::Tab);
        assert_eq!(menu.active_field, 1);
        menu.handle_char_input('4');
        menu.handle_char_input('2');
        assert_eq!(menu.seed_input, "42");
    }
}
