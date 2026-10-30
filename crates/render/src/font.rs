//! TrueType font atlas generator using `ab_glyph`.
//!
//! Packs `eas-vhs.ttf` (regular UI / body text) and `upheavtt.ttf` (headlines / titles)
//! into a single 512x512 R8_UNORM texture atlas with baked glyph metrics.

use ab_glyph::{Font, FontRef, PxScale, ScaleFont};
use std::sync::OnceLock;

pub const ATLAS_W: u32 = 512;
pub const ATLAS_H: u32 = 512;

const FIRST_CHAR: u8 = 32;
const CHAR_COUNT: usize = 95; // 32..=126

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FontKind {
    /// eas-vhs.ttf: used for all general UI, labels, descriptions, values, HUD.
    Regular,
    /// upheavtt.ttf: used for headers, titles, and button emphasis.
    Headline,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct GlyphMetric {
    pub uv_min: [f32; 2],
    pub uv_max: [f32; 2],
    pub width: f32,
    pub height: f32,
    pub offset_x: f32,
    pub offset_y: f32,
    pub advance: f32,
}

struct AtlasData {
    pixels: Vec<u8>,
    regular: [GlyphMetric; CHAR_COUNT],
    headline: [GlyphMetric; CHAR_COUNT],
    white_uv: [f32; 2],
}

static ATLAS_DATA: OnceLock<AtlasData> = OnceLock::new();

fn get_data() -> &'static AtlasData {
    ATLAS_DATA.get_or_init(bake_atlas)
}

fn bake_atlas() -> AtlasData {
    let mut pixels = vec![0u8; (ATLAS_W * ATLAS_H) as usize];
    let regular_font = FontRef::try_from_slice(include_bytes!("../../../assets/fonts/eas-vhs.ttf"))
        .expect("load eas-vhs.ttf");
    let headline_font =
        FontRef::try_from_slice(include_bytes!("../../../assets/fonts/upheavtt.ttf"))
            .expect("load upheavtt.ttf");

    let mut regular_metrics = [GlyphMetric::default(); CHAR_COUNT];
    let mut headline_metrics = [GlyphMetric::default(); CHAR_COUNT];

    let mut cursor_x = 2;
    let mut cursor_y = 2;
    let mut row_height = 0;

    // 1. Pack Headline font (upheavtt at 28px)
    let headline_scale = PxScale::from(28.0);
    let scaled_headline = headline_font.as_scaled(headline_scale);
    let headline_ascent = scaled_headline.ascent();

    for (code, slot) in headline_metrics.iter_mut().enumerate() {
        let ch = (FIRST_CHAR as usize + code) as u8 as char;
        let glyph = scaled_headline.scaled_glyph(ch);
        let advance = scaled_headline.h_advance(glyph.id);

        if let Some(outlined) = scaled_headline.outline_glyph(glyph) {
            let bounds = outlined.px_bounds();
            let gw = (bounds.width().ceil() as u32).max(1);
            let gh = (bounds.height().ceil() as u32).max(1);

            if cursor_x + gw + 2 >= ATLAS_W {
                cursor_x = 2;
                cursor_y += row_height + 2;
                row_height = 0;
            }

            outlined.draw(|gx, gy, c| {
                let px = cursor_x + gx;
                let py = cursor_y + gy;
                if px < ATLAS_W && py < ATLAS_H {
                    let alpha = (c * 255.0).round() as u8;
                    pixels[(py * ATLAS_W + px) as usize] = alpha;
                }
            });

            *slot = GlyphMetric {
                uv_min: [
                    cursor_x as f32 / ATLAS_W as f32,
                    cursor_y as f32 / ATLAS_H as f32,
                ],
                uv_max: [
                    (cursor_x + gw) as f32 / ATLAS_W as f32,
                    (cursor_y + gh) as f32 / ATLAS_H as f32,
                ],
                width: gw as f32,
                height: gh as f32,
                offset_x: bounds.min.x,
                offset_y: bounds.min.y + headline_ascent,
                advance,
            };

            cursor_x += gw + 2;
            row_height = row_height.max(gh);
        } else {
            *slot = GlyphMetric {
                advance,
                ..Default::default()
            };
        }
    }

    cursor_x = 2;
    cursor_y += row_height + 4;
    row_height = 0;

    // 2. Pack Regular font (eas-vhs at 18px)
    let regular_scale = PxScale::from(18.0);
    let scaled_regular = regular_font.as_scaled(regular_scale);
    let regular_ascent = scaled_regular.ascent();

    for (code, slot) in regular_metrics.iter_mut().enumerate() {
        let ch = (FIRST_CHAR as usize + code) as u8 as char;
        let glyph = scaled_regular.scaled_glyph(ch);
        let advance = scaled_regular.h_advance(glyph.id);

        if let Some(outlined) = scaled_regular.outline_glyph(glyph) {
            let bounds = outlined.px_bounds();
            let gw = (bounds.width().ceil() as u32).max(1);
            let gh = (bounds.height().ceil() as u32).max(1);

            if cursor_x + gw + 2 >= ATLAS_W {
                cursor_x = 2;
                cursor_y += row_height + 2;
                row_height = 0;
            }

            outlined.draw(|gx, gy, c| {
                let px = cursor_x + gx;
                let py = cursor_y + gy;
                if px < ATLAS_W && py < ATLAS_H {
                    let alpha = (c * 255.0).round() as u8;
                    pixels[(py * ATLAS_W + px) as usize] = alpha;
                }
            });

            *slot = GlyphMetric {
                uv_min: [
                    cursor_x as f32 / ATLAS_W as f32,
                    cursor_y as f32 / ATLAS_H as f32,
                ],
                uv_max: [
                    (cursor_x + gw) as f32 / ATLAS_W as f32,
                    (cursor_y + gh) as f32 / ATLAS_H as f32,
                ],
                width: gw as f32,
                height: gh as f32,
                offset_x: bounds.min.x,
                offset_y: bounds.min.y + regular_ascent,
                advance,
            };

            cursor_x += gw + 2;
            row_height = row_height.max(gh);
        } else {
            *slot = GlyphMetric {
                advance,
                ..Default::default()
            };
        }
    }

    // 3. Set bottom-right pixel to solid white (255) for rendering colored rects/panels
    let white_x = ATLAS_W - 2;
    let white_y = ATLAS_H - 2;
    for dy in 0..2 {
        for dx in 0..2 {
            pixels[((white_y + dy) * ATLAS_W + white_x + dx) as usize] = 255;
        }
    }

    let white_uv = [
        (white_x as f32 + 0.5) / ATLAS_W as f32,
        (white_y as f32 + 0.5) / ATLAS_H as f32,
    ];

    AtlasData {
        pixels,
        regular: regular_metrics,
        headline: headline_metrics,
        white_uv,
    }
}

