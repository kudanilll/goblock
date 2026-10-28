//! A fixed-size column of blocks and its CPU mesh.

use glam::Vec3;

use crate::block::Block;
use crate::mesh::{build_mesh, grid_lookup, Mesh};
use crate::terrain::column_height;

/// Side length of a chunk on X and Z.
pub const CHUNK_SIZE: i32 = 16;

/// A `CHUNK_SIZE` x `CHUNK_SIZE` column of terrain starting at `origin`.
///
/// `origin` is the chunk's minimum corner in block units; block centres sit at
/// `origin + (x + 0.5, y + 0.5, z + 0.5)`.
#[derive(Debug, Clone)]
pub struct Chunk {
    origin: Vec3,
    height: i32,
    blocks: Vec<Block>,
    mesh: Mesh,
}

impl Chunk {
    /// Generate terrain for the chunk whose minimum corner is `origin`.
    pub fn generate(origin: Vec3) -> Self {
        let ox = origin.x as i32;
        let oy = origin.y as i32;
        let oz = origin.z as i32;

        let mut max_height = 0;
        let mut blocks = Vec::new();

        for x in 0..CHUNK_SIZE {
            for z in 0..CHUNK_SIZE {
                let h = column_height(ox + x, oz + z);
                max_height = max_height.max(h);
                for y in 0..h {
                    // Block centres are at +0.5 inside the cell.
                    blocks.push(Block::new(Vec3::new(
                        (ox + x) as f32 + 0.5,
                        (oy + y) as f32 + 0.5,
                        (oz + z) as f32 + 0.5,
                    )));
                }
            }
        }

        let mut chunk = Self {
            origin,
            height: max_height.max(1),
            blocks,
            mesh: Mesh::default(),
        };
        chunk.rebuild_mesh();
        chunk
    }

    /// Build a chunk directly from blocks. Used by tests and tools.
    pub fn from_blocks(origin: Vec3, blocks: Vec<Block>) -> Self {
        let max_y = blocks
            .iter()
            .map(|b| (b.position().y - origin.y).floor().max(0.0) as i32)
            .max()
            .unwrap_or(0);
        let mut chunk = Self {
            origin,
            height: (max_y + 1).max(1),
            blocks,
            mesh: Mesh::default(),
        };
        chunk.rebuild_mesh();
        chunk
    }

    /// Rebuild the mesh from the current blocks.
    ///
    /// Faces between two blocks of this chunk are dropped. Faces on the chunk
    /// border are always kept because the chunk does not know its neighbours;
    /// where the neighbour is solid they end up hidden inside the terrain.
    pub fn rebuild_mesh(&mut self) {
        let size = CHUNK_SIZE as usize;
        let mut grid = vec![false; size * size * self.height as usize];
        for block in self.blocks.iter().filter(|b| b.is_active()) {
            let cell = (block.position() - self.origin).floor();
            if cell.min_element() < 0.0 {
                continue;
            }
            let (x, y, z) = (cell.x as usize, cell.y as usize, cell.z as usize);
            if x < size && z < size {
                if let Some(solid) = grid.get_mut(x + z * size + y * size * size) {
                    *solid = true;
                }
            }
        }
        self.mesh = build_mesh(&self.blocks, self.origin, grid_lookup(&grid));
    }

    pub fn mesh(&self) -> &Mesh {
        &self.mesh
    }

    pub fn blocks(&self) -> &[Block] {
        &self.blocks
    }

    pub fn origin(&self) -> Vec3 {
        self.origin
    }

    pub fn height(&self) -> i32 {
        self.height
    }

    pub fn min(&self) -> Vec3 {
        self.origin
    }

    pub fn max(&self) -> Vec3 {
        self.origin + Vec3::new(CHUNK_SIZE as f32, self.height as f32, CHUNK_SIZE as f32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunk_fills_every_column() {
        let chunk = Chunk::generate(Vec3::ZERO);
        let expected: usize = (0..CHUNK_SIZE)
            .flat_map(|x| (0..CHUNK_SIZE).map(move |z| (x, z)))
            .map(|(x, z)| column_height(x, z) as usize)
            .sum();
        assert_eq!(chunk.blocks().len(), expected);
    }

    #[test]
    fn bounds_cover_the_generated_volume() {
        let chunk = Chunk::generate(Vec3::new(16.0, 0.0, 32.0));
        assert_eq!(chunk.min(), Vec3::new(16.0, 0.0, 32.0));
        let max = chunk.max();
        assert_eq!(max.x, 32.0);
        assert_eq!(max.z, 48.0);
        assert!(max.y >= 1.0);
    }

    #[test]
    fn generated_chunk_has_geometry() {
        let chunk = Chunk::generate(Vec3::ZERO);
        assert!(!chunk.mesh().is_empty());
    }

    #[test]
    fn block_centres_are_offset_from_origin() {
        let chunk = Chunk::from_blocks(Vec3::ZERO, vec![Block::new(Vec3::new(0.5, 0.5, 0.5))]);
        assert!(chunk.mesh().vertex_count() > 0);
    }

    #[test]
    fn mesh_stays_inside_chunk_bounds() {
        let chunk = Chunk::generate(Vec3::new(16.0, 0.0, 32.0));
        let (min, max) = (chunk.min(), chunk.max());
        for v in &chunk.mesh().vertices {
            let p = Vec3::from(v.position);
            assert!(
                p.cmpge(min).all() && p.cmple(max).all(),
                "vertex {p} outside {min}..{max}"
            );
        }
    }

    #[test]
    fn faces_between_blocks_are_dropped() {
        let chunk = Chunk::from_blocks(
            Vec3::ZERO,
            vec![
                Block::new(Vec3::new(0.5, 0.5, 0.5)),
                Block::new(Vec3::new(0.5, 1.5, 0.5)),
            ],
        );
        // Two cubes, twelve faces, minus the pair they share.
        assert_eq!(chunk.mesh().vertex_count(), 10 * 6);
    }

    #[test]
    fn mesh_is_deterministic() {
        let a = Chunk::generate(Vec3::new(0.0, 0.0, 0.0));
        let b = Chunk::generate(Vec3::new(0.0, 0.0, 0.0));
        assert_eq!(a.mesh(), b.mesh());
    }
}
