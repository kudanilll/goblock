//! Per-frame work: wait, acquire, upload, record, submit, present.

use ash::vk;
use glam::{IVec2, Mat4, Vec3};
use goblock_world::ChunkMesh;

use super::chunks::{ChunkBuffers, VisibleChunk, LAYERS, VERTEX_STRIDE};
use super::device::{Device, MAX_FRAMES_IN_FLIGHT};
use super::error::VkError;
use super::resources::{create_pipeline, DynamicBuffer, ImageData, PipelineConfig, Texture};
use super::swapchain::{FrameSync, Swapchain};
use crate::block_textures::{build_mips, BlockTextures};
use crate::font;

/// Block push constants: view-projection (64 bytes), chunk offset (16), fog
/// colour (16) and fog distances (16). Vulkan 1.0 only guarantees 128 bytes,
/// so anything added has to fit in the last 16.
const BLOCK_PUSH_SIZE: u32 = 112;
/// Byte offset of the chunk offset inside the block push constants.
const CHUNK_OFFSET_PUSH: u32 = 64;
/// Sky push constants: inverse view-projection, zenith colour, horizon
/// colour and sun direction.
const SKY_PUSH_SIZE: u32 = 112;
/// HUD vertices one frame can hold, six per glyph.
const HUD_VERTEX_CAPACITY: usize = 4096;

/// Sky colours in linear RGB. The horizon colour doubles as the fog colour,
/// so distant terrain fades into the sky.
const ZENITH: [f32; 4] = [0.16, 0.34, 0.75, 1.0];
const HORIZON: [f32; 4] = [0.60, 0.74, 0.90, 1.0];
/// Where fog begins, as a fraction of [`FrameView::fog_end`].
const FOG_START: f32 = 0.6;

/// What the camera sees this frame.
pub struct FrameView {
    /// Camera position in world space.
    pub camera: Vec3,
    /// Projection times view, with the camera at the origin.
    pub view_projection: Mat4,
    /// Distance at which fog hides the world completely. Keep it inside the
    /// loaded range so the edge of the world never shows.
    pub fog_end: f32,
}

pub struct Renderer {
    device: Device,
    swapchain: Swapchain,
    frames: Vec<FrameSync>,
    current_frame: usize,
    command_pool: vk::CommandPool,
    /// One per frame in flight, indexed like `frames`.
    command_buffers: Vec<vk::CommandBuffer>,

    chunks: ChunkBuffers,
    /// Opaque, cutout and translucent block pipelines, in draw order.
    block_pipelines: [(vk::Pipeline, vk::PipelineLayout); LAYERS],
    sky_pipeline: vk::Pipeline,
    sky_pipeline_layout: vk::PipelineLayout,
    hud_pipeline: vk::Pipeline,
    hud_pipeline_layout: vk::PipelineLayout,

    texture_set_layout: vk::DescriptorSetLayout,
    descriptor_pool: vk::DescriptorPool,
    block_set: vk::DescriptorSet,
    hud_set: vk::DescriptorSet,
    block_texture: Texture,
    hud_texture: Texture,
    /// One per frame in flight, so the CPU never rewrites vertices the GPU
    /// may still be reading for the previous frame.
    hud_vertices: Vec<DynamicBuffer>,
    hud_vertex_count: u32,

    width: u32,
    height: u32,
}

impl Renderer {
    /// # Safety
    /// Requires a live Vulkan device whose surface matches the window that
    /// produced it. The renderer takes ownership of the device and destroys
    /// it, along with everything created here, when dropped.
    pub unsafe fn new(
        device: Device,
        width: u32,
        height: u32,
        textures: &BlockTextures,
    ) -> Result<Self, VkError> {
        let swapchain = Swapchain::new(&device, width, height)?;
        let frames = FrameSync::new(&device)?;

        // Blocks and the HUD each sample one texture through one descriptor.
        let bindings = [vk::DescriptorSetLayoutBinding::default()
            .binding(0)
            .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .descriptor_count(1)
            .stage_flags(vk::ShaderStageFlags::FRAGMENT)];
        let layout_info = vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings);
        let texture_set_layout = device
            .device
            .create_descriptor_set_layout(&layout_info, None)
            .map_err(|e| VkError::new("create_descriptor_set_layout", e))?;

