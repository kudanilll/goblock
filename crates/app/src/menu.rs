//! Title screen, World Select, and Create World menus.

use crate::saves::WorldMeta;
use crate::screens::Transition;
use crate::ui::{self, Rect, Rgba, Widget};
use goblock_render::{draw_text, measure_text, push_rect, FontKind, HudVertex};

const WHITE: Rgba = [255, 255, 255, 255];
const GREEN: Rgba = [80, 230, 160, 255];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TitleItem {
    Play,
    Settings,
    Quit,
}

pub struct MainMenu {
    hovered: Option<TitleItem>,
}

impl MainMenu {
    pub fn new() -> Self {
        Self { hovered: None }
    }

    fn widgets(width: f32, height: f32) -> [Widget<TitleItem>; 3] {
        let y = height - 57.0;
        let quit = Rect::new(width - 150.0, y, 110.0, 44.0);
        let settings = Rect::new(quit.x - 144.0, y, 130.0, 44.0);
        let play = Rect::new(settings.x - 178.0, y, 160.0, 44.0);
        [
            Widget {
                rect: play,
                action: TitleItem::Play,
            },
            Widget {
                rect: settings,
                action: TitleItem::Settings,
            },
            Widget {
                rect: quit,
                action: TitleItem::Quit,
            },
        ]
    }

    pub fn handle_mouse_move(&mut self, x: f32, y: f32, width: f32, height: f32) {
        self.hovered = ui::hit(&Self::widgets(width, height), x, y);
    }

    pub fn handle_mouse_down(&mut self, x: f32, y: f32, width: f32, height: f32) -> Transition {
        self.handle_mouse_move(x, y, width, height);
        match self.hovered {
            Some(TitleItem::Play) => Transition::OpenWorldList,
            Some(TitleItem::Settings) => Transition::OpenSettings { from_title: true },
            Some(TitleItem::Quit) => Transition::Quit,
            None => Transition::None,
        }
    }

    pub fn handle_key_down(&mut self, key: winit::keyboard::KeyCode) -> Transition {
        use winit::keyboard::KeyCode;
        match key {
            KeyCode::Enter | KeyCode::Space => Transition::OpenWorldList,
            KeyCode::KeyS => Transition::OpenSettings { from_title: true },
            KeyCode::Escape => Transition::Quit,
            _ => Transition::None,
        }
    }

