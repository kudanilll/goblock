//! Per-frame work: wait, acquire, upload, record, submit, present.
//!
//! Each frame first renders the sun's shadow map, then the main pass: the
//! sky, the chunks layer by layer, and the HUD. Lighting, fog and tone
//! mapping happen in the shaders, fed by one uniform block per frame.

use ash::vk;
use glam::{IVec2, Mat4, Vec3};
use goblock_world::ChunkMesh;

use super::chunks::{ChunkBuffers, VisibleChunk, LAYERS, VERTEX_STRIDE};
use super::device::{Device, MAX_FRAMES_IN_FLIGHT};
use super::error::VkError;
use super::resources::{create_pipeline, DynamicBuffer, ImageData, PipelineConfig, Texture};
use super::shadow::{shadow_matrix, ShadowMap};
use super::swapchain::{FrameSync, Swapchain};
use crate::block_textures::{build_mips, BlockTextures};
use crate::font;

/// Chunk pipelines push one vec4: the chunk's origin relative to the camera.
const CHUNK_PUSH_SIZE: u32 = 16;
/// Size of the `Frame` uniform block in common.glsl: three mat4s and three
/// vec4s.
const FRAME_UNIFORM_SIZE: usize = 3 * 64 + 3 * 16;
/// HUD vertices one frame can hold, six per glyph.
const HUD_VERTEX_CAPACITY: usize = 4096;
/// Where fog begins, as a fraction of [`FrameView::fog_end`].
const FOG_START: f32 = 0.6;
/// Shadow-casting layers, in draw order: opaque blocks, then leaves.
const SHADOW_LAYERS: usize = 2;

/// Direction toward the sun: mid-afternoon, about 44° above the horizon.
fn sun_direction() -> Vec3 {
    Vec3::new(0.55, 0.62, 0.35).normalize()
}

/// What the camera sees this frame.
pub struct FrameView {
    /// Camera position in world space.
    pub camera: Vec3,
    /// Projection times view, with the camera at the origin.
    pub view_projection: Mat4,
    /// Distance at which fog hides the world completely. Keep it inside the
    /// loaded range so the edge of the world never shows. Sun shadows reach
    /// this far as well.
    pub fog_end: f32,
    /// Seconds since the game started, for moving water.
    pub time: f32,
    /// Whether the camera is in water, which swaps the sky and the fog for
    /// murky water.
    pub underwater: bool,
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
    shadow: ShadowMap,
    /// Opaque, cutout and translucent block pipelines, in draw order.
    block_pipelines: [(vk::Pipeline, vk::PipelineLayout); LAYERS],
    /// Opaque and cutout shadow casters. Water casts no shadow.
    shadow_pipelines: [(vk::Pipeline, vk::PipelineLayout); SHADOW_LAYERS],
    sky_pipeline: vk::Pipeline,
    sky_pipeline_layout: vk::PipelineLayout,
    hud_pipeline: vk::Pipeline,
    hud_pipeline_layout: vk::PipelineLayout,

    /// Frame uniforms, block textures and the shadow map.
    frame_set_layout: vk::DescriptorSetLayout,
    hud_set_layout: vk::DescriptorSetLayout,
    descriptor_pool: vk::DescriptorPool,
    /// One per frame in flight, each pointing at its own uniform buffer.
    frame_sets: Vec<vk::DescriptorSet>,
    frame_uniforms: Vec<DynamicBuffer>,
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
        let shadow = ShadowMap::new(&device)?;

