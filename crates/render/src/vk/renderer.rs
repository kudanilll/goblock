//! Per-frame work: wait, acquire, record, submit, present.

use ash::vk;
use glam::Mat4;

use super::device::{Device, MAX_FRAMES_IN_FLIGHT};
use super::error::VkError;
use super::resources::{create_pipeline, DynamicBuffer, PipelineConfig, StaticBuffer, Texture};
use super::swapchain::{FrameSync, Swapchain};
use super::Vertex;
use crate::font;

/// Block shader push constants: a mat4 view-projection, then the light
/// direction as a vec3 padded to 16 bytes. Vulkan 1.0 only guarantees 128
/// bytes of push constants, so anything added here has to fit in that.
const BLOCK_PUSH_SIZE: usize = 64 + 16;
/// HUD vertices one frame can hold, six per glyph.
const HUD_VERTEX_CAPACITY: usize = 4096;

pub struct Renderer {
    device: Device,
    swapchain: Swapchain,
    frames: Vec<FrameSync>,
    current_frame: usize,
    command_pool: vk::CommandPool,
    /// One per frame in flight, indexed like `frames`.
    command_buffers: Vec<vk::CommandBuffer>,

    block_pipeline: vk::Pipeline,
    block_pipeline_layout: vk::PipelineLayout,
    block_vertices: Vec<StaticBuffer>,

    sky_pipeline: vk::Pipeline,
    sky_pipeline_layout: vk::PipelineLayout,

