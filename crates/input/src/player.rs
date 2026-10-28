//! The player's body: a box that walks under gravity, jumps, and flies after
//! a double tap on jump.
//!
//! Collision moves the box along one axis at a time and checks every block
//! it would pass through, so it slides along walls and cannot tunnel through
//! a block however long the frame was. Positions are f64: in f32 the spacing
//! between values far from the origin outgrows the gap kept to walls.

use glam::{DVec3, IVec3, Vec3};

use crate::InputState;

/// Width of the body on X and Z, in blocks.
pub const WIDTH: f64 = 0.6;
/// Height of the body, in blocks.
pub const HEIGHT: f64 = 1.8;
/// Height of the eyes above the feet.
pub const EYE_HEIGHT: f64 = 1.62;

const WALK_SPEED: f32 = 4.3;
const SPRINT_SPEED: f32 = 7.0;
const FLY_SPEED: f32 = 15.0;
const FLY_SPRINT_SPEED: f32 = 37.5;
const FLY_VERTICAL_SPEED: f32 = 10.0;
const GRAVITY: f32 = 28.0;
/// Rises about 1.45 blocks (v² / 2g), enough to step onto the next block.
const JUMP_SPEED: f32 = 9.0;
const TERMINAL_SPEED: f32 = 60.0;
/// Two jump presses closer together than this, in seconds, toggle flying.
const DOUBLE_TAP: f32 = 0.3;
/// Longest time simulated in one update, so a stalled frame does not throw
/// the body a long way in one step.
const MAX_STEP: f32 = 0.1;
/// Gap kept between the body and any block it touches.
const SKIN: f64 = 1e-3;

#[derive(Debug, Clone, PartialEq)]
pub struct Player {
    /// Centre of the bottom of the body.
    pub feet: DVec3,
    pub velocity: Vec3,
    pub on_ground: bool,
    pub flying: bool,
    jump_held: bool,
    /// Seconds since jump was last pressed.
    since_jump: f32,
}

impl Player {
    pub fn new(feet: DVec3) -> Self {
        Self {
            feet,
            velocity: Vec3::ZERO,
            on_ground: false,
            flying: false,
            jump_held: false,
            since_jump: f32::INFINITY,
        }
    }

    /// Where the camera goes.
    pub fn eye(&self) -> Vec3 {
        (self.feet + DVec3::Y * EYE_HEIGHT).as_vec3()
    }

    /// Advance by `dt` seconds. `forward` and `right` are the camera's
    /// directions; only their horizontal parts steer, so looking up or down
    /// does not change the walking speed. `solid` reports whether a block
    /// cell stops the body.
    pub fn update(
        &mut self,
        input: &InputState,
        forward: Vec3,
        right: Vec3,
        dt: f32,
        solid: impl Fn(IVec3) -> bool,
    ) {
        let dt = dt.min(MAX_STEP);
        let jump = input.vertical > 0.0;
        let pressed = jump && !self.jump_held;
        self.jump_held = jump;
        self.since_jump += dt;
        if pressed {
            if self.since_jump < DOUBLE_TAP {
                self.flying = !self.flying;
                self.velocity.y = 0.0;
                // A third tap starts a new pair instead of toggling back.
                self.since_jump = f32::INFINITY;
            } else {
                self.since_jump = 0.0;
            }
        }

        let flat = |v: Vec3| Vec3::new(v.x, 0.0, v.z).normalize_or_zero();
        let wish = (flat(forward) * input.move_axes.y + flat(right) * input.move_axes.x)
            .normalize_or_zero();

        if self.flying {
            let speed = if input.sprint {
                FLY_SPRINT_SPEED
            } else {
                FLY_SPEED
            };
            self.velocity = wish * speed + Vec3::Y * input.vertical * FLY_VERTICAL_SPEED;
        } else {
            let speed = if input.sprint {
                SPRINT_SPEED
            } else {
                WALK_SPEED
            };
            self.velocity.x = wish.x * speed;
            self.velocity.z = wish.z * speed;
            self.velocity.y = (self.velocity.y - GRAVITY * dt).max(-TERMINAL_SPEED);
            if pressed && self.on_ground {
                self.velocity.y = JUMP_SPEED;
            }
        }

        // Vertical first, so a body standing on a ledge edge is not pushed
        // into the ledge by gravity before it moves sideways.
        let step = (self.velocity * dt).as_dvec3();
        let hit_floor_or_ceiling = self.move_axis(1, step.y, &solid);
        self.on_ground = hit_floor_or_ceiling && step.y < 0.0;
        if hit_floor_or_ceiling {
            self.velocity.y = 0.0;
        }
        for axis in [0, 2] {
            if self.move_axis(axis, step[axis], &solid) {
                self.velocity[axis] = 0.0;
            }
        }

        // Touching down ends a flight.
        if self.flying && self.on_ground {
            self.flying = false;
        }
    }

