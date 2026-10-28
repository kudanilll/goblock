//! Fills a chunk from the terrain: ground layers by biome, water up to sea
//! level, and trees.

use glam::IVec2;

use crate::biome::{Biome, TreeKind};
use crate::block::BlockId;
use crate::chunk::{Chunk, CHUNK_SIZE};
use crate::terrain::{hash, ColumnSample, Terrain, SEA_LEVEL};

/// How far a tree's leaves reach from its trunk.
const TREE_REACH: i32 = 3;
/// Trees grow at most one per cell of this size, which keeps trunks apart.
const TREE_CELL: i32 = 5;

/// Columns around the chunk that the generator needs: the chunk itself plus
/// any column whose tree could reach into it.
struct Columns {
    min_x: i32,
    min_z: i32,
    span: i32,
    samples: Vec<ColumnSample>,
}

impl Columns {
    fn new(terrain: &Terrain, pos: IVec2) -> Self {
        let span = CHUNK_SIZE + 2 * TREE_REACH;
        let min_x = pos.x * CHUNK_SIZE - TREE_REACH;
        let min_z = pos.y * CHUNK_SIZE - TREE_REACH;

        // Elevation for one more column on every side, for the slope filter.
        let grid = span + 2;
        let mut elevation = Vec::with_capacity((grid * grid) as usize);
        for z in 0..grid {
            for x in 0..grid {
                elevation.push(terrain.elevation((min_x + x - 1) as f32, (min_z + z - 1) as f32));
            }
        }

        let mut samples = Vec::with_capacity((span * span) as usize);
        for z in 0..span {
            for x in 0..span {
                let around: [f32; 9] = std::array::from_fn(|i| {
                    let (dx, dz) = (i as i32 % 3, i as i32 / 3);
                    elevation[((z + dz) * grid + x + dx) as usize]
                });
                samples.push(terrain.column(min_x + x, min_z + z, &around));
            }
        }

        Self {
            min_x,
            min_z,
            span,
            samples,
        }
    }

    /// Sample for world column (x, z), if it is in range.
    fn get(&self, x: i32, z: i32) -> Option<&ColumnSample> {
        let (lx, lz) = (x - self.min_x, z - self.min_z);
        let inside = (0..self.span).contains(&lx) && (0..self.span).contains(&lz);
        inside.then(|| &self.samples[(lz * self.span + lx) as usize])
    }
}

pub(crate) fn generate_chunk(terrain: &Terrain, pos: IVec2) -> Chunk {
    let columns = Columns::new(terrain, pos);
    let mut chunk = Chunk::new(pos);
    let origin = chunk.origin();

    for z in 0..CHUNK_SIZE {
        for x in 0..CHUNK_SIZE {
            let sample = columns
                .get(origin.x + x, origin.z + z)
                .expect("chunk columns are always sampled");
            chunk.set_biome(x, z, sample.biome);
            fill_column(&mut chunk, x, z, sample);
        }
    }

    plant_trees(&mut chunk, &columns, terrain.seed());
    chunk
}

fn fill_column(chunk: &mut Chunk, x: i32, z: i32, sample: &ColumnSample) {
    let height = sample.height;
    let mut surface = sample.biome.surface();

    let underwater = height < SEA_LEVEL;
    let beach = !underwater && height <= SEA_LEVEL + 1 && surface.top == BlockId::Grass;
    if beach {
        surface.top = BlockId::Sand;
        surface.filler = BlockId::Sand;
    } else if underwater && !sample.biome.is_ocean() {
        // Land biomes dipping below sea level get a gravel floor.
        surface.top = BlockId::Gravel;
        surface.filler = BlockId::Dirt;
    }

    for y in 0..=height {
        let block = if y == height {
            surface.top
        } else if y >= height - surface.depth {
            surface.filler
        } else {
            BlockId::Stone
        };
        chunk.set(x, y, z, block);
    }

    for y in height + 1..=SEA_LEVEL {
        chunk.set(x, y, z, BlockId::Water);
    }
    if underwater && sample.biome == Biome::FrozenOcean {
        chunk.set(x, SEA_LEVEL, z, BlockId::Ice);
    }
}

fn plant_trees(chunk: &mut Chunk, columns: &Columns, seed: u64) {
    let origin = chunk.origin();
    let first = |o: i32| (o - TREE_REACH).div_euclid(TREE_CELL);
    let last = |o: i32| (o + CHUNK_SIZE - 1 + TREE_REACH).div_euclid(TREE_CELL);

    for cell_z in first(origin.z)..=last(origin.z) {
        for cell_x in first(origin.x)..=last(origin.x) {
            let roll = hash(seed, cell_x, cell_z);
            // One spot per cell, kept off the cell's last row and column so
            // neighbouring trunks never touch.
            let tx = cell_x * TREE_CELL + (roll % 4) as i32;
            let tz = cell_z * TREE_CELL + ((roll >> 8) % 4) as i32;
            let Some(sample) = columns.get(tx, tz) else {
                continue;
            };

            let chance = ((roll >> 16) & 0xffff) as f32 / 65535.0;
            let ground = sample.biome.surface().top;
            let on_soil = matches!(ground, BlockId::Grass | BlockId::SnowyGrass);
            if chance >= sample.biome.tree_density() || !on_soil || sample.height <= SEA_LEVEL + 1 {
                continue;
            }

            let base = (tx - origin.x, sample.height + 1, tz - origin.z);
            grow_tree(chunk, base, sample.biome.tree_kind(), roll >> 32);
        }
    }
}

