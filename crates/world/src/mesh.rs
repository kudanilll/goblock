//! Chunk meshing: turns a chunk's blocks into a triangle list for the GPU.
//!
//! Faces that touch a solid neighbour are left out. The code has no GPU
//! dependency, so tests can check its output directly.

use glam::Vec3;

use crate::block::Block;
use crate::chunk::CHUNK_SIZE;

/// One vertex as uploaded to the GPU: position, normal, colour.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeshVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub color: [f32; 3],
}

/// CPU-side triangle soup for one chunk.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Mesh {
    pub vertices: Vec<MeshVertex>,
}

impl Mesh {
    pub fn vertex_count(&self) -> usize {
        self.vertices.len()
    }

    pub fn is_empty(&self) -> bool {
        self.vertices.is_empty()
    }
}

/// A face direction with its four corners in counter-clockwise order as seen
/// from outside the block, and the colour used for that side.
struct Face {
    normal: Vec3,
    corners: [Vec3; 4],
    color: [f32; 3],
}

const fn v(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3::new(x, y, z)
}

/// Faces in the order top, bottom, front, back, left, right.
fn faces() -> [Face; 6] {
    use goblock_core::color::{DIRT_BOTTOM, DIRT_SIDE, GRASS_TOP};
    [
        Face {
            normal: v(0.0, 1.0, 0.0),
            corners: [
                v(0.0, 1.0, 1.0),
                v(1.0, 1.0, 1.0),
                v(1.0, 1.0, 0.0),
                v(0.0, 1.0, 0.0),
            ],
            color: GRASS_TOP,
        },
        Face {
            normal: v(0.0, -1.0, 0.0),
            corners: [
                v(0.0, 0.0, 0.0),
                v(1.0, 0.0, 0.0),
                v(1.0, 0.0, 1.0),
                v(0.0, 0.0, 1.0),
            ],
            color: DIRT_BOTTOM,
        },
        Face {
            normal: v(0.0, 0.0, 1.0),
            corners: [
                v(0.0, 0.0, 1.0),
                v(1.0, 0.0, 1.0),
                v(1.0, 1.0, 1.0),
                v(0.0, 1.0, 1.0),
            ],
            color: DIRT_SIDE,
        },
        Face {
            normal: v(0.0, 0.0, -1.0),
            corners: [
                v(1.0, 0.0, 0.0),
                v(0.0, 0.0, 0.0),
                v(0.0, 1.0, 0.0),
                v(1.0, 1.0, 0.0),
            ],
            color: DIRT_SIDE,
        },
        Face {
            normal: v(-1.0, 0.0, 0.0),
            corners: [
                v(0.0, 0.0, 0.0),
                v(0.0, 0.0, 1.0),
                v(0.0, 1.0, 1.0),
                v(0.0, 1.0, 0.0),
            ],
            color: DIRT_SIDE,
        },
        Face {
            normal: v(1.0, 0.0, 0.0),
            corners: [
                v(1.0, 0.0, 1.0),
                v(1.0, 0.0, 0.0),
                v(1.0, 1.0, 0.0),
                v(1.0, 1.0, 1.0),
            ],
            color: DIRT_SIDE,
        },
    ]
}

/// Offsets to the neighbouring cell for each face, in the same order.
const NEIGHBOURS: [(i32, i32, i32); 6] = [
    (0, 1, 0),
    (0, -1, 0),
    (0, 0, 1),
    (0, 0, -1),
    (-1, 0, 0),
    (1, 0, 0),
];

/// Build a mesh from the blocks of the chunk whose minimum corner is `origin`.
///
/// `solid(x, y, z)` reports whether a chunk-local cell holds a solid block.
/// A face is skipped when the cell in front of it is solid; the caller
/// decides how cells outside the chunk count.
pub fn build_mesh<F>(blocks: &[Block], origin: Vec3, solid: F) -> Mesh
where
    F: Fn(i32, i32, i32) -> bool,
{
    let face_list = faces();
    let mut vertices = Vec::with_capacity(blocks.len() * 6 * 6);

    for block in blocks {
        if !block.is_active() {
            continue;
        }

        // Vertices come from the block's own extent, so the mesh lines up
        // with the collision box in `Block::contains_point`.
        let centre = block.position();
        let min = centre - Vec3::splat(0.5);
        let cell = (centre - origin).floor();
        let (lx, ly, lz) = (cell.x as i32, cell.y as i32, cell.z as i32);

        for (face, (dx, dy, dz)) in face_list.iter().zip(NEIGHBOURS.iter()) {
            if solid(lx + dx, ly + dy, lz + dz) {
                continue;
            }

            let corner = |c: Vec3| MeshVertex {
                position: (min + c).to_array(),
                normal: face.normal.to_array(),
                color: face.color,
            };

            let [a, b, c, d] = face.corners;
            // Two triangles: a-b-c and a-c-d.
            vertices.extend_from_slice(&[corner(a), corner(b), corner(c)]);
            vertices.extend_from_slice(&[corner(a), corner(c), corner(d)]);
        }
    }

    Mesh { vertices }
}

