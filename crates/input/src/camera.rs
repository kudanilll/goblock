//! First-person camera.
//!
//! Free flight with yaw/pitch mouse look, separate horizontal and vertical
//! speeds, and a point collision test against the world.

use glam::{Mat4, Vec3};

/// Camera state and the movement rules that act on it.
#[derive(Debug, Clone)]
pub struct Camera {
    pub position: Vec3,
    pub front: Vec3,
    pub up: Vec3,
    pub right: Vec3,
    yaw: f32,
    pitch: f32,
    movement_speed: f32,
    vertical_speed: f32,
    mouselook_sensitivity: f32,
    fov_degrees: f32,
}

impl Default for Camera {
    fn default() -> Self {
        let mut camera = Self {
            position: Vec3::new(64.0, 24.0, 96.0),
            front: Vec3::new(0.0, 0.0, -1.0),
            up: Vec3::Y,
            right: Vec3::X,
            yaw: -135.0,
            pitch: -20.0,
            movement_speed: 15.0,
            vertical_speed: 10.0,
            mouselook_sensitivity: 0.3,
            fov_degrees: 45.0,
        };
        camera.update_vectors();
        camera
    }
}

impl Camera {
    pub fn movement_speed(&self) -> f32 {
        self.movement_speed
    }

    pub fn vertical_speed(&self) -> f32 {
        self.vertical_speed
    }

    pub fn fov_degrees(&self) -> f32 {
        self.fov_degrees
    }

    /// Apply a mouse delta in screen pixels.
    pub fn process_mouse_delta(&mut self, delta_x: f32, delta_y: f32) {
        self.yaw += delta_x * self.mouselook_sensitivity;
        self.pitch += delta_y * self.mouselook_sensitivity;
        self.pitch = self.pitch.clamp(-89.0, 89.0);
        self.update_vectors();
    }

    /// Move along `direction` scaled by `speed`, stopping where `solid`
    /// reports a solid point.
    ///
    /// The direction is normalised when non-zero so diagonals do not move
    /// faster. Each axis moves on its own, so the camera slides along a wall
    /// instead of sticking to it. Only the camera's own point is tested, so
    /// the camera has no body width.
    pub fn move_and_collide(
        &mut self,
        direction: Vec3,
        delta_time: f32,
        speed: f32,
        solid: impl Fn(Vec3) -> bool,
    ) {
        if direction.length_squared() == 0.0 {
            return;
        }
        let step = direction.normalize() * speed * delta_time;
        for axis in [Vec3::X, Vec3::Y, Vec3::Z] {
            let candidate = self.position + step * axis;
            if !solid(candidate) {
                self.position = candidate;
            }
        }
    }

    /// View matrix with the camera at the origin. The renderer draws the
    /// world relative to the camera, which keeps vertex positions small and
    /// precise however far the player travels.
    pub fn view_matrix(&self) -> Mat4 {
        Mat4::look_to_rh(Vec3::ZERO, self.front, self.up)
    }

    /// Vulkan projection.
    ///
    /// Vulkan's clip space has Y pointing down, so the projection negates Y.
    /// Without it the scene renders upside down.
    pub fn projection_matrix(&self, aspect: f32) -> Mat4 {
        let perspective = Mat4::perspective_rh(self.fov_degrees.to_radians(), aspect, 0.1, 1000.0);
        let flip_y = Mat4::from_scale(Vec3::new(1.0, -1.0, 1.0));
        flip_y * perspective
    }

