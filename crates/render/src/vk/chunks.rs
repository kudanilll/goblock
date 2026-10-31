//! Chunk meshes on the GPU.
//!
//! Each chunk gets its own device-local vertex buffer, filled by a staging
//! copy recorded at the start of the next frame. One shared index buffer
//! turns every four vertices into two triangles. Buffers the GPU may still
//! be reading are kept until the frame that last used them has finished.

use std::collections::HashMap;

use ash::vk;
use glam::{IVec2, Mat4, Vec3};
use goblock_core::frustum::Frustum;
use goblock_world::{ChunkMesh, ChunkVertex, CHUNK_SIZE};

use super::device::{Device, MAX_FRAMES_IN_FLIGHT};
use super::error::VkError;
use super::resources::{DynamicBuffer, GpuBuffer};

/// Render layers in draw order, matching the quad order in [`ChunkMesh`].
pub const LAYERS: usize = 3;

/// Byte size of one chunk vertex, which is the vertex binding stride.
pub const VERTEX_STRIDE: u32 = std::mem::size_of::<ChunkVertex>() as u32;
/// Includes wind and player bending beyond the static mesh bounds.
const FOLIAGE_MARGIN: f32 = 0.75;

struct GpuChunk {
    vertices: GpuBuffer,
    /// Quads per render layer.
    quads: [u32; LAYERS],
    min_y: i32,
    max_y: i32,
}

enum Retired {
    Buffer(GpuBuffer),
    Staging(DynamicBuffer),
}

struct PendingCopy {
    staging: DynamicBuffer,
    target: vk::Buffer,
    size: u64,
}

/// A chunk that passed frustum culling this frame.
pub struct VisibleChunk {
    pos: IVec2,
    /// Chunk origin relative to the camera.
    offset: Vec3,
    distance: f32,
}

#[derive(Default)]
pub struct ChunkBuffers {
    chunks: HashMap<IVec2, GpuChunk>,
    indices: Option<GpuBuffer>,
    index_quads: u32,
    pending: Vec<PendingCopy>,
    /// `retired[frame]` is freed the next time that frame slot's fence is
    /// waited on, when the GPU can no longer be using it.
    retired: [Vec<Retired>; MAX_FRAMES_IN_FLIGHT],
    /// Retired since the current frame started.
    released: Vec<Retired>,
}

impl ChunkBuffers {
    /// Chunks with geometry on the GPU.
    pub fn count(&self) -> usize {
        self.chunks.len()
    }

    /// Replace the mesh at `pos`. An empty mesh just removes the old one.
    pub fn upload(&mut self, device: &Device, pos: IVec2, mesh: &ChunkMesh) -> Result<(), VkError> {
        self.remove(pos);
        if mesh.is_empty() {
            return Ok(());
        }
        self.reserve_indices(device, mesh.quad_count())?;

        let size = std::mem::size_of_val(mesh.vertices.as_slice()) as u64;
        let mut vertices = GpuBuffer::new(device, size, vk::BufferUsageFlags::VERTEX_BUFFER)?;
        let mut staging = match DynamicBuffer::new(device, size, vk::BufferUsageFlags::TRANSFER_SRC)
        {
            Ok(staging) => staging,
            Err(e) => {
                // Never handed to the GPU, so it can go right away.
                unsafe { vertices.destroy(device) };
                return Err(e);
            }
        };
        staging.write(&mesh.vertices);
        self.pending.push(PendingCopy {
            staging,
            target: vertices.buffer,
            size,
        });

        self.chunks.insert(
            pos,
            GpuChunk {
                vertices,
                quads: [mesh.opaque_quads, mesh.cutout_quads, mesh.translucent_quads],
                min_y: mesh.min_y,
                max_y: mesh.max_y,
            },
        );
        Ok(())
    }

    pub fn remove(&mut self, pos: IVec2) {
        if let Some(chunk) = self.chunks.remove(&pos) {
            self.released.push(Retired::Buffer(chunk.vertices));
        }
    }

    /// Free every meshed chunk buffer.
    pub fn clear(&mut self) {
        for (_, chunk) in self.chunks.drain() {
            self.released.push(Retired::Buffer(chunk.vertices));
        }
    }

    /// Make the shared index buffer hold at least `quads` quads.
    fn reserve_indices(&mut self, device: &Device, quads: u32) -> Result<(), VkError> {
        if quads <= self.index_quads {
            return Ok(());
        }
        let capacity = quads.next_power_of_two().max(1 << 14);
        let indices: Vec<u32> = (0..capacity)
            .flat_map(|quad| {
                let v = quad * 4;
                [v, v + 1, v + 2, v, v + 2, v + 3]
            })
            .collect();
        let size = std::mem::size_of_val(indices.as_slice()) as u64;

        let buffer = GpuBuffer::new(device, size, vk::BufferUsageFlags::INDEX_BUFFER)?;
        let mut staging = DynamicBuffer::new(device, size, vk::BufferUsageFlags::TRANSFER_SRC)?;
        staging.write(&indices);
        self.pending.push(PendingCopy {
            staging,
            target: buffer.buffer,
            size,
        });
        if let Some(old) = self.indices.replace(buffer) {
            self.released.push(Retired::Buffer(old));
        }
        self.index_quads = capacity;
        Ok(())
    }