/// Solidity lookup over a chunk grid indexed as
/// `x + z * CHUNK_SIZE + y * CHUNK_SIZE * CHUNK_SIZE`. Cells outside the grid
/// count as empty.
pub fn grid_lookup(grid: &[bool]) -> impl Fn(i32, i32, i32) -> bool + '_ {
    let size = CHUNK_SIZE as usize;
    move |x, y, z| {
        if x < 0 || y < 0 || z < 0 {
            return false;
        }
        let (x, y, z) = (x as usize, y as usize, z as usize);
        if x >= size || z >= size {
            return false;
        }
        grid.get(x + z * size + y * size * size)
            .copied()
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cube_blocks() -> Vec<Block> {
        vec![Block::new(Vec3::new(0.0, 0.0, 0.0))]
    }

    #[test]
    fn single_block_has_six_faces() {
        let mesh = build_mesh(&cube_blocks(), Vec3::ZERO, |_, _, _| false);
        // 6 faces * 2 triangles * 3 vertices
        assert_eq!(mesh.vertex_count(), 36);
    }

    #[test]
    fn buried_faces_are_skipped() {
        // Two blocks stacked: the shared top/bottom pair disappears when the
        // solidity lookup reports both cells as occupied.
        let blocks = vec![
            Block::new(Vec3::new(0.0, 0.0, 0.0)),
            Block::new(Vec3::new(0.0, 1.0, 0.0)),
        ];
        let mesh = build_mesh(&blocks, Vec3::ZERO, |x, y, z| {
            x == 0 && z == 0 && (y == 0 || y == 1)
        });
        // 12 faces minus the 2 shared ones
        assert_eq!(mesh.vertex_count(), 10 * 6);
    }

    #[test]
    fn fully_enclosed_block_has_no_geometry() {
        let blocks = vec![Block::new(Vec3::new(0.0, 0.0, 0.0))];
        let mesh = build_mesh(&blocks, Vec3::ZERO, |_, _, _| true);
        assert!(mesh.is_empty());
    }

    #[test]
    fn inactive_blocks_are_skipped() {
        let blocks = vec![Block::with_flags(Vec3::ZERO, false, true)];
        let mesh = build_mesh(&blocks, Vec3::ZERO, |_, _, _| false);
        assert!(mesh.is_empty());
    }

    #[test]
    fn vertices_are_placed_around_block_centre() {
        let mesh = build_mesh(&cube_blocks(), Vec3::ZERO, |_, _, _| false);
        for vert in &mesh.vertices {
            // Block centre (0,0,0) spans [-0.5, 0.5]; every corner is on that box.
            for axis in 0..3 {
                let c = vert.position[axis];
                assert!(
                    (c + 0.5).abs() < 1e-6 || (c - 0.5).abs() < 1e-6,
                    "axis {axis} = {c}"
                );
            }
        }
    }

    #[test]
    fn chunk_origin_offsets_geometry() {
        let blocks = vec![Block::new(Vec3::new(16.0, 0.0, 32.0))];
        let mesh = build_mesh(&blocks, Vec3::new(16.0, 0.0, 32.0), |_, _, _| false);
        // Same local shape as the origin case.
        assert_eq!(mesh.vertex_count(), 36);
        let min_y = mesh
            .vertices
            .iter()
            .map(|v| v.position[1])
            .fold(f32::INFINITY, f32::min);
        let max_y = mesh
            .vertices
            .iter()
            .map(|v| v.position[1])
            .fold(f32::NEG_INFINITY, f32::max);
        assert!((min_y + 0.5).abs() < 1e-6, "min_y {min_y}");
        assert!((max_y - 0.5).abs() < 1e-6, "max_y {max_y}");
    }

    #[test]
    fn normals_point_outward() {
        let mesh = build_mesh(&cube_blocks(), Vec3::ZERO, |_, _, _| false);
        for tri in mesh.vertices.chunks(3) {
            let a = Vec3::from(tri[0].position);
            let b = Vec3::from(tri[1].position);
            let c = Vec3::from(tri[2].position);
            let face_normal = (b - a).cross(c - a).normalize();
            let declared = Vec3::from(tri[0].normal);
            assert!(
                face_normal.dot(declared) > 0.99,
                "winding {} disagrees with normal {}",
                face_normal,
                declared
            );
        }
    }

    #[test]
    fn grid_lookup_matches_grid_contents() {
        let mut grid = vec![false; (CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE) as usize];
        grid[0] = true;
        let solid = grid_lookup(&grid);
        assert!(solid(0, 0, 0));
        assert!(!solid(1, 0, 0));
        assert!(!solid(-1, 0, 0));
        assert!(!solid(0, 0, CHUNK_SIZE));
    }
}
