//! Error type that records which Vulkan call failed.

use std::fmt;

#[derive(Debug)]
pub struct VkError {
    context: &'static str,
    detail: String,
}

impl VkError {
    pub fn new(context: &'static str, code: ash::vk::Result) -> Self {
        Self {
            context,
            detail: format!("{code:?}"),
        }
    }

    pub fn msg(detail: impl Into<String>) -> Self {
        Self {
            context: "",
            detail: detail.into(),
        }
    }
}

impl fmt::Display for VkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.context.is_empty() {
            write!(f, "vulkan: {}", self.detail)
        } else {
            write!(f, "vulkan: {} failed ({})", self.context, self.detail)
        }
    }
}

impl std::error::Error for VkError {}
