//! Vulkan renderer built on `ash`.
//!
//! It targets Vulkan 1.0 core, so it runs on old desktop GPUs and on Android
//! devices with a Vulkan 1.0 driver. The world and input crates do not depend
//! on it.

pub mod font;
pub mod vk;

pub use vk::error::VkError;
pub use vk::renderer::Renderer;
pub use vk::Vertex;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vertex_matches_pipeline_layout() {
        // The block pipeline reads position, normal and colour as three
        // tightly packed vec3s at offsets 0, 12 and 24.
        assert_eq!(std::mem::size_of::<Vertex>(), 36);
    }
}
