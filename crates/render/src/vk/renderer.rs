//! Per-frame work: wait, acquire, upload, record, submit, present.
//!
//! Each frame first renders the sun's shadow map, then the scene pass: the
//! sky and the opaque and cut-out blocks, into offscreen images. The final
//! pass copies that scene to the screen and draws the water over it, which
//! reads the scene for refraction and screen-space reflections, then the
//! HUD. Lighting, fog and tone mapping happen in the shaders, fed by one
//! uniform block per frame.

use ash::vk;
use glam::{IVec2, Mat4, Vec3};
use goblock_world::ChunkMesh;

use super::chunks::{ChunkBuffers, LAYERS, VERTEX_STRIDE};
use super::device::{Device, MAX_FRAMES_IN_FLIGHT};
use super::error::VkError;
use super::resources::{create_pipeline, DynamicBuffer, ImageData, PipelineConfig, Texture};
use super::shadow::{shadow_matrix, ShadowMap};
use super::swapchain::{FrameSync, Swapchain};
use crate::background_textures::BackgroundTextures;
use crate::block_textures::{build_mips, BlockTextures};
use crate::font;

/// Chunk pipelines push one vec4: the chunk's origin relative to the camera.
const CHUNK_PUSH_SIZE: u32 = 16;
/// Size of the `Frame` uniform block in common.glsl: three mat4s and five
/// vec4s.
const FRAME_UNIFORM_SIZE: usize = 3 * 64 + 5 * 16;
/// HUD vertices one frame can hold, six per glyph or panel.
const HUD_VERTEX_CAPACITY: usize = 16384;
/// Where fog begins, as a fraction of [`FrameView::fog_end`].
const FOG_START: f32 = 0.6;
/// Layers drawn in the scene pass and into the shadow map: opaque blocks,
/// then cutout vegetation. Water, the last layer, is drawn in the final pass.
const SOLID_LAYERS: usize = 2;

/// Direction toward the sun: mid-afternoon, about 44° above the horizon.
fn sun_direction() -> Vec3 {
    Vec3::new(0.55, 0.62, 0.35).normalize()
}

/// What the camera sees this frame.
pub struct FrameView {
    /// Camera position in world space.
    pub camera: Vec3,
    /// Player's feet in world space and body height, for bending grass.
    pub player_feet: Vec3,
    pub player_height: f32,
    /// Projection times view, with the camera at the origin.
    pub view_projection: Mat4,
    /// Distance at which fog hides the world completely. Keep it inside the
    /// loaded range so the edge of the world never shows. Sun shadows reach
    /// this far as well.
    pub fog_end: f32,
    /// Seconds since the game started, for water and vegetation.
    pub time: f32,
    /// Whether the camera is in water, which swaps the sky and the fog for
    /// murky water.
    pub underwater: bool,
    /// Whether the settings menu is open, blurring and dimming the game scene.
    pub blur_background: bool,
    /// When Some((layer_a, layer_b, blend)), renders the static background slideshow instead of 3D scene.
    pub menu_background: Option<(u32, u32, f32)>,
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
    /// Opaque and cutout block pipelines, in draw order.
    block_pipelines: [(vk::Pipeline, vk::PipelineLayout); SOLID_LAYERS],
    /// Opaque and cutout shadow casters. Water casts no shadow.
    shadow_pipelines: [(vk::Pipeline, vk::PipelineLayout); SOLID_LAYERS],
    water_pipeline: (vk::Pipeline, vk::PipelineLayout),
    /// Copies the scene onto the swapchain image.
    composite_pipeline: (vk::Pipeline, vk::PipelineLayout),
    /// Background slideshow pipeline for menu screens.
    background_pipeline: (vk::Pipeline, vk::PipelineLayout),
    sky_pipeline: vk::Pipeline,
    sky_pipeline_layout: vk::PipelineLayout,
    hud_pipeline: vk::Pipeline,
    hud_pipeline_layout: vk::PipelineLayout,
    /// For the scene's colour (filtered) and its depth (exact texels).
    scene_samplers: [vk::Sampler; 2],
    /// Whether water traces screen-space reflections.
    reflections: bool,

