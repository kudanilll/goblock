//! Blocks, chunks, terrain generation, meshing and chunk streaming.
//!
//! The crate has no window or GPU dependency, so the world can be built and
//! tested without a graphics device.

pub mod biome;
pub mod block;
pub mod chunk;
mod generate;
pub mod mesh;
pub mod terrain;
mod world;

pub use biome::Biome;
pub use block::{BlockId, Face, RenderLayer, Texture};
pub use chunk::{chunk_of, Chunk, CHUNK_SIZE, WORLD_HEIGHT};
pub use mesh::{ChunkMesh, ChunkVertex};
pub use terrain::{Terrain, SEA_LEVEL};
pub use world::{World, WorldEvent};