        let sampler_binding = |binding: u32| {
            vk::DescriptorSetLayoutBinding::default()
                .binding(binding)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT)
        };
        let frame_bindings = [
            vk::DescriptorSetLayoutBinding::default()
                .binding(0)
                .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT),
            sampler_binding(1),
            sampler_binding(2),
        ];
        let frame_set_layout = device
            .device
            .create_descriptor_set_layout(
                &vk::DescriptorSetLayoutCreateInfo::default().bindings(&frame_bindings),
                None,
            )
            .map_err(|e| VkError::new("create_descriptor_set_layout", e))?;
        let hud_bindings = [sampler_binding(0)];
        let hud_set_layout = device
            .device
            .create_descriptor_set_layout(
                &vk::DescriptorSetLayoutCreateInfo::default().bindings(&hud_bindings),
                None,
            )
            .map_err(|e| VkError::new("create_descriptor_set_layout", e))?;

        let chunk_binding = vec![vk::VertexInputBindingDescription::default()
            .binding(0)
            .stride(VERTEX_STRIDE)
            .input_rate(vk::VertexInputRate::VERTEX)];
        let chunk_attribute = |location: u32, format: vk::Format, offset: u32| {
            vk::VertexInputAttributeDescription::default()
                .location(location)
                .binding(0)
                .format(format)
                .offset(offset)
        };

        let mut block_pipelines = [(vk::Pipeline::null(), vk::PipelineLayout::null()); LAYERS];
        for (layer, slot) in block_pipelines.iter_mut().enumerate() {
            // The shaders' ALPHA_MODE: 0 opaque, 1 cutout, 2 translucent.
            let translucent = layer == 2;
            let config = PipelineConfig {
                vs_bytes: include_bytes!(concat!(env!("OUT_DIR"), "/block.vert.spv")),
                fs_bytes: include_bytes!(concat!(env!("OUT_DIR"), "/block.frag.spv")),
                vertex_bindings: chunk_binding.clone(),
                vertex_attributes: vec![
                    chunk_attribute(0, vk::Format::R16G16B16A16_UINT, 0),
                    chunk_attribute(1, vk::Format::R8G8B8A8_UINT, 8),
                    chunk_attribute(2, vk::Format::R8G8B8A8_UNORM, 12),
                ],
                push_constant_size: CHUNK_PUSH_SIZE,
                depth_test: true,
                depth_write: !translucent,
                blend: translucent,
                // Leaves and water are seen from both sides.
                cull_back: layer == 0,
                depth_only: false,
                descriptor_layouts: vec![frame_set_layout],
                specialization: Some(layer as u32),
            };
            *slot = create_pipeline(&device, &config, device.render_pass)?;
        }

        let mut shadow_pipelines =
            [(vk::Pipeline::null(), vk::PipelineLayout::null()); SHADOW_LAYERS];
        for (layer, slot) in shadow_pipelines.iter_mut().enumerate() {
            let config = PipelineConfig {
                vs_bytes: include_bytes!(concat!(env!("OUT_DIR"), "/shadow.vert.spv")),
                fs_bytes: include_bytes!(concat!(env!("OUT_DIR"), "/shadow.frag.spv")),
                vertex_bindings: chunk_binding.clone(),
                vertex_attributes: vec![
                    chunk_attribute(0, vk::Format::R16G16B16A16_UINT, 0),
                    chunk_attribute(1, vk::Format::R8G8B8A8_UINT, 8),
                ],
                push_constant_size: CHUNK_PUSH_SIZE,
                depth_test: true,
                depth_write: true,
                blend: false,
                cull_back: false,
                depth_only: true,
                descriptor_layouts: vec![frame_set_layout],
                specialization: Some(layer as u32),
            };
            *slot = create_pipeline(&device, &config, shadow.render_pass)?;
        }

        let sky_config = PipelineConfig {
            vs_bytes: include_bytes!(concat!(env!("OUT_DIR"), "/sky.vert.spv")),
            fs_bytes: include_bytes!(concat!(env!("OUT_DIR"), "/sky.frag.spv")),
            vertex_bindings: vec![],
            vertex_attributes: vec![],
            push_constant_size: 0,
            depth_test: false,
            depth_write: false,
            blend: false,
            cull_back: false,
            depth_only: false,
            descriptor_layouts: vec![frame_set_layout],
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
            depth_only: false,
            descriptor_layouts: vec![hud_set_layout],
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

        let frame_uniforms = (0..MAX_FRAMES_IN_FLIGHT)
            .map(|_| {
                DynamicBuffer::new(
                    &device,
                    FRAME_UNIFORM_SIZE as u64,
                    vk::BufferUsageFlags::UNIFORM_BUFFER,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;

        let frames_in_flight = MAX_FRAMES_IN_FLIGHT as u32;
        let pool_sizes = [
            vk::DescriptorPoolSize::default()
                .ty(vk::DescriptorType::UNIFORM_BUFFER)
                .descriptor_count(frames_in_flight),
            vk::DescriptorPoolSize::default()
                .ty(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .descriptor_count(2 * frames_in_flight + 1),
        ];
        let descriptor_pool = device
            .device
            .create_descriptor_pool(
                &vk::DescriptorPoolCreateInfo::default()
                    .max_sets(frames_in_flight + 1)
                    .pool_sizes(&pool_sizes),
                None,
            )
            .map_err(|e| VkError::new("create_descriptor_pool", e))?;

        let mut set_layouts = vec![frame_set_layout; MAX_FRAMES_IN_FLIGHT];
        set_layouts.push(hud_set_layout);
        let mut sets = device
            .device
            .allocate_descriptor_sets(
                &vk::DescriptorSetAllocateInfo::default()
                    .descriptor_pool(descriptor_pool)
                    .set_layouts(&set_layouts),
            )
            .map_err(|e| VkError::new("allocate_descriptor_sets", e))?;
        let hud_set = sets.pop().expect("one set per layout");
        let frame_sets = sets;

        let image_info = |texture: &Texture| {
            [vk::DescriptorImageInfo::default()
                .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                .image_view(texture.view)
                .sampler(texture.sampler)]
        };
        let block_image = image_info(&block_texture);
        let hud_image = image_info(&hud_texture);
        let shadow_image = [vk::DescriptorImageInfo::default()
            .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
            .image_view(shadow.view)
            .sampler(shadow.sampler)];
        let uniform_infos: Vec<[vk::DescriptorBufferInfo; 1]> = frame_uniforms
            .iter()
            .map(|buffer| {
                [vk::DescriptorBufferInfo::default()
                    .buffer(buffer.buffer)
                    .range(FRAME_UNIFORM_SIZE as u64)]
            })
            .collect();

        let image_write = |set: vk::DescriptorSet, binding: u32, info| {
            vk::WriteDescriptorSet::default()
                .dst_set(set)
                .dst_binding(binding)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .image_info(info)
        };
        let mut writes = vec![image_write(hud_set, 0, &hud_image)];
        for (&set, uniform) in frame_sets.iter().zip(&uniform_infos) {
            writes.push(
                vk::WriteDescriptorSet::default()
                    .dst_set(set)
                    .dst_binding(0)
                    .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                    .buffer_info(uniform),
            );
            writes.push(image_write(set, 1, &block_image));
            writes.push(image_write(set, 2, &shadow_image));
        }
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
            .command_buffer_count(frames_in_flight);
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
            shadow,
            block_pipelines,
            shadow_pipelines,
            sky_pipeline,
            sky_pipeline_layout,
            hud_pipeline,
            hud_pipeline_layout,
            frame_set_layout,
            hud_set_layout,
            descriptor_pool,
            frame_sets,
            frame_uniforms,
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

            let shadow_matrix =
                shadow_matrix(view.camera, sun_direction(), view.fog_end, self.shadow.size);
            self.frame_uniforms[frame].write(&frame_uniforms(
                view,
                &shadow_matrix,
                self.shadow.texel(),
            ));
            self.upload_hud(frame, hud_lines);

            let command_buffer = self.command_buffers[frame];
            self.device
                .device
                .reset_command_buffer(command_buffer, vk::CommandBufferResetFlags::empty())
                .map_err(|e| VkError::new("reset_command_buffer", e))?;

            self.record(command_buffer, frame, image_index, view, &shadow_matrix)?;

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
        shadow_matrix: &Mat4,
    ) -> Result<(), VkError> {
        let device = &self.device;
        let d = &device.device;
        d.begin_command_buffer(cmd, &vk::CommandBufferBeginInfo::default())
            .map_err(|e| VkError::new("begin_command_buffer", e))?;

        // Copies have to land before either pass reads them.
        self.chunks.record_uploads(device, cmd, frame);
        let has_geometry = self.chunks.bind_indices(device, cmd);
        let frame_set = [self.frame_sets[frame]];

        // The shadow pass always runs, so the map is cleared and in the
        // right layout even when shadows are off.
        self.shadow.begin(device, cmd);
        if has_geometry && self.shadow.size > 0 {
            let casters = self.chunks.visible(view.camera, shadow_matrix);
            for (layer, &(pipeline, layout)) in self.shadow_pipelines.iter().enumerate() {
                d.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, pipeline);
                d.cmd_bind_descriptor_sets(
                    cmd,
                    vk::PipelineBindPoint::GRAPHICS,
                    layout,
                    0,
                    &frame_set,
                    &[],
                );
                self.chunks.draw(device, cmd, layout, layer, casters.iter());
            }
        }
        d.cmd_end_render_pass(cmd);

        let color = vk::ClearValue {
            color: vk::ClearColorValue {
                float32: [0.0, 0.0, 0.0, 1.0],
            },
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
        d.cmd_begin_render_pass(cmd, &info, vk::SubpassContents::INLINE);

        let viewport = vk::Viewport::default()
            .width(extent.width as f32)
            .height(extent.height as f32)
            .min_depth(0.0)
            .max_depth(1.0);
        d.cmd_set_viewport(cmd, 0, &[viewport]);
        d.cmd_set_scissor(cmd, 0, &[render_area]);

        // Sky first, with depth writes off, so the world draws over it.
        d.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.sky_pipeline);
        d.cmd_bind_descriptor_sets(
            cmd,
            vk::PipelineBindPoint::GRAPHICS,
            self.sky_pipeline_layout,
            0,
            &frame_set,
            &[],
        );
        d.cmd_draw(cmd, 3, 1, 0, 0);

        if has_geometry {
            let visible = self.chunks.visible(view.camera, &view.view_projection);
            for (layer, &(pipeline, layout)) in self.block_pipelines.iter().enumerate() {
                d.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, pipeline);
                d.cmd_bind_descriptor_sets(
                    cmd,
                    vk::PipelineBindPoint::GRAPHICS,
                    layout,
                    0,
                    &frame_set,
                    &[],
                );
                // Solid layers go near to far so early depth testing skips
                // hidden pixels; blended water goes far to near.
                let chunks: Box<dyn Iterator<Item = &VisibleChunk>> = if layer == LAYERS - 1 {
                    Box::new(visible.iter().rev())
                } else {
                    Box::new(visible.iter())
                };
                self.chunks.draw(device, cmd, layout, layer, chunks);
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
            for buffer in self.hud_vertices.iter_mut().chain(&mut self.frame_uniforms) {
                buffer.destroy(&self.device);
            }
            self.block_texture.destroy(&self.device);
            self.hud_texture.destroy(&self.device);

            device.destroy_descriptor_pool(self.descriptor_pool, None);
            device.destroy_descriptor_set_layout(self.frame_set_layout, None);
            device.destroy_descriptor_set_layout(self.hud_set_layout, None);

            let others = [
                (self.sky_pipeline, self.sky_pipeline_layout),
                (self.hud_pipeline, self.hud_pipeline_layout),
            ];
            let pipelines = self
                .block_pipelines
                .into_iter()
                .chain(self.shadow_pipelines)
                .chain(others);
            for (pipeline, layout) in pipelines {
                device.destroy_pipeline(pipeline, None);
                device.destroy_pipeline_layout(layout, None);
            }
            self.shadow.destroy(&self.device);

            FrameSync::destroy_all(&self.device, &self.frames);
            self.swapchain.destroy(&self.device);
            self.device.destroy();
        }
    }
}

/// The `Frame` uniform block from common.glsl, laid out by std140 rules:
/// view-projection, its inverse and the shadow matrix, then the camera
/// position with the time, the sun direction with the shadow texel size,
/// and the fog distances with the underwater flag.
fn frame_uniforms(view: &FrameView, shadow_matrix: &Mat4, shadow_texel: f32) -> Vec<u8> {
    let sun = sun_direction();
    let camera = view.camera;
    let underwater = if view.underwater { 1.0 } else { 0.0 };
    let floats: Vec<f32> = [
        view.view_projection.to_cols_array().as_slice(),
        view.view_projection.inverse().to_cols_array().as_slice(),
        shadow_matrix.to_cols_array().as_slice(),
        &[camera.x, camera.y, camera.z, view.time],
        &[sun.x, sun.y, sun.z, shadow_texel],
        &[view.fog_end * FOG_START, view.fog_end, underwater, 0.0],
    ]
    .concat();
    bytemuck::cast_slice(&floats).to_vec()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_uniforms_follow_the_std140_layout() {
        let view = FrameView {
            camera: Vec3::new(1.0, 2.0, 3.0),
            view_projection: Mat4::IDENTITY,
            fog_end: 100.0,
            time: 7.0,
            underwater: true,
        };
        let bytes = frame_uniforms(&view, &Mat4::IDENTITY, 0.5);
        assert_eq!(bytes.len(), FRAME_UNIFORM_SIZE);
        let float =
            |offset: usize| f32::from_ne_bytes(bytes[offset..offset + 4].try_into().unwrap());
        // cameraPosition starts after three mat4s.
        assert_eq!(
            [float(192), float(196), float(200), float(204)],
            [1.0, 2.0, 3.0, 7.0]
        );
        assert_eq!(float(220), 0.5, "shadow texel size in sunDirection.w");
        assert!((float(224) - 100.0 * FOG_START).abs() < 1e-4, "fog start");
        assert_eq!(float(228), 100.0, "fog end");
        assert_eq!(float(232), 1.0, "underwater flag in fog.z");
    }
}