        let mut block_pipelines = [(vk::Pipeline::null(), vk::PipelineLayout::null()); LAYERS];
        for (layer, slot) in block_pipelines.iter_mut().enumerate() {
            // The shaders' ALPHA_MODE: 0 opaque, 1 cutout, 2 translucent.
            let translucent = layer == 2;
            let config = PipelineConfig {
                vs_bytes: include_bytes!(concat!(env!("OUT_DIR"), "/block.vert.spv")),
                fs_bytes: include_bytes!(concat!(env!("OUT_DIR"), "/block.frag.spv")),
                vertex_bindings: vec![vk::VertexInputBindingDescription::default()
                    .binding(0)
                    .stride(VERTEX_STRIDE)
                    .input_rate(vk::VertexInputRate::VERTEX)],
                vertex_attributes: vec![
                    vk::VertexInputAttributeDescription::default()
                        .location(0)
                        .binding(0)
                        .format(vk::Format::R16G16B16A16_UINT)
                        .offset(0),
                    vk::VertexInputAttributeDescription::default()
                        .location(1)
                        .binding(0)
                        .format(vk::Format::R8G8B8A8_UINT)
                        .offset(8),
                    vk::VertexInputAttributeDescription::default()
                        .location(2)
                        .binding(0)
                        .format(vk::Format::R8G8B8A8_UNORM)
                        .offset(12),
                ],
                push_constant_size: BLOCK_PUSH_SIZE,
                depth_test: true,
                depth_write: !translucent,
                blend: translucent,
                // Leaves and water are seen from both sides.
                cull_back: layer == 0,
                descriptor_layouts: vec![texture_set_layout],
                specialization: Some(layer as u32),
            };
            *slot = create_pipeline(&device, &config, device.render_pass)?;
        }

        let sky_config = PipelineConfig {
            vs_bytes: include_bytes!(concat!(env!("OUT_DIR"), "/sky.vert.spv")),
            fs_bytes: include_bytes!(concat!(env!("OUT_DIR"), "/sky.frag.spv")),
            vertex_bindings: vec![],
            vertex_attributes: vec![],
            push_constant_size: SKY_PUSH_SIZE,
            depth_test: false,
            depth_write: false,
            blend: false,
            cull_back: false,
            descriptor_layouts: vec![],
            specialization: None,
        };
        let (sky_pipeline, sky_pipeline_layout) =
            create_pipeline(&device, &sky_config, device.render_pass)?;

        let hud_config = PipelineConfig {
            vs_bytes: include_bytes!(concat!(env!("OUT_DIR"), "/hud.vert.spv")),
            fs_bytes: include_bytes!(concat!(env!("OUT_DIR"), "/hud.frag.spv")),
            vertex_bindings: vec![vk::VertexInputBindingDescription::default()
                .binding(0)
                .stride(std::mem::size_of::<HudVertex>() as u32)
                .input_rate(vk::VertexInputRate::VERTEX)],
            vertex_attributes: vec![
                vk::VertexInputAttributeDescription::default()
                    .location(0)
                    .binding(0)
                    .format(vk::Format::R32G32_SFLOAT)
                    .offset(0),
                vk::VertexInputAttributeDescription::default()
                    .location(1)
                    .binding(0)
                    .format(vk::Format::R32G32_SFLOAT)
                    .offset(8),
            ],
            // vec2 at offset 0, then a vec4, which has to start on a 16-byte boundary.
            push_constant_size: 32,
            depth_test: false,
            depth_write: false,
            blend: true,
            cull_back: false,
            descriptor_layouts: vec![texture_set_layout],
            specialization: None,
        };
        let (hud_pipeline, hud_pipeline_layout) =
            create_pipeline(&device, &hud_config, device.render_pass)?;

