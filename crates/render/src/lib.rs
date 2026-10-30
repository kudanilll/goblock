//! Vulkan renderer built on `ash`.
//!
//! It targets Vulkan 1.0 core, so it runs on old desktop GPUs and on Android
//! devices with a Vulkan 1.0 driver. The world and input crates do not depend
//! on it.

pub mod block_textures;
pub mod font;
pub mod vk;

pub use block_textures::BlockTextures;
pub use font::FontKind;
pub use vk::error::VkError;
pub use vk::renderer::{
    build_hud_vertices, draw_text, measure_text, push_glyph, push_rect, FrameView, HudVertex,
    Renderer,
};
