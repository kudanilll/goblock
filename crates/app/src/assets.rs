//! Block textures from `assets/textures/blocks/<name>.png`, with generated
//! placeholders for any file that is missing or unreadable.

use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

use goblock_render::block_textures::{linear_to_srgb, srgb_to_linear};
use goblock_render::BlockTextures;
use goblock_world::Texture;

/// Edge length every block texture is resampled to.
pub const TILE_SIZE: u32 = 64;

/// Where the block PNGs live, relative to the working directory or to the
/// executable.
const BLOCK_DIR: &str = "assets/textures/blocks";

pub struct LoadedTextures {
    pub textures: BlockTextures,
    /// Names of the textures that fell back to a placeholder.
    pub missing: Vec<&'static str>,
}

pub fn load_block_textures() -> LoadedTextures {
    let mut dirs = vec![PathBuf::from(BLOCK_DIR)];
    if let Some(exe_dir) = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
    {
        dirs.push(exe_dir.join(BLOCK_DIR));
    }

    let mut pixels = Vec::with_capacity((TILE_SIZE * TILE_SIZE * 4) as usize * Texture::ALL.len());
    let mut missing = Vec::new();
    for texture in Texture::ALL {
        let file = format!("{}.png", texture.name());
        let image = dirs
            .iter()
            .map(|dir| dir.join(&file))
            .find(|path| path.exists())
            .and_then(|path| match decode_file(&path) {
                Ok(image) => Some(image),
                Err(e) => {
                    eprintln!("goblock: cannot read {}: {e}", path.display());
                    None
                }
            });
        match image {
            Some(image) => pixels.extend(resample(&image, TILE_SIZE)),
            None => {
                missing.push(texture.name());
                pixels.extend(placeholder(texture, TILE_SIZE));
            }
        }
    }

    LoadedTextures {
        textures: BlockTextures {
            size: TILE_SIZE,
            pixels,
        },
        missing,
    }
}

/// An RGBA8 image.
struct Image {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

fn decode_file(path: &Path) -> Result<Image, String> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    decode(BufReader::new(file))
}

fn decode(reader: impl Read) -> Result<Image, String> {
    let mut decoder = png::Decoder::new(reader);
    // Expand palettes and small bit depths and drop 16-bit precision, so
    // every image arrives with 8-bit channels.
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
    let mut buffer = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buffer).map_err(|e| e.to_string())?;
    let data = &buffer[..info.buffer_size()];

    let rgba = match info.color_type {
        png::ColorType::Rgba => data.to_vec(),
        png::ColorType::Rgb => data
            .chunks(3)
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        png::ColorType::GrayscaleAlpha => data
            .chunks(2)
            .flat_map(|p| [p[0], p[0], p[0], p[1]])
            .collect(),
        png::ColorType::Grayscale => data.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::Indexed => return Err("palette was not expanded".into()),
    };
    Ok(Image {
        width: info.width,
        height: info.height,
        rgba,
    })
}

/// Scale the centre square of `image` to `size` x `size`. Each output pixel
/// averages the source pixels it covers, in linear light and weighted by
/// alpha, so large textures shrink cleanly and small ones grow blocky.
fn resample(image: &Image, size: u32) -> Vec<u8> {
    let side = image.width.min(image.height);
    let (x0, y0) = ((image.width - side) / 2, (image.height - side) / 2);
    // Source pixels covered by output pixel `i`, always at least one.
    let span = |i: u32| {
        let start = i * side / size;
        (start, ((i + 1) * side / size).max(start + 1))
    };

    let mut out = Vec::with_capacity((size * size * 4) as usize);
    for oy in 0..size {
        let (sy0, sy1) = span(oy);
        for ox in 0..size {
            let (sx0, sx1) = span(ox);
            let mut rgb = [0.0f32; 3];
            let mut alpha = 0.0;
            for sy in sy0..sy1 {
                for sx in sx0..sx1 {
                    let i = (((y0 + sy) * image.width + x0 + sx) * 4) as usize;
                    let a = image.rgba[i + 3] as f32 / 255.0;
                    for (c, sum) in rgb.iter_mut().enumerate() {
                        *sum += srgb_to_linear(image.rgba[i + c]) * a;
                    }
                    alpha += a;
                }
            }
            let count = ((sy1 - sy0) * (sx1 - sx0)) as f32;
            for value in rgb {
                out.push(if alpha > 0.0 {
                    linear_to_srgb(value / alpha)
                } else {
                    0
                });
            }
            out.push((alpha / count * 255.0).round() as u8);
        }
    }
    out
}