        let atlas = font::build_atlas();
        let hud_texture = Texture::new(
            &device,
            &ImageData {
                format: vk::Format::R8_UNORM,
                bytes_per_texel: 1,
                width: font::ATLAS_W,
                height: font::ATLAS_H,
                layers: 1,
                levels: 1,
                view_type: vk::ImageViewType::TYPE_2D,
                pixels: &atlas,
            },
            &sampler_info(vk::Filter::NEAREST, vk::SamplerMipmapMode::NEAREST, 0.0),
        )?;

        let (mips, levels) = build_mips(textures);
        let block_texture = Texture::new(
            &device,
            &ImageData {
                format: vk::Format::R8G8B8A8_SRGB,
                bytes_per_texel: 4,
                width: textures.size,
                height: textures.size,
                layers: textures.layers(),
                levels,
                view_type: vk::ImageViewType::TYPE_2D_ARRAY,
                pixels: &mips,
            },
            // Crisp texels up close, smooth mip blending in the distance.
            &sampler_info(
                vk::Filter::LINEAR,
                vk::SamplerMipmapMode::LINEAR,
                levels as f32,
            ),
        )?;

        let pool_sizes = [vk::DescriptorPoolSize::default()
            .ty(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .descriptor_count(2)];
        let descriptor_pool_info = vk::DescriptorPoolCreateInfo::default()
            .max_sets(2)
            .pool_sizes(&pool_sizes);
        let descriptor_pool = device
            .device
            .create_descriptor_pool(&descriptor_pool_info, None)
            .map_err(|e| VkError::new("create_descriptor_pool", e))?;

        let set_layouts = [texture_set_layout; 2];
        let alloc_info = vk::DescriptorSetAllocateInfo::default()
            .descriptor_pool(descriptor_pool)
            .set_layouts(&set_layouts);
        let sets = device
            .device
            .allocate_descriptor_sets(&alloc_info)
            .map_err(|e| VkError::new("allocate_descriptor_sets", e))?;
        let (block_set, hud_set) = (sets[0], sets[1]);

        let block_image = [vk::DescriptorImageInfo::default()
            .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
            .image_view(block_texture.view)
            .sampler(block_texture.sampler)];
        let hud_image = [vk::DescriptorImageInfo::default()
            .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
            .image_view(hud_texture.view)
            .sampler(hud_texture.sampler)];
        let writes = [
            vk::WriteDescriptorSet::default()
                .dst_set(block_set)
                .dst_binding(0)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .image_info(&block_image),
            vk::WriteDescriptorSet::default()
                .dst_set(hud_set)
                .dst_binding(0)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .image_info(&hud_image),
        ];
        device.device.update_descriptor_sets(&writes, &[]);

        let pool_info = vk::CommandPoolCreateInfo::default()
            .queue_family_index(device.queue_family)
            .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER);
        let command_pool = device
            .device
            .create_command_pool(&pool_info, None)
            .map_err(|e| VkError::new("create_command_pool", e))?;