    fn update_vectors(&mut self) {
        let yaw = self.yaw.to_radians();
        let pitch = self.pitch.to_radians();
        self.front = Vec3::new(
            yaw.cos() * pitch.cos(),
            pitch.sin(),
            yaw.sin() * pitch.cos(),
        )
        .normalize();
        self.right = self.front.cross(self.up).normalize();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;

    #[test]
    fn default_orientation_is_unit_length() {
        let c = Camera::default();
        assert!((c.front.length() - 1.0).abs() < 1e-5);
        assert!((c.right.length() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn right_is_perpendicular_to_front() {
        let c = Camera::default();
        assert!(c.front.dot(c.right).abs() < 1e-5);
    }

    #[test]
    fn pitch_is_clamped() {
        let mut c = Camera::default();
        c.process_mouse_delta(0.0, 10_000.0);
        assert!(c.pitch <= 89.0);
        c.process_mouse_delta(0.0, -20_000.0);
        assert!(c.pitch >= -89.0);
    }

    #[test]
    fn moving_forward_changes_position() {
        let mut c = Camera::default();
        let start = c.position;
        let front = c.front;
        c.move_and_collide(front, 1.0, 15.0, |_| false);
        assert!((c.position - start).length() > 1.0);
    }

    #[test]
    fn diagonal_movement_is_not_faster() {
        let mut straight = Camera::default();
        let start = straight.position;
        straight.move_and_collide(Vec3::X, 1.0, 10.0, |_| false);
        let straight_dist = (straight.position - start).length();

        let mut diagonal = Camera::default();
        let start = diagonal.position;
        diagonal.move_and_collide(Vec3::new(1.0, 0.0, 1.0), 1.0, 10.0, |_| false);
        let diagonal_dist = (diagonal.position - start).length();

        assert!((straight_dist - diagonal_dist).abs() < 1e-4);
    }

    #[test]
    fn zero_direction_does_not_move() {
        let mut c = Camera::default();
        let start = c.position;
        c.move_and_collide(Vec3::ZERO, 1.0, 15.0, |_| false);
        assert_eq!(c.position, start);
    }

    #[test]
    fn movement_stops_at_a_wall() {
        let mut c = Camera::default();
        let before = c.position;
        let wall = before.x + 1.0;
        c.move_and_collide(Vec3::X, 1.0, 15.0, |p| p.x >= wall);
        assert_eq!(c.position, before, "camera moved into a solid block");
    }

    #[test]
    fn movement_slides_along_a_wall() {
        let mut c = Camera::default();
        let before = c.position;
        let wall = before.x + 1.0;
        c.move_and_collide(Vec3::new(1.0, 0.0, 1.0), 1.0, 10.0, |p| p.x >= wall);
        assert_eq!(c.position.x, before.x);
        assert!(c.position.z > before.z + 5.0);
    }

    #[test]
    fn screen_right_matches_camera_right() {
        let c = Camera::default();
        let aspect = 4.0 / 3.0;
        let vp = c.projection_matrix(aspect) * c.view_matrix();

        // The view matrix puts the camera at the origin, so points are
        // given relative to it.
        let target = c.front * 10.0;
        let p_center = vp.project_point3(target);
        let p_right = vp.project_point3(target + c.right * 5.0);

        // Screen X must increase for a point toward camera.right.
        assert!(
            p_right.x > p_center.x,
            "right moved to x={} vs center={}",
            p_right.x,
            p_center.x
        );
    }

    #[test]
    fn screen_up_matches_camera_up() {
        let c = Camera::default();
        let vp = c.projection_matrix(4.0 / 3.0) * c.view_matrix();
        let target = c.front * 10.0;
        let p_center = vp.project_point3(target);
        let p_up = vp.project_point3(target + c.up * 5.0);

        // NDC Y grows downward in Vulkan, so up must reduce Y.
        assert!(
            p_up.y < p_center.y,
            "up moved to y={} vs center={}",
            p_up.y,
            p_center.y
        );
    }

    #[test]
    fn mouse_up_raises_view_direction() {
        let mut c = Camera::default();
        let before = c.front.y;
        // Input layer converts "mouse moved up" (negative screen Y) to +y.
        c.process_mouse_delta(0.0, 10.0);
        assert!(
            c.front.y > before,
            "pitch did not rise: {} -> {}",
            before,
            c.front.y
        );
    }

    #[test]
    fn projection_uses_requested_aspect() {
        let c = Camera::default();
        let m: [f32; 16] = c.projection_matrix(16.0 / 9.0).to_cols_array();
        // m[0] is 1 / (aspect * tan(fov/2)); a wider aspect shrinks it.
        let narrow: [f32; 16] = c.projection_matrix(4.0 / 3.0).to_cols_array();
        assert!(m[0] < narrow[0]);
    }
}
