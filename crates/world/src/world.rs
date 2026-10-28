//! The collection of chunks making up the playable world.

use glam::{Mat4, Vec3};

use goblock_core::frustum::Frustum;

use crate::chunk::{Chunk, CHUNK_SIZE};

/// Chunks per side of the world.
pub const WORLD_SIZE: i32 = 8;

/// A square grid of chunks plus the frustum test used to skip off-screen ones.
#[derive(Debug, Clone)]
pub struct World {
    chunks: Vec<Chunk>,
}

impl World {
    /// Generate a `WORLD_SIZE` x `WORLD_SIZE` grid of chunks.
    pub fn generate() -> Self {
        let mut chunks = Vec::with_capacity((WORLD_SIZE * WORLD_SIZE) as usize);
        for cx in 0..WORLD_SIZE {
            for cz in 0..WORLD_SIZE {
                chunks.push(Chunk::generate(Vec3::new(
                    (cx * CHUNK_SIZE) as f32,
                    0.0,
                    (cz * CHUNK_SIZE) as f32,
                )));
            }
        }
        Self { chunks }
    }

    pub fn chunks(&self) -> &[Chunk] {
        &self.chunks
    }

    /// Total block count across every chunk.
    pub fn block_count(&self) -> usize {
        self.chunks.iter().map(|c| c.blocks().len()).sum()
    }

    /// Indices of chunks visible from the given combined `projection * view`.
    pub fn visible_chunks(&self, view_projection: &Mat4) -> Vec<usize> {
        let frustum = Frustum::from_view_projection(view_projection);
        self.chunks
            .iter()
            .enumerate()
            .filter(|(_, chunk)| frustum.intersects_box(chunk.min(), chunk.max()))
            .map(|(i, _)| i)
            .collect()
    }

    /// Every block in the world, used for camera collision.
    pub fn all_blocks(&self) -> Vec<crate::block::Block> {
        self.chunks
            .iter()
            .flat_map(|c| c.blocks().iter().copied())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_full_grid() {
        let world = World::generate();
        assert_eq!(world.chunks().len(), (WORLD_SIZE * WORLD_SIZE) as usize);
    }

    #[test]
    fn chunks_tile_without_overlap() {
        let world = World::generate();
        for chunk in world.chunks() {
            assert_eq!(chunk.origin().y, 0.0);
            assert_eq!(chunk.origin().x % CHUNK_SIZE as f32, 0.0);
            assert_eq!(chunk.origin().z % CHUNK_SIZE as f32, 0.0);
        }
    }

    #[test]
    fn world_has_blocks() {
        let world = World::generate();
        assert!(world.block_count() > 0);
    }

    fn camera_vp(eye: Vec3, target: Vec3) -> Mat4 {
        let view = Mat4::look_at_rh(eye, target, Vec3::Y);
        let proj = Mat4::perspective_rh(45f32.to_radians(), 4.0 / 3.0, 0.1, 500.0);
        proj * view
    }

    #[test]
    fn camera_inside_world_sees_some_chunks() {
        let world = World::generate();
        let vp = camera_vp(Vec3::new(64.0, 40.0, 128.0), Vec3::new(64.0, 0.0, 64.0));
        let visible = world.visible_chunks(&vp);
        assert!(!visible.is_empty());
        assert!(visible.len() <= world.chunks().len());
    }

    #[test]
    fn camera_far_away_sees_nothing() {
        let world = World::generate();
        // Looking away from the world entirely.
        let vp = camera_vp(Vec3::new(0.0, 0.0, 10_000.0), Vec3::new(0.0, 0.0, 10_100.0));
        assert!(world.visible_chunks(&vp).is_empty());
    }

    #[test]
    fn looking_at_empty_region_culls_most_chunks() {
        let world = World::generate();
        // Stand off to one side and look away from the world centre.
        let vp = camera_vp(
            Vec3::new(-400.0, 10.0, -400.0),
            Vec3::new(-800.0, 10.0, -800.0),
        );
        assert!(world.visible_chunks(&vp).is_empty());
    }

    #[test]
    fn chunk_centres_are_inside_their_bounds() {
        let world = World::generate();
        for chunk in world.chunks() {
            let centre = Vec3::new(
                (chunk.min().x + chunk.max().x) * 0.5,
                chunk.min().y,
                (chunk.min().z + chunk.max().z) * 0.5,
            );
            let (min, max) = (chunk.min(), chunk.max());
            assert!(centre.x >= min.x && centre.x <= max.x);
            assert!(centre.y >= min.y && centre.y <= max.y);
            assert!(centre.z >= min.z && centre.z <= max.z);
        }
    }
}