    /// Frame uniforms, block textures, the shadow map, and the scene's
    /// colour and depth.
    frame_set_layout: vk::DescriptorSetLayout,
    hud_set_layout: vk::DescriptorSetLayout,
    background_set_layout: vk::DescriptorSetLayout,
    descriptor_pool: vk::DescriptorPool,
    /// One per frame in flight, each pointing at its own uniform buffer.
    frame_sets: Vec<vk::DescriptorSet>,
    frame_uniforms: Vec<DynamicBuffer>,
    hud_set: vk::DescriptorSet,
    background_set: vk::DescriptorSet,
    block_texture: Texture,
    hud_texture: Texture,
    background_texture: Texture,
    pub background_layers: u32,
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
        backgrounds: &BackgroundTextures,
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
            sampler_binding(3),
            sampler_binding(4),
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
        let background_set_layout = device
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

        let block_vs = include_bytes!(concat!(env!("OUT_DIR"), "/block.vert.spv"));
        let block_attributes = vec![
            chunk_attribute(0, vk::Format::R16G16B16A16_UINT, 0),
            chunk_attribute(1, vk::Format::R8G8B8A8_UINT, 8),
            chunk_attribute(2, vk::Format::R8G8B8A8_UNORM, 12),
        ];
        let mut block_pipelines =
            [(vk::Pipeline::null(), vk::PipelineLayout::null()); SOLID_LAYERS];
        for (layer, slot) in block_pipelines.iter_mut().enumerate() {
            // The shaders' ALPHA_MODE: 0 opaque, 1 cutout.
            let config = PipelineConfig {
                vs_bytes: block_vs,
                fs_bytes: include_bytes!(concat!(env!("OUT_DIR"), "/block.frag.spv")),
                vertex_bindings: chunk_binding.clone(),
                vertex_attributes: block_attributes.clone(),
                push_constant_size: CHUNK_PUSH_SIZE,
                depth_test: true,
                depth_write: true,
                blend: false,
                // Leaves are seen from both sides.
                cull_back: layer == 0,
                depth_only: false,
                // The colour and the depth copy.
                color_attachments: 2,
                descriptor_layouts: vec![frame_set_layout],
                specialization: Some(layer as u32),
            };
            *slot = create_pipeline(&device, &config, device.scene_pass)?;
        }

        // Water mixes in the scene behind it itself, so it needs no blending.
        // It writes depth only so the nearest of two water faces wins.
        let water_config = PipelineConfig {
            vs_bytes: block_vs,
            fs_bytes: include_bytes!(concat!(env!("OUT_DIR"), "/water.frag.spv")),
            vertex_bindings: chunk_binding.clone(),
            vertex_attributes: block_attributes,
            push_constant_size: CHUNK_PUSH_SIZE,
            depth_test: true,
            depth_write: true,
            blend: false,
            cull_back: false,
            depth_only: false,
            color_attachments: 1,
            descriptor_layouts: vec![frame_set_layout],
            specialization: None,
        };
        let water_pipeline = create_pipeline(&device, &water_config, device.final_pass)?;