    pub fn build_vertices(&self, fps: f32, width: f32, height: f32) -> Vec<HudVertex> {
        let mut out = Vec::with_capacity(4096);

        ui::header_bar(&mut out, width, 80.0, [0, 0, 0, 110], [45, 55, 75, 120]);
        draw_text(
            &mut out,
            FontKind::Headline,
            "GOBLOCK",
            36.0,
            20.0,
            1.5,
            WHITE,
        );
        draw_text(
            &mut out,
            FontKind::Regular,
            "ENDLESS VOXEL WORLD",
            210.0,
            27.0,
            0.9,
            GREEN,
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

        let bot_y = height - 70.0;
        ui::action_bar(
            &mut out,
            width,
            height,
            70.0,
            [0, 0, 0, 140],
            [50, 60, 80, 160],
        );

        let dot_x = 36.0;
        let dot_y = bot_y + 24.0;
        push_rect(&mut out, dot_x, dot_y + 4.0, 8.0, 8.0, GREEN);
        draw_text(
            &mut out,
            FontKind::Headline,
            "READY",
            dot_x + 16.0,
            dot_y + 2.0,
            0.65,
            WHITE,
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

        draw_text(
            &mut out,
            FontKind::Regular,
            &format!("{:.0} FPS", fps),
            width * 0.46,
            bot_y + 28.0,
            0.9,
            [140, 155, 175, 180],
        );

        for widget in Self::widgets(width, height) {
            let hovered = self.hovered == Some(widget.action);
            let r = widget.rect;
            match widget.action {
                TitleItem::Play => {
                    let bg = if hovered {
                        [95, 245, 175, 255]
                    } else {
                        [80, 230, 160, 230]
                    };
                    ui::panel(
                        &mut out,
                        r,
                        bg,
                        [180, 255, 220, 255],
                        [40, 160, 100, 255],
                        2.0,
                    );
                    ui::centered_text(
                        &mut out,
                        FontKind::Headline,
                        "PLAY",
                        r,
                        9.0,
                        1.1,
                        [15, 30, 20, 255],
                    );
                }
                TitleItem::Settings => {
                    let (bg, border) = if hovered {
                        ([40, 52, 72, 220], [80, 160, 240, 255])
                    } else {
                        ([20, 26, 38, 160], [55, 65, 85, 180])
                    };
                    ui::outlined(&mut out, r, bg, border);
                    ui::centered_text(
                        &mut out,
                        FontKind::Headline,
                        "SETTINGS",
                        r,
                        12.0,
                        0.75,
                        WHITE,
                    );
                }
                TitleItem::Quit => {
                    let (bg, border) = if hovered {
                        ([50, 25, 25, 220], [240, 80, 80, 255])
                    } else {
                        ([25, 20, 22, 160], [75, 50, 55, 160])
                    };
                    ui::outlined(&mut out, r, bg, border);
                    ui::centered_text(
                        &mut out,
                        FontKind::Headline,
                        "QUIT",
                        r,
                        12.0,
                        0.75,
                        [220, 225, 235, 240],
                    );
                }
            }
        }

        out
    }
}

// ==========================================
// SELECT WORLD MENU
// ==========================================

const LIST_TOP: f32 = 80.0;
const ROW_HEIGHT: f32 = 60.0;
const ROW_STEP: f32 = 68.0;
/// Rows stop this far above the bottom of the screen, clear of the action bar.
const LIST_BOTTOM_MARGIN: f32 = 70.0;

/// How many world rows fit on the screen. Never zero, so the selected world
/// always has a row even in a very short window.
fn visible_rows(height: f32) -> usize {
    let space = height - LIST_TOP - LIST_BOTTOM_MARGIN - ROW_HEIGHT;
    ((space / ROW_STEP).floor() as usize + 1).max(1)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SelectItem {
    Play,
    Create,
    Delete,
    Back,
    Row(usize),
}

pub struct SelectWorldMenu {
    pub selected: usize,
    /// First listed world. [`Self::first_row`] keeps the selection in view.
    scroll: usize,
    hovered: Option<SelectItem>,
    pub pending_delete: Option<String>,
}

impl SelectWorldMenu {
    pub fn new() -> Self {
        Self {
            selected: 0,
            scroll: 0,
            hovered: None,
            pending_delete: None,
        }
    }

    fn first_row(&self, count: usize, height: f32) -> usize {
        let rows = visible_rows(height);
        let first = self
            .scroll
            .min(self.selected)
            .max((self.selected + 1).saturating_sub(rows));
        first.min(count.saturating_sub(rows))
    }

    /// Selects a world. A delete armed for another world does not carry over.
    fn select(&mut self, index: usize, count: usize, height: f32) {
        if self.selected != index {
            self.selected = index;
            self.pending_delete = None;
        }
        self.scroll = self.first_row(count, height);
    }

    /// Buttons are listed top first: they draw in reverse order, so in a narrow
    /// window the button on top of an overlap is the one that takes the click.
    fn widgets(&self, count: usize, width: f32, height: f32) -> Vec<Widget<SelectItem>> {
        let y = height - 47.0;
        let button = |x: f32, w: f32, action: SelectItem| Widget {
            rect: Rect::new(x, y, w, 38.0),
            action,
        };

        let mut widgets = vec![button(width - 140.0, 100.0, SelectItem::Back)];
        if count > 0 {
            widgets.push(button(458.0, 120.0, SelectItem::Delete));
        }
        widgets.push(button(264.0, 180.0, SelectItem::Create));
        widgets.push(button(40.0, 210.0, SelectItem::Play));

        let first = self.first_row(count, height);
        let last = (first + visible_rows(height)).min(count);
        for (slot, index) in (first..last).enumerate() {
            widgets.push(Widget {
                rect: Rect::new(
                    40.0,
                    LIST_TOP + slot as f32 * ROW_STEP,
                    width - 80.0,
                    ROW_HEIGHT,
                ),
                action: SelectItem::Row(index),
            });
        }
        widgets
    }

    pub fn handle_mouse_move(
        &mut self,
        worlds: &[WorldMeta],
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    ) {
        self.hovered = ui::hit(&self.widgets(worlds.len(), width, height), x, y);
    }

    pub fn handle_mouse_down(
        &mut self,
        worlds: &[WorldMeta],
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    ) -> Transition {
        self.handle_mouse_move(worlds, x, y, width, height);

        if self.hovered != Some(SelectItem::Delete) {
            self.pending_delete = None;
        }

        match self.hovered {
            Some(SelectItem::Back) => Transition::OpenTitle,
            Some(SelectItem::Create) => Transition::OpenCreateWorld,
            Some(SelectItem::Play) => self.play_selected(worlds),
            Some(SelectItem::Delete) => self.confirm_delete(worlds),
            Some(SelectItem::Row(index)) => {
                self.select(index, worlds.len(), height);
                Transition::None
            }
            None => Transition::None,
        }
    }

    fn play_selected(&self, worlds: &[WorldMeta]) -> Transition {
        match worlds.get(self.selected) {
            Some(world) => Transition::StartWorld(world.clone()),
            None => Transition::None,
        }
    }

    /// The first click arms the selected world and the second deletes it.
    fn confirm_delete(&mut self, worlds: &[WorldMeta]) -> Transition {
        let Some(world) = worlds.get(self.selected) else {
            return Transition::None;
        };
        if self.pending_delete.as_deref() == Some(world.id.as_str()) {
            self.pending_delete = None;
            Transition::DeleteWorld(world.id.clone())
        } else {
            self.pending_delete = Some(world.id.clone());
            Transition::None
        }
    }

    /// One step of the mouse wheel moves the selection, and the list follows.
    pub fn handle_wheel(&mut self, worlds: &[WorldMeta], lines: f32, height: f32) {
        if worlds.is_empty() || lines == 0.0 {
            return;
        }
        let index = if lines > 0.0 {
            self.selected.saturating_sub(1)
        } else {
            (self.selected + 1).min(worlds.len() - 1)
        };
        self.select(index, worlds.len(), height);
    }

    pub fn handle_key_down(
        &mut self,
        worlds: &[WorldMeta],
        key: winit::keyboard::KeyCode,
        height: f32,
    ) -> Transition {
        use winit::keyboard::KeyCode;
        let count = worlds.len();
        match key {
            KeyCode::Escape => {
                if self.pending_delete.take().is_some() {
                    Transition::None
                } else {
                    Transition::OpenTitle
                }
            }
            KeyCode::ArrowUp if count > 0 => {
                self.select((self.selected + count - 1) % count, count, height);
                Transition::None
            }
            KeyCode::ArrowDown if count > 0 => {
                self.select((self.selected + 1) % count, count, height);
                Transition::None
            }
            KeyCode::Enter => {
                self.pending_delete = None;
                self.play_selected(worlds)
            }
            _ => Transition::None,
        }
    }

    fn draw_row(&self, out: &mut Vec<HudVertex>, world: &WorldMeta, index: usize, r: Rect) {
        let selected = self.selected == index;
        let bg = if selected {
            [32, 50, 75, 200]
        } else if self.hovered == Some(SelectItem::Row(index)) {
            [22, 34, 52, 170]
        } else {
            [0, 0, 0, 110]
        };
        let border = if selected { GREEN } else { [55, 65, 85, 140] };
        ui::outlined(out, r, bg, border);
        if selected {
            ui::fill(out, Rect::new(r.x, r.y, 4.0, r.h), GREEN);
        }

        let name_color = if selected {
            WHITE
        } else {
            [210, 220, 235, 240]
        };
        draw_text(
            out,
            FontKind::Headline,
            &world.name,
            r.x + 20.0,
            r.y + 12.0,
            0.85,
            name_color,
        );
        let detail = format!("SEED {}   |   FOLDER saves/{}", world.seed, world.id);
        draw_text(
            out,
            FontKind::Regular,
            &detail,
            r.x + 20.0,
            r.y + 36.0,
            0.85,
            [150, 170, 195, 220],
        );
    }

    fn draw_button(
        &self,
        out: &mut Vec<HudVertex>,
        worlds: &[WorldMeta],
        widget: &Widget<SelectItem>,
    ) {
        let r = widget.rect;
        let hovered = self.hovered == Some(widget.action);
        match widget.action {
            SelectItem::Play => {
                let has_world = !worlds.is_empty();
                let bg = if !has_world {
                    [40, 45, 55, 120]
                } else if hovered {
                    [95, 245, 175, 255]
                } else {
                    [80, 230, 160, 240]
                };
                let text = if has_world {
                    [15, 30, 20, 255]
                } else {
                    [120, 130, 140, 200]
                };
                ui::banded(out, r, bg, [180, 255, 220, 255], [40, 160, 100, 255]);
                ui::centered_text(
                    out,
                    FontKind::Headline,
                    "PLAY SELECTED WORLD",
                    r,
                    11.0,
                    0.65,
                    text,
                );
            }
            SelectItem::Create => {
                let bg = if hovered {
                    [45, 60, 85, 220]
                } else {
                    [25, 32, 48, 170]
                };
                ui::outlined(out, r, bg, [70, 85, 115, 180]);
                ui::centered_text(
                    out,
                    FontKind::Headline,
                    "CREATE NEW WORLD",
                    r,
                    11.0,
                    0.65,
                    WHITE,
                );
            }
            SelectItem::Delete => {
                let armed = worlds
                    .get(self.selected)
                    .is_some_and(|w| self.pending_delete.as_deref() == Some(w.id.as_str()));
                let label = if armed { "CONFIRM" } else { "DELETE" };
                let bg = if armed || hovered {
                    [220, 45, 45, 240]
                } else {
                    [140, 30, 30, 190]
                };
                ui::panel(out, r, bg, [255, 120, 120, 255], [100, 20, 20, 255], 1.0);
                ui::centered_text(out, FontKind::Headline, label, r, 11.0, 0.65, WHITE);
            }
            SelectItem::Back => {
                let bg = if hovered {
                    [50, 60, 80, 220]
                } else {
                    [0, 0, 0, 130]
                };
                ui::outlined(out, r, bg, [65, 75, 95, 160]);
                ui::centered_text(
                    out,
                    FontKind::Headline,
                    "BACK",
                    r,
                    11.0,
                    0.65,
                    [210, 220, 235, 255],
                );
            }
            SelectItem::Row(_) => {}
        }
    }

    pub fn build_vertices(&self, worlds: &[WorldMeta], width: f32, height: f32) -> Vec<HudVertex> {
        let mut out = Vec::with_capacity(8192);

        ui::header_bar(&mut out, width, 64.0, [0, 0, 0, 130], [50, 60, 80, 160]);
        draw_text(
            &mut out,
            FontKind::Headline,
            "SELECT WORLD",
            40.0,
            18.0,
            1.0,
            WHITE,
        );

        let widgets = self.widgets(worlds.len(), width, height);

        if worlds.is_empty() {
            let msg = "NO WORLDS FOUND. CLICK 'CREATE NEW WORLD' TO START ADVENTURE!";
            let tw = measure_text(FontKind::Regular, msg, 1.0);
            draw_text(
                &mut out,
                FontKind::Regular,
                msg,
                (width - tw) * 0.5,
                220.0,
                1.0,
                [180, 195, 215, 255],
            );
        }
        for widget in &widgets {
            if let SelectItem::Row(index) = widget.action {
                self.draw_row(&mut out, &worlds[index], index, widget.rect);
            }
        }

        ui::action_bar(
            &mut out,
            width,
            height,
            56.0,
            [0, 0, 0, 140],
            [50, 60, 80, 160],
        );
        for widget in widgets.iter().rev() {
            self.draw_button(&mut out, worlds, widget);
        }

        out
    }
}

// ==========================================
// CREATE WORLD MENU
// ==========================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CreateItem {
    NameField,
    SeedField,
    Create,
    Cancel,
}

pub struct CreateWorldMenu {
    pub name_input: String,
    pub seed_input: String,
    pub active_field: usize, // 0 = Name, 1 = Seed
    hovered: Option<CreateItem>,
}

impl CreateWorldMenu {
    pub fn new() -> Self {
        Self {
            name_input: "New World".to_string(),
            seed_input: String::new(),
            active_field: 0,
            hovered: None,
        }
    }

    fn widgets(width: f32, height: f32) -> [Widget<CreateItem>; 4] {
        let field_x = (width - 440.0) * 0.5;
        let create = Rect::new((width - 340.0) * 0.5, height - 48.0, 160.0, 42.0);
        let cancel = Rect::new(create.x + 180.0, create.y, 160.0, 42.0);
        [
            Widget {
                rect: Rect::new(field_x, 150.0, 440.0, 42.0),
                action: CreateItem::NameField,
            },
            Widget {
                rect: Rect::new(field_x, 240.0, 440.0, 42.0),
                action: CreateItem::SeedField,
            },
            Widget {
                rect: create,
                action: CreateItem::Create,
            },
            Widget {
                rect: cancel,
                action: CreateItem::Cancel,
            },
        ]
    }

    pub fn handle_mouse_move(&mut self, x: f32, y: f32, width: f32, height: f32) {
        self.hovered = ui::hit(&Self::widgets(width, height), x, y);
    }

    fn create(&self) -> Transition {
        Transition::CreateWorld {
            name: self.name_input.clone(),
            seed: self.seed_input.clone(),
        }
    }

    pub fn handle_mouse_down(&mut self, x: f32, y: f32, width: f32, height: f32) -> Transition {
        self.handle_mouse_move(x, y, width, height);
        match self.hovered {
            Some(CreateItem::NameField) => {
                self.active_field = 0;
                Transition::None
            }
            Some(CreateItem::SeedField) => {
                self.active_field = 1;
                Transition::None
            }
            Some(CreateItem::Create) => self.create(),
            Some(CreateItem::Cancel) => Transition::OpenWorldList,
            None => Transition::None,
        }
    }

    pub fn handle_key_down(&mut self, key: winit::keyboard::KeyCode) -> Transition {
        use winit::keyboard::KeyCode;
        match key {
            KeyCode::Escape => Transition::OpenWorldList,
            KeyCode::Enter => self.create(),
            KeyCode::Tab | KeyCode::ArrowDown | KeyCode::ArrowUp => {
                self.active_field = (self.active_field + 1) % 2;
                Transition::None
            }
            KeyCode::Backspace => {
                if self.active_field == 0 {
                    self.name_input.pop();
                } else {
                    self.seed_input.pop();
                }
                Transition::None
            }
            _ => Transition::None,
        }
    }

    pub fn handle_char_input(&mut self, ch: char) {
        if !(32..=126).contains(&(ch as u32)) {
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

        ui::header_bar(&mut out, width, 64.0, [0, 0, 0, 130], [50, 60, 80, 160]);
        draw_text(
            &mut out,
            FontKind::Headline,
            "CREATE NEW WORLD",
            40.0,
            18.0,
            1.0,
            WHITE,
        );

        let widgets = Self::widgets(width, height);
        let field = TextField {
            label: "WORLD NAME",
            placeholder: "Enter world name...",
            value: &self.name_input,
            active: self.active_field == 0,
        };
        field.draw(&mut out, widgets[0].rect, time);
        let field = TextField {
            label: "SEED (OPTIONAL)",
            placeholder: "Leave blank for random seed",
            value: &self.seed_input,
            active: self.active_field == 1,
        };
        field.draw(&mut out, widgets[1].rect, time);

        ui::action_bar(
            &mut out,
            width,
            height,
            56.0,
            [0, 0, 0, 140],
            [50, 60, 80, 160],
        );

        let r = widgets[2].rect;
        let bg = if self.hovered == Some(CreateItem::Create) {
            [95, 245, 175, 255]
        } else {
            [80, 230, 160, 240]
        };
        ui::banded(&mut out, r, bg, [180, 255, 220, 255], [40, 160, 100, 255]);
        ui::centered_text(
            &mut out,
            FontKind::Headline,
            "CREATE WORLD",
            r,
            12.0,
            0.75,
            [15, 30, 20, 255],
        );

        let r = widgets[3].rect;
        let bg = if self.hovered == Some(CreateItem::Cancel) {
            [50, 60, 80, 220]
        } else {
            [25, 32, 45, 160]
        };
        ui::banded(&mut out, r, bg, [65, 75, 95, 160], [65, 75, 95, 160]);
        ui::centered_text(
            &mut out,
            FontKind::Headline,
            "CANCEL",
            r,
            12.0,
            0.75,
            [210, 220, 235, 255],
        );

        out
    }
}

/// A labelled text box on the new world screen.
struct TextField<'a> {
    label: &'a str,
    placeholder: &'a str,
    value: &'a str,
    active: bool,
}

impl TextField<'_> {
    fn draw(&self, out: &mut Vec<HudVertex>, r: Rect, time: f32) {
        draw_text(
            out,
            FontKind::Headline,
            self.label,
            r.x,
            r.y - 30.0,
            0.75,
            GREEN,
        );

        let border = if self.active {
            GREEN
        } else {
            [60, 70, 90, 180]
        };
        ui::outlined(out, r, [0, 0, 0, 150], border);

        let (text, color) = if self.value.is_empty() {
            (self.placeholder, [120, 130, 150, 180])
        } else {
            (self.value, WHITE)
        };
        draw_text(
            out,
            FontKind::Regular,
            text,
            r.x + 14.0,
            r.y + 11.0,
            1.0,
            color,
        );

        if self.active && (time * 2.0).fract() < 0.5 {
            let width = measure_text(FontKind::Regular, self.value, 1.0);
            push_rect(out, r.x + 15.0 + width, r.y + 10.0, 2.0, 20.0, WHITE);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use winit::keyboard::KeyCode;

    const W: f32 = 800.0;
    const H: f32 = 600.0;
    const SIZES: [(f32, f32); 3] = [(800.0, 600.0), (1920.0, 1080.0), (3840.0, 2160.0)];
    /// A point on the DELETE button in the default window.
    const DELETE: (f32, f32) = (500.0, 570.0);

    fn worlds(count: usize) -> Vec<WorldMeta> {
        (0..count)
            .map(|i| WorldMeta::new(format!("w{i}"), format!("World {i}"), i as u64))
            .collect()
    }

    fn click(menu: &mut SelectWorldMenu, list: &[WorldMeta], (x, y): (f32, f32)) -> Transition {
        menu.handle_mouse_down(list, x, y, W, H)
    }

    fn rect_of(menu: &SelectWorldMenu, count: usize, item: SelectItem) -> Rect {
        menu.widgets(count, W, H)
            .iter()
            .find(|widget| widget.action == item)
            .expect("widget is listed")
            .rect
    }

    #[test]
    fn title_buttons_fire_at_their_center() {
        for (w, h) in SIZES {
            for widget in MainMenu::widgets(w, h) {
                let (x, y) = widget.rect.center();
                let action = MainMenu::new().handle_mouse_down(x, y, w, h);
                let expected = match widget.action {
                    TitleItem::Play => Transition::OpenWorldList,
                    TitleItem::Settings => Transition::OpenSettings { from_title: true },
                    TitleItem::Quit => Transition::Quit,
                };
                assert_eq!(action, expected, "at {w}x{h}");
            }
        }
    }

    #[test]
    fn world_list_buttons_fire_at_their_center() {
        let list = worlds(3);
        for (w, h) in SIZES {
            let widgets = SelectWorldMenu::new().widgets(list.len(), w, h);
            assert_eq!(widgets.len(), 7, "four buttons and three rows");
            for widget in widgets {
                let (x, y) = widget.rect.center();
                let mut menu = SelectWorldMenu::new();
                let action = menu.handle_mouse_down(&list, x, y, w, h);
                match widget.action {
                    SelectItem::Play => {
                        assert_eq!(action, Transition::StartWorld(list[0].clone()))
                    }
                    SelectItem::Create => assert_eq!(action, Transition::OpenCreateWorld),
                    SelectItem::Back => assert_eq!(action, Transition::OpenTitle),
                    SelectItem::Delete => {
                        assert_eq!(action, Transition::None);
                        assert_eq!(menu.pending_delete.as_deref(), Some("w0"));
                    }
                    SelectItem::Row(index) => {
                        assert_eq!(action, Transition::None);
                        assert_eq!(menu.selected, index);
                    }
                }
            }
        }
    }

    #[test]
    fn create_world_widgets_fire_at_their_center() {
        for (w, h) in SIZES {
            for widget in CreateWorldMenu::widgets(w, h) {
                let (x, y) = widget.rect.center();
                let mut menu = CreateWorldMenu::new();
                menu.active_field = 1 - usize::from(widget.action == CreateItem::SeedField);
                let action = menu.handle_mouse_down(x, y, w, h);
                match widget.action {
                    CreateItem::NameField => assert_eq!(menu.active_field, 0),
                    CreateItem::SeedField => assert_eq!(menu.active_field, 1),
                    CreateItem::Create => {
                        assert!(matches!(action, Transition::CreateWorld { .. }))
                    }
                    CreateItem::Cancel => assert_eq!(action, Transition::OpenWorldList),
                }
            }
        }
    }

    #[test]
    fn delete_button_reacts_across_its_whole_drawn_width() {
        let list = worlds(1);
        let delete = rect_of(&SelectWorldMenu::new(), 1, SelectItem::Delete);
        for x in [delete.x + 1.0, delete.x + delete.w - 1.0] {
            let mut menu = SelectWorldMenu::new();
            click(&mut menu, &list, (x, DELETE.1));
            assert_eq!(menu.pending_delete.as_deref(), Some("w0"), "at x {x}");
        }
    }

    #[test]
    fn create_button_edge_does_not_arm_delete() {
        let list = worlds(1);
        let create = rect_of(&SelectWorldMenu::new(), 1, SelectItem::Create);
        let mut menu = SelectWorldMenu::new();
        let action = click(&mut menu, &list, (create.x + create.w - 1.0, DELETE.1));
        assert_eq!(action, Transition::OpenCreateWorld);
        assert_eq!(menu.pending_delete, None);
    }

    #[test]
    fn back_takes_the_click_where_it_covers_delete() {
        let list = worlds(1);
        let mut menu = SelectWorldMenu::new();
        let action = menu.handle_mouse_down(&list, 540.0, 450.0, 640.0, 480.0);
        assert_eq!(action, Transition::OpenTitle);
        assert_eq!(menu.pending_delete, None);
    }

    #[test]
    fn hover_does_not_change_the_selected_world() {
        let list = worlds(4);
        let mut menu = SelectWorldMenu::new();
        click(&mut menu, &list, (490.0, 110.0));
        assert_eq!(menu.selected, 0);

        // The cursor crosses every row on its way down to the DELETE button.
        for step in 0..=46 {
            menu.handle_mouse_move(&list, 490.0, 110.0 + step as f32 * 10.0, W, H);
        }
        assert_eq!(menu.selected, 0);
        click(&mut menu, &list, DELETE);
        assert_eq!(
            click(&mut menu, &list, DELETE),
            Transition::DeleteWorld("w0".into())
        );
    }

    #[test]
    fn worlds_past_the_list_take_no_clicks() {
        let list = worlds(8);
        let mut menu = SelectWorldMenu::new();
        menu.handle_key_down(&list, KeyCode::ArrowDown, H);

        click(&mut menu, &list, DELETE);
        assert_eq!(menu.pending_delete.as_deref(), Some("w1"));
        assert_eq!(
            click(&mut menu, &list, DELETE),
            Transition::DeleteWorld("w1".into())
        );

        assert_eq!(
            click(&mut menu, &list, (100.0, 570.0)),
            Transition::StartWorld(list[1].clone())
        );
    }

    #[test]
    fn list_shows_as_many_rows_as_fit() {
        assert_eq!(visible_rows(600.0), 6);
        assert_eq!(visible_rows(1080.0), 13);
        assert_eq!(visible_rows(100.0), 1);
    }

    #[test]
    fn selected_world_stays_on_screen() {
        let list = worlds(10);
        let mut menu = SelectWorldMenu::new();
        for expected in 1..10 {
            menu.handle_key_down(&list, KeyCode::ArrowDown, H);
            assert_eq!(menu.selected, expected);
            rect_of(&menu, 10, SelectItem::Row(expected));
        }
        assert_eq!(menu.first_row(10, H), 4);

        // Walking back up leaves the list alone until the top row is passed.
        menu.handle_key_down(&list, KeyCode::ArrowUp, H);
        assert_eq!(menu.first_row(10, H), 4);
        for _ in 0..5 {
            menu.handle_key_down(&list, KeyCode::ArrowUp, H);
        }
        assert_eq!((menu.selected, menu.first_row(10, H)), (3, 3));

        // ArrowDown wraps from the last world to the first.
        for _ in 0..7 {
            menu.handle_key_down(&list, KeyCode::ArrowDown, H);
        }
        assert_eq!((menu.selected, menu.first_row(10, H)), (0, 0));
    }

    #[test]
    fn shrinking_the_window_keeps_the_selection_listed() {
        let list = worlds(10);
        let mut menu = SelectWorldMenu::new();
        for _ in 0..5 {
            menu.handle_key_down(&list, KeyCode::ArrowDown, 1080.0);
        }
        assert_eq!(menu.first_row(10, 1080.0), 0);
        assert_eq!(menu.first_row(10, 400.0), 3);
        let widgets = menu.widgets(10, W, 400.0);
        assert!(widgets
            .iter()
            .any(|widget| widget.action == SelectItem::Row(5)));
    }

    #[test]
    fn wheel_moves_the_selection_and_cancels_delete() {
        let list = worlds(8);
        let mut menu = SelectWorldMenu::new();
        click(&mut menu, &list, DELETE);
        assert!(menu.pending_delete.is_some());

        menu.handle_wheel(&list, -1.0, H);
        assert_eq!(menu.selected, 1);
        assert_eq!(menu.pending_delete, None);

        for _ in 0..20 {
            menu.handle_wheel(&list, -1.0, H);
        }
        assert_eq!(menu.selected, 7);
        rect_of(&menu, 8, SelectItem::Row(7));

        menu.handle_wheel(&list, 1.0, H);
        assert_eq!(menu.selected, 6);
    }

    #[test]
    fn enter_or_space_opens_the_world_list() {
        let mut menu = MainMenu::new();
        assert_eq!(
            menu.handle_key_down(KeyCode::Enter),
            Transition::OpenWorldList
        );
        assert_eq!(
            menu.handle_key_down(KeyCode::Space),
            Transition::OpenWorldList
        );
    }

    #[test]
    fn select_world_menu_navigation() {
        let mut menu = SelectWorldMenu::new();
        let list = worlds(2);

        assert_eq!(menu.selected, 0);
        let action = menu.handle_key_down(&list, KeyCode::ArrowDown, H);
        assert_eq!(action, Transition::None);
        assert_eq!(menu.selected, 1);

        let play = menu.handle_key_down(&list, KeyCode::Enter, H);
        assert_eq!(play, Transition::StartWorld(list[1].clone()));
    }

    #[test]
    fn delete_needs_two_clicks_on_the_same_world() {
        let mut menu = SelectWorldMenu::new();
        let list = worlds(2);
        assert_eq!(click(&mut menu, &list, DELETE), Transition::None);
        assert_eq!(menu.pending_delete.as_deref(), Some("w0"));

        assert_eq!(
            click(&mut menu, &list, DELETE),
            Transition::DeleteWorld("w0".into())
        );
        assert_eq!(menu.pending_delete, None);
    }

    #[test]
    fn changing_selection_cancels_delete() {
        let mut menu = SelectWorldMenu::new();
        let list = worlds(2);
        click(&mut menu, &list, DELETE);
        assert!(menu.pending_delete.is_some());

        menu.handle_key_down(&list, KeyCode::ArrowDown, H);
        assert_eq!(menu.selected, 1);
        assert_eq!(menu.pending_delete, None);

        click(&mut menu, &list, DELETE);
        assert_eq!(menu.pending_delete.as_deref(), Some("w1"));

        click(&mut menu, &list, (100.0, 100.0));
        assert_eq!(menu.selected, 0);
        assert_eq!(menu.pending_delete, None);
    }

    #[test]
    fn hovering_elsewhere_keeps_the_delete_armed() {
        let mut menu = SelectWorldMenu::new();
        let list = worlds(2);
        click(&mut menu, &list, DELETE);
        menu.handle_mouse_move(&list, 100.0, 170.0, W, H);
        assert_eq!(menu.pending_delete.as_deref(), Some("w0"));
    }

    #[test]
    fn clicking_elsewhere_cancels_delete() {
        let mut menu = SelectWorldMenu::new();
        let list = worlds(2);
        click(&mut menu, &list, DELETE);
        assert_eq!(click(&mut menu, &list, (400.0, 30.0)), Transition::None);
        assert_eq!(menu.pending_delete, None);
    }

    #[test]
    fn escape_cancels_delete_before_leaving() {
        let mut menu = SelectWorldMenu::new();
        let list = worlds(1);

        click(&mut menu, &list, DELETE);
        assert!(menu.pending_delete.is_some());

        assert_eq!(
            menu.handle_key_down(&list, KeyCode::Escape, H),
            Transition::None
        );
        assert_eq!(menu.pending_delete, None);
        assert_eq!(
            menu.handle_key_down(&list, KeyCode::Escape, H),
            Transition::OpenTitle
        );
    }

    #[test]
    fn no_worlds_means_no_delete_button_and_no_play() {
        let list = worlds(0);
        let mut menu = SelectWorldMenu::new();
        assert_eq!(click(&mut menu, &list, DELETE), Transition::None);
        assert_eq!(menu.pending_delete, None);
        assert_eq!(click(&mut menu, &list, (100.0, 570.0)), Transition::None);
        assert_eq!(
            menu.handle_key_down(&list, KeyCode::ArrowDown, H),
            Transition::None
        );
    }

    #[test]
    fn non_ascii_input_is_ignored() {
        let mut menu = CreateWorldMenu::new();
        menu.name_input.clear();
        for ch in ['a', 'é', ' ', '世', '~'] {
            menu.handle_char_input(ch);
        }
        assert_eq!(menu.name_input, "a ~");

        menu.active_field = 1;
        menu.seed_input.clear();
        for ch in ['1', 'é', ' ', '世', '~'] {
            menu.handle_char_input(ch);
        }
        assert_eq!(menu.seed_input, "1 ~");
    }

    #[test]
    fn create_world_menu_text_editing() {
        let mut menu = CreateWorldMenu::new();
        assert_eq!(menu.name_input, "New World");

        menu.active_field = 0;
        menu.handle_char_input('!');
        assert_eq!(menu.name_input, "New World!");

        menu.handle_key_down(KeyCode::Backspace);
        assert_eq!(menu.name_input, "New World");

        // Tab switches field
        menu.handle_key_down(KeyCode::Tab);
        assert_eq!(menu.active_field, 1);
        menu.handle_char_input('4');
        menu.handle_char_input('2');
        assert_eq!(menu.seed_input, "42");
    }
}
