//! Block types, their textures, and how each one is drawn.

use glam::IVec3;

/// One layer of the block texture array. The renderer loads layers in the
/// order of [`Texture::ALL`], and each name doubles as the PNG file name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Texture {
    Stone,
    Dirt,
    GrassTop,
    GrassSide,
    GrassSideSnowy,
    Sand,
    Sandstone,
    Gravel,
    Snow,
    Ice,
    LogSide,
    LogTop,
    Leaves,
    GrassTuft,
    GrassTuftShort,
    GrassTuftTall,
    GrassTuftSeeds,
}

impl Texture {
    pub const ALL: [Texture; 17] = [
        Texture::Stone,
        Texture::Dirt,
        Texture::GrassTop,
        Texture::GrassSide,
        Texture::GrassSideSnowy,
        Texture::Sand,
        Texture::Sandstone,
        Texture::Gravel,
        Texture::Snow,
        Texture::Ice,
        Texture::LogSide,
        Texture::LogTop,
        Texture::Leaves,
        Texture::GrassTuft,
        Texture::GrassTuftShort,
        Texture::GrassTuftTall,
        Texture::GrassTuftSeeds,
    ];

    /// The sprites a grass tuft can show. The mesher picks one per tuft from
    /// its position.
    pub const GRASS_TUFTS: [Texture; 4] = [
        Texture::GrassTuftShort,
        Texture::GrassTuft,
        Texture::GrassTuftTall,
        Texture::GrassTuftSeeds,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Texture::Stone => "stone",
            Texture::Dirt => "dirt",
            Texture::GrassTop => "grass_top",
            Texture::GrassSide => "grass_side",
            Texture::GrassSideSnowy => "grass_side_snowy",
            Texture::Sand => "sand",
            Texture::Sandstone => "sandstone",
            Texture::Gravel => "gravel",
            Texture::Snow => "snow",
            Texture::Ice => "ice",
            Texture::LogSide => "log_side",
            Texture::LogTop => "log_top",
            Texture::Leaves => "leaves",
            Texture::GrassTuft => "grass_tuft",
            Texture::GrassTuftShort => "grass_tuft_short",
            Texture::GrassTuftTall => "grass_tuft_tall",
            Texture::GrassTuftSeeds => "grass_tuft_seeds",
        }
    }

    /// sRGB colour and alpha of the placeholder tile used while this layer's
    /// PNG is missing.
    pub fn placeholder_color(self) -> [u8; 4] {
        match self {
            Texture::Stone => [125, 125, 128, 255],
            Texture::Dirt => [121, 85, 58, 255],
            Texture::GrassTop => [104, 160, 72, 255],
            Texture::GrassSide => [112, 116, 64, 255],
            Texture::GrassSideSnowy => [176, 168, 160, 255],
            Texture::Sand => [218, 205, 150, 255],
            Texture::Sandstone => [205, 186, 132, 255],
            Texture::Gravel => [134, 128, 124, 255],
            Texture::Snow => [240, 246, 250, 255],
            Texture::Ice => [158, 194, 240, 255],
            Texture::LogSide => [104, 80, 50, 255],
            Texture::LogTop => [160, 128, 84, 255],
            Texture::Leaves => [60, 120, 48, 230],
            // A missing grass sprite stays invisible instead of a solid card.
            Texture::GrassTuft
            | Texture::GrassTuftShort
            | Texture::GrassTuftTall
            | Texture::GrassTuftSeeds => [118, 158, 72, 0],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Face {
    Up,
    Down,
    /// Facing -Z.
    North,
    /// Facing +Z.
    South,
    /// Facing -X.
    West,
    /// Facing +X.
    East,
}

impl Face {
    pub const ALL: [Face; 6] = [
        Face::Up,
        Face::Down,
        Face::North,
        Face::South,
        Face::West,
        Face::East,
    ];

    pub fn normal(self) -> IVec3 {
        match self {
            Face::Up => IVec3::Y,
            Face::Down => IVec3::NEG_Y,
            Face::North => IVec3::NEG_Z,
            Face::South => IVec3::Z,
            Face::West => IVec3::NEG_X,
            Face::East => IVec3::X,
        }
    }
}

/// Which block pipeline draws a block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderLayer {
    Opaque,
    /// Opaque pixels only; transparent pixels are discarded.
    Cutout,
    /// Alpha blended, drawn after everything else.
    Translucent,
}

/// Which biome colour a face is multiplied by.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tint {
    None,
    Foliage,
    Water,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum BlockId {
    #[default]
    Air,
    Stone,
    Dirt,
    Grass,
    SnowyGrass,
    Sand,
    Sandstone,
    Gravel,
    Snow,
    Ice,
    Water,
    Log,
    Leaves,
    GrassTuft,
}

impl BlockId {
    /// Whether the camera collides with the block.
    pub fn is_solid(self) -> bool {
        !matches!(self, BlockId::Air | BlockId::Water | BlockId::GrassTuft)
    }

    /// Whether the block fills its whole cell, hiding the faces of the blocks
    /// next to it.
    pub fn is_opaque(self) -> bool {
        !matches!(
            self,
            BlockId::Air | BlockId::Water | BlockId::Leaves | BlockId::GrassTuft
        )
    }

    pub fn render_layer(self) -> Option<RenderLayer> {
        match self {
            BlockId::Air => None,
            BlockId::Water => Some(RenderLayer::Translucent),
            BlockId::Leaves | BlockId::GrassTuft => Some(RenderLayer::Cutout),
            _ => Some(RenderLayer::Opaque),
        }
    }

    pub fn texture(self, face: Face) -> Texture {
        let top = face == Face::Up;
        let end = matches!(face, Face::Up | Face::Down);
        match self {
            BlockId::Air | BlockId::Stone => Texture::Stone,
            BlockId::Dirt => Texture::Dirt,
            BlockId::Grass if top => Texture::GrassTop,
            BlockId::SnowyGrass if top => Texture::Snow,
            BlockId::Grass | BlockId::SnowyGrass if face == Face::Down => Texture::Dirt,
            BlockId::Grass => Texture::GrassSide,
            BlockId::SnowyGrass => Texture::GrassSideSnowy,
            BlockId::Sand => Texture::Sand,
            BlockId::Sandstone => Texture::Sandstone,
            BlockId::Gravel => Texture::Gravel,
            BlockId::Snow => Texture::Snow,
            BlockId::Ice => Texture::Ice,
            // Water is shaded entirely in the shader; its layer is never read.
            BlockId::Water => Texture::Stone,
            BlockId::Log if end => Texture::LogTop,
            BlockId::Log => Texture::LogSide,
            BlockId::Leaves => Texture::Leaves,
            BlockId::GrassTuft => Texture::GrassTuft,
        }
    }

    pub fn tint(self, face: Face) -> Tint {
        match self {
            BlockId::Grass if face != Face::Down => Tint::Foliage,
            BlockId::Leaves | BlockId::GrassTuft => Tint::Foliage,
            BlockId::Water => Tint::Water,
            _ => Tint::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn texture_names_are_unique() {
        let mut names: Vec<_> = Texture::ALL.iter().map(|t| t.name()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), Texture::ALL.len());
    }

    #[test]
    fn texture_order_matches_layer_index() {
        for (index, texture) in Texture::ALL.iter().enumerate() {
            assert_eq!(*texture as usize, index);
        }
    }

    #[test]
    fn grass_uses_three_textures() {
        assert_eq!(BlockId::Grass.texture(Face::Up), Texture::GrassTop);
        assert_eq!(BlockId::Grass.texture(Face::Down), Texture::Dirt);
        assert_eq!(BlockId::Grass.texture(Face::East), Texture::GrassSide);
    }

    #[test]
    fn water_and_air_are_not_solid() {
        assert!(!BlockId::Air.is_solid());
        assert!(!BlockId::Water.is_solid());
        assert!(BlockId::Leaves.is_solid());
    }

    #[test]
    fn see_through_blocks_are_not_opaque() {
        for block in [BlockId::Air, BlockId::Water, BlockId::Leaves] {
            assert!(!block.is_opaque(), "{block:?}");
        }
        assert!(BlockId::Stone.is_opaque());
    }

    #[test]
    fn grass_tufts_are_walk_through_cutouts() {
        let grass = BlockId::GrassTuft;
        assert!(!grass.is_solid());
        assert!(!grass.is_opaque());
        assert_eq!(grass.render_layer(), Some(RenderLayer::Cutout));
        assert_eq!(grass.texture(Face::North), Texture::GrassTuft);
        assert_eq!(grass.tint(Face::North), Tint::Foliage);
    }

    #[test]
    fn face_normals_are_unit_axes() {
        for face in Face::ALL {
            assert_eq!(face.normal().abs().element_sum(), 1);
        }
    }
}
