//! Background slideshow textures for menu screens.

pub const BG_WIDTH: u32 = 640;
pub const BG_HEIGHT: u32 = 360;

/// Background texture layers for the menu slideshow.
pub struct BackgroundTextures {
    pub width: u32,
    pub height: u32,
    pub layers: u32,
    pub pixels: Vec<u8>,
}

impl BackgroundTextures {
    pub fn new(width: u32, height: u32, layers: u32, pixels: Vec<u8>) -> Self {
        Self {
            width,
            height,
            layers,
            pixels,
        }
    }

    /// Single fallback layer if no images are found.
    pub fn fallback() -> Self {
        let pixels = [30, 35, 45, 255].repeat((BG_WIDTH * BG_HEIGHT) as usize);
        Self {
            width: BG_WIDTH,
            height: BG_HEIGHT,
            layers: 1,
            pixels,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fallback_has_valid_dimensions() {
        let bg = BackgroundTextures::fallback();
        assert_eq!(bg.width, BG_WIDTH);
        assert_eq!(bg.height, BG_HEIGHT);
        assert_eq!(bg.layers, 1);
        assert_eq!(bg.pixels.len(), (BG_WIDTH * BG_HEIGHT * 4) as usize);
    }
}
