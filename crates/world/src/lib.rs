//! Blocks, chunks, terrain and chunk meshing.
//!
//! The crate has no window or GPU dependency, so the world can be built and
//! tested without a graphics device.

pub mod block;
pub mod chunk;
pub mod mesh;
pub mod terrain;
pub mod world;

pub use block::Block;
pub use chunk::{Chunk, CHUNK_SIZE};
pub use mesh::{Mesh, MeshVertex};
pub use world::{World, WORLD_SIZE};
