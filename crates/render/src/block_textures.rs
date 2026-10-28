//! Block textures as the renderer receives them, and the mip chain built
//! from them.

/// One square RGBA8 sRGB image per block texture layer, in the order of
/// `goblock_world::Texture::ALL`.
pub struct BlockTextures {
    /// Width and height of every layer, in pixels. Must be a power of two.
    pub size: u32,
    /// Layers back to back, `size * size * 4` bytes each.
    pub pixels: Vec<u8>,
}

impl BlockTextures {
    pub fn layers(&self) -> u32 {
        (self.pixels.len() / (self.size * self.size * 4) as usize) as u32
    }
}

/// Every mip level of `textures` from full size down to 1x1, each level
/// holding all layers, and the number of levels.
///
/// Colour is averaged in linear space and weighted by alpha, so smaller
/// levels keep their brightness and cut-out edges do not pick up the colour
/// of transparent pixels.
pub(crate) fn build_mips(textures: &BlockTextures) -> (Vec<u8>, u32) {
    let layers = textures.layers() as usize;
    let mut levels = vec![textures.pixels.clone()];
    let mut size = textures.size as usize;

    while size > 1 {
        let half = size / 2;
        let src_level = &levels[levels.len() - 1];
        let mut next = vec![0u8; layers * half * half * 4];
        for layer in 0..layers {
            let src = &src_level[layer * size * size * 4..][..size * size * 4];
            let dst = &mut next[layer * half * half * 4..][..half * half * 4];
            for y in 0..half {
                for x in 0..half {
                    let mut rgb = [0.0f32; 3];
                    let mut alpha = 0.0;
                    for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                        let i = ((2 * y + dy) * size + 2 * x + dx) * 4;
                        let a = src[i + 3] as f32 / 255.0;
                        for c in 0..3 {
                            rgb[c] += srgb_to_linear(src[i + c]) * a;
                        }
                        alpha += a;
                    }
                    let o = (y * half + x) * 4;
                    for c in 0..3 {
                        let value = if alpha > 0.0 { rgb[c] / alpha } else { 0.0 };
                        dst[o + c] = linear_to_srgb(value);
                    }
                    dst[o + 3] = (alpha / 4.0 * 255.0).round() as u8;
                }
            }
        }
        levels.push(next);
        size = half;
    }

    let count = levels.len() as u32;
    (levels.concat(), count)
}

/// Decode an 8-bit sRGB channel to linear light in 0..1.
pub fn srgb_to_linear(value: u8) -> f32 {
    let c = value as f32 / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// Encode linear light in 0..1 as an 8-bit sRGB channel.
pub fn linear_to_srgb(value: f32) -> u8 {
    let c = value.clamp(0.0, 1.0);
    let s = if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    (s * 255.0).round() as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(size: u32, layers: u32, color: [u8; 4]) -> BlockTextures {
        BlockTextures {
            size,
            pixels: color.repeat((size * size * layers) as usize),
        }
    }

    #[test]
    fn mip_chain_reaches_one_pixel() {
        let textures = solid(4, 2, [10, 20, 30, 255]);
        let (pixels, levels) = build_mips(&textures);
        assert_eq!(levels, 3);
        assert_eq!(pixels.len(), 2 * (16 + 4 + 1) * 4);
    }

    #[test]
    fn uniform_colour_survives_every_level() {
        let color = [200, 120, 40, 255];
        let (pixels, _) = build_mips(&solid(8, 1, color));
        for texel in pixels.chunks(4) {
            for c in 0..4 {
                assert!(texel[c].abs_diff(color[c]) <= 1, "{texel:?}");
            }
        }
    }

    #[test]
    fn transparent_pixels_do_not_darken_edges() {
        // Left column opaque green, right column transparent black.
        let mut textures = solid(2, 1, [0, 0, 0, 0]);
        for row in 0..2 {
            textures.pixels[row * 8..row * 8 + 4].copy_from_slice(&[40, 200, 40, 255]);
        }
        let (pixels, _) = build_mips(&textures);
        let last = &pixels[pixels.len() - 4..];
        assert_eq!(&last[..3], &[40, 200, 40]);
        assert_eq!(last[3], 128);
    }

    #[test]
    fn srgb_round_trips() {
        for v in [0u8, 1, 50, 128, 200, 255] {
            assert_eq!(linear_to_srgb(srgb_to_linear(v)), v);
        }
    }
}
