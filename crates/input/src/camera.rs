//! First-person camera: yaw/pitch mouse look and the projection. Where the
//! camera is comes from the [`crate::Player`] it rides on.

use glam::{Mat4, Vec3};

/// Vertical field of view in degrees, at rest and while sprinting.
const FOV: f32 = 45.0;
const SPRINT_FOV: f32 = 52.0;
/// How fast the field of view follows a change, per second.
const FOV_EASE: f32 = 8.0;

#[derive(Debug, Clone)]
pub struct Camera {
    pub position: Vec3,
    pub front: Vec3,
    pub up: Vec3,
    pub right: Vec3,
    yaw: f32,
    pitch: f32,
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
            mouselook_sensitivity: 0.3,
            fov_degrees: FOV,
        };
        camera.update_vectors();
        camera
    }
}

impl Camera {
    pub fn fov_degrees(&self) -> f32 {
        self.fov_degrees
    }

    pub fn yaw(&self) -> f32 {
        self.yaw
    }

    pub fn pitch(&self) -> f32 {
        self.pitch
    }

    pub fn set_orientation(&mut self, yaw: f32, pitch: f32) {
        self.yaw = yaw;
        self.pitch = pitch.clamp(-89.0, 89.0);
        self.update_vectors();
    }

    /// Widen the view while sprinting and narrow it again afterwards,
    /// easing over a fraction of a second so the change reads as speed
    /// rather than a jump cut.
    pub fn ease_fov(&mut self, sprinting: bool, dt: f32) {
        let target = if sprinting { SPRINT_FOV } else { FOV };
        self.fov_degrees += (target - self.fov_degrees) * (1.0 - (-FOV_EASE * dt).exp());
    }

    /// Apply a mouse delta in screen pixels.
    pub fn process_mouse_delta(&mut self, delta_x: f32, delta_y: f32) {
        self.yaw += delta_x * self.mouselook_sensitivity;
        self.pitch += delta_y * self.mouselook_sensitivity;
        self.pitch = self.pitch.clamp(-89.0, 89.0);
        self.update_vectors();
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
    fn sprinting_widens_the_view_and_stopping_narrows_it() {
        let mut c = Camera::default();
        // The vertical scale of the projection; negative because Y is flipped.
        let vertical_scale = |c: &Camera| c.projection_matrix(1.0).to_cols_array()[5].abs();
        let rest = vertical_scale(&c);
        for _ in 0..60 {
            c.ease_fov(true, 1.0 / 60.0);
        }
        assert!((c.fov_degrees() - SPRINT_FOV).abs() < 0.01);
        // A wider view shrinks it.
        assert!(vertical_scale(&c) < rest);

        for _ in 0..60 {
            c.ease_fov(false, 1.0 / 60.0);
        }
        assert!((c.fov_degrees() - FOV).abs() < 0.01);
    }

    #[test]
    fn the_view_eases_instead_of_jumping() {
        let mut c = Camera::default();
        c.ease_fov(true, 1.0 / 60.0);
        assert!(c.fov_degrees() > FOV && c.fov_degrees() < FOV + 2.0);
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
