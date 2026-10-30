//! Chunk meshing: one quad per visible block face, with ambient occlusion,
//! face shading and biome tint baked into the vertices.

use glam::IVec3;

use crate::biome::Biome;
use crate::block::{BlockId, Face, Texture, Tint};
use crate::chunk::{Chunk, CHUNK_SIZE};
use crate::terrain::hash;

/// One corner of a block face, in the layout the GPU reads.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ChunkVertex {
    /// Position inside the chunk in sixteenths of a block. `w` is the face,
    /// an index into [`Face::ALL`], or 6 and 7 for crossed grass planes.
    pub position: [u16; 4],
    /// Texture u and v (0 or 1), texture array layer, and ambient occlusion
    /// (255 for fully open).
    pub uv_layer_shade: [u8; 4],
    /// Biome tint in sRGB. Alpha is wind weight: 0 fixed, 64 leaves, 255 grass tips.
    pub tint: [u8; 4],
}

/// Geometry for one chunk, grouped by render layer.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ChunkMesh {
    /// Four vertices per quad: the opaque quads, then cutout, then translucent.
    pub vertices: Vec<ChunkVertex>,
    pub opaque_quads: u32,
    pub cutout_quads: u32,
    pub translucent_quads: u32,
    /// Lowest and highest y the geometry covers, for culling.
    pub min_y: i32,
    pub max_y: i32,
}

impl ChunkMesh {
    pub fn quad_count(&self) -> u32 {
        self.opaque_quads + self.cutout_quads + self.translucent_quads
    }

    pub fn is_empty(&self) -> bool {
        self.vertices.is_empty()
    }
}

/// Ambient occlusion for 0 to 3 unblocked directions around a vertex.
const AO_LEVELS: [u8; 4] = [115, 166, 209, 255];
/// Texture coordinates for the four corners returned by [`corners`].
const UVS: [[u8; 2]; 4] = [[0, 1], [1, 1], [1, 0], [0, 0]];
/// Height of a water surface, in sixteenths of a block.
pub(crate) const WATER_SURFACE: i32 = 14;

/// Corners of a face in counter-clockwise order seen from outside, starting
/// at the bottom-left of the texture.
fn corners(face: Face) -> [[i32; 3]; 4] {
    match face {
        Face::Up => [[0, 1, 1], [1, 1, 1], [1, 1, 0], [0, 1, 0]],
        Face::Down => [[0, 0, 0], [1, 0, 0], [1, 0, 1], [0, 0, 1]],
        Face::North => [[1, 0, 0], [0, 0, 0], [0, 1, 0], [1, 1, 0]],
        Face::South => [[0, 0, 1], [1, 0, 1], [1, 1, 1], [0, 1, 1]],
        Face::West => [[0, 0, 0], [0, 0, 1], [0, 1, 1], [0, 1, 0]],
        Face::East => [[1, 0, 1], [1, 0, 0], [1, 1, 0], [1, 1, 1]],
    }
}

/// A chunk and its eight neighbours, so faces and occlusion along the
/// border see the blocks next door.
struct Area<'a> {
    /// Row by row from -z, each row from -x. The chunk being meshed is index 4.
    chunks: [&'a Chunk; 9],
}

impl Area<'_> {
    /// Block at coordinates local to the centre chunk; x and z may be -1 or 16.
    fn get(&self, x: i32, y: i32, z: i32) -> BlockId {
        let column = |v: i32| (v + CHUNK_SIZE).div_euclid(CHUNK_SIZE) as usize;
        let chunk = self.chunks[column(z) * 3 + column(x)];
        chunk.get(x.rem_euclid(CHUNK_SIZE), y, z.rem_euclid(CHUNK_SIZE))
    }

    fn occludes(&self, p: IVec3) -> bool {
        self.get(p.x, p.y, p.z).is_opaque()
    }
}

