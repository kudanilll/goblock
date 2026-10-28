//! Deterministic terrain height generation.
//!
//! Each column's height mixes two layers of hashed value noise, which gives
//! the world some relief without pulling in a noise crate.

/// Height in blocks for the column at world coordinates `(x, z)`.
///
/// Fully deterministic: the same input always produces the same height, which
/// keeps chunks independent of generation order.
pub fn column_height(x: i32, z: i32) -> i32 {
    const BASE: f32 = 3.0;
    const AMPLITUDE: f32 = 3.0;

    let broad = value_noise(x as f32 * 0.05, z as f32 * 0.05);
    let detail = value_noise(x as f32 * 0.13 + 31.7, z as f32 * 0.13 + 11.3);

    let h = BASE + (broad * AMPLITUDE) + (detail * 1.5);
    (h.round() as i32).max(1)
}

/// Smoothly interpolated value noise in roughly `[0, 1]`.
fn value_noise(x: f32, z: f32) -> f32 {
    let x0 = x.floor();
    let z0 = z.floor();
    let fx = x - x0;
    let fz = z - z0;

    let sx = smoothstep(fx);
    let sz = smoothstep(fz);

    let v00 = hash2(x0 as i32, z0 as i32);
    let v10 = hash2(x0 as i32 + 1, z0 as i32);
    let v01 = hash2(x0 as i32, z0 as i32 + 1);
    let v11 = hash2(x0 as i32 + 1, z0 as i32 + 1);

    let top = lerp(v00, v10, sx);
    let bottom = lerp(v01, v11, sx);
    lerp(top, bottom, sz)
}

fn smoothstep(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Stable 2D hash in `[0, 1]`.
fn hash2(x: i32, z: i32) -> f32 {
    // Wrapping arithmetic keeps this deterministic and panic-free for any input.
    let mut h = (x as u32).wrapping_mul(0x8da6_b343) ^ (z as u32).wrapping_mul(0xd816_3841);
    h ^= h >> 13;
    h = h.wrapping_mul(0xb529_7a4d);
    h ^= h >> 15;
    (h & 0x00ff_ffff) as f32 / 0x00ff_ffff as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn height_is_deterministic() {
        for (x, z) in [(0, 0), (17, -42), (128, 128), (-7, 300)] {
            assert_eq!(column_height(x, z), column_height(x, z));
        }
    }

    #[test]
    fn height_is_always_positive() {
        for x in -64..64 {
            for z in -64..64 {
                assert!(column_height(x, z) >= 1, "height at ({x}, {z})");
            }
        }
    }

    #[test]
    fn height_stays_within_expected_band() {
        // BASE 3 + broad*3 + detail*1.5, both noise terms in [0, 1].
        for x in 0..256 {
            for z in 0..256 {
                let h = column_height(x, z);
                assert!(
                    (1..=8).contains(&h),
                    "height {h} at ({x}, {z}) outside expected band"
                );
            }
        }
    }

    #[test]
    fn noise_is_bounded() {
        for x in -50..50 {
            for z in -50..50 {
                let v = value_noise(x as f32 * 0.37, z as f32 * 0.37);
                assert!((0.0..=1.0).contains(&v), "noise {v} out of range");
            }
        }
    }

    #[test]
    fn terrain_is_not_flat() {
        let heights: std::collections::HashSet<i32> = (0..64)
            .flat_map(|x| (0..64).map(move |z| column_height(x, z)))
            .collect();
        assert!(heights.len() > 1, "terrain used only {heights:?}");
    }
}
