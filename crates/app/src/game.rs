//! Game state: camera, world, and per-frame update rules.

use std::time::Instant;

use glam::DVec3;

use goblock_input::{Camera, InputState, Player};
use goblock_world::{World, WorldEvent, CHUNK_SIZE};

/// Chunks kept meshed around the player.
pub const VIEW_DISTANCE: i32 = 16;

pub struct Game {
    /// Rides on the player's eyes.
    pub camera: Camera,
    pub player: Player,
    pub world: World,
    screen_width: f32,
    screen_height: f32,
    start: Instant,
}

impl Game {
    /// Start a world for `seed` with the player above dry land near the
    /// origin. The player drops onto the ground, or onto a tree, once the
    /// chunk below has loaded.
    pub fn new(seed: u64) -> std::io::Result<Self> {
        let world = World::new(seed, VIEW_DISTANCE)?;
        let (x, z) = world.terrain().find_spawn();
        let ground = world.terrain().sample(x, z).height;

        let player = Player::new(DVec3::new(
            x as f64 + 0.5,
            (ground + 12) as f64,
            z as f64 + 0.5,
        ));
        let mut camera = Camera::default();
        camera.position = player.eye();

        Ok(Self {
            camera,
            player,
            world,
            screen_width: 800.0,
            screen_height: 600.0,
            start: Instant::now(),
        })
    }

    /// Seconds since the game started.
    pub fn elapsed(&self) -> f32 {
        self.start.elapsed().as_secs_f32()
    }

    pub fn underwater(&self) -> bool {
        self.world.is_underwater(self.camera.position)
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
        self.camera
            .process_mouse_delta(input.look_delta.x, input.look_delta.y);
        let sprinting = input.sprint && input.move_axes != glam::Vec2::ZERO;
        self.camera.ease_fov(sprinting, delta_time);

        // Unloaded chunks read as air, so the player would fall straight
        // through them. Hold still until the ground below exists.
        let feet = self.player.feet;
        if self
            .world
            .is_loaded(feet.x.floor() as i32, feet.z.floor() as i32)
        {
            let world = &self.world;
            self.player.update(
                input,
                self.camera.front,
                self.camera.right,
                delta_time,
                |cell| world.block_at(cell).is_solid(),
            );
        }
        self.camera.position = self.player.eye();
    }

    /// Load and unload chunks around the camera.
    pub fn stream(&mut self) -> Vec<WorldEvent> {
        self.world.update(self.camera.position)
    }

    pub fn hud_lines(&self, gpu_chunks: usize, fps: f32) -> Vec<String> {
        let p = self.camera.position;
        let biome = self.world.biome_at(p.x.floor() as i32, p.z.floor() as i32);
        vec![
            format!("POS X{:.0} Y{:.0} Z{:.0}", p.x, p.y, p.z),
            format!("FPS {:.0}", fps),
            format!("BIOME {}", biome.name().to_uppercase()),
            format!("MODE {}", if self.player.flying { "FLY" } else { "WALK" }),
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
        let lines = game.hud_lines(7, 60.0);
        assert_eq!(lines.len(), 6);
        assert!(lines[0].starts_with("POS "));
        assert!(lines[1].starts_with("FPS "));
        assert!(lines[2].starts_with("BIOME "));
        assert_eq!(lines[3], "MODE WALK");
        assert_eq!(lines[4], "CHUNKS 7");
        assert_eq!(lines[5], "SEED -1");
    }

    #[test]
    fn the_player_waits_for_the_ground_to_load() {
        // Nothing has streamed in yet, so the space below reads as air.
        let mut game = Game::new(3).unwrap();
        let before = game.player.feet;
        for _ in 0..30 {
            game.update(1.0 / 60.0, &InputState::default());
        }
        assert_eq!(game.player.feet, before);
        assert_eq!(game.camera.position, game.player.eye());
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
