//! Game state: camera, world, and per-frame update rules.

use std::time::Instant;

use glam::Vec3;

use goblock_input::{Camera, InputState};
use goblock_world::World;

pub struct Game {
    pub camera: Camera,
    pub world: World,
    blocks: Vec<goblock_world::Block>,
    screen_width: f32,
    screen_height: f32,
    start: Instant,
    last_frame: Instant,
    fps_accum: f32,
    fps_frames: u32,
    fps: f32,
}

impl Game {
    pub fn new() -> Self {
        let world = World::generate();
        let blocks = world.all_blocks();
        let now = Instant::now();
        Self {
            camera: Camera::default(),
            world,
            blocks,
            screen_width: 800.0,
            screen_height: 600.0,
            start: now,
            last_frame: now,
            fps_accum: 0.0,
            fps_frames: 0,
            fps: 0.0,
        }
    }

    pub fn last_frame(&self) -> Instant {
        self.last_frame
    }

    pub fn set_last_frame(&mut self, now: Instant) {
        self.last_frame = now;
    }

    pub fn elapsed_secs(&self) -> f32 {
        self.start.elapsed().as_secs_f32()
    }

    pub fn set_screen_size(&mut self, size: winit::dpi::PhysicalSize<u32>) {
        self.screen_width = size.width as f32;
        self.screen_height = size.height as f32;
    }

    pub fn aspect(&self) -> f32 {
        if self.screen_height > 0.0 {
            self.screen_width / self.screen_height
        } else {
            1.0
        }
    }

    pub fn update(&mut self, delta_time: f32, input: &InputState) {
        if delta_time > 0.0 {
            self.fps_accum += delta_time;
            self.fps_frames += 1;
            if self.fps_accum >= 1.0 {
                self.fps = self.fps_frames as f32 / self.fps_accum;
                self.fps_accum = 0.0;
                self.fps_frames = 0;
            }
        }

        self.camera
            .process_mouse_delta(input.look_delta.x, input.look_delta.y);

        let forward = self.camera.front;
        let right = self.camera.right;
        let mut direction = Vec3::ZERO;
        direction += forward * input.move_axes.y;
        direction += right * input.move_axes.x;
        direction += Vec3::Y * input.vertical;

        if direction.length_squared() > 0.0 {
            let speed = if input.sprint {
                self.camera.movement_speed() * 2.5
            } else if input.vertical != 0.0 && input.move_axes == glam::Vec2::ZERO {
                self.camera.vertical_speed()
            } else {
                self.camera.movement_speed()
            };
            self.camera
                .move_and_collide(direction, delta_time, speed, &self.blocks);
        }
    }

    pub fn hud_lines(&self) -> Vec<String> {
        let p = self.camera.position;
        vec![
            format!("POS X{:.0} Y{:.0} Z{:.0}", p.x, p.y, p.z),
            format!("FPS {:.0}", self.fps),
        ]
    }
}

impl Default for Game {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_game_has_world_and_camera() {
        let game = Game::new();
        assert!(!game.world.chunks().is_empty());
        assert!(game.camera.front.length() > 0.9);
    }

    #[test]
    fn hud_reports_position_and_fps() {
        let game = Game::new();
        let lines = game.hud_lines();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].starts_with("POS "));
        assert!(lines[1].starts_with("FPS "));
    }

    #[test]
    fn aspect_uses_screen_size() {
        let mut game = Game::new();
        game.set_screen_size(winit::dpi::PhysicalSize::new(1600, 800));
        assert!((game.aspect() - 2.0).abs() < 1e-6);
    }

    #[test]
    fn zero_height_aspect_is_safe() {
        let mut game = Game::new();
        game.set_screen_size(winit::dpi::PhysicalSize::new(0, 0));
        assert_eq!(game.aspect(), 1.0);
    }
}