/// A tile in the texture's placeholder colour with light per-pixel noise, so
/// faces still read as surfaces. Grass sides get their top band, and leaves
/// get holes for the cut-out pass.
fn placeholder(texture: Texture, size: u32) -> Vec<u8> {
    let band = match texture {
        Texture::GrassSide => Some(Texture::GrassTop.placeholder_color()),
        Texture::GrassSideSnowy => Some(Texture::Snow.placeholder_color()),
        _ => None,
    };
    let mut out = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let noise = hash(x, y, texture as u32);
            let [r, g, b, a] = match band {
                Some(color) if y < size / 4 => color,
                _ => texture.placeholder_color(),
            };
            let offset = (noise % 25) as i32 - 12;
            let shade = |c: u8| (c as i32 + offset).clamp(0, 255) as u8;
            let hole = texture == Texture::Leaves && noise % 5 == 4;
            out.extend([shade(r), shade(g), shade(b), if hole { 0 } else { a }]);
        }
    }
    out
}

fn hash(x: u32, y: u32, salt: u32) -> u32 {
    let mut h =
        x.wrapping_mul(0x8da6_b343) ^ y.wrapping_mul(0xd816_3841) ^ salt.wrapping_mul(0xcb1a_b31f);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1_e995);
    h ^ (h >> 15)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode(width: u32, height: u32, color: png::ColorType, data: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::new();
        let mut encoder = png::Encoder::new(&mut bytes, width, height);
        encoder.set_color(color);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(data).unwrap();
        drop(writer);
        bytes
    }

    #[test]
    fn rgb_png_decodes_to_opaque_rgba() {
        let bytes = encode(2, 1, png::ColorType::Rgb, &[255, 0, 0, 0, 0, 255]);
        let image = decode(bytes.as_slice()).unwrap();
        assert_eq!((image.width, image.height), (2, 1));
        assert_eq!(image.rgba, vec![255, 0, 0, 255, 0, 0, 255, 255]);
    }

    #[test]
    fn grey_png_decodes_to_rgba() {
        let bytes = encode(1, 1, png::ColorType::GrayscaleAlpha, &[100, 50]);
        assert_eq!(
            decode(bytes.as_slice()).unwrap().rgba,
            vec![100, 100, 100, 50]
        );
    }

    #[test]
    fn garbage_is_an_error() {
        assert!(decode(&b"not a png"[..]).is_err());
    }

    #[test]
    fn uniform_image_keeps_its_colour_when_shrunk() {
        let image = Image {
            width: 8,
            height: 8,
            rgba: [90, 160, 30, 255].repeat(64),
        };
        assert_eq!(resample(&image, 2), [90, 160, 30, 255].repeat(4));
    }

    #[test]
    fn small_image_grows_by_repeating_pixels() {
        let image = Image {
            width: 2,
            height: 2,
            rgba: [
                [255, 0, 0, 255],
                [0, 255, 0, 255],
                [0, 0, 255, 255],
                [255, 255, 255, 255],
            ]
            .concat(),
        };
        let out = resample(&image, 4);
        // The top-left quarter is all red.
        for (x, y) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            let i = (y * 4 + x) * 4;
            assert_eq!(&out[i..i + 4], &[255, 0, 0, 255]);
        }
    }

    #[test]
    fn wide_image_uses_its_centre_square() {
        // Red, green, green, red: the centre square is green.
        let row = [
            [255, 0, 0, 255],
            [0, 255, 0, 255],
            [0, 255, 0, 255],
            [255, 0, 0, 255],
        ]
        .concat();
        let image = Image {
            width: 4,
            height: 2,
            rgba: row.repeat(2),
        };
        assert_eq!(resample(&image, 1), vec![0, 255, 0, 255]);
    }

    #[test]
    fn every_layer_gets_pixels() {
        let loaded = load_block_textures();
        let layer_bytes = (TILE_SIZE * TILE_SIZE * 4) as usize;
        assert_eq!(
            loaded.textures.pixels.len(),
            layer_bytes * Texture::ALL.len()
        );
        assert_eq!(loaded.textures.layers() as usize, Texture::ALL.len());
    }

    #[test]
    fn leaf_placeholder_has_holes() {
        let tile = placeholder(Texture::Leaves, 16);
        assert!(tile.chunks(4).any(|p| p[3] == 0));
        assert!(tile.chunks(4).any(|p| p[3] > 0));
    }
}