    /// Free whatever frame slot `frame` used last time round. Call right
    /// after waiting on that slot's fence.
    ///
    /// # Safety
    /// The fence for `frame` must have signalled.
    pub unsafe fn begin_frame(&mut self, device: &Device, frame: usize) {
        for item in self.retired[frame].drain(..) {
            destroy(device, item);
        }
        self.retired[frame].append(&mut self.released);
    }

    /// Record the pending uploads into `cmd`. Must come before the render
    /// pass that draws them.
    ///
    /// # Safety
    /// `cmd` must be recording, outside a render pass.
    pub unsafe fn record_uploads(&mut self, device: &Device, cmd: vk::CommandBuffer, frame: usize) {
        if self.pending.is_empty() {
            return;
        }
        for copy in self.pending.drain(..) {
            let region = vk::BufferCopy {
                src_offset: 0,
                dst_offset: 0,
                size: copy.size,
            };
            device
                .device
                .cmd_copy_buffer(cmd, copy.staging.buffer, copy.target, &[region]);
            self.retired[frame].push(Retired::Staging(copy.staging));
        }
        let barrier = vk::MemoryBarrier::default()
            .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
            .dst_access_mask(vk::AccessFlags::VERTEX_ATTRIBUTE_READ | vk::AccessFlags::INDEX_READ);
        device.device.cmd_pipeline_barrier(
            cmd,
            vk::PipelineStageFlags::TRANSFER,
            vk::PipelineStageFlags::VERTEX_INPUT,
            vk::DependencyFlags::empty(),
            &[barrier],
            &[],
            &[],
        );
    }

    /// Chunks inside the view frustum, nearest first. `view_projection` puts
    /// the camera at the origin.
    pub fn visible(&self, camera: Vec3, view_projection: &Mat4) -> Vec<VisibleChunk> {
        let frustum = Frustum::from_view_projection(view_projection);
        let size = CHUNK_SIZE as f32;
        let mut visible: Vec<VisibleChunk> = self
            .chunks
            .iter()
            .filter_map(|(&pos, chunk)| {
                let offset = Vec3::new(pos.x as f32 * size, 0.0, pos.y as f32 * size) - camera;
                let min = offset + Vec3::new(-FOLIAGE_MARGIN, chunk.min_y as f32, -FOLIAGE_MARGIN);
                let max = offset
                    + Vec3::new(
                        size + FOLIAGE_MARGIN,
                        chunk.max_y as f32,
                        size + FOLIAGE_MARGIN,
                    );
                frustum.intersects_box(min, max).then(|| VisibleChunk {
                    pos,
                    offset,
                    distance: ((min + max) * 0.5).length_squared(),
                })
            })
            .collect();
        visible.sort_by(|a, b| a.distance.total_cmp(&b.distance));
        visible
    }

    /// Bind the shared index buffer. Returns false when nothing has been
    /// uploaded yet.
    ///
    /// # Safety
    /// `cmd` must be recording.
    pub unsafe fn bind_indices(&self, device: &Device, cmd: vk::CommandBuffer) -> bool {
        match &self.indices {
            Some(indices) => {
                device
                    .device
                    .cmd_bind_index_buffer(cmd, indices.buffer, 0, vk::IndexType::UINT32);
                true
            }
            None => false,
        }
    }

    /// Draw `layer` of each chunk in `visible`, in the given order. Each
    /// chunk's offset from the camera is the pipeline's only push constant.
    ///
    /// # Safety
    /// `cmd` must be inside a render pass with a chunk pipeline and the
    /// index buffer bound.
    pub unsafe fn draw<'a>(
        &self,
        device: &Device,
        cmd: vk::CommandBuffer,
        layout: vk::PipelineLayout,
        layer: usize,
        visible: impl Iterator<Item = &'a VisibleChunk>,
    ) {
        for chunk in visible {
            let Some(gpu) = self.chunks.get(&chunk.pos) else {
                continue;
            };
            let count = gpu.quads[layer];
            if count == 0 {
                continue;
            }
            let first: u32 = gpu.quads[..layer].iter().sum();
            let offset = chunk.offset.extend(0.0).to_array();
            device.device.cmd_push_constants(
                cmd,
                layout,
                vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                0,
                bytemuck::cast_slice(&offset),
            );
            device
                .device
                .cmd_bind_vertex_buffers(cmd, 0, &[gpu.vertices.buffer], &[0]);
            device
                .device
                .cmd_draw_indexed(cmd, count * 6, 1, 0, (first * 4) as i32, 0);
        }
    }

    /// # Safety
    /// The device must be idle.
    pub unsafe fn destroy(&mut self, device: &Device) {
        for (_, chunk) in self.chunks.drain() {
            destroy(device, Retired::Buffer(chunk.vertices));
        }
        if let Some(indices) = self.indices.take() {
            destroy(device, Retired::Buffer(indices));
        }
        for copy in self.pending.drain(..) {
            destroy(device, Retired::Staging(copy.staging));
        }
        for list in &mut self.retired {
            for item in list.drain(..) {
                destroy(device, item);
            }
        }
        for item in self.released.drain(..) {
            destroy(device, item);
        }
    }
}

unsafe fn destroy(device: &Device, item: Retired) {
    match item {
        Retired::Buffer(mut buffer) => buffer.destroy(device),
        Retired::Staging(mut buffer) => buffer.destroy(device),
    }
}