/// Mesh `chunks[4]`, using the other eight chunks (row by row from -z, each
/// row from -x) for the faces and occlusion along its border.
pub fn build_mesh(chunks: [&Chunk; 9]) -> ChunkMesh {
    let area = Area { chunks };
    let centre = chunks[4];
    let mut layers: [Vec<ChunkVertex>; 3] = Default::default();
    let (mut min_y, mut max_y) = (i32::MAX, i32::MIN);

    for section in centre.filled_sections() {
        for y in section {
            for z in 0..CHUNK_SIZE {
                for x in 0..CHUNK_SIZE {
                    let block = centre.get(x, y, z);
                    let Some(layer) = block.render_layer() else {
                        continue;
                    };
                    if block == BlockId::GrassTuft {
                        emit_grass(centre, IVec3::new(x, y, z), &mut layers[layer as usize]);
                        min_y = min_y.min(y);
                        max_y = max_y.max(y + 1);
                        continue;
                    }
                    let water_surface =
                        block == BlockId::Water && area.get(x, y + 1, z) != BlockId::Water;

                    for face in Face::ALL {
                        let n = face.normal();
                        let neighbour = area.get(x + n.x, y + n.y, z + n.z);
                        if neighbour == block || neighbour.is_opaque() {
                            continue;
                        }
                        let quad = Quad {
                            block,
                            face,
                            pos: IVec3::new(x, y, z),
                            water_surface,
                            biome: centre.biome(x, z),
                        };
                        quad.emit(&area, &mut layers[layer as usize]);
                        min_y = min_y.min(y);
                        max_y = max_y.max(y + 1);
                    }
                }
            }
        }
    }

    let [opaque, cutout, translucent] = layers;
    let quads = |v: &Vec<ChunkVertex>| (v.len() / 4) as u32;
    let mut mesh = ChunkMesh {
        opaque_quads: quads(&opaque),
        cutout_quads: quads(&cutout),
        translucent_quads: quads(&translucent),
        vertices: opaque,
        min_y: if min_y == i32::MAX { 0 } else { min_y },
        max_y: if max_y == i32::MIN { 0 } else { max_y },
    };
    mesh.vertices.extend(cutout);
    mesh.vertices.extend(translucent);
    mesh
}

fn emit_grass(chunk: &Chunk, pos: IVec3, out: &mut Vec<ChunkVertex>) {
    let origin = chunk.origin();
    let roll = hash(0, origin.x + pos.x, origin.z + pos.z);
    let jitter_x = (roll % 5) as i32 - 2;
    let jitter_z = ((roll >> 4) % 5) as i32 - 2;
    let sprite = Texture::GRASS_TUFTS[(roll >> 8) as usize % Texture::GRASS_TUFTS.len()];
    let [r, g, b] = chunk.biome(pos.x, pos.z).foliage_tint();
    for (plane, [[ax, az], [bx, bz]]) in [[[2, 2], [14, 14]], [[14, 2], [2, 14]]]
        .into_iter()
        .enumerate()
    {
        // Each plane shows the sprite mirrored half the time.
        let mirror = ((roll >> (12 + plane)) & 1) as u8;
        // Every quad is a full block tall, so a sprite texel is as big as a
        // block texel. Short sprites are short in the image instead.
        for (i, [x, y, z]) in [[ax, 0, az], [bx, 0, bz], [bx, 16, bz], [ax, 16, az]]
            .into_iter()
            .enumerate()
        {
            out.push(ChunkVertex {
                position: [
                    (pos.x * 16 + x + jitter_x) as u16,
                    (pos.y * 16 + y) as u16,
                    (pos.z * 16 + z + jitter_z) as u16,
                    6 + plane as u16,
                ],
                uv_layer_shade: [UVS[i][0] ^ mirror, UVS[i][1], sprite as u8, 255],
                tint: [r, g, b, if y == 0 { 0 } else { 255 }],
            });
        }
    }
}

struct Quad {
    block: BlockId,
    face: Face,
    pos: IVec3,
    water_surface: bool,
    biome: Biome,
}

