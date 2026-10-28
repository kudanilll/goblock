//! Terrain shape and climate.
//!
//! Generation works in real-world units and converts to blocks at the end.
//! Elevation comes out in metres from fractal noise: continents, mountain
//! ridges where a separate mask allows them, and rolling hills. Climate
//! (temperature, seasonality, rainfall and how evenly it falls) is its own
//! low-frequency noise, and temperature drops with altitude. The biome rules
//! in [`crate::biome`] read both.

use fastnoise_lite::{DomainWarpType, FastNoiseLite, FractalType, NoiseType};

use crate::biome::{self, Biome, BiomeInput};
use crate::chunk::WORLD_HEIGHT;

pub const SEA_LEVEL: i32 = 64;
/// Size of one block in metres, horizontally and vertically.
pub const METERS_PER_BLOCK: f32 = 15.0;
/// Temperature drop per metre of altitude (the standard lapse rate).
const LAPSE_RATE: f32 = 0.0065;
/// Highest ground block, leaving headroom for trees.
const MAX_GROUND: i32 = WORLD_HEIGHT - 24;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Climate {
    /// Mean annual temperature in °C.
    pub temperature: f32,
    /// Standard deviation of monthly temperature times 100.
    pub temperature_seasonality: f32,
    /// Annual precipitation in mm.
    pub precipitation: f32,
    /// Coefficient of variation of monthly precipitation, in percent.
    pub precipitation_cv: f32,
}

/// What the generator needs to know about one column.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColumnSample {
    /// Metres above sea level.
    pub elevation: f32,
    /// Y of the top ground block.
    pub height: i32,
    pub biome: Biome,
}

pub struct Terrain {
    seed: u64,
    warp: FastNoiseLite,
    continents: FastNoiseLite,
    mountain_mask: FastNoiseLite,
    ridges: FastNoiseLite,
    hills: FastNoiseLite,
    temperature: FastNoiseLite,
    temperature_detail: FastNoiseLite,
    seasonality: FastNoiseLite,
    precipitation: FastNoiseLite,
    variability: FastNoiseLite,
    snow: FastNoiseLite,
}

impl Terrain {
    pub fn new(seed: u64) -> Self {
        // Every noise layer gets its own 32-bit seed derived from the world seed.
        let mut state = seed;
        let mut next = || splitmix64(&mut state) as i32;

        let mut warp = FastNoiseLite::with_seed(next());
        warp.set_domain_warp_type(Some(DomainWarpType::OpenSimplex2));
        warp.set_domain_warp_amp(Some(60.0));
        warp.set_frequency(Some(1.0 / 700.0));

        use FractalType::{FBm, Ridged};
        use NoiseType::{OpenSimplex2, Perlin};
        Self {
            seed,
            warp,
            continents: fractal(next(), OpenSimplex2, FBm, 1.0 / 2000.0, 5),
            mountain_mask: fractal(next(), OpenSimplex2, FBm, 1.0 / 1100.0, 3),
            ridges: fractal(next(), OpenSimplex2, Ridged, 1.0 / 420.0, 5),
            hills: fractal(next(), OpenSimplex2, FBm, 1.0 / 110.0, 4),
            temperature: fractal(next(), Perlin, FBm, 1.0 / 1800.0, 4),
            temperature_detail: fractal(next(), Perlin, FBm, 1.0 / 160.0, 2),
            seasonality: fractal(next(), Perlin, FBm, 1.0 / 2200.0, 2),
            precipitation: fractal(next(), Perlin, FBm, 1.0 / 1300.0, 4),
            variability: fractal(next(), Perlin, FBm, 1.0 / 1600.0, 4),
            snow: fractal(next(), Perlin, FBm, 1.0 / 90.0, 3),
        }
    }

    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// Elevation in metres above sea level at block column (x, z).
    pub fn elevation(&self, x: f32, z: f32) -> f32 {
        let (wx, wz) = self.warp.domain_warp_2d(x, z);
        let continent = self.continents.get_noise_2d(wx, wz);
        let land = smoothstep(-0.05, 0.25, continent);

        // Mountains rise only on land, and only where the mask allows them.
        let mask = smoothstep(0.0, 0.45, self.mountain_mask.get_noise_2d(wx, wz)) * land;
        let ridge = ((self.ridges.get_noise_2d(wx, wz) + 1.0) * 0.5).clamp(0.0, 1.0);
        let mountains = mask * ridge.powf(1.6) * 3200.0;

        let hills = self.hills.get_noise_2d(wx, wz) * (40.0 + 140.0 * land);
        continent_profile(continent) + mountains + hills
    }

