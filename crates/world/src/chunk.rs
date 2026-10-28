//! Chunk storage: a 16 x WORLD_HEIGHT x 16 column of blocks split into
//! 16-block-tall sections, plus the biome of each column.

use glam::{IVec2, IVec3};

use crate::biome::Biome;
use crate::block::BlockId;

/// Side length of a chunk on X and Z.
pub const CHUNK_SIZE: i32 = 16;
/// Number of blocks from the bottom of the world to the top.
pub const WORLD_HEIGHT: i32 = 384;

const SECTION_HEIGHT: i32 = 16;
const SECTION_VOLUME: usize = (CHUNK_SIZE * CHUNK_SIZE * SECTION_HEIGHT) as usize;
const SECTION_COUNT: usize = (WORLD_HEIGHT / SECTION_HEIGHT) as usize;
const COLUMNS: usize = (CHUNK_SIZE * CHUNK_SIZE) as usize;

type Section = [BlockId; SECTION_VOLUME];

#[derive(Clone, Debug)]
pub struct Chunk {
    pos: IVec2,
    /// `None` for sections that are entirely air, which covers most of the sky.
    sections: Vec<Option<Box<Section>>>,
    biomes: Box<[Biome; COLUMNS]>,
}

impl Chunk {
    pub fn new(pos: IVec2) -> Self {
        Self {
            pos,
            sections: vec![None; SECTION_COUNT],
            biomes: Box::new([Biome::default(); COLUMNS]),
        }
    }

    /// Chunk coordinates: world block x and z divided by [`CHUNK_SIZE`].
    pub fn pos(&self) -> IVec2 {
        self.pos
    }

    /// World coordinates of the chunk's minimum corner.
    pub fn origin(&self) -> IVec3 {
        IVec3::new(self.pos.x * CHUNK_SIZE, 0, self.pos.y * CHUNK_SIZE)
    }

    /// Block at chunk-local coordinates. Anything above or below the world is
    /// air. `x` and `z` must be in `0..CHUNK_SIZE`.
    pub fn get(&self, x: i32, y: i32, z: i32) -> BlockId {
        if !(0..WORLD_HEIGHT).contains(&y) {
            return BlockId::Air;
        }
        match &self.sections[(y / SECTION_HEIGHT) as usize] {
            Some(section) => section[index(x, y % SECTION_HEIGHT, z)],
            None => BlockId::Air,
        }
    }

    /// Set a block at chunk-local coordinates. Heights outside the world are
    /// ignored.
    pub fn set(&mut self, x: i32, y: i32, z: i32, block: BlockId) {
        if !(0..WORLD_HEIGHT).contains(&y) {
            return;
        }
        let slot = &mut self.sections[(y / SECTION_HEIGHT) as usize];
        if slot.is_none() && block == BlockId::Air {
            return;
        }
        let section = slot.get_or_insert_with(|| Box::new([BlockId::Air; SECTION_VOLUME]));
        section[index(x, y % SECTION_HEIGHT, z)] = block;
    }

    pub fn biome(&self, x: i32, z: i32) -> Biome {
        self.biomes[(z * CHUNK_SIZE + x) as usize]
    }

    pub fn set_biome(&mut self, x: i32, z: i32, biome: Biome) {
        self.biomes[(z * CHUNK_SIZE + x) as usize] = biome;
    }

    /// Y ranges of the sections that hold at least one block.
    pub(crate) fn filled_sections(&self) -> impl Iterator<Item = std::ops::Range<i32>> + '_ {
        self.sections
            .iter()
            .enumerate()
            .filter(|(_, section)| section.is_some())
            .map(|(i, _)| {
                let bottom = i as i32 * SECTION_HEIGHT;
                bottom..bottom + SECTION_HEIGHT
            })
    }
}

fn index(x: i32, y: i32, z: i32) -> usize {
    debug_assert!((0..CHUNK_SIZE).contains(&x) && (0..CHUNK_SIZE).contains(&z));
    ((y * CHUNK_SIZE + z) * CHUNK_SIZE + x) as usize
}

/// Chunk holding the world block column (x, z).
pub fn chunk_of(x: i32, z: i32) -> IVec2 {
    IVec2::new(x.div_euclid(CHUNK_SIZE), z.div_euclid(CHUNK_SIZE))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_chunk_is_air() {
        let chunk = Chunk::new(IVec2::ZERO);
        assert_eq!(chunk.get(3, 70, 9), BlockId::Air);
        assert_eq!(chunk.filled_sections().count(), 0);
    }

    #[test]
    fn set_then_get_round_trips() {
        let mut chunk = Chunk::new(IVec2::new(2, -3));
        chunk.set(15, 383, 0, BlockId::Stone);
        chunk.set(0, 0, 15, BlockId::Water);
        assert_eq!(chunk.get(15, 383, 0), BlockId::Stone);
        assert_eq!(chunk.get(0, 0, 15), BlockId::Water);
        assert_eq!(chunk.get(14, 383, 0), BlockId::Air);
    }

    #[test]
    fn outside_the_world_is_air_and_ignored() {
        let mut chunk = Chunk::new(IVec2::ZERO);
        chunk.set(0, -1, 0, BlockId::Stone);
        chunk.set(0, WORLD_HEIGHT, 0, BlockId::Stone);
        assert_eq!(chunk.get(0, -1, 0), BlockId::Air);
        assert_eq!(chunk.get(0, WORLD_HEIGHT, 0), BlockId::Air);
        assert_eq!(chunk.filled_sections().count(), 0);
    }

    #[test]
    fn writing_air_does_not_allocate_a_section() {
        let mut chunk = Chunk::new(IVec2::ZERO);
        chunk.set(1, 100, 1, BlockId::Air);
        assert_eq!(chunk.filled_sections().count(), 0);
        chunk.set(1, 100, 1, BlockId::Dirt);
        assert_eq!(chunk.filled_sections().collect::<Vec<_>>(), vec![96..112]);
    }

    #[test]
    fn origin_follows_position() {
        assert_eq!(
            Chunk::new(IVec2::new(-2, 5)).origin(),
            IVec3::new(-32, 0, 80)
        );
    }

    #[test]
    fn negative_coordinates_map_to_the_right_chunk() {
        assert_eq!(chunk_of(-1, -16), IVec2::new(-1, -1));
        assert_eq!(chunk_of(-17, 15), IVec2::new(-2, 0));
        assert_eq!(chunk_of(16, 0), IVec2::new(1, 0));
    }
}