impl Quad {
    fn emit(&self, area: &Area, out: &mut Vec<ChunkVertex>) {
        let corners = corners(self.face);
        let layer = self.block.texture(self.face) as u8;
        let face = self.face as u16;
        let [r, g, b] = match self.block.tint(self.face) {
            Tint::None => [255; 3],
            Tint::Foliage => self.biome.foliage_tint(),
            Tint::Water => self.biome.water_tint(),
        };

        // Water is see-through, so it neither casts nor shows occlusion.
        let ao: [usize; 4] = if self.block == BlockId::Water {
            [3; 4]
        } else {
            std::array::from_fn(|i| self.corner_occlusion(area, corners[i]))
        };

        // Split the quad along the diagonal with the brighter ends, so the
        // occlusion gradient does not crease across the face.
        let order = if ao[0] + ao[2] < ao[1] + ao[3] {
            [1, 2, 3, 0]
        } else {
            [0, 1, 2, 3]
        };

        for i in order {
            let [cx, cy, cz] = corners[i];
            let top = if self.water_surface && cy == 1 {
                WATER_SURFACE
            } else {
                cy * 16
            };
            out.push(ChunkVertex {
                position: [
                    ((self.pos.x + cx) * 16) as u16,
                    (self.pos.y * 16 + top) as u16,
                    ((self.pos.z + cz) * 16) as u16,
                    face,
                ],
                uv_layer_shade: [UVS[i][0], UVS[i][1], layer, AO_LEVELS[ao[i]]],
                tint: [r, g, b, if self.block == BlockId::Leaves { 64 } else { 0 }],
            });
        }
    }

