//! Layout and drawing helpers shared by the menu screens.
//!
//! A screen builds one list of widgets from its size. Drawing and hit tests
//! both read that list, so a button reacts exactly where it is drawn.

use goblock_render::{draw_text, measure_text, push_rect, FontKind, HudVertex};

pub type Rgba = [u8; 4];

/// Window height that one step of UI zoom covers.
const ZOOM_STEP_HEIGHT: f32 = 600.0;

/// Whole-number UI zoom for a window this tall. Whole steps keep the text
/// sharp, because the font atlas is sampled with nearest filtering.
pub fn scale_for(height: f32) -> f32 {
    (height / ZOOM_STEP_HEIGHT).floor().max(1.0)
}

/// Moves vertices laid out at UI scale onto window pixels.
pub fn scale_vertices(vertices: &mut [HudVertex], scale: f32) {
    for vertex in vertices {
        vertex.pos[0] *= scale;
        vertex.pos[1] *= scale;
    }
}

/// Frames per second, refreshed once a second.
#[derive(Debug, Default)]
pub struct FpsCounter {
    elapsed: f32,
    frames: u32,
    fps: f32,
}

impl FpsCounter {
    pub fn tick(&mut self, dt: f32) {
        if dt <= 0.0 {
            return;
        }
        self.elapsed += dt;
        self.frames += 1;
        if self.elapsed >= 1.0 {
            self.fps = self.frames as f32 / self.elapsed;
            self.elapsed = 0.0;
            self.frames = 0;
        }
    }

    pub fn fps(&self) -> f32 {
        self.fps
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }

    pub fn contains(self, x: f32, y: f32) -> bool {
        (self.x..=self.x + self.w).contains(&x) && (self.y..=self.y + self.h).contains(&y)
    }

    #[cfg(test)]
    pub fn center(self) -> (f32, f32) {
        (self.x + self.w * 0.5, self.y + self.h * 0.5)
    }
}

/// A clickable area and what it stands for on its screen.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Widget<A> {
    pub rect: Rect,
    pub action: A,
}

/// The first widget under the point. Earlier widgets win where two overlap.
pub fn hit<A: Copy>(widgets: &[Widget<A>], x: f32, y: f32) -> Option<A> {
    widgets
        .iter()
        .find(|widget| widget.rect.contains(x, y))
        .map(|widget| widget.action)
}

pub fn fill(out: &mut Vec<HudVertex>, r: Rect, color: Rgba) {
    push_rect(out, r.x, r.y, r.w, r.h, color);
}

/// A filled box with `light` on its top and left edges and `dark` on the rest.
pub fn panel(out: &mut Vec<HudVertex>, r: Rect, color: Rgba, light: Rgba, dark: Rgba, edge: f32) {
    fill(out, r, color);
    push_rect(out, r.x, r.y, r.w, edge, light);
    push_rect(out, r.x, r.y + r.h - edge, r.w, edge, dark);
    push_rect(out, r.x, r.y, edge, r.h, light);
    push_rect(out, r.x + r.w - edge, r.y, edge, r.h, dark);
}

/// A filled box with a one pixel border.
pub fn outlined(out: &mut Vec<HudVertex>, r: Rect, color: Rgba, border: Rgba) {
    panel(out, r, color, border, border, 1.0);
}

/// A filled box with one pixel lines along its top and bottom only.
pub fn banded(out: &mut Vec<HudVertex>, r: Rect, color: Rgba, top: Rgba, bottom: Rgba) {
    fill(out, r, color);
    push_rect(out, r.x, r.y, r.w, 1.0, top);
    push_rect(out, r.x, r.y + r.h - 1.0, r.w, 1.0, bottom);
}

/// Text centered across `r`, `dy` below its top edge.
pub fn centered_text(
    out: &mut Vec<HudVertex>,
    kind: FontKind,
    text: &str,
    r: Rect,
    dy: f32,
    scale: f32,
    color: Rgba,
) {
    let width = measure_text(kind, text, scale);
    draw_text(
        out,
        kind,
        text,
        r.x + (r.w - width) * 0.5,
        r.y + dy,
        scale,
        color,
    );
}

/// The dark strip with a line under it across the top of a screen.
pub fn header_bar(out: &mut Vec<HudVertex>, width: f32, height: f32, color: Rgba, line: Rgba) {
    push_rect(out, 0.0, 0.0, width, height, color);
    push_rect(out, 0.0, height - 1.0, width, 1.0, line);
}

/// The dark strip with a line over it across the bottom of a screen.
pub fn action_bar(
    out: &mut Vec<HudVertex>,
    width: f32,
    height: f32,
    bar_height: f32,
    color: Rgba,
    line: Rgba,
) {
    let y = height - bar_height;
    push_rect(out, 0.0, y, width, bar_height, color);
    push_rect(out, 0.0, y, width, 1.0, line);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zoom_steps_are_whole_numbers() {
        assert_eq!(scale_for(480.0), 1.0);
        assert_eq!(scale_for(600.0), 1.0);
        assert_eq!(scale_for(1080.0), 1.0);
        assert_eq!(scale_for(1200.0), 2.0);
        assert_eq!(scale_for(1440.0), 2.0);
        assert_eq!(scale_for(2160.0), 3.0);
    }

    #[test]
    fn scaling_moves_positions_and_nothing_else() {
        let mut vertices = [HudVertex {
            pos: [10.0, 20.0],
            uv: [0.25, 0.5],
            color: [1, 2, 3, 4],
        }];
        scale_vertices(&mut vertices, 3.0);
        assert_eq!(vertices[0].pos, [30.0, 60.0]);
        assert_eq!(vertices[0].uv, [0.25, 0.5]);
        assert_eq!(vertices[0].color, [1, 2, 3, 4]);
    }

    #[test]
    fn fps_counter_refreshes_once_a_second() {
        let mut counter = FpsCounter::default();
        for _ in 0..59 {
            counter.tick(1.0 / 60.0);
        }
        assert_eq!(counter.fps(), 0.0);
        counter.tick(1.0 / 30.0);
        assert!((counter.fps() - 60.0).abs() < 1.0, "fps {}", counter.fps());
    }

    #[test]
    fn fps_counter_ignores_empty_frames() {
        let mut counter = FpsCounter::default();
        counter.tick(0.0);
        assert_eq!(counter.frames, 0);
    }

    #[test]
    fn rect_edges_count_as_inside() {
        let r = Rect::new(10.0, 20.0, 30.0, 40.0);
        assert!(r.contains(10.0, 20.0));
        assert!(r.contains(40.0, 60.0));
        assert!(!r.contains(40.5, 60.0));
        assert!(!r.contains(10.0, 19.5));
        assert_eq!(r.center(), (25.0, 40.0));
    }

    #[test]
    fn first_widget_wins_where_two_overlap() {
        let widgets = [
            Widget {
                rect: Rect::new(0.0, 0.0, 10.0, 10.0),
                action: 'a',
            },
            Widget {
                rect: Rect::new(0.0, 0.0, 100.0, 100.0),
                action: 'b',
            },
        ];
        assert_eq!(hit(&widgets, 5.0, 5.0), Some('a'));
        assert_eq!(hit(&widgets, 50.0, 50.0), Some('b'));
        assert_eq!(hit(&widgets, 500.0, 500.0), None);
    }
}