        let mut shadow_pipelines =
            [(vk::Pipeline::null(), vk::PipelineLayout::null()); SOLID_LAYERS];
        for (layer, slot) in shadow_pipelines.iter_mut().enumerate() {
            let config = PipelineConfig {
                vs_bytes: include_bytes!(concat!(env!("OUT_DIR"), "/shadow.vert.spv")),
                fs_bytes: include_bytes!(concat!(env!("OUT_DIR"), "/shadow.frag.spv")),
                vertex_bindings: chunk_binding.clone(),
                vertex_attributes: vec![
                    chunk_attribute(0, vk::Format::R16G16B16A16_UINT, 0),
                    chunk_attribute(1, vk::Format::R8G8B8A8_UINT, 8),
                    chunk_attribute(2, vk::Format::R8G8B8A8_UNORM, 12),
                ],
                push_constant_size: CHUNK_PUSH_SIZE,
                depth_test: true,
                depth_write: true,
                blend: false,
                cull_back: false,
                depth_only: true,
                color_attachments: 0,
                descriptor_layouts: vec![frame_set_layout],
                specialization: Some(layer as u32),
            };
            *slot = create_pipeline(&device, &config, shadow.render_pass)?;
        }

        // The sky and the scene copy are both a fullscreen triangle.
        let fullscreen = |fs_bytes, color_attachments| PipelineConfig {
            vs_bytes: include_bytes!(concat!(env!("OUT_DIR"), "/sky.vert.spv")),
            fs_bytes,
            vertex_bindings: vec![],
            vertex_attributes: vec![],
            push_constant_size: 0,
            depth_test: false,
            depth_write: false,
            blend: false,
            cull_back: false,
            depth_only: false,
            color_attachments,
            descriptor_layouts: vec![frame_set_layout],
            specialization: None,
        };
        let sky_config = fullscreen(include_bytes!(concat!(env!("OUT_DIR"), "/sky.frag.spv")), 2);
        let (sky_pipeline, sky_pipeline_layout) =
            create_pipeline(&device, &sky_config, device.scene_pass)?;
        let composite_config = fullscreen(
            include_bytes!(concat!(env!("OUT_DIR"), "/composite.frag.spv")),
            1,
        );
        let composite_pipeline = create_pipeline(&device, &composite_config, device.final_pass)?;