/// Grow a tree whose trunk starts at chunk-local `base`. Parts outside the
/// chunk are skipped; the neighbouring chunk grows the same tree and keeps
/// them.
fn grow_tree(chunk: &mut Chunk, base: (i32, i32, i32), kind: TreeKind, roll: u64) {
    let (bx, by, bz) = base;
    let (trunk, crown_depth) = match kind {
        TreeKind::Broadleaf => (4 + (roll % 3) as i32, 3),
        TreeKind::Tall => (8 + (roll % 5) as i32, 4),
        // Conifer foliage runs most of the way down the trunk.
        TreeKind::Conifer => (6 + (roll % 4) as i32, 4 + (roll % 4) as i32),
    };
    let top = by + trunk - 1;

    for y in (top - crown_depth + 1).max(by + 1)..=top + 1 {
        let radius = leaf_radius(kind, top + 1 - y);
        for dz in -radius..=radius {
            for dx in -radius..=radius {
                if dx * dx + dz * dz <= radius * radius + radius {
                    set_in_chunk(chunk, bx + dx, y, bz + dz, BlockId::Leaves, true);
                }
            }
        }
    }

    for y in by..=top {
        set_in_chunk(chunk, bx, y, bz, BlockId::Log, false);
    }
}

/// Leaf radius `depth` layers below the top of the crown.
fn leaf_radius(kind: TreeKind, depth: i32) -> i32 {
    match (kind, depth) {
        (TreeKind::Conifer, 0) => 0,
        (TreeKind::Conifer, d) => 1 + (d + 1) % 2,
        (_, 0 | 1) => 1,
        (TreeKind::Tall, _) => 3,
        (TreeKind::Broadleaf, _) => 2,
    }
}

/// Set a block if (x, z) falls inside the chunk. With `only_air`, existing
/// blocks are left alone.
fn set_in_chunk(chunk: &mut Chunk, x: i32, y: i32, z: i32, block: BlockId, only_air: bool) {
    let inside = (0..CHUNK_SIZE).contains(&x) && (0..CHUNK_SIZE).contains(&z);
    if inside && (!only_air || chunk.get(x, y, z) == BlockId::Air) {
        chunk.set(x, y, z, block);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generation_is_deterministic() {
        let terrain = Terrain::new(5);
        let a = generate_chunk(&terrain, IVec2::new(3, -2));
        let b = generate_chunk(&terrain, IVec2::new(3, -2));
        for y in 0..200 {
            assert_eq!(a.get(4, y, 9), b.get(4, y, 9));
        }
    }

    #[test]
    fn ground_matches_the_terrain_height() {
        let terrain = Terrain::new(9);
        let chunk = generate_chunk(&terrain, IVec2::new(1, 1));
        for (x, z) in [(0, 0), (7, 12), (15, 15)] {
            let sample = terrain.sample(16 + x, 16 + z);
            let ground = chunk.get(x, sample.height, z);
            assert!(ground.is_solid(), "no ground at ({x}, {z}): {ground:?}");
            assert!(chunk.get(x, 0, z).is_solid());
            assert_eq!(chunk.biome(x, z), sample.biome);
        }
    }

    #[test]
    fn oceans_are_filled_to_sea_level() {
        let terrain = Terrain::new(1);
        for cx in -64..64 {
            let chunk = generate_chunk(&terrain, IVec2::new(cx * 8, 0));
            for x in 0..CHUNK_SIZE {
                let sample = terrain.sample(chunk.origin().x + x, 0);
                if sample.height < SEA_LEVEL - 2 && sample.biome != Biome::FrozenOcean {
                    assert_eq!(chunk.get(x, SEA_LEVEL, 0), BlockId::Water);
                    assert_eq!(chunk.get(x, SEA_LEVEL + 1, 0), BlockId::Air);
                    return;
                }
            }
        }
        panic!("no ocean found");
    }

    #[test]
    fn trees_grow_from_their_ground() {
        let mut chunk = Chunk::new(IVec2::ZERO);
        grow_tree(&mut chunk, (8, 70, 8), TreeKind::Broadleaf, 0);
        assert_eq!(chunk.get(8, 70, 8), BlockId::Log);
        assert_eq!(chunk.get(9, 72, 8), BlockId::Leaves);
        assert_eq!(chunk.get(8, 69, 8), BlockId::Air);
        // Nothing grows on the ground next to the trunk.
        assert_eq!(chunk.get(9, 70, 8), BlockId::Air);
    }

    #[test]
    fn conifers_end_in_a_single_leaf() {
        let mut chunk = Chunk::new(IVec2::ZERO);
        grow_tree(&mut chunk, (8, 70, 8), TreeKind::Conifer, 0);
        let tip = 70 + 6;
        assert_eq!(chunk.get(8, tip, 8), BlockId::Leaves);
        assert_eq!(chunk.get(9, tip, 8), BlockId::Air);
    }

    #[test]
    fn trees_near_the_edge_are_clipped_to_the_chunk() {
        let mut chunk = Chunk::new(IVec2::ZERO);
        grow_tree(&mut chunk, (-1, 70, 0), TreeKind::Tall, 0);
        assert!((70..90).any(|y| chunk.get(0, y, 0) == BlockId::Leaves));
        assert!((70..90).all(|y| chunk.get(0, y, 0) != BlockId::Log));
    }

    #[test]
    fn leaves_do_not_replace_ground() {
        let mut chunk = Chunk::new(IVec2::ZERO);
        chunk.set(9, 72, 8, BlockId::Stone);
        grow_tree(&mut chunk, (8, 70, 8), TreeKind::Broadleaf, 0);
        assert_eq!(chunk.get(9, 72, 8), BlockId::Stone);
    }
}