    hud_pipeline: vk::Pipeline,
    hud_pipeline_layout: vk::PipelineLayout,
    hud_set_layout: vk::DescriptorSetLayout,
    hud_descriptor_pool: vk::DescriptorPool,
    hud_descriptor_set: vk::DescriptorSet,
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
        block_meshes: &[Vec<Vertex>],
    ) -> Result<Self, VkError> {
        let swapchain = Swapchain::new(&device, width, height)?;
        let frames = FrameSync::new(&device)?;

        let block_vertices = block_meshes
            .iter()
            .map(|mesh| StaticBuffer::new(&device, mesh, vk::BufferUsageFlags::VERTEX_BUFFER))
            .collect::<Result<Vec<_>, _>>()?;

        let block_config = PipelineConfig {
            vs_bytes: include_bytes!(concat!(env!("OUT_DIR"), "/block.vert.spv")),
            fs_bytes: include_bytes!(concat!(env!("OUT_DIR"), "/block.frag.spv")),
            vertex_bindings: vec![vk::VertexInputBindingDescription::default()
                .binding(0)
                .stride(std::mem::size_of::<Vertex>() as u32)
                .input_rate(vk::VertexInputRate::VERTEX)],
            vertex_attributes: vec![
                vk::VertexInputAttributeDescription::default()
                    .location(0)
                    .binding(0)
                    .format(vk::Format::R32G32B32_SFLOAT)
                    .offset(0),
                vk::VertexInputAttributeDescription::default()
                    .location(1)
                    .binding(0)
                    .format(vk::Format::R32G32B32_SFLOAT)
                    .offset(12),
                vk::VertexInputAttributeDescription::default()
                    .location(2)
                    .binding(0)
                    .format(vk::Format::R32G32B32_SFLOAT)
                    .offset(24),
            ],
            push_constant_size: BLOCK_PUSH_SIZE as u32,
            depth_test: true,
            depth_write: true,
            blend: false,
            cull_back: true,
            descriptor_layouts: vec![],
        };
        let (block_pipeline, block_pipeline_layout) =
            create_pipeline(&device, &block_config, device.render_pass)?;

        let sky_config = PipelineConfig {
            vs_bytes: include_bytes!(concat!(env!("OUT_DIR"), "/sky.vert.spv")),
            fs_bytes: include_bytes!(concat!(env!("OUT_DIR"), "/sky.frag.spv")),
            vertex_bindings: vec![],
            vertex_attributes: vec![],
            push_constant_size: 4,
            depth_test: false,
            depth_write: false,
            blend: false,
            cull_back: false,
            descriptor_layouts: vec![],
        };
        let (sky_pipeline, sky_pipeline_layout) =
            create_pipeline(&device, &sky_config, device.render_pass)?;

        // The HUD samples the font atlas through a single descriptor.
        let binding = vk::DescriptorSetLayoutBinding::default()
            .binding(0)
            .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .descriptor_count(1)
            .stage_flags(vk::ShaderStageFlags::FRAGMENT);
        let bindings = [binding];
        let layout_info = vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings);
        let hud_set_layout = device
            .device
            .create_descriptor_set_layout(&layout_info, None)
            .map_err(|e| VkError::new("create_descriptor_set_layout", e))?;

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
            descriptor_layouts: vec![hud_set_layout],
        };
        let (hud_pipeline, hud_pipeline_layout) =
            create_pipeline(&device, &hud_config, device.render_pass)?;

        let atlas_pixels = font::build_atlas();
        let hud_texture = Texture::new(&device, &atlas_pixels, font::ATLAS_W, font::ATLAS_H)?;

        let pool_sizes = [vk::DescriptorPoolSize::default()
            .ty(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .descriptor_count(1)];
        let descriptor_pool_info = vk::DescriptorPoolCreateInfo::default()
            .max_sets(1)
            .pool_sizes(&pool_sizes);
        let hud_descriptor_pool = device
            .device
            .create_descriptor_pool(&descriptor_pool_info, None)
            .map_err(|e| VkError::new("create_descriptor_pool", e))?;

        let set_layouts = [hud_set_layout];
        let alloc_info = vk::DescriptorSetAllocateInfo::default()
            .descriptor_pool(hud_descriptor_pool)
            .set_layouts(&set_layouts);
        let hud_descriptor_set = device
            .device
            .allocate_descriptor_sets(&alloc_info)
            .map_err(|e| VkError::new("allocate_descriptor_sets", e))?[0];

        let image_info = [vk::DescriptorImageInfo::default()
            .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
            .image_view(hud_texture.view)
            .sampler(hud_texture.sampler)];
        let write = vk::WriteDescriptorSet::default()
            .dst_set(hud_descriptor_set)
            .dst_binding(0)
            .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .image_info(&image_info);
        device.device.update_descriptor_sets(&[write], &[]);

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
            block_pipeline,
            block_pipeline_layout,
            block_vertices,
            sky_pipeline,
            sky_pipeline_layout,
            hud_pipeline,
            hud_pipeline_layout,
            hud_set_layout,
            hud_descriptor_pool,
            hud_descriptor_set,
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

    fn recreate_swapchain(&mut self) -> Result<(), VkError> {
        // A minimised window has nothing to draw into.
        if self.width == 0 || self.height == 0 {
            return Ok(());
        }
        self.swapchain
            .recreate(&self.device, self.width, self.height)
    }

    /// Draw one frame. `visible` lists chunk indices to render.
    pub fn draw_frame(
        &mut self,
        view_projection: &Mat4,
        visible: &[usize],
        hud_lines: &[String],
        time: f32,
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

            self.upload_hud(frame, hud_lines);

            let command_buffer = self.command_buffers[frame];
            self.device
                .device
                .reset_command_buffer(command_buffer, vk::CommandBufferResetFlags::empty())
                .map_err(|e| VkError::new("reset_command_buffer", e))?;

            self.record(
                command_buffer,
                frame,
                image_index,
                view_projection,
                visible,
                time,
            )?;

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
        &self,
        cmd: vk::CommandBuffer,
        frame: usize,
        image_index: usize,
        view_projection: &Mat4,
        visible: &[usize],
        time: f32,
    ) -> Result<(), VkError> {
        let device = &self.device.device;
        device
            .begin_command_buffer(cmd, &vk::CommandBufferBeginInfo::default())
            .map_err(|e| VkError::new("begin_command_buffer", e))?;

        let color = vk::ClearValue {
            color: vk::ClearColorValue {
                float32: [0.08, 0.09, 0.12, 1.0],
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
        let clears: &[vk::ClearValue] = if self.device.msaa_samples == vk::SampleCountFlags::TYPE_1
        {
            &single_clears
        } else {
            &msaa_clears
        };

        let extent = self.swapchain.extent;
        let render_area = vk::Rect2D::default().extent(extent);

        let info = vk::RenderPassBeginInfo::default()
            .render_pass(self.device.render_pass)
            .framebuffer(self.swapchain.framebuffers[image_index])
            .render_area(render_area)
            .clear_values(clears);

        device.cmd_begin_render_pass(cmd, &info, vk::SubpassContents::INLINE);

        let viewport = vk::Viewport::default()
            .width(extent.width as f32)
            .height(extent.height as f32)
            .min_depth(0.0)
            .max_depth(1.0);
        device.cmd_set_viewport(cmd, 0, &[viewport]);
        device.cmd_set_scissor(cmd, 0, &[render_area]);

        // Sky first, with depth writes off, so the blocks draw over it.
        device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.sky_pipeline);
        device.cmd_push_constants(
            cmd,
            self.sky_pipeline_layout,
            vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
            0,
            bytemuck::cast_slice(&[time]),
        );
        device.cmd_draw(cmd, 3, 1, 0, 0);

        device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.block_pipeline);

        let mut push = [0u8; BLOCK_PUSH_SIZE];
        push[0..64].copy_from_slice(bytemuck::cast_slice(&view_projection.to_cols_array()));
        push[64..80].copy_from_slice(bytemuck::cast_slice(&[0.4f32, -1.0, 0.3, 0.0]));
        device.cmd_push_constants(
            cmd,
            self.block_pipeline_layout,
            vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
            0,
            &push,
        );

        for &chunk in visible {
            if let Some(buffer) = self.block_vertices.get(chunk) {
                if buffer.count == 0 {
                    continue;
                }
                device.cmd_bind_vertex_buffers(cmd, 0, &[buffer.buffer], &[0]);
                device.cmd_draw(cmd, buffer.count, 1, 0, 0);
            }
        }

        // HUD last, blended over everything else.
        if self.hud_vertex_count > 0 {
            device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.hud_pipeline);

            let size = [extent.width as f32, extent.height as f32];
            let text_color = [1.0f32, 1.0, 1.0, 1.0];
            let mut push = [0u8; 32];
            push[0..8].copy_from_slice(bytemuck::cast_slice(&size));
            push[16..32].copy_from_slice(bytemuck::cast_slice(&text_color));
            device.cmd_push_constants(
                cmd,
                self.hud_pipeline_layout,
                vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                0,
                &push,
            );
            device.cmd_bind_descriptor_sets(
                cmd,
                vk::PipelineBindPoint::GRAPHICS,
                self.hud_pipeline_layout,
                0,
                &[self.hud_descriptor_set],
                &[],
            );
            device.cmd_bind_vertex_buffers(cmd, 0, &[self.hud_vertices[frame].buffer], &[0]);
            device.cmd_draw(cmd, self.hud_vertex_count, 1, 0, 0);
        }

        device.cmd_end_render_pass(cmd);
        device
            .end_command_buffer(cmd)
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

            for buffer in &mut self.hud_vertices {
                buffer.destroy(&self.device);
            }
            for buffer in &mut self.block_vertices {
                buffer.destroy(&self.device);
            }
            self.hud_texture.destroy(&self.device);

            device.destroy_descriptor_pool(self.hud_descriptor_pool, None);
            device.destroy_descriptor_set_layout(self.hud_set_layout, None);

            for (pipeline, layout) in [
                (self.block_pipeline, self.block_pipeline_layout),
                (self.sky_pipeline, self.sky_pipeline_layout),
                (self.hud_pipeline, self.hud_pipeline_layout),
            ] {
                device.destroy_pipeline(pipeline, None);
                device.destroy_pipeline_layout(layout, None);
            }

            FrameSync::destroy_all(&self.device, &self.frames);
            self.swapchain.destroy(&self.device);
            self.device.destroy();
        }
    }
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