        let bg_config = PipelineConfig {
            vs_bytes: include_bytes!(concat!(env!("OUT_DIR"), "/sky.vert.spv")),
            fs_bytes: include_bytes!(concat!(env!("OUT_DIR"), "/background.frag.spv")),
            vertex_bindings: vec![],
            vertex_attributes: vec![],
            push_constant_size: 16,
            depth_test: false,
            depth_write: false,
            blend: false,
            cull_back: false,
            depth_only: false,
            color_attachments: 1,
            descriptor_layouts: vec![background_set_layout],
            specialization: None,
        };
        let background_pipeline = create_pipeline(&device, &bg_config, device.final_pass)?;

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
                vk::VertexInputAttributeDescription::default()
                    .location(2)
                    .binding(0)
                    .format(vk::Format::R8G8B8A8_UNORM)
                    .offset(16),
            ],
            // vec2 at offset 0, then a vec4, which has to start on a 16-byte boundary.
            push_constant_size: 32,
            depth_test: false,
            depth_write: false,
            blend: true,
            cull_back: false,
            depth_only: false,
            color_attachments: 1,
            descriptor_layouts: vec![hud_set_layout],
            specialization: None,
        };
        let (hud_pipeline, hud_pipeline_layout) =
            create_pipeline(&device, &hud_config, device.final_pass)?;

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

        let background_texture = Texture::new(
            &device,
            &ImageData {
                format: vk::Format::R8G8B8A8_SRGB,
                bytes_per_texel: 4,
                width: backgrounds.width,
                height: backgrounds.height,
                layers: backgrounds.layers,
                levels: 1,
                view_type: vk::ImageViewType::TYPE_2D_ARRAY,
                pixels: &backgrounds.pixels,
            },
            &sampler_info(vk::Filter::LINEAR, vk::SamplerMipmapMode::NEAREST, 1.0),
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
                .descriptor_count(4 * frames_in_flight + 2),
        ];
        let descriptor_pool = device
            .device
            .create_descriptor_pool(
                &vk::DescriptorPoolCreateInfo::default()
                    .max_sets(frames_in_flight + 2)
                    .pool_sizes(&pool_sizes),
                None,
            )
            .map_err(|e| VkError::new("create_descriptor_pool", e))?;

        let mut set_layouts = vec![frame_set_layout; MAX_FRAMES_IN_FLIGHT];
        set_layouts.push(hud_set_layout);
        set_layouts.push(background_set_layout);
        let mut sets = device
            .device
            .allocate_descriptor_sets(
                &vk::DescriptorSetAllocateInfo::default()
                    .descriptor_pool(descriptor_pool)
                    .set_layouts(&set_layouts),
            )
            .map_err(|e| VkError::new("allocate_descriptor_sets", e))?;
        let background_set = sets.pop().expect("one set per layout");
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
        let background_image = image_info(&background_texture);
        let mut writes = vec![
            image_write(hud_set, 0, &hud_image),
            image_write(background_set, 0, &background_image),
        ];
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

        let mut scene_samplers = [vk::Sampler::null(); 2];
        let infos = [
            sampler_info(vk::Filter::LINEAR, vk::SamplerMipmapMode::NEAREST, 0.0)
                .mag_filter(vk::Filter::LINEAR),
            sampler_info(vk::Filter::NEAREST, vk::SamplerMipmapMode::NEAREST, 0.0),
        ];
        for (sampler, info) in scene_samplers.iter_mut().zip(&infos) {
            *sampler = device
                .device
                .create_sampler(info, None)
                .map_err(|e| VkError::new("create_sampler", e))?;
        }
        write_scene_sets(
            &device,
            &frame_sets,
            swapchain.scene_views(),
            scene_samplers,
        );

        // A ray march for every water pixel is too slow for virtual and
        // CPU devices. Their water reflects only the sky.
        let reflections = matches!(
            device.properties.device_type,
            vk::PhysicalDeviceType::DISCRETE_GPU | vk::PhysicalDeviceType::INTEGRATED_GPU
        );

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
            water_pipeline,
            composite_pipeline,
            background_pipeline,
            sky_pipeline,
            sky_pipeline_layout,
            hud_pipeline,
            hud_pipeline_layout,
            scene_samplers,
            reflections,
            frame_set_layout,
            hud_set_layout,
            background_set_layout,
            descriptor_pool,
            frame_sets,
            frame_uniforms,
            hud_set,
            background_set,
            block_texture,
            hud_texture,
            background_texture,
            background_layers: backgrounds.layers,
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

    pub fn clear_chunks(&mut self) {
        self.chunks.clear();
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
        // The scene images are new, so the sets that read them must follow.
        // A rebuild waits for the device to go idle, so no frame uses them.
        if self
            .swapchain
            .recreate(&self.device, self.width, self.height)?
        {
            unsafe {
                write_scene_sets(
                    &self.device,
                    &self.frame_sets,
                    self.swapchain.scene_views(),
                    self.scene_samplers,
                );
            }
        }
        Ok(())
    }

    /// Draw one frame.
    pub fn draw_frame(
        &mut self,
        view: &FrameView,
        ui_vertices: &[HudVertex],
    ) -> Result<(), VkError> {
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
                self.reflections,
            ));
            self.upload_hud(frame, ui_vertices);

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
        // The depth copy starts out as far away as the depth buffer.
        let far = vk::ClearValue {
            color: vk::ClearColorValue {
                float32: [1.0, 0.0, 0.0, 0.0],
            },
        };
        let depth = vk::ClearValue {
            depth_stencil: vk::ClearDepthStencilValue {
                depth: 1.0,
                stencil: 0,
            },
        };
        // One value per scene pass attachment, in the same order. The MSAA
        // resolve targets are never cleared but still take a slot.
        let msaa_clears = [color, far, color, far, depth];
        let single_clears = [color, far, depth];
        let clears: &[vk::ClearValue] = if device.msaa_samples == vk::SampleCountFlags::TYPE_1 {
            &single_clears
        } else {
            &msaa_clears
        };

        let extent = self.swapchain.extent;
        let render_area = vk::Rect2D::default().extent(extent);
        let info = vk::RenderPassBeginInfo::default()
            .render_pass(device.scene_pass)
            .framebuffer(self.swapchain.scene_framebuffer)
            .render_area(render_area)
            .clear_values(clears);
        d.cmd_begin_render_pass(cmd, &info, vk::SubpassContents::INLINE);

        // Both passes draw at the swapchain size.
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

        // Near to far, so early depth testing skips hidden pixels.
        let visible = if has_geometry {
            self.chunks.visible(view.camera, &view.view_projection)
        } else {
            Vec::new()
        };
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
            self.chunks.draw(device, cmd, layout, layer, visible.iter());
        }
        d.cmd_end_render_pass(cmd);

        // Every attachment of the final pass is loaded or overwritten.
        let info = vk::RenderPassBeginInfo::default()
            .render_pass(device.final_pass)
            .framebuffer(self.swapchain.framebuffers[image_index])
            .render_area(render_area);
        d.cmd_begin_render_pass(cmd, &info, vk::SubpassContents::INLINE);

        if let Some((layer_a, layer_b, blend)) = view.menu_background {
            let (pipeline, layout) = self.background_pipeline;
            d.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, pipeline);
            let push = [layer_a as f32, layer_b as f32, blend, 0.0f32];
            d.cmd_push_constants(
                cmd,
                layout,
                vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                0,
                bytemuck::cast_slice(&push),
            );
            d.cmd_bind_descriptor_sets(
                cmd,
                vk::PipelineBindPoint::GRAPHICS,
                layout,
                0,
                &[self.background_set],
                &[],
            );
            d.cmd_draw(cmd, 3, 1, 0, 0);
        } else {
            let (pipeline, layout) = self.composite_pipeline;
            d.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, pipeline);
            d.cmd_bind_descriptor_sets(
                cmd,
                vk::PipelineBindPoint::GRAPHICS,
                layout,
                0,
                &frame_set,
                &[],
            );
            d.cmd_draw(cmd, 3, 1, 0, 0);

            let (pipeline, layout) = self.water_pipeline;
            d.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, pipeline);
            d.cmd_bind_descriptor_sets(
                cmd,
                vk::PipelineBindPoint::GRAPHICS,
                layout,
                0,
                &frame_set,
                &[],
            );
            self.chunks
                .draw(device, cmd, layout, LAYERS - 1, visible.iter());
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

    fn upload_hud(&mut self, frame: usize, vertices: &[HudVertex]) {
        let count = vertices.len().min(HUD_VERTEX_CAPACITY);
        if count > 0 {
            self.hud_vertices[frame].write(&vertices[..count]);
        }
        self.hud_vertex_count = count as u32;
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
            self.background_texture.destroy(&self.device);

            for sampler in self.scene_samplers {
                device.destroy_sampler(sampler, None);
            }
            device.destroy_descriptor_pool(self.descriptor_pool, None);
            device.destroy_descriptor_set_layout(self.frame_set_layout, None);
            device.destroy_descriptor_set_layout(self.hud_set_layout, None);
            device.destroy_descriptor_set_layout(self.background_set_layout, None);

            let others = [
                self.water_pipeline,
                self.composite_pipeline,
                self.background_pipeline,
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
/// the fog distances with the underwater and reflection flags, and the
/// camera-relative player's feet with the body height.
fn frame_uniforms(
    view: &FrameView,
    shadow_matrix: &Mat4,
    shadow_texel: f32,
    reflections: bool,
) -> Vec<u8> {
    let sun = sun_direction();
    let camera = view.camera;
    let feet = view.player_feet - camera;
    let flag = |on: bool| if on { 1.0 } else { 0.0 };
    let floats: Vec<f32> = [
        view.view_projection.to_cols_array().as_slice(),
        view.view_projection.inverse().to_cols_array().as_slice(),
        shadow_matrix.to_cols_array().as_slice(),
        &[camera.x, camera.y, camera.z, view.time],
        &[sun.x, sun.y, sun.z, shadow_texel],
        &[
            view.fog_end * FOG_START,
            view.fog_end,
            flag(view.underwater),
            flag(reflections),
        ],
        &[feet.x, feet.y, feet.z, view.player_height],
        &[flag(view.blur_background), 0.0, 0.0, 0.0],
    ]
    .concat();
    bytemuck::cast_slice(&floats).to_vec()
}

/// Point bindings 3 and 4 of every frame set at the scene's colour and
/// depth.
///
/// # Safety
/// No submitted frame may still be using the sets.
unsafe fn write_scene_sets(
    device: &Device,
    sets: &[vk::DescriptorSet],
    views: [vk::ImageView; 2],
    samplers: [vk::Sampler; 2],
) {
    let infos = [0, 1].map(|i| {
        [vk::DescriptorImageInfo::default()
            .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
            .image_view(views[i])
            .sampler(samplers[i])]
    });
    let writes: Vec<_> = sets
        .iter()
        .flat_map(|&set| {
            infos.iter().zip(3..).map(move |(info, binding)| {
                vk::WriteDescriptorSet::default()
                    .dst_set(set)
                    .dst_binding(binding)
                    .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                    .image_info(info)
            })
        })
        .collect();
    device.device.update_descriptor_sets(&writes, &[]);
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
#[derive(Clone, Copy, Debug, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct HudVertex {
    pub pos: [f32; 2],
    pub uv: [f32; 2],
    pub color: [u8; 4],
}

pub fn build_hud_vertices(lines: &[String]) -> Vec<HudVertex> {
    const SCALE: f32 = 1.0;
    const MARGIN: f32 = 12.0;
    const LINE_SPACING: f32 = 6.0;
    let white = [255, 255, 255, 255];

    let mut out = Vec::new();
    let mut y = MARGIN;
    for line in lines {
        draw_text(
            &mut out,
            font::FontKind::Regular,
            line,
            MARGIN,
            y,
            SCALE,
            white,
        );
        y += 18.0 * SCALE + LINE_SPACING;
    }
    out
}

pub fn push_glyph(
    out: &mut Vec<HudVertex>,
    kind: font::FontKind,
    ch: char,
    x: f32,
    y: f32,
    scale: f32,
    color: [u8; 4],
) -> f32 {
    let m = font::glyph_metric(kind, ch);
    if m.width > 0.0 && m.height > 0.0 {
        let gx = x + m.offset_x * scale;
        let gy = y + m.offset_y * scale;
        let gw = m.width * scale;
        let gh = m.height * scale;
        let (u0, v0) = (m.uv_min[0], m.uv_min[1]);
        let (u1, v1) = (m.uv_max[0], m.uv_max[1]);
        let quad = [
            (gx, gy, u0, v0),
            (gx, gy + gh, u0, v1),
            (gx + gw, gy + gh, u1, v1),
            (gx, gy, u0, v0),
            (gx + gw, gy + gh, u1, v1),
            (gx + gw, gy, u1, v0),
        ];
        for (px, py, u, v) in quad {
            out.push(HudVertex {
                pos: [px, py],
                uv: [u, v],
                color,
            });
        }
    }
    m.advance * scale
}

pub fn draw_text(
    out: &mut Vec<HudVertex>,
    kind: font::FontKind,
    text: &str,
    mut x: f32,
    y: f32,
    scale: f32,
    color: [u8; 4],
) -> f32 {
    let start_x = x;
    for ch in text.chars() {
        if out.len() + 6 > HUD_VERTEX_CAPACITY {
            break;
        }
        x += push_glyph(out, kind, ch, x, y, scale, color);
    }
    x - start_x
}

pub fn measure_text(kind: font::FontKind, text: &str, scale: f32) -> f32 {
    let mut width = 0.0;
    for ch in text.chars() {
        width += font::glyph_metric(kind, ch).advance * scale;
    }
    width
}

pub fn push_rect(out: &mut Vec<HudVertex>, x: f32, y: f32, w: f32, h: f32, color: [u8; 4]) {
    let [u, v] = font::white_pixel_uv();
    let quad = [
        (x, y),
        (x, y + h),
        (x + w, y + h),
        (x, y),
        (x + w, y + h),
        (x + w, y),
    ];
    for (px, py) in quad {
        out.push(HudVertex {
            pos: [px, py],
            uv: [u, v],
            color,
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
            player_feet: Vec3::new(1.0, 0.5, 3.0),
            player_height: 1.8,
            view_projection: Mat4::IDENTITY,
            fog_end: 100.0,
            time: 7.0,
            underwater: true,
            blur_background: false,
            menu_background: None,
        };
        let bytes = frame_uniforms(&view, &Mat4::IDENTITY, 0.5, true);
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
        assert_eq!(float(236), 1.0, "reflection flag in fog.w");
        assert_eq!(bytes.len(), 272);
        assert_eq!(
            [float(240), float(244), float(248), float(252)],
            [0.0, -1.5, 0.0, 1.8]
        );
        assert_eq!(float(256), 0.0, "blur_background flag in uiState.x");
    }

    #[test]
    fn frame_uniforms_include_ui_state() {
        let view = FrameView {
            camera: Vec3::new(1.0, 2.0, 3.0),
            player_feet: Vec3::new(1.0, 0.5, 3.0),
            player_height: 1.8,
            view_projection: Mat4::IDENTITY,
            fog_end: 100.0,
            time: 7.0,
            underwater: false,
            blur_background: true,
            menu_background: None,
        };
        let bytes = frame_uniforms(&view, &Mat4::IDENTITY, 0.5, true);
        assert_eq!(bytes.len(), 272);
        let float =
            |offset: usize| f32::from_ne_bytes(bytes[offset..offset + 4].try_into().unwrap());
        assert_eq!(float(256), 1.0, "blur_background flag in uiState.x");
    }

    #[test]
    fn common_shader_declares_caustics_helper() {
        let src = include_str!("../../../../shaders/vulkan/common.glsl");
        assert!(src.contains("float caustics(vec2 worldXz, float time)"));
    }

    #[test]
    fn composite_shader_includes_common_definitions() {
        let src = include_str!("../../../../shaders/vulkan/composite.frag");
        assert!(src.contains("#include \"common.glsl\""));
    }

    #[test]
    fn underwater_godrays_bounds_upward_rays_to_water_surface() {
        let src = include_str!("../../../../shaders/vulkan/common.glsl");
        assert!(src.contains("toSurface"));
    }

    #[test]
    fn shader_water_level_matches_the_mesh() {
        let src = include_str!("../../../../shaders/vulkan/common.glsl");
        let level: f32 = src
            .lines()
            .find_map(|line| line.strip_prefix("const float WATER_LEVEL = "))
            .expect("shared water surface level")
            .trim_end_matches(';')
            .parse()
            .unwrap();
        let mut chunk = goblock_world::Chunk::new(glam::IVec2::ZERO);
        chunk.set(
            0,
            goblock_world::SEA_LEVEL,
            0,
            goblock_world::BlockId::Water,
        );
        let mesh = goblock_world::mesh::build_mesh([&chunk; 9]);
        let mesh_top = mesh.vertices.iter().map(|v| v.position[1]).max().unwrap();
        assert_eq!(level, mesh_top as f32 / 16.0);
    }

    #[test]
    fn background_shader_bytes_are_available() {
        let bytes = include_bytes!(concat!(env!("OUT_DIR"), "/background.frag.spv"));
        assert!(!bytes.is_empty());
        assert_eq!(bytes.len() % 4, 0);
    }
}