    /// Number of unblocked directions (0 to 3) at a corner: the two blocks
    /// beside it in the plane in front of the face, and the one diagonal.
    fn corner_occlusion(&self, area: &Area, corner: [i32; 3]) -> usize {
        let normal = self.face.normal();
        let front = self.pos + normal;
        let mut steps = [IVec3::ZERO; 2];
        let mut next = 0;
        for axis in 0..3 {
            if normal[axis] == 0 {
                steps[next][axis] = if corner[axis] == 1 { 1 } else { -1 };
                next += 1;
            }
        }

        let side_a = area.occludes(front + steps[0]);
        let side_b = area.occludes(front + steps[1]);
        let diagonal = area.occludes(front + steps[0] + steps[1]);
        if side_a && side_b {
            0
        } else {
            3 - (side_a as usize + side_b as usize + diagonal as usize)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::{IVec2, Vec3};

    /// Mesh `centre` with eight empty neighbours.
    fn mesh_alone(centre: &Chunk) -> ChunkMesh {
        let empty = Chunk::new(IVec2::new(100, 100));
        let mut chunks = [&empty; 9];
        chunks[4] = centre;
        build_mesh(chunks)
    }

    fn quad_positions(mesh: &ChunkMesh, quad: usize) -> [Vec3; 4] {
        std::array::from_fn(|i| {
            let p = mesh.vertices[quad * 4 + i].position;
            Vec3::new(p[0] as f32, p[1] as f32, p[2] as f32) / 16.0
        })
    }

    #[test]
    fn vertex_is_sixteen_bytes() {
        assert_eq!(std::mem::size_of::<ChunkVertex>(), 16);
    }

    #[test]
    fn single_block_has_six_quads() {
        let mut chunk = Chunk::new(IVec2::ZERO);
        chunk.set(5, 70, 5, BlockId::Stone);
        let mesh = mesh_alone(&chunk);
        assert_eq!(mesh.opaque_quads, 6);
        assert_eq!(mesh.vertices.len(), 24);
        assert_eq!((mesh.min_y, mesh.max_y), (70, 71));
    }

    #[test]
    fn faces_between_blocks_are_dropped() {
        let mut chunk = Chunk::new(IVec2::ZERO);
        chunk.set(5, 70, 5, BlockId::Stone);
        chunk.set(5, 71, 5, BlockId::Dirt);
        assert_eq!(mesh_alone(&chunk).quad_count(), 10);
    }

    #[test]
    fn neighbouring_chunks_hide_border_faces() {
        let mut centre = Chunk::new(IVec2::ZERO);
        centre.set(15, 70, 3, BlockId::Stone);
        let mut east = Chunk::new(IVec2::new(1, 0));
        east.set(0, 70, 3, BlockId::Stone);
        let empty = Chunk::new(IVec2::new(9, 9));
        let mut chunks = [&empty; 9];
        chunks[4] = &centre;
        chunks[5] = &east;
        assert_eq!(build_mesh(chunks).quad_count(), 5);
    }

    #[test]
    fn vertices_name_their_face() {
        let mut chunk = Chunk::new(IVec2::ZERO);
        chunk.set(5, 70, 5, BlockId::Stone);
        let mesh = mesh_alone(&chunk);
        let centre = Vec3::new(5.5, 70.5, 5.5);
        for quad in 0..6 {
            let [a, _, c, _] = quad_positions(&mesh, quad);
            let face = Face::ALL[mesh.vertices[quad * 4].position[3] as usize];
            let outward = ((a + c) * 0.5 - centre) * 2.0;
            assert_eq!(outward, face.normal().as_vec3(), "quad {quad}");
        }
    }

    #[test]
    fn quads_wind_counter_clockwise_from_outside() {
        let mut chunk = Chunk::new(IVec2::ZERO);
        chunk.set(5, 70, 5, BlockId::Stone);
        let mesh = mesh_alone(&chunk);
        let centre = Vec3::new(5.5, 70.5, 5.5);
        for quad in 0..6 {
            let [a, b, c, _] = quad_positions(&mesh, quad);
            let normal = (b - a).cross(c - a).normalize();
            let outward = ((a + c) * 0.5 - centre).normalize();
            assert!(normal.dot(outward) > 0.99, "quad {quad} faces inward");
        }
    }

    #[test]
    fn corners_next_to_walls_are_darker() {
        let mut chunk = Chunk::new(IVec2::ZERO);
        for z in 0..CHUNK_SIZE {
            for x in 0..CHUNK_SIZE {
                chunk.set(x, 70, z, BlockId::Stone);
            }
        }
        chunk.set(8, 71, 8, BlockId::Stone);
        let mesh = mesh_alone(&chunk);

        // Top face of the floor block just east of the pillar.
        let top = (0..mesh.quad_count() as usize)
            .find(|&q| {
                let p = quad_positions(&mesh, q);
                p.iter().all(|v| v.y == 71.0)
                    && p.iter().any(|v| v.x == 9.0 && v.z == 8.0)
                    && p.iter().all(|v| v.x >= 9.0 && v.z >= 8.0 && v.z <= 9.0)
            })
            .expect("floor quad east of the pillar");
        let shades: Vec<u8> = (0..4)
            .map(|i| mesh.vertices[top * 4 + i].uv_layer_shade[3])
            .collect();
        assert!(shades.iter().min() < shades.iter().max(), "{shades:?}");
    }

    #[test]
    fn water_surface_sits_below_the_block_top() {
        let mut chunk = Chunk::new(IVec2::ZERO);
        chunk.set(2, 64, 2, BlockId::Water);
        let mesh = mesh_alone(&chunk);
        assert_eq!(mesh.translucent_quads, 6);
        let highest = mesh.vertices.iter().map(|v| v.position[1]).max();
        assert_eq!(highest, Some((64 * 16 + WATER_SURFACE) as u16));
    }

    #[test]
    fn water_under_water_keeps_full_height_and_no_shared_face() {
        let mut chunk = Chunk::new(IVec2::ZERO);
        chunk.set(2, 63, 2, BlockId::Water);
        chunk.set(2, 64, 2, BlockId::Water);
        let mesh = mesh_alone(&chunk);
        assert_eq!(mesh.translucent_quads, 10);
    }

    #[test]
    fn layers_are_ordered_opaque_cutout_translucent() {
        let mut chunk = Chunk::new(IVec2::ZERO);
        chunk.set(1, 70, 1, BlockId::Water);
        chunk.set(5, 70, 5, BlockId::Leaves);
        chunk.set(9, 70, 9, BlockId::Stone);
        let mesh = mesh_alone(&chunk);
        let layer_of = |quad: u32| mesh.vertices[quad as usize * 4].uv_layer_shade[2];
        assert_eq!(layer_of(0), Texture::Stone as u8);
        assert_eq!(layer_of(mesh.opaque_quads), Texture::Leaves as u8);
        // Water has no texture, so it is told apart by its tint.
        let first_water = (mesh.opaque_quads + mesh.cutout_quads) as usize * 4;
        assert_eq!(
            mesh.vertices[first_water].tint[..3],
            chunk.biome(1, 1).water_tint()
        );
    }

    #[test]
    fn grass_takes_the_biome_tint() {
        let mut chunk = Chunk::new(IVec2::ZERO);
        chunk.set(3, 70, 3, BlockId::Grass);
        chunk.set_biome(3, 3, Biome::Savanna);
        let mesh = mesh_alone(&chunk);
        let tint = Biome::Savanna.foliage_tint();
        assert!(mesh.vertices.iter().any(|v| v.tint[..3] == tint));
    }

    #[test]
    fn wind_weights_only_mark_tree_leaves() {
        for block in [
            BlockId::Stone,
            BlockId::Grass,
            BlockId::Log,
            BlockId::Leaves,
            BlockId::Water,
        ] {
            let mut chunk = Chunk::new(IVec2::ZERO);
            chunk.set(5, 70, 5, block);
            let mesh = mesh_alone(&chunk);
            for vertex in mesh.vertices {
                if block == BlockId::Leaves {
                    assert!((1..128).contains(&vertex.tint[3]));
                } else {
                    assert_eq!(vertex.tint[3], 0, "{block:?} must stay still");
                }
            }
        }
    }

    #[test]
    fn grass_tufts_have_two_crossed_quads_with_fixed_roots() {
        let mut chunk = Chunk::new(IVec2::new(-1, 0));
        chunk.set(15, 70, 0, BlockId::GrassTuft);
        let mesh = mesh_alone(&chunk);
        assert_eq!(
            (mesh.opaque_quads, mesh.cutout_quads, mesh.translucent_quads),
            (0, 2, 0)
        );
        assert_eq!(mesh.vertices.len(), 8);
        assert_eq!(mesh.vertices.iter().filter(|v| v.tint[3] == 0).count(), 4);
        assert_eq!(mesh.vertices.iter().filter(|v| v.tint[3] == 255).count(), 4);
        for vertex in mesh.vertices {
            assert!(vertex.position[0] <= 256 && vertex.position[2] <= 256);
            assert_eq!(vertex.tint[..3], Biome::Plains.foliage_tint());
            if vertex.tint[3] == 0 {
                assert_eq!(vertex.position[1], 70 * 16);
            } else {
                assert_eq!(vertex.position[1], 71 * 16);
            }
        }
    }

    #[test]
    fn grass_tufts_vary_their_sprite_and_mirror_it() {
        let mut chunk = Chunk::new(IVec2::ZERO);
        for x in 0..CHUNK_SIZE {
            chunk.set(x, 70, 0, BlockId::GrassTuft);
        }
        let mesh = mesh_alone(&chunk);
        let mut layers: Vec<u8> = mesh.vertices.iter().map(|v| v.uv_layer_shade[2]).collect();
        layers.sort_unstable();
        layers.dedup();
        assert!(layers.len() > 1, "a row of tufts all used one sprite");
        assert!(layers
            .iter()
            .all(|&layer| Texture::GRASS_TUFTS.iter().any(|&t| t as u8 == layer)));
        // The first corner of a quad has u 0, or 1 when the quad is mirrored.
        let first_u: Vec<u8> = mesh
            .vertices
            .chunks(4)
            .map(|quad| quad[0].uv_layer_shade[0])
            .collect();
        assert!(first_u.contains(&0) && first_u.contains(&1));
    }

    #[test]
    fn grass_tufts_do_not_hide_the_ground_below_them() {
        let mut chunk = Chunk::new(IVec2::ZERO);
        chunk.set(5, 70, 5, BlockId::Grass);
        chunk.set(5, 71, 5, BlockId::GrassTuft);
        let mesh = mesh_alone(&chunk);
        assert_eq!(mesh.opaque_quads, 6);
        assert_eq!(mesh.cutout_quads, 2);
    }

    #[test]
    fn empty_chunk_has_no_mesh() {
        let mesh = mesh_alone(&Chunk::new(IVec2::ZERO));
        assert!(mesh.is_empty());
        assert_eq!((mesh.min_y, mesh.max_y), (0, 0));
    }
}
