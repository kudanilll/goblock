//! A built-in 5x7 bitmap font for the debug HUD.
//!
//! The glyphs live in this file, so the HUD needs no font asset and looks the
//! same on every platform.

/// Glyph cell size in font pixels.
pub const GLYPH_W: u32 = 5;
pub const GLYPH_H: u32 = 7;
/// Blank pixels between glyphs.
pub const GLYPH_PAD: u32 = 1;

/// Atlas is 16 glyphs per row.
const ATLAS_COLS: u32 = 16;
const ATLAS_ROWS: u32 = 6;
/// First printable ASCII character.
const FIRST_CHAR: u8 = 32;
const CHAR_COUNT: u32 = 96;

pub const ATLAS_W: u32 = ATLAS_COLS * (GLYPH_W + GLYPH_PAD);
pub const ATLAS_H: u32 = ATLAS_ROWS * (GLYPH_H + GLYPH_PAD);

/// 5x7 glyph bitmaps, row-major, `1` = ink. Only the printable ASCII range.
fn glyph_bitmap(ch: char) -> [u8; 7] {
    match ch {
        ' ' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
        '!' => [0x04, 0x04, 0x04, 0x04, 0x00, 0x04, 0x00],
        '+' => [0x00, 0x04, 0x1f, 0x04, 0x00, 0x00, 0x00],
        '-' => [0x00, 0x00, 0x1f, 0x00, 0x00, 0x00, 0x00],
        '.' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x0c, 0x0c],
        ',' => [0x00, 0x00, 0x00, 0x00, 0x0c, 0x04, 0x08],
        ':' => [0x00, 0x0c, 0x0c, 0x00, 0x0c, 0x0c, 0x00],
        '/' => [0x01, 0x02, 0x02, 0x04, 0x08, 0x08, 0x10],
        '(' => [0x02, 0x04, 0x08, 0x08, 0x08, 0x04, 0x02],
        ')' => [0x08, 0x04, 0x02, 0x02, 0x02, 0x04, 0x08],
        '0'..='9' => digit_bitmap(ch),
        'A'..='Z' => letter_bitmap(ch, false),
        _ => letter_bitmap(ch, true),
    }
}

fn digit_bitmap(ch: char) -> [u8; 7] {
    match ch {
        '0' => [0x0e, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0e],
        '1' => [0x04, 0x0c, 0x04, 0x04, 0x04, 0x04, 0x0e],
        '2' => [0x0e, 0x11, 0x01, 0x02, 0x04, 0x08, 0x1f],
        '3' => [0x1f, 0x02, 0x04, 0x02, 0x01, 0x11, 0x0e],
        '4' => [0x02, 0x06, 0x0a, 0x12, 0x1f, 0x02, 0x02],
        '5' => [0x1f, 0x10, 0x1e, 0x01, 0x01, 0x11, 0x0e],
        '6' => [0x06, 0x08, 0x10, 0x1e, 0x11, 0x11, 0x0e],
        '7' => [0x1f, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08],
        '8' => [0x0e, 0x11, 0x11, 0x0e, 0x11, 0x11, 0x0e],
        _ => [0x0e, 0x11, 0x11, 0x0f, 0x01, 0x02, 0x0c],
    }
}

fn letter_bitmap(ch: char, lowercase: bool) -> [u8; 7] {
    let upper = ch.to_ascii_uppercase();
    let base = match upper {
        'A' => [0x0e, 0x11, 0x11, 0x1f, 0x11, 0x11, 0x11],
        'B' => [0x1e, 0x11, 0x11, 0x1e, 0x11, 0x11, 0x1e],
        'C' => [0x0e, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0e],
        'D' => [0x1e, 0x11, 0x11, 0x11, 0x11, 0x11, 0x1e],
        'E' => [0x1f, 0x10, 0x10, 0x1e, 0x10, 0x10, 0x1f],
        'F' => [0x1f, 0x10, 0x10, 0x1e, 0x10, 0x10, 0x10],
        'G' => [0x0e, 0x11, 0x10, 0x17, 0x11, 0x11, 0x0f],
        'H' => [0x11, 0x11, 0x11, 0x1f, 0x11, 0x11, 0x11],
        'I' => [0x0e, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0e],
        'J' => [0x07, 0x02, 0x02, 0x02, 0x02, 0x12, 0x0c],
        'K' => [0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11],
        'L' => [0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1f],
        'M' => [0x11, 0x1b, 0x15, 0x15, 0x11, 0x11, 0x11],
        'N' => [0x11, 0x19, 0x15, 0x13, 0x11, 0x11, 0x11],
        'O' => [0x0e, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0e],
        'P' => [0x1e, 0x11, 0x11, 0x1e, 0x10, 0x10, 0x10],
        'Q' => [0x0e, 0x11, 0x11, 0x11, 0x15, 0x12, 0x0d],
        'R' => [0x1e, 0x11, 0x11, 0x1e, 0x14, 0x12, 0x11],
        'S' => [0x0f, 0x10, 0x10, 0x0e, 0x01, 0x01, 0x1e],
        'T' => [0x1f, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04],
        'U' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0e],
        'V' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x0a, 0x04],
        'W' => [0x11, 0x11, 0x11, 0x15, 0x15, 0x1b, 0x11],
        'X' => [0x11, 0x11, 0x0a, 0x04, 0x0a, 0x11, 0x11],
        'Y' => [0x11, 0x11, 0x0a, 0x04, 0x04, 0x04, 0x04],
        'Z' => [0x1f, 0x01, 0x02, 0x04, 0x08, 0x10, 0x1f],
        '_' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x1f],
        _ => [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    };

    if lowercase {
        to_lowercase_shape(upper, base)
    } else {
        base
    }
}