    /// Climate at (x, z) for ground at `elevation` metres.
    pub fn climate(&self, x: f32, z: f32, elevation: f32) -> Climate {
        let sea_level_temperature = 13.0
            + 20.0 * self.temperature.get_noise_2d(x, z)
            + 0.6 * self.temperature_detail.get_noise_2d(x, z);
        // Cold places swing further between summer and winter.
        let seasonality = 650.0
            + 450.0 * self.seasonality.get_noise_2d(x, z)
            + 25.0 * (12.0 - sea_level_temperature);
        Climate {
            temperature: sea_level_temperature - LAPSE_RATE * elevation.max(0.0),
            temperature_seasonality: seasonality.clamp(100.0, 1800.0),
            precipitation: 750.0 * (1.6 * self.precipitation.get_noise_2d(x, z)).exp(),
            precipitation_cv: (45.0 + 40.0 * self.variability.get_noise_2d(x, z)).clamp(5.0, 120.0),
        }
    }

    /// Elevation, ground height and biome of one column.
    pub fn sample(&self, x: i32, z: i32) -> ColumnSample {
        let mut around = [0.0; 9];
        for (i, elevation) in around.iter_mut().enumerate() {
            let (dx, dz) = (i as i32 % 3 - 1, i as i32 / 3 - 1);
            *elevation = self.elevation((x + dx) as f32, (z + dz) as f32);
        }
        self.column(x, z, &around)
    }

    /// Same as [`Terrain::sample`], with the elevations of the 3x3 columns
    /// centred on (x, z) already known. `around` is row by row from -z, each
    /// row from -x.
    pub(crate) fn column(&self, x: i32, z: i32, around: &[f32; 9]) -> ColumnSample {
        let elevation = around[4];
        let climate = self.climate(x as f32, z as f32, elevation);
        let biome = biome::classify(&BiomeInput {
            elevation,
            temperature: climate.temperature,
            temperature_seasonality: climate.temperature_seasonality,
            precipitation: climate.precipitation,
            precipitation_cv: climate.precipitation_cv,
            slope: slope(around),
            snow_offset: 3.0 * self.snow.get_noise_2d(x as f32, z as f32),
        });
        ColumnSample {
            elevation,
            height: surface_height(elevation),
            biome,
        }
    }

    /// A dry, fairly flat column near the origin to start the player on.
    pub fn find_spawn(&self) -> (i32, i32) {
        const STEP: i32 = 32;
        for ring in 0..128 {
            for i in -ring..=ring {
                for (x, z) in [(i, -ring), (i, ring), (-ring, i), (ring, i)] {
                    let (x, z) = (x * STEP, z * STEP);
                    let column = self.sample(x, z);
                    let good = (20.0..1500.0).contains(&column.elevation)
                        && column.biome != Biome::RockyPeaks;
                    if good {
                        return (x, z);
                    }
                }
            }
        }
        (0, 0)
    }
}

/// Ground block height for an elevation in metres. Land scales linearly. The
/// sea floor is compressed with a square root, so even the deepest ocean
/// stays inside the world.
pub fn surface_height(elevation: f32) -> i32 {
    let offset = if elevation >= 0.0 {
        (elevation / METERS_PER_BLOCK) as i32
    } else {
        (10f32.sqrt() - (10.0 - elevation).sqrt()) as i32 - 1
    };
    (SEA_LEVEL + offset).clamp(1, MAX_GROUND)
}

/// Base elevation for a continent noise value: deep ocean, continental shelf,
/// coast, lowland and upland.
fn continent_profile(continent: f32) -> f32 {
    const POINTS: [(f32, f32); 6] = [
        (-1.0, -4200.0),
        (-0.45, -2000.0),
        (-0.15, -80.0),
        (-0.05, 10.0),
        (0.3, 350.0),
        (1.0, 1100.0),
    ];
    let c = continent.clamp(-1.0, 1.0);
    for pair in POINTS.windows(2) {
        let ((c0, e0), (c1, e1)) = (pair[0], pair[1]);
        if c <= c1 {
            return e0 + (e1 - e0) * (c - c0) / (c1 - c0);
        }
    }
    POINTS[POINTS.len() - 1].1
}