pub fn build_atlas() -> Vec<u8> {
    get_data().pixels.clone()
}

pub fn white_pixel_uv() -> [f32; 2] {
    get_data().white_uv
}

pub fn glyph_metric(kind: FontKind, ch: char) -> GlyphMetric {
    let code = ch as u32;
    if code < FIRST_CHAR as u32 || code >= (FIRST_CHAR as usize + CHAR_COUNT) as u32 {
        return GlyphMetric::default();
    }
    let idx = (code - FIRST_CHAR as u32) as usize;
    let data = get_data();
    match kind {
        FontKind::Regular => data.regular[idx],
        FontKind::Headline => data.headline[idx],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atlas_has_expected_dimensions() {
        assert_eq!(build_atlas().len(), (ATLAS_W * ATLAS_H) as usize);
    }

    #[test]
    fn white_pixel_is_solid_ink() {
        let atlas = build_atlas();
        let uv = white_pixel_uv();
        let x = (uv[0] * ATLAS_W as f32).floor() as usize;
        let y = (uv[1] * ATLAS_H as f32).floor() as usize;
        assert_eq!(atlas[y * ATLAS_W as usize + x], 255);
    }

    #[test]
    fn glyph_metrics_for_letters_have_advance() {
        for kind in [FontKind::Regular, FontKind::Headline] {
            let metric_a = glyph_metric(kind, 'A');
            assert!(metric_a.advance > 5.0, "letter A should have advance");
            assert!(metric_a.width > 0.0, "letter A should have width");
            assert!(metric_a.height > 0.0, "letter A should have height");

            let metric_space = glyph_metric(kind, ' ');
            assert!(metric_space.advance > 2.0, "space should have advance");
            assert_eq!(metric_space.width, 0.0, "space has 0 ink width");
        }
    }
}