/// Lowercase letters reuse the capital's shape minus its top two rows, so
/// they come out shorter than capitals.
fn to_lowercase_shape(upper: char, base: [u8; 7]) -> [u8; 7] {
    // Punctuation also arrives here through the fallback arm in
    // `glyph_bitmap` and keeps its shape.
    if !upper.is_ascii_uppercase() {
        return base;
    }
    [0x00, base[2], base[3], base[4], base[5], base[6], 0x00]
}

/// Build the single-channel atlas texture.
pub fn build_atlas() -> Vec<u8> {
    let mut pixels = vec![0u8; (ATLAS_W * ATLAS_H) as usize];

    for code in 0..CHAR_COUNT {
        let ch = (FIRST_CHAR as u32 + code) as u8 as char;
        let glyph = glyph_bitmap(ch);
        let cell = code;
        let cx = (cell % ATLAS_COLS) * (GLYPH_W + GLYPH_PAD);
        let cy = (cell / ATLAS_COLS) * (GLYPH_H + GLYPH_PAD);

        for (row, bits) in glyph.iter().enumerate() {
            for col in 0..GLYPH_W {
                let on = bits & (1 << (GLYPH_W - 1 - col)) != 0;
                if on {
                    let x = cx + col;
                    let y = cy + row as u32;
                    pixels[(y * ATLAS_W + x) as usize] = 255;
                }
            }
        }
    }

    pixels
}

/// Texture coordinates for `ch`, in `[0, 1]`.
pub fn glyph_uv(ch: char) -> ([f32; 2], [f32; 2]) {
    let code = ch as u32;
    if !(FIRST_CHAR as u32..FIRST_CHAR as u32 + CHAR_COUNT).contains(&code) {
        return ([0.0, 0.0], [0.0, 0.0]);
    }
    let cell = code - FIRST_CHAR as u32;
    let cx = (cell % ATLAS_COLS) * (GLYPH_W + GLYPH_PAD);
    let cy = (cell / ATLAS_COLS) * (GLYPH_H + GLYPH_PAD);

    let u0 = cx as f32 / ATLAS_W as f32;
    let v0 = cy as f32 / ATLAS_H as f32;
    let u1 = (cx + GLYPH_W) as f32 / ATLAS_W as f32;
    let v1 = (cy + GLYPH_H) as f32 / ATLAS_H as f32;
    ([u0, v0], [u1, v1])
}

/// Advance in font pixels for one character, including padding.
pub fn advance() -> u32 {
    GLYPH_W + GLYPH_PAD
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atlas_has_expected_size() {
        assert_eq!(build_atlas().len(), (ATLAS_W * ATLAS_H) as usize);
    }

    #[test]
    fn space_is_blank() {
        assert_eq!(glyph_bitmap(' '), [0; 7]);
    }

    #[test]
    fn digits_have_ink() {
        for ch in '0'..='9' {
            let g = glyph_bitmap(ch);
            assert!(g.iter().any(|&b| b != 0), "digit {ch} is blank");
        }
    }

    #[test]
    fn letters_have_ink() {
        for ch in 'A'..='Z' {
            let g = glyph_bitmap(ch);
            assert!(g.iter().any(|&b| b != 0), "letter {ch} is blank");
        }
        for ch in 'a'..='z' {
            let g = glyph_bitmap(ch);
            assert!(g.iter().any(|&b| b != 0), "letter {ch} is blank");
        }
    }

    #[test]
    fn uv_is_inside_atlas() {
        for ch in ' '..='~' {
            let (min, max) = glyph_uv(ch);
            for v in [min[0], min[1], max[0], max[1]] {
                assert!((0.0..=1.0).contains(&v), "uv {v} out of range for {ch}");
            }
            assert!(min[0] < max[0] && min[1] < max[1]);
        }
    }

    #[test]
    fn atlas_contains_ink() {
        let atlas = build_atlas();
        assert!(atlas.iter().any(|&p| p > 0));
    }
}
