use glam::Vec3;

/// A single block in the world.
///
/// Positions are block centres, so a block at `p` occupies
/// `[p - 0.5, p + 0.5]` on each axis.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Block {
    position: Vec3,
    active: bool,
    has_collision: bool,
}

impl Block {
    pub fn new(position: Vec3) -> Self {
        Self {
            position,
            active: true,
            has_collision: true,
        }
    }

    pub fn with_flags(position: Vec3, active: bool, has_collision: bool) -> Self {
        Self {
            position,
            active,
            has_collision,
        }
    }

    pub fn position(&self) -> Vec3 {
        self.position
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn has_collision(&self) -> bool {
        self.has_collision
    }

    /// True when `point` lies inside this block's unit cube.
    pub fn contains_point(&self, point: Vec3) -> bool {
        const HALF: f32 = 0.5;
        (point.x >= self.position.x - HALF && point.x <= self.position.x + HALF)
            && (point.y >= self.position.y - HALF && point.y <= self.position.y + HALF)
            && (point.z >= self.position.z - HALF && point.z <= self.position.z + HALF)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn centre_is_inside() {
        let b = Block::new(Vec3::new(2.0, 3.0, 4.0));
        assert!(b.contains_point(Vec3::new(2.0, 3.0, 4.0)));
    }

    #[test]
    fn corner_is_inside() {
        let b = Block::new(Vec3::new(0.0, 0.0, 0.0));
        assert!(b.contains_point(Vec3::new(0.5, 0.5, 0.5)));
        assert!(b.contains_point(Vec3::new(-0.5, -0.5, -0.5)));
    }

    #[test]
    fn just_outside_is_rejected() {
        let b = Block::new(Vec3::new(0.0, 0.0, 0.0));
        assert!(!b.contains_point(Vec3::new(0.51, 0.0, 0.0)));
        assert!(!b.contains_point(Vec3::new(0.0, -0.51, 0.0)));
        assert!(!b.contains_point(Vec3::new(0.0, 0.0, 0.51)));
    }

    #[test]
    fn defaults_are_active_with_collision() {
        let b = Block::new(Vec3::ZERO);
        assert!(b.is_active());
        assert!(b.has_collision());
    }
}