/// Rise over run from a 3x3 elevation patch, using a Sobel filter.
fn slope(e: &[f32; 9]) -> f32 {
    let dx = (e[2] + 2.0 * e[5] + e[8] - e[0] - 2.0 * e[3] - e[6]) / 8.0;
    let dz = (e[6] + 2.0 * e[7] + e[8] - e[0] - 2.0 * e[1] - e[2]) / 8.0;
    (dx * dx + dz * dz).sqrt() / METERS_PER_BLOCK
}

fn fractal(
    seed: i32,
    noise: NoiseType,
    fractal: FractalType,
    frequency: f32,
    octaves: i32,
) -> FastNoiseLite {
    let mut n = FastNoiseLite::with_seed(seed);
    n.set_noise_type(Some(noise));
    n.set_frequency(Some(frequency));
    n.set_fractal_type(Some(fractal));
    n.set_fractal_octaves(Some(octaves));
    n.set_fractal_lacunarity(Some(2.0));
    n.set_fractal_gain(Some(0.5));
    n
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Stable hash of a world seed and a pair of coordinates.
pub(crate) fn hash(seed: u64, x: i32, z: i32) -> u64 {
    let mut state = seed ^ (((x as u32 as u64) << 32) | z as u32 as u64);
    splitmix64(&mut state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sea_level_elevation_sits_on_sea_level() {
        assert_eq!(surface_height(0.0), SEA_LEVEL);
        assert_eq!(surface_height(150.0), SEA_LEVEL + 10);
    }

    #[test]
    fn ocean_floor_is_compressed_but_inside_the_world() {
        let shallow = surface_height(-50.0);
        let deep = surface_height(-4200.0);
        assert!(shallow < SEA_LEVEL);
        assert!(deep < shallow);
        assert!(deep >= 1);
    }

    #[test]
    fn ground_never_reaches_the_top_of_the_world() {
        assert_eq!(surface_height(1.0e6), MAX_GROUND);
    }

    #[test]
    fn continent_profile_is_monotonic() {
        let mut last = f32::NEG_INFINITY;
        for i in -100..=100 {
            let e = continent_profile(i as f32 / 100.0);
            assert!(e >= last);
            last = e;
        }
    }

    #[test]
    fn same_seed_gives_same_terrain() {
        let (a, b) = (Terrain::new(7), Terrain::new(7));
        for (x, z) in [(0, 0), (1234, -987), (-40_000, 25_000)] {
            assert_eq!(a.sample(x, z), b.sample(x, z));
        }
    }

    #[test]
    fn different_seeds_give_different_terrain() {
        let (a, b) = (Terrain::new(1), Terrain::new(2));
        let differs =
            (0..32).any(|i| a.elevation(i as f32 * 97.0, 0.0) != b.elevation(i as f32 * 97.0, 0.0));
        assert!(differs);
    }

    #[test]
    fn world_has_land_and_sea() {
        let terrain = Terrain::new(1);
        let samples: Vec<f32> = (0..400)
            .map(|i| terrain.elevation((i % 20) as f32 * 600.0, (i / 20) as f32 * 600.0))
            .collect();
        assert!(samples.iter().any(|&e| e < 0.0), "no sea");
        assert!(samples.iter().any(|&e| e > 0.0), "no land");
    }

    #[test]
    fn climate_cools_with_altitude() {
        let terrain = Terrain::new(3);
        let low = terrain.climate(100.0, 100.0, 0.0);
        let high = terrain.climate(100.0, 100.0, 3000.0);
        assert!((low.temperature - high.temperature - 19.5).abs() < 1e-3);
    }

    #[test]
    fn flat_ground_has_no_slope() {
        assert_eq!(slope(&[5.0; 9]), 0.0);
        // One block of rise per block of run along x.
        let ramp: [f32; 9] = std::array::from_fn(|i| (i % 3) as f32 * METERS_PER_BLOCK);
        assert!((slope(&ramp) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn spawn_is_on_land() {
        let terrain = Terrain::new(11);
        let (x, z) = terrain.find_spawn();
        assert!(terrain.sample(x, z).elevation > 0.0);
    }
}
