//! Game state: camera, world, and per-frame update rules.

use std::time::Instant;

use glam::{DVec3, IVec3};

use goblock_input::{Camera, InputState, Player};
use goblock_render::HudVertex;
use goblock_world::{BlockId, World, WorldEvent, CHUNK_SIZE};

use crate::effects::{EffectsSystem, MiningTarget};

/// Chunks kept meshed around the player.
pub const VIEW_DISTANCE: i32 = 16;

pub struct Game {
    /// Rides on the player's eyes.
    pub camera: Camera,
    pub player: Player,
    pub world: World,
    pub effects: EffectsSystem,
    screen_width: f32,
    screen_height: f32,
    start: Instant,
    pub spawn_settled: bool,
    break_target: Option<IVec3>,
    break_progress: f32,
    pending_events: Vec<WorldEvent>,
}

impl Game {
    /// Start a world for `seed` with the player standing on the ground near
    /// the origin.
    pub fn new(seed: u64) -> std::io::Result<Self> {
        let world = World::new(seed, VIEW_DISTANCE)?;
        let (x, z) = world.terrain().find_spawn();
        let ground = world.terrain().sample(x, z).height;

        let player = Player::new(DVec3::new(
            x as f64 + 0.5,
            (ground + 1) as f64,
            z as f64 + 0.5,
        ));
        let mut camera = Camera::default();
        camera.position = player.eye();

        Ok(Self {
            camera,
            player,
            world,
            effects: EffectsSystem::new(),
            screen_width: 800.0,
            screen_height: 600.0,
            start: Instant::now(),
            spawn_settled: false,
            break_target: None,
            break_progress: 0.0,
            pending_events: Vec::new(),
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
        (self.world.view_distance() as f32 - 1.5) * CHUNK_SIZE as f32
    }

    pub fn view_distance(&self) -> i32 {
        self.world.view_distance()
    }

    pub fn set_view_distance(&mut self, view_distance: i32) {
        self.world.set_view_distance(view_distance);
    }

    pub fn update(&mut self, delta_time: f32, input: &InputState) {
        self.camera
            .process_mouse_delta(input.look_delta.x, input.look_delta.y);
        let sprinting = input.sprint && input.move_axes != glam::Vec2::ZERO;
        self.camera.ease_fov(sprinting, delta_time);

        // Unloaded chunks read as air, so the player would fall straight
        // through them. Hold still until the ground below exists.
        let feet = self.player.feet;
        let cell_x = feet.x.floor() as i32;
        let cell_z = feet.z.floor() as i32;
        if self.world.is_loaded(cell_x, cell_z) {
            if !self.spawn_settled {
                self.spawn_settled = true;
                let start_y = (feet.y.floor() as i32 + 16).min(goblock_world::WORLD_HEIGHT - 1);
                for y in (0..=start_y).rev() {
                    if self
                        .world
                        .block_at(glam::IVec3::new(cell_x, y, cell_z))
                        .is_solid()
                    {
                        self.player.feet.y = (y + 1) as f64;
                        self.player.on_ground = true;
                        self.player.velocity = glam::Vec3::ZERO;
                        break;
                    }
                }
            }
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

        const REACH_DISTANCE: f32 = 5.0;
        let hit = goblock_world::raycast(
            self.camera.position,
            self.camera.front,
            REACH_DISTANCE,
            |cell| self.world.block_at(cell),
        );

        if let Some(hit) = hit {
            if input.dig {
                if let Some(hardness) = hit.block.hardness() {
                    if self.break_target == Some(hit.block_pos) {
                        if self.player.flying {
                            self.break_progress = 1.0;
                        } else {
                            self.break_progress += delta_time / hardness;
                        }
                    } else {
                        self.break_target = Some(hit.block_pos);
                        self.break_progress = if self.player.flying {
                            1.0
                        } else {
                            delta_time / hardness
                        };
                    }
                    self.effects.target = Some(MiningTarget {
                        pos: hit.block_pos,
                        face: hit.face,
                        block: hit.block,
                        progress: self.break_progress,
                    });
                } else {
                    self.break_target = None;
                    self.break_progress = 0.0;
                    self.effects.target = None;
                }
            } else {
                self.break_target = None;
                self.break_progress = 0.0;
                self.effects.target = Some(MiningTarget {
                    pos: hit.block_pos,
                    face: hit.face,
                    block: hit.block,
                    progress: 0.0,
                });
            }
        } else {
            self.break_target = None;
            self.break_progress = 0.0;
            self.effects.target = None;
        }

        if self.break_progress >= 1.0 {
            if let Some(target_pos) = self.break_target {
                let broken_block = self.world.block_at(target_pos);
                let above_pos = target_pos + IVec3::Y;
                let above_block = self.world.block_at(above_pos);

                let events = self.world.set_block(target_pos, BlockId::Air);
                self.pending_events.extend(events);
                self.effects.spawn_break_burst(target_pos, broken_block);

                if above_block.requires_floor() {
                    self.effects.spawn_break_burst(above_pos, above_block);
                }

                self.break_target = None;
                self.break_progress = 0.0;
                self.effects.target = None;
            }
        }

        let world = &self.world;
        self.effects.update(delta_time, |p| world.is_solid(p));
    }

    /// Load and unload chunks around the camera, and yield any block change events.
    pub fn stream(&mut self) -> Vec<WorldEvent> {
        let mut events = std::mem::take(&mut self.pending_events);
        events.extend(self.world.update(self.camera.position));
        events
    }

    /// Render 3D in-world crack overlay and flying debris particles to `out`.
    pub fn render_effects(&self, out: &mut Vec<HudVertex>, screen_w: f32, screen_h: f32) {
        let view_proj = self.camera.projection_matrix(self.aspect()) * self.camera.view_matrix();
        self.effects
            .render(out, self.camera.position, view_proj, screen_w, screen_h);
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
    fn player_starts_on_the_ground_surface() {
        let game = Game::new(3).unwrap();
        let ground = game.world.terrain().sample(
            game.player.feet.x.floor() as i32,
            game.player.feet.z.floor() as i32,
        );
        assert_eq!(game.player.feet.y, (ground.height + 1) as f64);
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
    fn fog_end_adjusts_with_view_distance() {
        let mut game = Game::new(3).unwrap();
        assert_eq!(game.view_distance(), VIEW_DISTANCE);
        let default_fog = game.fog_end();
        game.set_view_distance(8);
        assert_eq!(game.view_distance(), 8);
        assert!(game.fog_end() < default_fog);
        assert_eq!(game.fog_end(), (8.0 - 1.5) * CHUNK_SIZE as f32);
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

    #[test]
    fn digging_breaks_targeted_block_and_spawns_particles() {
        let mut game = Game::new(3).unwrap();
        for _ in 0..60 {
            game.stream();
            std::thread::sleep(std::time::Duration::from_millis(2));
        }

        // Run one update to settle the player
        game.update(0.1, &InputState::default());

        game.camera.set_orientation(90.0, 0.0);
        let target = game.camera.position.floor().as_ivec3() + IVec3::new(0, 0, 2);
        let _ = game.world.set_block(target, BlockId::Dirt);
        assert_eq!(game.world.block_at(target), BlockId::Dirt);

        let dig_input = InputState {
            dig: true,
            ..Default::default()
        };

        game.player.flying = true;
        game.update(0.1, &dig_input);

        assert_eq!(game.world.block_at(target), BlockId::Air);
        assert!(!game.effects.particles.is_empty());
    }

    #[test]
    fn breaking_ground_destroys_grass_tuft_above() {
        let mut game = Game::new(3).unwrap();
        for _ in 0..60 {
            game.stream();
            std::thread::sleep(std::time::Duration::from_millis(2));
        }

        game.update(0.1, &InputState::default());

        game.camera.set_orientation(90.0, 0.0);
        let target = game.camera.position.floor().as_ivec3() + IVec3::new(0, 0, 2);
        let above = target + IVec3::Y;
        let _ = game.world.set_block(target, BlockId::Dirt);
        let _ = game.world.set_block(above, BlockId::GrassTuft);
        assert_eq!(game.world.block_at(target), BlockId::Dirt);
        assert_eq!(game.world.block_at(above), BlockId::GrassTuft);

        let dig_input = InputState {
            dig: true,
            ..Default::default()
        };

        game.player.flying = true;
        game.update(0.1, &dig_input);

        assert_eq!(game.world.block_at(target), BlockId::Air);
        assert_eq!(game.world.block_at(above), BlockId::Air);
    }
}