    /// Lowest and highest corners of the body.
    fn bounds(&self) -> (DVec3, DVec3) {
        let half = DVec3::new(WIDTH / 2.0, 0.0, WIDTH / 2.0);
        (self.feet - half, self.feet + half + DVec3::Y * HEIGHT)
    }

    /// Move `amount` along `axis` (0 x, 1 y, 2 z), stopping short of the
    /// first solid block in the way. Only blocks ahead of the body count, so
    /// a body that ends up inside a block can still move out of it. Returns
    /// whether a block stopped the move.
    fn move_axis(&mut self, axis: usize, amount: f64, solid: &impl Fn(IVec3) -> bool) -> bool {
        if amount == 0.0 {
            return false;
        }
        let (min, max) = self.bounds();
        let across = match axis {
            0 => [1, 2],
            1 => [0, 2],
            _ => [0, 1],
        };
        // Cells the body covers across the direction of travel.
        let span = |a: usize| {
            let first = (min[a] + SKIN).floor() as i32;
            let last = (max[a] - SKIN).floor() as i32;
            first..=last
        };
        let blocked = |layer: i32| {
            span(across[0]).any(|a| {
                span(across[1]).any(|b| {
                    let mut cell = IVec3::ZERO;
                    cell[axis] = layer;
                    cell[across[0]] = a;
                    cell[across[1]] = b;
                    solid(cell)
                })
            })
        };

        let mut allowed = amount;
        let mut hit = false;
        if amount > 0.0 {
            let ahead = (max[axis] - SKIN).floor() as i32 + 1;
            let end = (max[axis] + amount).floor() as i32;
            if let Some(layer) = (ahead..=end).find(|&layer| blocked(layer)) {
                allowed = (layer as f64 - SKIN - max[axis]).max(0.0);
                hit = true;
            }
        } else {
            let ahead = (min[axis] + SKIN).floor() as i32 - 1;
            let end = (min[axis] + amount).floor() as i32;
            if let Some(layer) = (end..=ahead).rev().find(|&layer| blocked(layer)) {
                allowed = ((layer + 1) as f64 + SKIN - min[axis]).min(0.0);
                hit = true;
            }
        }
        self.feet[axis] += allowed;
        hit
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec2;

    const FRAME: f32 = 1.0 / 60.0;

    /// Solid ground below y = 0.
    fn ground(cell: IVec3) -> bool {
        cell.y < 0
    }

    fn walk(x: f32, y: f32) -> InputState {
        InputState {
            move_axes: Vec2::new(x, y),
            ..Default::default()
        }
    }

    fn jump() -> InputState {
        InputState {
            vertical: 1.0,
            ..Default::default()
        }
    }

    /// Run `frames` updates facing -Z, with +X to the right.
    fn run(player: &mut Player, input: &InputState, frames: usize, solid: impl Fn(IVec3) -> bool) {
        for _ in 0..frames {
            player.update(input, Vec3::NEG_Z, Vec3::X, FRAME, &solid);
        }
    }

    fn standing() -> Player {
        let mut player = Player::new(DVec3::new(0.5, 3.0, 0.5));
        run(&mut player, &InputState::default(), 120, ground);
        player
    }

    #[test]
    fn falls_and_lands_on_the_ground() {
        let player = standing();
        assert!(player.on_ground);
        assert!(
            (player.feet.y - SKIN).abs() < 1e-9,
            "feet at {}",
            player.feet.y
        );
    }

    #[test]
    fn standing_still_does_not_drift() {
        let mut player = standing();
        let before = player.feet;
        run(&mut player, &InputState::default(), 600, ground);
        assert_eq!(player.feet, before);
    }

    #[test]
    fn walls_stop_the_body() {
        let wall = |c: IVec3| ground(c) || c.x >= 3;
        let mut player = standing();
        run(&mut player, &walk(1.0, 0.0), 200, wall);
        let front = player.feet.x + WIDTH / 2.0;
        assert!(front <= 3.0 && front > 2.99, "front at {front}");
    }

    #[test]
    fn the_body_slides_along_a_wall() {
        let wall = |c: IVec3| ground(c) || c.x >= 3;
        let mut player = standing();
        run(&mut player, &walk(1.0, 1.0), 200, wall);
        assert!(player.feet.x + WIDTH / 2.0 <= 3.0);
        assert!(player.feet.z < -5.0, "did not slide: z = {}", player.feet.z);
    }

    #[test]
    fn a_fast_fall_cannot_pass_through_a_thin_floor() {
        let thin_floor = |c: IVec3| c.y == -1;
        let mut player = Player::new(DVec3::new(0.5, 40.0, 0.5));
        player.velocity.y = -TERMINAL_SPEED;
        // Long frames: every step covers several blocks.
        for _ in 0..30 {
            player.update(
                &InputState::default(),
                Vec3::NEG_Z,
                Vec3::X,
                0.5,
                thin_floor,
            );
        }
        assert!(player.on_ground);
        assert!(player.feet.y >= 0.0, "fell through to {}", player.feet.y);
    }

    #[test]
    fn a_jump_clears_one_block() {
        let mut player = standing();
        player.update(&jump(), Vec3::NEG_Z, Vec3::X, FRAME, ground);
        let mut peak = player.feet.y;
        for _ in 0..60 {
            player.update(&InputState::default(), Vec3::NEG_Z, Vec3::X, FRAME, ground);
            peak = peak.max(player.feet.y);
        }
        assert!(peak > 1.0, "peak {peak}");
        assert!(player.on_ground, "did not come back down");
    }

    #[test]
    fn jumping_needs_the_ground() {
        let mut player = Player::new(DVec3::new(0.5, 30.0, 0.5));
        player.update(&jump(), Vec3::NEG_Z, Vec3::X, FRAME, ground);
        assert!(player.velocity.y < 0.0);
    }

    #[test]
    fn a_ceiling_stops_the_jump() {
        let low_ceiling = |c: IVec3| ground(c) || c.y == 2;
        let mut player = standing();
        player.update(&jump(), Vec3::NEG_Z, Vec3::X, FRAME, low_ceiling);
        for _ in 0..60 {
            player.update(
                &InputState::default(),
                Vec3::NEG_Z,
                Vec3::X,
                FRAME,
                low_ceiling,
            );
            assert!(player.feet.y + HEIGHT <= 2.0, "head in the ceiling");
        }
    }

    #[test]
    fn a_double_tap_toggles_flying() {
        let mut player = standing();
        let idle = InputState::default();
        run(&mut player, &jump(), 2, ground);
        run(&mut player, &idle, 5, ground);
        run(&mut player, &jump(), 2, ground);
        assert!(player.flying);

        let height = player.feet.y;
        run(&mut player, &idle, 120, ground);
        assert_eq!(player.feet.y, height, "a flying body should hover");
    }

    #[test]
    fn slow_taps_only_jump() {
        let mut player = standing();
        let idle = InputState::default();
        run(&mut player, &jump(), 2, ground);
        run(&mut player, &idle, 60, ground);
        run(&mut player, &jump(), 2, ground);
        assert!(!player.flying);
    }

    #[test]
    fn landing_ends_a_flight() {
        let mut player = Player::new(DVec3::new(0.5, 5.0, 0.5));
        player.flying = true;
        let descend = InputState {
            vertical: -1.0,
            ..Default::default()
        };
        run(&mut player, &descend, 120, ground);
        assert!(!player.flying);
        assert!(player.on_ground);
    }

    #[test]
    fn a_body_inside_a_block_can_walk_out() {
        let stuck = |c: IVec3| ground(c) || c == IVec3::ZERO;
        let mut player = Player::new(DVec3::new(0.5, SKIN, 0.5));
        run(&mut player, &walk(1.0, 0.0), 60, stuck);
        assert!(
            player.feet.x - WIDTH / 2.0 > 1.0,
            "still stuck at {}",
            player.feet.x
        );
    }

    #[test]
    fn walls_hold_far_from_the_origin() {
        let far = 10_000_000;
        let wall = move |c: IVec3| ground(c) || c.x >= far + 3;
        let mut player = Player::new(DVec3::new(far as f64 + 0.5, SKIN, 0.5));
        run(&mut player, &walk(1.0, 0.0), 200, wall);
        let front = player.feet.x + WIDTH / 2.0;
        assert!(front <= (far + 3) as f64, "walked into the wall: {front}");
    }

    #[test]
    fn eyes_sit_above_the_feet() {
        let player = Player::new(DVec3::new(1.0, 2.0, 3.0));
        let expected = Vec3::new(1.0, 2.0 + EYE_HEIGHT as f32, 3.0);
        assert!((player.eye() - expected).length() < 1e-5);
    }
}
