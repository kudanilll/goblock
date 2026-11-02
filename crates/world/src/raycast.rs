use glam::{IVec3, Vec3};

use crate::block::{BlockId, Face};

/// Result of a successful raycast against a block.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RaycastHit {
    /// World integer coordinates of the hit block.
    pub block_pos: IVec3,
    /// Face that the ray entered.
    pub face: Face,
    /// Distance from the ray origin to the intersection face.
    pub distance: f32,
    /// Block found at the position.
    pub block: BlockId,
}

/// Fast voxel traversal along a ray, reporting the first breakable block.
pub fn raycast(
    origin: Vec3,
    direction: Vec3,
    max_distance: f32,
    block_at: impl Fn(IVec3) -> BlockId,
) -> Option<RaycastHit> {
    let dir = direction.normalize_or_zero();
    if dir == Vec3::ZERO || max_distance <= 0.0 {
        return None;
    }

    let mut current = origin.floor().as_ivec3();
    let initial_block = block_at(current);
    if initial_block.hardness().is_some() {
        return Some(RaycastHit {
            block_pos: current,
            face: Face::Up,
            distance: 0.0,
            block: initial_block,
        });
    }

    let step_x = if dir.x > 0.0 { 1 } else { -1 };
    let step_y = if dir.y > 0.0 { 1 } else { -1 };
    let step_z = if dir.z > 0.0 { 1 } else { -1 };

    let inv_dir_x = if dir.x != 0.0 {
        1.0 / dir.x
    } else {
        f32::INFINITY
    };
    let inv_dir_y = if dir.y != 0.0 {
        1.0 / dir.y
    } else {
        f32::INFINITY
    };
    let inv_dir_z = if dir.z != 0.0 {
        1.0 / dir.z
    } else {
        f32::INFINITY
    };

    let next_x = if dir.x > 0.0 {
        (current.x + 1) as f32
    } else {
        current.x as f32
    };
    let next_y = if dir.y > 0.0 {
        (current.y + 1) as f32
    } else {
        current.y as f32
    };
    let next_z = if dir.z > 0.0 {
        (current.z + 1) as f32
    } else {
        current.z as f32
    };

    let mut t_max_x = if dir.x != 0.0 {
        (next_x - origin.x) * inv_dir_x
    } else {
        f32::INFINITY
    };
    let mut t_max_y = if dir.y != 0.0 {
        (next_y - origin.y) * inv_dir_y
    } else {
        f32::INFINITY
    };
    let mut t_max_z = if dir.z != 0.0 {
        (next_z - origin.z) * inv_dir_z
    } else {
        f32::INFINITY
    };

    let t_delta_x = if dir.x != 0.0 {
        inv_dir_x.abs()
    } else {
        f32::INFINITY
    };
    let t_delta_y = if dir.y != 0.0 {
        inv_dir_y.abs()
    } else {
        f32::INFINITY
    };
    let t_delta_z = if dir.z != 0.0 {
        inv_dir_z.abs()
    } else {
        f32::INFINITY
    };

    let mut distance = 0.0;

    while distance <= max_distance {
        let hit_face = if t_max_x < t_max_y {
            if t_max_x < t_max_z {
                current.x += step_x;
                distance = t_max_x;
                t_max_x += t_delta_x;
                if step_x > 0 {
                    Face::West
                } else {
                    Face::East
                }
            } else {
                current.z += step_z;
                distance = t_max_z;
                t_max_z += t_delta_z;
                if step_z > 0 {
                    Face::North
                } else {
                    Face::South
                }
            }
        } else if t_max_y < t_max_z {
            current.y += step_y;
            distance = t_max_y;
            t_max_y += t_delta_y;
            if step_y > 0 {
                Face::Down
            } else {
                Face::Up
            }
        } else {
            current.z += step_z;
            distance = t_max_z;
            t_max_z += t_delta_z;
            if step_z > 0 {
                Face::North
            } else {
                Face::South
            }
        };

        if distance > max_distance {
            break;
        }

        let block = block_at(current);
        if block.hardness().is_some() {
            return Some(RaycastHit {
                block_pos: current,
                face: hit_face,
                distance,
                block,
            });
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn straight_ray_hits_expected_block_and_face() {
        let origin = Vec3::new(0.5, 1.5, 0.5);
        let dir = Vec3::new(1.0, 0.0, 0.0);
        let hit = raycast(origin, dir, 5.0, |p| {
            if p == IVec3::new(3, 1, 0) {
                BlockId::Stone
            } else {
                BlockId::Air
            }
        });

        assert_eq!(
            hit,
            Some(RaycastHit {
                block_pos: IVec3::new(3, 1, 0),
                face: Face::West,
                distance: 2.5,
                block: BlockId::Stone,
            })
        );
    }

    #[test]
    fn looking_down_hits_top_face() {
        let origin = Vec3::new(0.5, 3.5, 0.5);
        let dir = Vec3::new(0.0, -1.0, 0.0);
        let hit = raycast(origin, dir, 5.0, |p| {
            if p == IVec3::new(0, 1, 0) {
                BlockId::Dirt
            } else {
                BlockId::Air
            }
        });

        assert_eq!(
            hit,
            Some(RaycastHit {
                block_pos: IVec3::new(0, 1, 0),
                face: Face::Up,
                distance: 1.5,
                block: BlockId::Dirt,
            })
        );
    }

    #[test]
    fn ray_beyond_max_distance_returns_none() {
        let origin = Vec3::new(0.5, 1.5, 0.5);
        let dir = Vec3::new(1.0, 0.0, 0.0);
        let hit = raycast(origin, dir, 2.0, |p| {
            if p == IVec3::new(4, 1, 0) {
                BlockId::Stone
            } else {
                BlockId::Air
            }
        });

        assert_eq!(hit, None);
    }

    #[test]
    fn zero_direction_returns_none() {
        let hit = raycast(Vec3::ZERO, Vec3::ZERO, 5.0, |_| BlockId::Stone);
        assert_eq!(hit, None);
    }
}
