//! View frustum extraction and axis-aligned box tests.
//!
//! The planes are read straight from the rows of the view-projection matrix
//! (the Gribb-Hartmann method).

use glam::{Mat4, Vec3, Vec4};

/// The six clip-space planes of a combined projection * view matrix.
///
/// Plane `i` is stored as `(a, b, c, d)` in the form `ax + by + cz + d = 0`
/// with the normal pointing into the frustum.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frustum {
    planes: [Vec4; 6],
}

impl Frustum {
    /// Build a frustum from a combined `projection * view` matrix.
    ///
    /// Assumes Vulkan clip space, where visible depth runs over `0 <= z <= w`.
    /// A projection built for a `-w <= z <= w` depth range would put the near
    /// plane in the wrong place.
    pub fn from_view_projection(vp: &Mat4) -> Self {
        let r0 = vp.row(0);
        let r1 = vp.row(1);
        let r2 = vp.row(2);
        let r3 = vp.row(3);

        let mut planes = [
            r3 + r0, // left:   x >= -w
            r3 - r0, // right:  x <=  w
            r3 + r1, // bottom: y >= -w
            r3 - r1, // top:    y <=  w
            r2,      // near:   z >= 0
            r3 - r2, // far:    z <=  w
        ];

        for plane in &mut planes {
            let len = plane.truncate().length();
            if len > 0.0 {
                *plane /= len;
            }
        }

        Self { planes }
    }

    /// True when the box is inside or intersects the frustum.
    ///
    /// Uses the "positive vertex" test: for each plane, pick the box corner
    /// furthest along the plane normal. If that corner is behind the plane,
    /// every corner is, so the box is fully outside.
    pub fn intersects_box(&self, min: Vec3, max: Vec3) -> bool {
        for plane in &self.planes {
            let p = Vec3::new(
                if plane.x > 0.0 { max.x } else { min.x },
                if plane.y > 0.0 { max.y } else { min.y },
                if plane.z > 0.0 { max.z } else { min.z },
            );
            if plane.truncate().dot(p) + plane.w < 0.0 {
                return false;
            }
        }
        true
    }

    /// Expose the planes for tests and debugging.
    pub fn planes(&self) -> &[Vec4; 6] {
        &self.planes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Mat4;

    fn vp() -> Mat4 {
        let view = Mat4::look_at_rh(Vec3::new(0.0, 0.0, 10.0), Vec3::ZERO, Vec3::Y);
        let proj = Mat4::perspective_rh(45f32.to_radians(), 4.0 / 3.0, 0.1, 100.0);
        proj * view
    }

    #[test]
    fn planes_are_normalised() {
        for plane in Frustum::from_view_projection(&vp()).planes() {
            let len = plane.truncate().length();
            assert!((len - 1.0).abs() < 1e-5, "plane length was {len}");
        }
    }

    #[test]
    fn box_at_origin_is_visible() {
        let frustum = Frustum::from_view_projection(&vp());
        assert!(frustum.intersects_box(Vec3::new(-1.0, -1.0, -1.0), Vec3::new(1.0, 1.0, 1.0)));
    }

    #[test]
    fn box_behind_camera_is_culled() {
        let frustum = Frustum::from_view_projection(&vp());
        // Camera sits at z = 10 looking toward -Z, so z = 20 is behind it.
        assert!(!frustum.intersects_box(Vec3::new(-1.0, -1.0, 20.0), Vec3::new(1.0, 1.0, 22.0)));
    }

    #[test]
    fn box_beside_camera_is_culled() {
        let frustum = Frustum::from_view_projection(&vp());
        assert!(!frustum.intersects_box(Vec3::new(500.0, -1.0, -1.0), Vec3::new(510.0, 1.0, 1.0)));
    }

    #[test]
    fn box_past_far_plane_is_culled() {
        let frustum = Frustum::from_view_projection(&vp());
        assert!(!frustum.intersects_box(Vec3::new(-1.0, -1.0, -200.0), Vec3::new(1.0, 1.0, -198.0)));
    }
}
