//! Biomes and the rules that pick them.
//!
//! The rules follow the biome classifier in terrain-diffusion-mc (MIT, see
//! THIRD-PARTY-NOTICES.md). Rainfall measured against potential
//! evapotranspiration decides how many trees grow, a short growing season
//! rules forests out, steep ground goes bare, and snow needs both cold and
//! enough precipitation.

use crate::block::BlockId;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Biome {
    #[default]
    Plains,
    Tundra,
    Desert,
    Savanna,
    Shrubland,
    Highlands,
    Woodland,
    Forest,
    Jungle,
    Swamp,
    SparseTaiga,
    Taiga,
    SnowyTaiga,
    SnowySparseTaiga,
    AlpineSnow,
    Glacier,
    RockyPeaks,
    Ocean,
    WarmOcean,
    ColdOcean,
    FrozenOcean,
}

/// Everything the rules look at for one column.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BiomeInput {
    /// Metres above sea level; negative under the sea.
    pub elevation: f32,
    /// Mean annual temperature in °C.
    pub temperature: f32,
    /// Standard deviation of monthly temperature times 100.
    pub temperature_seasonality: f32,
    /// Annual precipitation in mm.
    pub precipitation: f32,
    /// Coefficient of variation of monthly precipitation, in percent.
    pub precipitation_cv: f32,
    /// Rise over run of the ground.
    pub slope: f32,
    /// Local offset in °C added to the temperature for the snow test only,
    /// so the snow line wanders instead of following a contour.
    pub snow_offset: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Cover {
    None,
    Sparse,
    Forest,
    Dense,
    Rainforest,
}

/// Kinds of tree the generator can grow.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TreeKind {
    Broadleaf,
    Tall,
    Conifer,
}

/// Ground layers: `top` on the surface, then `depth` blocks of `filler`,
/// then stone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Surface {
    pub top: BlockId,
    pub filler: BlockId,
    pub depth: i32,
}

pub fn classify(input: &BiomeInput) -> Biome {
    let temp = input.temperature;
    let precip = input.precipitation.max(0.0);
    let altitude = input.elevation.max(0.0);

    // Water left over for plants once evaporation is accounted for.
    let t_std = input.temperature_seasonality / 100.0;
    let t_eff = (temp + 0.5 * t_std).max(0.0);
    let pet = (250.0 + 25.0 * t_eff + 0.7 * t_eff * t_eff).max(250.0);
    let aridity = precip / pet;
    let season_penalty = 1.0 - 0.35 * (input.precipitation_cv / 100.0).min(1.0);
    let tree_moisture = aridity * season_penalty;

    // Days per year above 5 °C, treating the year as a sine wave.
    let amplitude = t_std * std::f32::consts::SQRT_2;
    let growing_season = if amplitude < 0.1 {
        if temp > 5.0 {
            365.0
        } else {
            0.0
        }
    } else {
        let x = (5.0 - temp) / amplitude;
        if x <= -1.0 {
            365.0
        } else if x >= 1.0 {
            0.0
        } else {
            365.0 * (0.5 - x.asin() / std::f32::consts::PI)
        }
    };
    let season_factor = ((growing_season - 60.0) / 90.0).clamp(0.0, 1.0);
    let effective_moisture = tree_moisture * season_factor;

    let mut cover = match effective_moisture {
        m if m < 0.2 => Cover::None,
        m if m < 0.5 => Cover::Sparse,
        m if m < 0.8 => Cover::Forest,
        m if m < 1.3 => Cover::Dense,
        _ => Cover::Rainforest,
    };
    let barren = tree_moisture < 0.05 || growing_season < 60.0;

    // Wetter ground holds steeper slopes before it erodes to rock.
    let moisture_factor = ((tree_moisture - 0.35) / 0.45).clamp(0.0, 1.0);
    let bare_threshold = 0.7 + 0.49 * moisture_factor;
    let slope_medium = input.slope >= 0.62 && input.slope < bare_threshold;
    let slope_bare = input.slope >= bare_threshold;
    if slope_medium {
        cover = cover.min(Cover::Sparse);
    }
    if slope_bare {
        cover = Cover::None;
    }

    let has_snow = temp + input.snow_offset < 0.0 && precip > 150.0 && input.slope <= 0.78;

    let frozen = temp < -5.0;
    let cold = (-5.0..5.0).contains(&temp);
    let cool = (5.0..12.0).contains(&temp);
    let temperate = (12.0..20.0).contains(&temp);
    let warm = (20.0..26.0).contains(&temp);
    let hot = temp >= 26.0;
    let lowland = altitude < 200.0;
    let few_trees = matches!(cover, Cover::Sparse | Cover::Forest);
    let dry = tree_moisture < 0.35 || precip < 350.0;

    if input.elevation < 0.0 {
        return if frozen {
            Biome::FrozenOcean
        } else if cold {
            Biome::ColdOcean
        } else if warm || hot {
            Biome::WarmOcean
        } else {
            Biome::Ocean
        };
    }

    let biome = if altitude > 2500.0 {
        if slope_bare {
            if has_snow {
                Biome::Glacier
            } else {
                Biome::RockyPeaks
            }
        } else if has_snow {
            match cover {
                Cover::None => Biome::AlpineSnow,
                Cover::Sparse | Cover::Forest => Biome::SnowySparseTaiga,
                _ => Biome::SnowyTaiga,
            }
        } else if cover == Cover::None {
            if barren {
                Biome::Highlands
            } else if dry {
                Biome::Shrubland
            } else {
                Biome::Plains
            }
        } else if few_trees {
            Biome::SparseTaiga
        } else {
            Biome::Taiga
        }
    } else if has_snow {
        match cover {
            Cover::None => Biome::Tundra,
            Cover::Sparse | Cover::Forest => Biome::SnowySparseTaiga,
            _ => Biome::SnowyTaiga,
        }
    } else {
        match cover {
            Cover::None => {
                if warm || hot {
                    Biome::Desert
                } else if (barren && !lowland && (cold || cool || temperate)) || dry {
                    Biome::Shrubland
                } else {
                    Biome::Plains
                }
            }
            Cover::Sparse | Cover::Forest => {
                if hot {
                    Biome::Jungle
                } else if warm && cover == Cover::Sparse && !slope_medium {
                    Biome::Savanna
                } else if warm || temperate {
                    Biome::Woodland
                } else {
                    Biome::SparseTaiga
                }
            }
            Cover::Dense => {
                if hot {
                    Biome::Jungle
                } else if warm && lowland {
                    Biome::Swamp
                } else if cool || cold {
                    Biome::Taiga
                } else {
                    Biome::Forest
                }
            }
            Cover::Rainforest => {
                if hot || (warm && temp >= 18.0 && t_std < 5.0) {
                    Biome::Jungle
                } else if lowland {
                    Biome::Swamp
                } else if cool || cold {
                    Biome::Taiga
                } else {
                    Biome::Forest
                }
            }
        }
    };

    // Cliffs below the mountain band are bare rock too.
    if slope_bare && altitude <= 2500.0 {
        return if has_snow {
            Biome::Glacier
        } else {
            Biome::RockyPeaks
        };
    }
    biome
}

