//! Vulkan backend modules.

pub mod device;
pub mod error;
pub mod renderer;
pub mod resources;
pub mod swapchain;

pub use error::VkError;
pub use renderer::Renderer;

pub use goblock_world::MeshVertex as Vertex;
