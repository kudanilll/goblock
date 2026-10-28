//! Game state: camera, world, and per-frame update rules.

use std::time::Instant;

use glam::Vec3;

use goblock_input::{Camera, InputState};
use goblock_world::{BlockId, World, WorldEvent, CHUNK_SIZE};

/// Chunks kept meshed around the player.
pub const VIEW_DISTANCE: i32 = 16;

pub struct Game {
    pub camera: Camera,
    pub world: World,
    screen_width: f32,
    screen_height: f32,
    start: Instant,
    last_frame: Instant,
    fps_accum: f32,
    fps_frames: u32,
    fps: f32,
}

impl Game {
    /// Start a world for `seed` with the camera hovering over dry land near
    /// the origin.
    pub fn new(seed: u64) -> std::io::Result<Self> {
        let world = World::new(seed, VIEW_DISTANCE)?;
        let (x, z) = world.terrain().find_spawn();
        let ground = world.terrain().sample(x, z).height;

        let mut camera = Camera::default();
        camera.position = Vec3::new(x as f32 + 0.5, (ground + 12) as f32, z as f32 + 0.5);

        Ok(Self {
            camera,
            world,
            screen_width: 800.0,
            screen_height: 600.0,
            start: Instant::now(),
            last_frame: Instant::now(),
            fps_accum: 0.0,
            fps_frames: 0,
            fps: 0.0,
        })
    }

    pub fn last_frame(&self) -> Instant {
        self.last_frame
    }

    pub fn set_last_frame(&mut self, now: Instant) {
        self.last_frame = now;
    }

    /// Seconds since the game started.
    pub fn elapsed(&self) -> f32 {
        self.start.elapsed().as_secs_f32()
    }

    pub fn underwater(&self) -> bool {
        let eye = self.camera.position.floor().as_ivec3();
        self.world.block_at(eye) == BlockId::Water
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

    /// Distance at which fog hides the world completely: a chunk and a half
    /// short of the edge of the meshed area, wherever the camera stands in its
    /// own chunk.
    pub fn fog_end(&self) -> f32 {
        (VIEW_DISTANCE as f32 - 1.5) * CHUNK_SIZE as f32
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

        let mut direction = Vec3::ZERO;
        direction += self.camera.front * input.move_axes.y;
        direction += self.camera.right * input.move_axes.x;
        direction += Vec3::Y * input.vertical;

        if direction.length_squared() > 0.0 {
            let speed = if input.sprint {
                self.camera.movement_speed() * 2.5
            } else if input.vertical != 0.0 && input.move_axes == glam::Vec2::ZERO {
                self.camera.vertical_speed()
            } else {
                self.camera.movement_speed()
            };
            let world = &self.world;
            self.camera
                .move_and_collide(direction, delta_time, speed, |p| world.is_solid(p));
        }
    }

    /// Load and unload chunks around the camera.
    pub fn stream(&mut self) -> Vec<WorldEvent> {
        self.world.update(self.camera.position)
    }

    pub fn hud_lines(&self, gpu_chunks: usize) -> Vec<String> {
        let p = self.camera.position;
        let biome = self.world.biome_at(p.x.floor() as i32, p.z.floor() as i32);
        vec![
            format!("POS X{:.0} Y{:.0} Z{:.0}", p.x, p.y, p.z),
            format!("FPS {:.0}", self.fps),
            format!("BIOME {}", biome.name().to_uppercase()),
            format!("CHUNKS {gpu_chunks}"),
            // Signed, so it can be typed back on the command line as is.
            format!("SEED {}", self.world.terrain().seed() as i64),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camera_starts_above_the_ground() {
        let game = Game::new(3).unwrap();
        let p = game.camera.position;
        let ground = game
            .world
            .terrain()
            .sample(p.x.floor() as i32, p.z.floor() as i32);
        assert!(p.y > ground.height as f32 + 1.0);
    }

    #[test]
    fn hud_reports_position_fps_biome_chunks_and_seed() {
        let game = Game::new(u64::MAX).unwrap();
        let lines = game.hud_lines(7);
        assert_eq!(lines.len(), 5);
        assert!(lines[0].starts_with("POS "));
        assert!(lines[1].starts_with("FPS "));
        assert!(lines[2].starts_with("BIOME "));
        assert_eq!(lines[3], "CHUNKS 7");
        assert_eq!(lines[4], "SEED -1");
    }

    #[test]
    fn fog_closes_before_the_loaded_edge() {
        let game = Game::new(3).unwrap();
        let edge = (VIEW_DISTANCE * CHUNK_SIZE) as f32;
        assert!(game.fog_end() < edge - CHUNK_SIZE as f32);
    }

    #[test]
    fn aspect_uses_screen_size() {
        let mut game = Game::new(3).unwrap();
        game.set_screen_size(winit::dpi::PhysicalSize::new(1600, 800));
        assert!((game.aspect() - 2.0).abs() < 1e-6);
    }

    #[test]
    fn zero_height_aspect_is_safe() {
        let mut game = Game::new(3).unwrap();
        game.set_screen_size(winit::dpi::PhysicalSize::new(0, 0));
        assert_eq!(game.aspect(), 1.0);
    }
}