        let alloc_info = vk::CommandBufferAllocateInfo::default()
            .command_pool(command_pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(MAX_FRAMES_IN_FLIGHT as u32);
        let command_buffers = device
            .device
            .allocate_command_buffers(&alloc_info)
            .map_err(|e| VkError::new("allocate_command_buffers", e))?;

        let hud_vertices = (0..MAX_FRAMES_IN_FLIGHT)
            .map(|_| {
                DynamicBuffer::new(
                    &device,
                    (HUD_VERTEX_CAPACITY * std::mem::size_of::<HudVertex>()) as u64,
                    vk::BufferUsageFlags::VERTEX_BUFFER,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self {
            device,
            swapchain,
            frames,
            current_frame: 0,
            command_pool,
            command_buffers,
            chunks: ChunkBuffers::default(),
            block_pipelines,
            sky_pipeline,
            sky_pipeline_layout,
            hud_pipeline,
            hud_pipeline_layout,
            texture_set_layout,
            descriptor_pool,
            block_set,
            hud_set,
            block_texture,
            hud_texture,
            hud_vertices,
            hud_vertex_count: 0,
            width,
            height,
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) -> Result<(), VkError> {
        self.width = width;
        self.height = height;
        self.recreate_swapchain()
    }

    /// Upload or replace the mesh of chunk `pos`. It is drawn from the next
    /// frame on.
    pub fn upload_chunk(&mut self, pos: IVec2, mesh: &ChunkMesh) -> Result<(), VkError> {
        self.chunks.upload(&self.device, pos, mesh)
    }

    pub fn remove_chunk(&mut self, pos: IVec2) {
        self.chunks.remove(pos);
    }

    /// Chunks currently on the GPU.
    pub fn chunk_count(&self) -> usize {
        self.chunks.count()
    }

    fn recreate_swapchain(&mut self) -> Result<(), VkError> {
        // A minimised window has nothing to draw into.
        if self.width == 0 || self.height == 0 {
            return Ok(());
        }
        self.swapchain
            .recreate(&self.device, self.width, self.height)
    }

    /// Draw one frame.
    pub fn draw_frame(&mut self, view: &FrameView, hud_lines: &[String]) -> Result<(), VkError> {
        if self.width == 0 || self.height == 0 {
            return Ok(());
        }

        let frame = self.current_frame;
        let fence = self.frames[frame].in_flight;
        let image_available = self.frames[frame].image_available;

        unsafe {
            self.device
                .device
                .wait_for_fences(&[fence], true, u64::MAX)
                .map_err(|e| VkError::new("wait_for_fences", e))?;

            let acquired = self.device.swapchain_fn.acquire_next_image(
                self.swapchain.handle,
                u64::MAX,
                image_available,
                vk::Fence::null(),
            );

            let image_index = match acquired {
                Ok((index, _)) => index as usize,
                Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => return self.recreate_swapchain(),
                Err(e) => return Err(VkError::new("acquire_next_image", e)),
            };

            // Reset only once work is certain to be submitted, or the next
            // wait on this fence would never return.
            self.device
                .device
                .reset_fences(&[fence])
                .map_err(|e| VkError::new("reset_fences", e))?;
            self.chunks.begin_frame(&self.device, frame);

            self.upload_hud(frame, hud_lines);

            let command_buffer = self.command_buffers[frame];
            self.device
                .device
                .reset_command_buffer(command_buffer, vk::CommandBufferResetFlags::empty())
                .map_err(|e| VkError::new("reset_command_buffer", e))?;

            self.record(command_buffer, frame, image_index, view)?;

            let wait_semaphores = [image_available];
            let wait_stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
            let signal_semaphores = [self.swapchain.render_finished[image_index]];

            let submit = vk::SubmitInfo::default()
                .wait_semaphores(&wait_semaphores)
                .wait_dst_stage_mask(&wait_stages)
                .command_buffers(std::slice::from_ref(&command_buffer))
                .signal_semaphores(&signal_semaphores);

            self.device
                .device
                .queue_submit(self.device.queue, &[submit], fence)
                .map_err(|e| VkError::new("queue_submit", e))?;

            let swapchains = [self.swapchain.handle];
            let indices = [image_index as u32];
            let present = vk::PresentInfoKHR::default()
                .wait_semaphores(&signal_semaphores)
                .swapchains(&swapchains)
                .image_indices(&indices);

            // `Ok(true)` means the image was shown but the swapchain no longer
            // matches the surface exactly.
            match self
                .device
                .swapchain_fn
                .queue_present(self.device.queue, &present)
            {
                Ok(false) => {}
                Ok(true) | Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => self.recreate_swapchain()?,
                Err(e) => return Err(VkError::new("queue_present", e)),
            }
        }

        self.current_frame = (frame + 1) % MAX_FRAMES_IN_FLIGHT;

        Ok(())
    }

    unsafe fn record(
        &mut self,
        cmd: vk::CommandBuffer,
        frame: usize,
        image_index: usize,
        view: &FrameView,
    ) -> Result<(), VkError> {
        let device = &self.device;
        device
            .device
            .begin_command_buffer(cmd, &vk::CommandBufferBeginInfo::default())
            .map_err(|e| VkError::new("begin_command_buffer", e))?;

        // Copies have to land before the render pass reads them.
        self.chunks.record_uploads(device, cmd, frame);

        let color = vk::ClearValue {
            color: vk::ClearColorValue { float32: HORIZON },
        };
        let depth = vk::ClearValue {
            depth_stencil: vk::ClearDepthStencilValue {
                depth: 1.0,
                stencil: 0,
            },
        };
        // One value per render pass attachment, in the same order. The MSAA
        // resolve target is never cleared but still takes a slot.
        let msaa_clears = [color, color, depth];
        let single_clears = [color, depth];
        let clears: &[vk::ClearValue] = if device.msaa_samples == vk::SampleCountFlags::TYPE_1 {
            &single_clears
        } else {
            &msaa_clears
        };

        let extent = self.swapchain.extent;
        let render_area = vk::Rect2D::default().extent(extent);
        let info = vk::RenderPassBeginInfo::default()
            .render_pass(device.render_pass)
            .framebuffer(self.swapchain.framebuffers[image_index])
            .render_area(render_area)
            .clear_values(clears);
        let d = &device.device;
        d.cmd_begin_render_pass(cmd, &info, vk::SubpassContents::INLINE);

        let viewport = vk::Viewport::default()
            .width(extent.width as f32)
            .height(extent.height as f32)
            .min_depth(0.0)
            .max_depth(1.0);
        d.cmd_set_viewport(cmd, 0, &[viewport]);
        d.cmd_set_scissor(cmd, 0, &[render_area]);

        // Sky first, with depth writes off, so the world draws over it.
        let sun = Vec3::new(0.35, 0.55, 0.25)
            .normalize()
            .extend(0.0)
            .to_array();
        let mut sky_push = [0u8; SKY_PUSH_SIZE as usize];
        sky_push[0..64].copy_from_slice(bytemuck::cast_slice(
            &view.view_projection.inverse().to_cols_array(),
        ));
        sky_push[64..80].copy_from_slice(bytemuck::cast_slice(&ZENITH));
        sky_push[80..96].copy_from_slice(bytemuck::cast_slice(&HORIZON));
        sky_push[96..112].copy_from_slice(bytemuck::cast_slice(&sun));
        d.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.sky_pipeline);
        d.cmd_push_constants(
            cmd,
            self.sky_pipeline_layout,
            vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
            0,
            &sky_push,
        );
        d.cmd_draw(cmd, 3, 1, 0, 0);

        let visible = self.chunks.visible(view.camera, &view.view_projection);
        if !visible.is_empty() && self.chunks.bind_indices(device, cmd) {
            let fog = [view.fog_end * FOG_START, view.fog_end, 0.0, 0.0];
            let mut block_push = [0u8; BLOCK_PUSH_SIZE as usize];
            block_push[0..64]
                .copy_from_slice(bytemuck::cast_slice(&view.view_projection.to_cols_array()));
            block_push[80..96].copy_from_slice(bytemuck::cast_slice(&HORIZON));
            block_push[96..112].copy_from_slice(bytemuck::cast_slice(&fog));

            for (layer, &(pipeline, layout)) in self.block_pipelines.iter().enumerate() {
                d.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, pipeline);
                d.cmd_bind_descriptor_sets(
                    cmd,
                    vk::PipelineBindPoint::GRAPHICS,
                    layout,
                    0,
                    &[self.block_set],
                    &[],
                );
                d.cmd_push_constants(
                    cmd,
                    layout,
                    vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                    0,
                    &block_push,
                );
                // Solid layers go near to far so early depth testing skips
                // hidden pixels; blended water goes far to near.
                let chunks: Box<dyn Iterator<Item = &VisibleChunk>> = if layer == LAYERS - 1 {
                    Box::new(visible.iter().rev())
                } else {
                    Box::new(visible.iter())
                };
                self.chunks
                    .draw(device, cmd, layout, CHUNK_OFFSET_PUSH, layer, chunks);
            }
        }

        // HUD last, blended over everything else.
        if self.hud_vertex_count > 0 {
            d.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.hud_pipeline);

            let size = [extent.width as f32, extent.height as f32];
            let text_color = [1.0f32, 1.0, 1.0, 1.0];
            let mut push = [0u8; 32];
            push[0..8].copy_from_slice(bytemuck::cast_slice(&size));
            push[16..32].copy_from_slice(bytemuck::cast_slice(&text_color));
            d.cmd_push_constants(
                cmd,
                self.hud_pipeline_layout,
                vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                0,
                &push,
            );
            d.cmd_bind_descriptor_sets(
                cmd,
                vk::PipelineBindPoint::GRAPHICS,
                self.hud_pipeline_layout,
                0,
                &[self.hud_set],
                &[],
            );
            d.cmd_bind_vertex_buffers(cmd, 0, &[self.hud_vertices[frame].buffer], &[0]);
            d.cmd_draw(cmd, self.hud_vertex_count, 1, 0, 0);
        }

        d.cmd_end_render_pass(cmd);
        d.end_command_buffer(cmd)
            .map_err(|e| VkError::new("end_command_buffer", e))
    }

    fn upload_hud(&mut self, frame: usize, lines: &[String]) {
        let vertices = build_hud_vertices(lines, HUD_VERTEX_CAPACITY);
        self.hud_vertex_count = vertices.len() as u32;
        self.hud_vertices[frame].write(&vertices);
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        unsafe {
            // Nothing below may be destroyed while the GPU can still use it.
            let _ = self.device.device.device_wait_idle();

            let device = &self.device.device;
            device.destroy_command_pool(self.command_pool, None);

            self.chunks.destroy(&self.device);
            for buffer in &mut self.hud_vertices {
                buffer.destroy(&self.device);
            }
            self.block_texture.destroy(&self.device);
            self.hud_texture.destroy(&self.device);

            device.destroy_descriptor_pool(self.descriptor_pool, None);
            device.destroy_descriptor_set_layout(self.texture_set_layout, None);

            let others = [
                (self.sky_pipeline, self.sky_pipeline_layout),
                (self.hud_pipeline, self.hud_pipeline_layout),
            ];
            for (pipeline, layout) in self.block_pipelines.into_iter().chain(others) {
                device.destroy_pipeline(pipeline, None);
                device.destroy_pipeline_layout(layout, None);
            }

            FrameSync::destroy_all(&self.device, &self.frames);
            self.swapchain.destroy(&self.device);
            self.device.destroy();
        }
    }
}

fn sampler_info(
    min_filter: vk::Filter,
    mipmap_mode: vk::SamplerMipmapMode,
    max_lod: f32,
) -> vk::SamplerCreateInfo<'static> {
    vk::SamplerCreateInfo::default()
        .mag_filter(vk::Filter::NEAREST)
        .min_filter(min_filter)
        .mipmap_mode(mipmap_mode)
        .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
        .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE)
        .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_EDGE)
        .max_lod(max_lod)
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct HudVertex {
    pub pos: [f32; 2],
    pub uv: [f32; 2],
}