impl Biome {
    pub fn name(self) -> &'static str {
        match self {
            Biome::Plains => "Plains",
            Biome::Tundra => "Tundra",
            Biome::Desert => "Desert",
            Biome::Savanna => "Savanna",
            Biome::Shrubland => "Shrubland",
            Biome::Highlands => "Highlands",
            Biome::Woodland => "Woodland",
            Biome::Forest => "Forest",
            Biome::Jungle => "Jungle",
            Biome::Swamp => "Swamp",
            Biome::SparseTaiga => "Sparse taiga",
            Biome::Taiga => "Taiga",
            Biome::SnowyTaiga => "Snowy taiga",
            Biome::SnowySparseTaiga => "Snowy sparse taiga",
            Biome::AlpineSnow => "Alpine snow",
            Biome::Glacier => "Glacier",
            Biome::RockyPeaks => "Rocky peaks",
            Biome::Ocean => "Ocean",
            Biome::WarmOcean => "Warm ocean",
            Biome::ColdOcean => "Cold ocean",
            Biome::FrozenOcean => "Frozen ocean",
        }
    }

    pub fn is_ocean(self) -> bool {
        matches!(
            self,
            Biome::Ocean | Biome::WarmOcean | Biome::ColdOcean | Biome::FrozenOcean
        )
    }

    fn is_snowy(self) -> bool {
        matches!(
            self,
            Biome::Tundra
                | Biome::SnowyTaiga
                | Biome::SnowySparseTaiga
                | Biome::AlpineSnow
                | Biome::Glacier
                | Biome::FrozenOcean
        )
    }

    pub(crate) fn surface(self) -> Surface {
        let (top, filler, depth) = match self {
            Biome::Desert => (BlockId::Sand, BlockId::Sandstone, 6),
            Biome::RockyPeaks => (BlockId::Stone, BlockId::Stone, 0),
            Biome::AlpineSnow => (BlockId::Snow, BlockId::Dirt, 2),
            Biome::Glacier => (BlockId::Snow, BlockId::Snow, 3),
            Biome::Highlands => (BlockId::Grass, BlockId::Gravel, 2),
            Biome::Tundra | Biome::SnowyTaiga | Biome::SnowySparseTaiga => {
                (BlockId::SnowyGrass, BlockId::Dirt, 3)
            }
            Biome::Ocean | Biome::WarmOcean => (BlockId::Sand, BlockId::Sand, 3),
            Biome::ColdOcean | Biome::FrozenOcean => (BlockId::Gravel, BlockId::Gravel, 3),
            _ => (BlockId::Grass, BlockId::Dirt, 3),
        };
        Surface { top, filler, depth }
    }

    /// Chance that a tree spot (one per 5x5 block cell) grows a tree.
    pub(crate) fn tree_density(self) -> f32 {
        match self {
            Biome::Jungle => 0.95,
            Biome::Forest => 0.8,
            Biome::Taiga => 0.75,
            Biome::SnowyTaiga => 0.6,
            Biome::Swamp => 0.35,
            Biome::Woodland => 0.3,
            Biome::SparseTaiga => 0.3,
            Biome::SnowySparseTaiga => 0.25,
            Biome::Savanna => 0.08,
            Biome::Plains => 0.02,
            _ => 0.0,
        }
    }

    pub(crate) fn tree_kind(self) -> TreeKind {
        match self {
            Biome::Jungle => TreeKind::Tall,
            Biome::SparseTaiga | Biome::Taiga | Biome::SnowyTaiga | Biome::SnowySparseTaiga => {
                TreeKind::Conifer
            }
            _ => TreeKind::Broadleaf,
        }
    }

    /// Multiplier for grass and leaf textures, sRGB.
    pub fn foliage_tint(self) -> [u8; 3] {
        match self {
            Biome::Jungle => [190, 255, 170],
            Biome::Forest | Biome::Woodland => [225, 250, 215],
            Biome::Swamp => [175, 190, 140],
            Biome::Savanna => [250, 235, 170],
            Biome::Shrubland | Biome::Highlands => [235, 225, 180],
            Biome::Desert => [245, 225, 160],
            _ if self.is_snowy() => [200, 225, 220],
            Biome::SparseTaiga | Biome::Taiga => [200, 225, 205],
            _ => [255, 255, 255],
        }
    }

    /// Multiplier for the water texture, sRGB.
    pub fn water_tint(self) -> [u8; 3] {
        match self {
            Biome::WarmOcean => [200, 255, 245],
            Biome::ColdOcean | Biome::FrozenOcean => [215, 225, 255],
            Biome::Swamp => [170, 190, 150],
            _ => [255, 255, 255],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(elevation: f32, temperature: f32, precipitation: f32) -> BiomeInput {
        BiomeInput {
            elevation,
            temperature,
            temperature_seasonality: 600.0,
            precipitation,
            precipitation_cv: 40.0,
            slope: 0.1,
            snow_offset: 0.0,
        }
    }

    #[test]
    fn below_sea_level_is_ocean() {
        assert_eq!(classify(&input(-200.0, 15.0, 800.0)), Biome::Ocean);
        assert_eq!(classify(&input(-200.0, -10.0, 800.0)), Biome::FrozenOcean);
        assert_eq!(classify(&input(-200.0, 28.0, 800.0)), Biome::WarmOcean);
    }

    #[test]
    fn hot_and_dry_is_desert() {
        assert_eq!(classify(&input(100.0, 28.0, 100.0)), Biome::Desert);
    }

    #[test]
    fn hot_and_wet_is_jungle() {
        assert_eq!(classify(&input(100.0, 28.0, 3000.0)), Biome::Jungle);
    }

    #[test]
    fn mild_and_wet_is_forest() {
        assert_eq!(classify(&input(500.0, 15.0, 1400.0)), Biome::Forest);
    }

    #[test]
    fn cold_and_snowy_lowland_is_tundra() {
        assert_eq!(classify(&input(100.0, -8.0, 300.0)), Biome::Tundra);
    }

    #[test]
    fn steep_ground_is_bare_rock() {
        let mut steep = input(800.0, 15.0, 1400.0);
        steep.slope = 2.0;
        assert_eq!(classify(&steep), Biome::RockyPeaks);
        // Snow does not settle on slopes this steep, even in the cold.
        steep.temperature = -10.0;
        assert_eq!(classify(&steep), Biome::RockyPeaks);
    }

    #[test]
    fn high_snowy_treeless_ground_is_alpine_snow() {
        assert_eq!(classify(&input(3200.0, -6.0, 300.0)), Biome::AlpineSnow);
    }

    #[test]
    fn only_vegetated_biomes_grow_trees() {
        assert_eq!(Biome::Desert.tree_density(), 0.0);
        assert_eq!(Biome::RockyPeaks.tree_density(), 0.0);
        assert!(Biome::Forest.tree_density() > 0.5);
    }
}
