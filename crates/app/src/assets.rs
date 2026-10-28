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
            None if Texture::GRASS_TUFTS.contains(&texture) => {
                pixels.extend(grass_tuft(texture, TILE_SIZE));
            }
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

/// Grass tuft sprites are pixel art on a grid this many cells across. At the
/// 64-texel tile size a cell is 2x2 texels, about the detail of the block
/// textures.
const TUFT_GRID: usize = 32;

/// Tuft greens in sRGB, from the roots to the tips.
const TUFT_GREENS: [[u8; 3]; 5] = [
    [52, 88, 40],
    [68, 110, 46],
    [88, 134, 52],
    [108, 154, 58],
    [134, 178, 68],
];

/// The dark and light shade of a seed head, sRGB.
const TUFT_SEEDS: [[u8; 3]; 2] = [[166, 148, 86], [204, 190, 112]];

/// The built-in sprite of a grass tuft texture, rooted along the bottom
/// edge. Each blade is a root column, a height in cells and how many cells
/// its tip leans to the side. The lean grows with the square of the height,
/// so a blade rises straight and arcs over near its tip. In the seeded tuft,
/// stems taller than 20 cells end in a seed head.
fn grass_tuft(texture: Texture, size: u32) -> Vec<u8> {
    let blades: &[(i32, i32, i32)] = match texture {
        Texture::GrassTuftShort => &[
            (8, 7, -5),
            (10, 9, -5),
            (12, 12, -3),
            (13, 8, -2),
            (15, 13, 0),
            (16, 10, 1),
            (17, 11, 2),
            (18, 9, 3),
            (19, 12, 4),
            (21, 8, 6),
        ],
        Texture::GrassTuftTall => &[
            (8, 16, -6),
            (10, 21, -6),
            (12, 25, -4),
            (13, 19, -2),
            (14, 29, -1),
            (16, 23, 1),
            (17, 18, 2),
            (18, 27, 4),
            (20, 22, 6),
            (22, 16, 7),
        ],
        Texture::GrassTuftSeeds => &[
            (9, 11, -6),
            (11, 13, -5),
            (12, 10, -3),
            (14, 26, -3),
            (15, 14, 0),
            (17, 23, 3),
            (18, 11, 3),
            (20, 13, 5),
            (22, 10, 6),
        ],
        _ => &[
            (8, 12, -6),
            (10, 15, -5),
            (12, 18, -4),
            (13, 14, -2),
            (14, 21, -1),
            (16, 17, 1),
            (17, 13, 2),
            (18, 20, 3),
            (20, 15, 5),
            (22, 12, 6),
        ],
    };
    let seeds = texture == Texture::GrassTuftSeeds;

    let mut cells = [[None; TUFT_GRID]; TUFT_GRID];
    for &(root, height, lean) in blades {
        for i in 0..height {
            let t = i as f32 / (height - 1) as f32;
            let x = (root + (lean as f32 * t * t).round() as i32) as usize;
            let y = TUFT_GRID - 1 - i as usize;
            let shade = match i {
                0 => 0,
                _ if i == height - 1 => 4,
                _ if t < 0.25 => 1,
                _ if t < 0.6 => 2,
                _ => 3,
            };
            cells[y][x] = Some(TUFT_GREENS[shade]);

            // The head covers the top five cells of the stem in alternating
            // shades, two cells wide below its tip.
            let from_tip = (height - i) as usize;
            if seeds && height > 20 && from_tip <= 5 {
                cells[y][x] = Some(TUFT_SEEDS[from_tip % 2]);
                if (2..=4).contains(&from_tip) {
                    let side = if lean < 0 { x - 1 } else { x + 1 };
                    cells[y][side] = Some(TUFT_SEEDS[(from_tip + 1) % 2]);
                }
            }
        }
    }

    let cell = |texel: u32| texel as usize * TUFT_GRID / size as usize;
    (0..size * size)
        .flat_map(|i| match cells[cell(i / size)][cell(i % size)] {
            Some([r, g, b]) => [r, g, b, 255],
            None => [0; 4],
        })
        .collect()
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
    fn grass_tufts_are_distinct_rooted_pixel_art() {
        let tiles = Texture::GRASS_TUFTS.map(|texture| grass_tuft(texture, TILE_SIZE));
        let cell = TILE_SIZE / TUFT_GRID as u32;
        for tile in &tiles {
            let texel = |x: u32, y: u32| {
                let i = ((y * TILE_SIZE + x) * 4) as usize;
                &tile[i..i + 4]
            };
            let opaque = |y: u32| (0..TILE_SIZE).filter(|&x| texel(x, y)[3] == 255).count();
            assert!(
                opaque(TILE_SIZE - 1) > 0,
                "roots must reach the bottom edge"
            );
            assert_eq!(opaque(0), 0, "tips must not be cut off by the top edge");
            for y in 0..TILE_SIZE {
                for x in 0..TILE_SIZE {
                    assert_eq!(texel(x, y), texel(x - x % cell, y - y % cell));
                }
            }
        }
        for (i, tile) in tiles.iter().enumerate() {
            assert!(tiles[i + 1..].iter().all(|other| other != tile));
        }
    }

    #[test]
    fn leaf_placeholder_has_holes() {
        let tile = placeholder(Texture::Leaves, 16);
        assert!(tile.chunks(4).any(|p| p[3] == 0));
        assert!(tile.chunks(4).any(|p| p[3] > 0));
    }
}