fn build_hud_vertices(lines: &[String], capacity: usize) -> Vec<HudVertex> {
    const SCALE: f32 = 2.0;
    const MARGIN: f32 = 8.0;
    const LINE_SPACING: f32 = 4.0;

    let glyph_w = font::GLYPH_W as f32 * SCALE;
    let glyph_h = font::GLYPH_H as f32 * SCALE;
    let advance = font::advance() as f32 * SCALE;

    let mut out = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let y = MARGIN + index as f32 * (glyph_h + LINE_SPACING);
        for (col, ch) in line.chars().enumerate() {
            if out.len() + 6 > capacity {
                return out;
            }
            let x = MARGIN + col as f32 * advance;
            push_glyph(&mut out, ch, x, y, glyph_w, glyph_h);
        }
    }
    out
}

fn push_glyph(out: &mut Vec<HudVertex>, ch: char, x: f32, y: f32, w: f32, h: f32) {
    let (min, max) = font::glyph_uv(ch);
    let (u0, v0) = (min[0], min[1]);
    let (u1, v1) = (max[0], max[1]);
    let quad = [
        (x, y, u0, v0),
        (x, y + h, u0, v1),
        (x + w, y + h, u1, v1),
        (x, y, u0, v0),
        (x + w, y + h, u1, v1),
        (x + w, y, u1, v0),
    ];
    for (px, py, u, v) in quad {
        out.push(HudVertex {
            pos: [px, py],
            uv: [u, v],
        });
    }
}
